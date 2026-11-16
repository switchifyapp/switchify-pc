//! Radar pointer targeting for the mouse panel. A quarter of the display, or
//! all of it, is chosen first. A beam then turns around that area's centre,
//! and the pointer follows a marker out along it to the area's edge. Pure
//! geometry; no OS input: it only says where the pointer should go.
use crate::{
    scan_preferences::{Direction, Resolved},
    scan_sketch::{Shape, Sketch},
    scanning::{Action, Frame, FrameLabel, HudPresentation, Rect},
};
use std::f64::consts::TAU;

/// The beam turns as fast as a point this far out moves at the radar speed.
const SWEEP_RADIUS: f64 = 300.0;
/// Manual steps, in degrees and logical pixels.
const ANGLE_STEP: f64 = 5.0;
const DISTANCE_STEP: f64 = 20.0;
/// The drawing is cut into squares this many logical pixels wide, so a
/// turning beam only redraws the squares it passes through.
const CELL: f64 = 160.0;
/// Top left, top right, bottom left, bottom right, then the whole display.
const AREAS: usize = 5;
const WHOLE: usize = AREAS - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Area,
    Angle,
    Distance,
}

/// One of the areas in scan order. Quarters split on whole units, so
/// neighbours share no pixel and together cover the display.
fn area(screen: Rect, index: usize) -> Rect {
    if index >= WHOLE {
        return screen;
    }
    let left = (screen.width / 2.0).floor();
    let top = (screen.height / 2.0).floor();
    let (x, width) = if index.is_multiple_of(2) {
        (screen.x, left)
    } else {
        (screen.x + left, screen.width - left)
    };
    let (y, height) = if index < 2 {
        (screen.y, top)
    } else {
        (screen.y + top, screen.height - top)
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn inset(rect: Rect, by: f64) -> Rect {
    let by = by.min(rect.width / 2.0).min(rect.height / 2.0);
    Rect {
        x: rect.x + by,
        y: rect.y + by,
        width: rect.width - by * 2.0,
        height: rect.height - by * 2.0,
    }
}

fn centre(rect: Rect) -> (f64, f64) {
    (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
}

pub struct Radar {
    options: Resolved,
    /// Logical pixels per second, on point scanning's line speed scale.
    speed: f64,
    screen: Rect,
    units: f64,
    /// The highlighted area while one is being chosen, then the chosen one.
    index: usize,
    /// The chosen area. The marker stays inside it.
    area: Rect,
    origin: (f64, f64),
    pub phase: Phase,
    /// Radians clockwise from straight up.
    angle: f64,
    /// Display units from the origin along the beam.
    distance: f64,
    direction: f64,
    /// Full automatic passes of the current phase since it began.
    cycles: usize,
    /// Time since the last counted pass, so a short beam is not used up at
    /// once. While an area is being chosen, time since the highlight moved.
    pass_ms: u64,
    /// Where the pointer was last sent since an area was chosen.
    placed: Option<(i32, i32)>,
}

impl Radar {
    /// `speed` is the radar speed index, on the line speed scale. It starts by
    /// choosing an area of `screen`.
    pub fn new(options: Resolved, speed: usize, screen: Rect, units: f64) -> Self {
        let mut radar = Self {
            options,
            speed: [45.0, 75.0, 120.0, 180.0, 270.0][speed.min(4)],
            screen,
            units,
            index: 0,
            area: screen,
            origin: centre(screen),
            phase: Phase::Area,
            angle: 0.0,
            distance: 0.0,
            direction: 1.0,
            cycles: 0,
            pass_ms: 0,
            placed: None,
        };
        radar.choose_area();
        radar
    }
    /// Highlights the areas in turn again, from the first in the scan direction.
    fn choose_area(&mut self) {
        self.phase = Phase::Area;
        self.direction = self.initial_direction();
        self.index = if self.direction > 0.0 { 0 } else { WHOLE };
        self.area = self.screen;
        self.origin = centre(self.screen);
        self.angle = 0.0;
        self.distance = 0.0;
        self.cycles = 0;
        self.pass_ms = 0;
        // The pointer stays where it is until another area is chosen.
        self.placed = None;
    }
    /// Goes back to choosing an area. False when an area is already being
    /// chosen, so the radar should end.
    pub fn back(&mut self) -> bool {
        if self.phase == Phase::Area {
            return false;
        }
        self.choose_area();
        true
    }
    fn initial_direction(&self) -> f64 {
        if self.options.direction == Direction::Forward {
            1.0
        } else {
            -1.0
        }
    }
    fn unit(&self) -> (f64, f64) {
        (self.angle.sin(), -self.angle.cos())
    }
    /// How far the beam reaches before leaving the chosen area.
    pub fn reach(&self) -> f64 {
        let (dx, dy) = self.unit();
        let (x, y) = self.origin;
        let right = self.area.x + self.area.width - 1.0;
        let bottom = self.area.y + self.area.height - 1.0;
        let along = |position: f64, delta: f64, low: f64, high: f64| {
            if delta > 1e-9 {
                (high - position) / delta
            } else if delta < -1e-9 {
                (low - position) / delta
            } else {
                f64::INFINITY
            }
        };
        along(x, dx, self.area.x, right)
            .min(along(y, dy, self.area.y, bottom))
            .max(0.0)
    }
    fn at(&self, distance: f64) -> (f64, f64) {
        let (dx, dy) = self.unit();
        (self.origin.0 + dx * distance, self.origin.1 + dy * distance)
    }
    pub fn target(&self) -> (i32, i32) {
        let (x, y) = self.at(self.distance);
        let a = self.area;
        (
            x.round().min(a.x + a.width - 1.0).max(a.x) as i32,
            y.round().min(a.y + a.height - 1.0).max(a.y) as i32,
        )
    }
    /// Where the pointer should go, only when it was not sent there already:
    /// the chosen area's centre while the beam turns, then the marker.
    /// Nothing while an area is being chosen.
    pub fn follow(&mut self) -> Option<(i32, i32)> {
        if self.phase == Phase::Area {
            return None;
        }
        let target = self.target();
        (self.placed != Some(target)).then(|| {
            self.placed = Some(target);
            target
        })
    }
    pub fn exhausted(&self) -> bool {
        self.options.exhausted(self.cycles)
    }
    /// Select chooses the area, then fixes the angle, then returns the chosen point.
    pub fn handle(&mut self, action: Action) -> Option<(i32, i32)> {
        match action {
            Action::Select => {
                self.cycles = 0;
                self.pass_ms = 0;
                self.direction = self.initial_direction();
                match self.phase {
                    Phase::Area => {
                        self.area = area(self.screen, self.index);
                        self.origin = centre(self.area);
                        self.angle = 0.0;
                        self.distance = 0.0;
                        self.phase = Phase::Angle;
                    }
                    Phase::Angle => {
                        self.phase = Phase::Distance;
                        self.distance = if self.direction > 0.0 {
                            0.0
                        } else {
                            self.reach()
                        };
                    }
                    Phase::Distance => return Some(self.target()),
                }
            }
            Action::Next | Action::Back => {
                self.direction = if action == Action::Next { 1.0 } else { -1.0 };
                match self.phase {
                    Phase::Area => {
                        self.pass_ms = 0;
                        self.step_area();
                    }
                    Phase::Angle => {
                        self.turn(self.direction * ANGLE_STEP.to_radians());
                    }
                    Phase::Distance => {
                        self.extend(self.direction * DISTANCE_STEP * self.units);
                    }
                }
            }
            Action::Reverse => self.direction = -self.direction,
            _ => {}
        }
        None
    }
    /// Moves automatically. Only these passes count towards the pass limit,
    /// and a pass lasts at least one scan interval, however short the beam.
    pub fn advance(&mut self, ms: u64) {
        self.pass_ms = self.pass_ms.saturating_add(ms);
        let seconds = ms as f64 / 1000.0;
        let wrapped = match self.phase {
            Phase::Area => {
                // The highlight moves once each scan interval, as the panel's keys do.
                if self.pass_ms >= self.options.interval_ms {
                    self.pass_ms = 0;
                    if self.step_area() {
                        self.cycles += 1;
                    }
                }
                return;
            }
            Phase::Angle => self.turn(self.direction * self.speed / SWEEP_RADIUS * seconds),
            Phase::Distance => {
                let amount = (self.speed * self.units * seconds).max(1.0);
                self.extend(self.direction * amount)
            }
        };
        if wrapped && self.pass_ms >= self.options.interval_ms {
            self.cycles += 1;
            self.pass_ms = 0;
        }
    }
    /// Highlights the next area and reports whether it wrapped past either end.
    fn step_area(&mut self) -> bool {
        if self.direction > 0.0 {
            self.index = (self.index + 1) % AREAS;
            self.index == 0
        } else {
            self.index = (self.index + WHOLE) % AREAS;
            self.index == WHOLE
        }
    }
    /// Turns the beam and reports whether it passed straight up. Leaving
    /// straight up backwards at the start is not a pass.
    fn turn(&mut self, delta: f64) -> bool {
        let next = self.angle + delta;
        let wrapped = next >= TAU || (next < 0.0 && self.angle > 0.0);
        self.angle = next.rem_euclid(TAU);
        wrapped
    }
    /// Moves the marker and reports whether it wrapped past either end.
    fn extend(&mut self, delta: f64) -> bool {
        let reach = self.reach();
        let next = self.distance + delta;
        if next > reach {
            self.distance = 0.0;
            true
        } else if next < 0.0 {
            self.distance = reach;
            true
        } else {
            self.distance = next;
            false
        }
    }
    /// The dashed sides of `rect`, showing how far the marker can go.
    fn sides(rect: Rect) -> [Shape; 4] {
        let (l, t) = (rect.x, rect.y);
        let (r, b) = (rect.x + rect.width, rect.y + rect.height);
        [
            ((l, t), (r, t)),
            ((r, t), (r, b)),
            ((r, b), (l, b)),
            ((l, b), (l, t)),
        ]
        .map(|(from, to)| Shape::Divider { from, to })
    }
    /// Dashed lines split the display into quarters, and the highlighted
    /// area is filled and outlined with rounded corners.
    fn area_shapes(&self, width: f64, fills: &mut Vec<Rect>) -> Vec<Shape> {
        let s = self.screen;
        let quarter = area(s, 0);
        let (x, y) = (s.x + quarter.width, s.y + quarter.height);
        let highlighted = inset(area(s, self.index), width * 1.5);
        fills.push(highlighted);
        vec![
            Shape::Divider {
                from: (x, s.y),
                to: (x, s.y + s.height),
            },
            Shape::Divider {
                from: (s.x, y),
                to: (s.x + s.width, y),
            },
            Shape::Outline {
                rect: highlighted,
                radius: width * 2.0,
            },
        ]
    }
    /// A solid line from a ring at the centre. While the direction is chosen
    /// it reaches the edge of the area and ends in a dot. While the distance
    /// is chosen it stops at the pointer, which is its tip, and the rest of
    /// the way is dotted.
    fn beam_shapes(&self, width: f64) -> Vec<Shape> {
        // A chosen quarter stays outlined, showing how far the marker can go.
        let mut shapes = if self.area == self.screen {
            vec![]
        } else {
            Self::sides(self.area).to_vec()
        };
        let reach = self.reach();
        let end = self.at(reach);
        if self.phase == Phase::Angle {
            shapes.push(Shape::Line {
                from: self.origin,
                to: end,
            });
            shapes.push(Shape::Dot {
                centre: end,
                radius: width * 1.25,
            });
        } else {
            let tip = self.at(self.distance);
            if reach - self.distance > 0.5 {
                shapes.push(Shape::Dotted { from: tip, to: end });
            }
            if self.distance > 0.5 {
                shapes.push(Shape::Line {
                    from: self.origin,
                    to: tip,
                });
            }
        }
        shapes.push(Shape::Ring {
            centre: self.origin,
            radius: width * 2.25,
        });
        shapes
    }
    pub fn frame(&self) -> Frame {
        let t = 2.0 * self.units * self.options.thickness.scale();
        let width = t * 3.0;
        let mut fills = vec![];
        let shapes = if self.phase == Phase::Area {
            self.area_shapes(width, &mut fills)
        } else {
            self.beam_shapes(width)
        };
        let mut frame = Frame {
            fills,
            sketch: Some(Sketch {
                shapes,
                // The scan workflow sets the cursor overlay's colour.
                color: [255, 255, 255],
                width,
                bounds: self.screen,
                cell: CELL * self.units,
            }),
            ..Frame::default()
        };
        let width = (480.0 * self.units).min(self.screen.width);
        frame.label = Some(FrameLabel {
            text: match self.phase {
                Phase::Area => "Radar · Select an area",
                Phase::Angle => "Radar · Select a direction",
                Phase::Distance => "Radar · Select a distance",
            }
            .into(),
            rect: Rect {
                x: self.screen.x + (self.screen.width - width) / 2.0,
                y: self.screen.y + 20.0 * self.units,
                width,
                height: (64.0 * self.units).min(self.screen.height),
            },
            scale: self.units,
            hud: Some(HudPresentation {
                screen: self.screen,
                scale: self.units,
            }),
        });
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: -1000.0,
        y: 100.0,
        width: 1000.0,
        height: 800.0,
    };
    /// Odd sizes left of and above the main display, at 150%.
    const SCALED: Rect = Rect {
        x: -2561.0,
        y: -1441.0,
        width: 2561.0,
        height: 1441.0,
    };

    fn radar() -> Radar {
        Radar::new(Resolved::default(), 2, SCREEN, 1.0)
    }

    /// A radar sweeping from the centre of area `index`.
    fn chosen(screen: Rect, units: f64, index: usize) -> Radar {
        let mut r = Radar::new(Resolved::default(), 2, screen, units);
        for _ in 0..index {
            r.handle(Action::Next);
        }
        assert_eq!(r.handle(Action::Select), None);
        assert_eq!(r.phase, Phase::Angle);
        r
    }

    fn inside(rect: Rect, (x, y): (i32, i32)) -> bool {
        let (x, y) = (f64::from(x), f64::from(y));
        x >= rect.x
            && x <= rect.x + rect.width - 1.0
            && y >= rect.y
            && y <= rect.y + rect.height - 1.0
    }

    #[test]
    fn areas_are_the_four_quarters_then_the_whole_display() {
        let quarter = |x, y| Rect {
            x,
            y,
            width: 500.0,
            height: 400.0,
        };
        assert_eq!(area(SCREEN, 0), quarter(-1000.0, 100.0));
        assert_eq!(area(SCREEN, 1), quarter(-500.0, 100.0));
        assert_eq!(area(SCREEN, 2), quarter(-1000.0, 500.0));
        assert_eq!(area(SCREEN, 3), quarter(-500.0, 500.0));
        assert_eq!(area(SCREEN, 4), SCREEN);

        // Odd sizes split on whole units, leaving no gap and no overlap.
        let [top_left, top_right, bottom_left, bottom_right] =
            [0, 1, 2, 3].map(|i| area(SCALED, i));
        assert_eq!(
            top_left,
            Rect {
                x: -2561.0,
                y: -1441.0,
                width: 1280.0,
                height: 720.0
            }
        );
        assert_eq!(top_right.x, top_left.x + top_left.width);
        assert_eq!(top_right.x + top_right.width, 0.0);
        assert_eq!(bottom_left.y, top_left.y + top_left.height);
        assert_eq!(bottom_left.y + bottom_left.height, 0.0);
        assert_eq!(
            (bottom_right.x, bottom_right.y),
            (top_right.x, bottom_left.y)
        );
        assert_eq!(area(SCALED, 4), SCALED);
    }

    #[test]
    fn areas_are_highlighted_in_turn_and_wrap() {
        let mut r = radar();
        assert_eq!(r.phase, Phase::Area);
        let mut order = vec![r.index];
        for _ in 0..5 {
            r.advance(999);
            assert_eq!(r.index, *order.last().unwrap(), "one move per interval");
            r.advance(1);
            order.push(r.index);
        }
        assert_eq!(order, [0, 1, 2, 3, 4, 0]);
        assert_eq!(r.cycles, 1, "wrapping is a pass");
        for _ in 0..10 {
            r.advance(1000);
        }
        assert!(r.exhausted());

        // Stepping wraps both ways and is not counted.
        let mut r = radar();
        r.handle(Action::Back);
        assert_eq!(r.index, 4);
        r.handle(Action::Next);
        assert_eq!(r.index, 0);
        assert_eq!(r.cycles, 0);

        // Reverse direction starts at the whole display and goes backwards.
        let mut r = Radar::new(
            Resolved {
                direction: Direction::Reverse,
                ..Default::default()
            },
            2,
            SCREEN,
            1.0,
        );
        assert_eq!(r.index, 4);
        r.advance(1000);
        assert_eq!(r.index, 3);
    }

    #[test]
    fn the_beam_turns_around_the_centre_of_the_chosen_area() {
        for (screen, units) in [(SCREEN, 1.0), (SCALED, 1.5)] {
            for index in 0..AREAS {
                let r = chosen(screen, units, index);
                let a = area(screen, index);
                assert_eq!(r.area, a);
                assert_eq!(r.origin, (a.x + a.width / 2.0, a.y + a.height / 2.0));
                assert_eq!(
                    r.target(),
                    (r.origin.0.round() as i32, r.origin.1.round() as i32)
                );
            }
        }
        assert_eq!(chosen(SCREEN, 1.0, 0).origin, (-750.0, 300.0));
        assert_eq!(chosen(SCREEN, 1.0, 4).origin, (-500.0, 500.0));
    }

    #[test]
    fn the_marker_never_leaves_the_chosen_area() {
        for (screen, units) in [(SCREEN, 1.0), (SCALED, 1.5)] {
            for index in 0..AREAS {
                let mut r = chosen(screen, units, index);
                for degree in 0..360 {
                    r.angle = f64::from(degree).to_radians();
                    r.distance = r.reach();
                    assert!(r.reach() > 0.0);
                    assert!(inside(r.area, r.target()), "{index} at {degree}°");
                    let (x, y) = r.at(r.reach());
                    assert!(x >= r.area.x - 1e-6 && x <= r.area.x + r.area.width - 1.0 + 1e-6);
                    assert!(y >= r.area.y - 1e-6 && y <= r.area.y + r.area.height - 1.0 + 1e-6);
                }
                // Stepping and sweeping out wrap at the edge instead of crossing it.
                r.angle = TAU / 8.0;
                r.handle(Action::Select);
                for _ in 0..200 {
                    r.handle(Action::Next);
                    assert!(inside(r.area, r.target()));
                    r.advance(100);
                    assert!(inside(r.area, r.target()));
                }
            }
        }
    }

    #[test]
    fn back_returns_to_choosing_an_area_then_ends() {
        let mut r = chosen(SCREEN, 1.0, 3);
        r.handle(Action::Next);
        assert!(r.back());
        assert_eq!(r.phase, Phase::Area);
        assert_eq!(r.index, 0, "areas are highlighted from the start again");
        assert_eq!(r.area, SCREEN);
        assert_eq!(r.angle, 0.0);

        let mut r = chosen(SCREEN, 1.0, 1);
        r.handle(Action::Select);
        assert_eq!(r.phase, Phase::Distance);
        r.handle(Action::Next);
        for _ in 0..30 {
            r.advance(1000);
        }
        assert!(r.exhausted());
        assert!(r.back());
        assert_eq!(r.phase, Phase::Area);
        assert!(!r.exhausted(), "passes start again");
        assert!(!r.back(), "back from choosing an area ends the radar");
    }

    #[test]
    fn beam_reaches_the_display_edge_in_each_direction() {
        let mut r = chosen(SCREEN, 1.0, 4);
        assert!((r.reach() - 400.0).abs() < 1e-9, "up");
        r.angle = TAU / 4.0;
        assert!((r.reach() - 499.0).abs() < 1e-9, "right");
        r.angle = TAU / 2.0;
        assert!((r.reach() - 399.0).abs() < 1e-9, "down");
        r.angle = TAU * 3.0 / 4.0;
        assert!((r.reach() - 500.0).abs() < 1e-9, "left");
        r.angle = TAU / 8.0;
        let (x, y) = r.at(r.reach());
        assert!((y - 100.0).abs() < 1e-6 && x > -500.0 && x < -1.0);
    }

    #[test]
    fn a_quarter_beam_reaches_the_quarter_edge() {
        // The top left quarter is -1000..-501 by 100..499, around (-750, 300).
        let mut r = chosen(SCREEN, 1.0, 0);
        assert!((r.reach() - 200.0).abs() < 1e-9, "up");
        r.angle = TAU / 4.0;
        assert!((r.reach() - 249.0).abs() < 1e-9, "right");
        r.angle = TAU / 2.0;
        assert!((r.reach() - 199.0).abs() < 1e-9, "down");
        r.angle = TAU * 3.0 / 4.0;
        assert!((r.reach() - 250.0).abs() < 1e-9, "left");
    }

    #[test]
    fn select_fixes_the_angle_then_returns_the_marker_in_the_area() {
        let mut r = chosen(SCREEN, 1.0, 4);
        r.handle(Action::Next);
        r.handle(Action::Next);
        assert!((r.angle - 10f64.to_radians()).abs() < 1e-9);
        assert_eq!(r.handle(Action::Select), None);
        assert_eq!(r.phase, Phase::Distance);
        assert_eq!(r.target(), (-500, 500));
        for _ in 0..10 {
            r.handle(Action::Next);
        }
        let (x, y) = r.target();
        let expected = (
            -500.0 + 200.0 * 10f64.to_radians().sin(),
            500.0 - 200.0 * 10f64.to_radians().cos(),
        );
        assert_eq!(
            (x, y),
            (expected.0.round() as i32, expected.1.round() as i32)
        );
        assert_eq!(r.handle(Action::Select), Some((x, y)));
    }

    #[test]
    fn automatic_passes_count_towards_the_limit_but_manual_steps_do_not() {
        let mut r = chosen(SCREEN, 1.0, 4);
        for _ in 0..200 {
            r.handle(Action::Back);
        }
        assert!(!r.exhausted());
        // Speed 120 at a 300 pixel radius takes about 15.7 s per turn.
        for _ in 0..50 {
            r.advance(1000);
        }
        assert_eq!(r.cycles, 3);
        assert!(r.exhausted());

        r.angle = 0.0;
        r.handle(Action::Select);
        assert_eq!(r.cycles, 0);
        for _ in 0..4 {
            r.advance(1000);
        }
        assert_eq!(r.cycles, 1, "400 pixels up at 120 per second wraps once");
        assert!(r.distance < 120.0);
    }

    #[test]
    fn a_beam_with_no_room_still_gives_each_pass_a_full_interval() {
        let mut r = chosen(SCREEN, 1.0, 4);
        // On the top edge, straight up reaches nowhere.
        r.origin = (-500.0, 100.0);
        r.handle(Action::Select);
        assert_eq!(r.reach(), 0.0);
        for _ in 0..29 {
            r.advance(100);
        }
        assert_eq!(r.cycles, 2, "one pass per 1000 ms interval");
        assert!(!r.exhausted());
        r.advance(100);
        assert!(r.exhausted());
        assert_eq!(r.handle(Action::Select), Some((-500, 100)));
    }

    #[test]
    fn reverse_direction_turns_anticlockwise_and_starts_at_the_edge() {
        let mut r = Radar::new(
            Resolved {
                direction: Direction::Reverse,
                ..Default::default()
            },
            4,
            SCREEN,
            1.0,
        );
        // Reverse highlights the whole display first.
        r.handle(Action::Select);
        assert_eq!(r.area, SCREEN);
        r.advance(100);
        assert!(r.angle > TAU * 0.9);
        assert_eq!(r.cycles, 0, "leaving straight up is not a pass");
        r.handle(Action::Select);
        assert!((r.distance - r.reach()).abs() < 1e-9);
        r.advance(100);
        assert!(r.distance < r.reach());
    }

    fn shapes(frame: &Frame) -> &[Shape] {
        &frame.sketch.as_ref().expect("radar draws a sketch").shapes
    }

    #[test]
    fn choosing_an_area_draws_the_quarters_and_highlights_one() {
        let mut r = Radar::new(Resolved::default(), 2, SCALED, 1.5);
        let frame = r.frame();
        assert_eq!(frame.label.as_ref().unwrap().text, "Radar · Select an area");
        assert!(frame.strips.is_empty() && frame.grid.is_empty());
        let sketch = frame.sketch.as_ref().unwrap();
        assert_eq!(sketch.bounds, SCALED);
        let dividers = |frame: &Frame| {
            shapes(frame)
                .iter()
                .filter(|s| matches!(s, Shape::Divider { .. }))
                .count()
        };
        assert_eq!(dividers(&frame), 2, "the cross between quarters");
        // The highlight is filled and outlined just inside its quarter.
        let quarter = area(SCALED, 0);
        let [fill] = frame.fills[..] else {
            panic!("one highlighted area")
        };
        assert!(fill.x > quarter.x && fill.y > quarter.y);
        assert!(fill.x + fill.width < quarter.x + quarter.width);
        assert!(fill.y + fill.height < quarter.y + quarter.height);
        assert!(shapes(&frame)
            .iter()
            .any(|s| matches!(s, Shape::Outline { rect, .. } if *rect == fill)));
        for _ in 0..4 {
            r.handle(Action::Next);
        }
        let whole = r.frame().fills[0];
        assert!(whole.width > SCALED.width * 0.95 && whole.height > SCALED.height * 0.95);
    }

    #[test]
    fn frame_draws_a_bounded_beam_inside_the_display_and_labels_the_phase() {
        let mut r = chosen(SCREEN, 1.0, 4);
        r.angle = TAU / 8.0;
        let frame = r.frame();
        assert!(frame.fills.is_empty() && frame.grid.is_empty() && frame.strips.is_empty());
        assert_eq!(
            frame.label.as_ref().unwrap().text,
            "Radar · Select a direction"
        );
        let end = r.at(r.reach());
        // A line from the centre to the edge, a dot at the end and a ring
        // around the centre.
        assert_eq!(
            shapes(&frame),
            [
                Shape::Line {
                    from: r.origin,
                    to: end
                },
                Shape::Dot {
                    centre: end,
                    radius: 7.5
                },
                Shape::Ring {
                    centre: r.origin,
                    radius: 13.5
                },
            ]
        );
        r.handle(Action::Select);
        let frame = r.frame();
        assert_eq!(
            frame.label.as_ref().unwrap().text,
            "Radar · Select a distance"
        );
        assert_eq!(
            shapes(&frame),
            [
                Shape::Dotted {
                    from: r.origin,
                    to: end
                },
                Shape::Ring {
                    centre: r.origin,
                    radius: 13.5
                },
            ],
            "the beam starts at the centre, under the pointer, with the way ahead dotted"
        );

        // It grows to the pointer, which is its tip.
        r.distance = r.reach() / 2.0;
        let tip = r.at(r.distance);
        assert_eq!(
            shapes(&r.frame())[..2],
            [
                Shape::Dotted { from: tip, to: end },
                Shape::Line {
                    from: r.origin,
                    to: tip
                },
            ]
        );
        r.distance = r.reach();
        assert!(
            !shapes(&r.frame())
                .iter()
                .any(|s| matches!(s, Shape::Dotted { .. })),
            "nothing is left to go"
        );
        for cell in r.frame().sketch.unwrap().cells() {
            assert!(cell.x >= SCREEN.x && cell.y >= SCREEN.y);
            assert!(cell.x + cell.width <= SCREEN.x + SCREEN.width + 1e-9);
            assert!(cell.y + cell.height <= SCREEN.y + SCREEN.height + 1e-9);
        }
    }

    #[test]
    fn the_beam_follows_the_thickness_setting() {
        let thick = Resolved {
            thickness: crate::scan_preferences::Thickness::Thick,
            ..Resolved::default()
        };
        let mut r = Radar::new(thick, 2, SCALED, 1.5);
        r.handle(Action::Select);
        assert_eq!(r.frame().sketch.unwrap().width, 2.0 * 1.5 * 2.0 * 3.0);
    }

    #[test]
    fn the_pointer_follows_the_centre_then_the_marker_only_when_it_moves() {
        let mut r = radar();
        assert_eq!(r.follow(), None, "the pointer stays while choosing an area");
        r.advance(1000);
        assert_eq!(r.follow(), None);

        // The top right quarter, around (-250, 300).
        r.handle(Action::Select);
        assert_eq!(r.follow(), Some((-250, 300)), "to the chosen centre");
        assert_eq!(r.follow(), None, "only once");
        // The beam turning leaves the pointer at the centre.
        for _ in 0..20 {
            r.advance(100);
            r.handle(Action::Next);
            assert_eq!(r.follow(), None);
        }
        r.angle = TAU / 4.0;
        r.handle(Action::Select);
        assert_eq!(r.follow(), None, "the marker starts at the centre");
        r.handle(Action::Next);
        assert_eq!(r.follow(), Some((-230, 300)));
        assert_eq!(r.follow(), None);
        // Less than a unit along moves nothing.
        r.distance += 0.2;
        assert_eq!(r.follow(), None);
        r.advance(100);
        assert_eq!(r.follow(), Some((-218, 300)));
        assert_eq!(r.handle(Action::Select), Some((-218, 300)));

        // Choosing an area again sends the pointer to its centre again,
        // having left it where it was meanwhile.
        let mut r = chosen(SCREEN, 1.0, 1);
        assert_eq!(r.follow(), Some((-250, 300)));
        r.handle(Action::Select);
        r.handle(Action::Back);
        assert!(r.follow().is_some());
        assert!(r.back());
        assert_eq!(r.follow(), None);
        r.handle(Action::Next);
        r.handle(Action::Select);
        assert_eq!(r.follow(), Some((-250, 300)));
    }

    #[test]
    fn a_chosen_quarter_stays_outlined_around_the_beam() {
        let r = chosen(SCREEN, 1.0, 3);
        let frame = r.frame();
        assert_eq!(shapes(&frame)[..4], Radar::sides(area(SCREEN, 3)));
        let whole = chosen(SCREEN, 1.0, 4);
        assert!(!shapes(&whole.frame())
            .iter()
            .any(|s| matches!(s, Shape::Divider { .. })));
    }
}
