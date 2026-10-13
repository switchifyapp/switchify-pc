//! Radar pointer targeting for the mouse panel. A quarter of the display, or
//! all of it, is chosen first. A beam then turns around that area's centre,
//! and a marker travels out along it to the area's edge. Pure geometry; no OS input.
use crate::{
    point_scan::outline,
    scan_preferences::{Direction, Resolved},
    scanning::{Action, Frame, FrameLabel, HudPresentation, Rect},
};
use std::f64::consts::TAU;

/// The beam turns as fast as a point this far out moves at the point speed.
const SWEEP_RADIUS: f64 = 300.0;
/// Manual steps, in degrees and logical pixels.
const ANGLE_STEP: f64 = 5.0;
const DISTANCE_STEP: f64 = 20.0;
/// The beam is drawn as separate squares, each its own native strip.
const MAX_DOTS: usize = 32;
const DOT_SPACING: f64 = 16.0;
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

fn centre(rect: Rect) -> (f64, f64) {
    (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
}

pub struct Radar {
    options: Resolved,
    /// Logical pixels per second, the same as point scanning.
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
}

impl Radar {
    /// `speed` is the point scan speed index. It starts by choosing an area of `screen`.
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
    fn square(&self, (x, y): (f64, f64), size: f64) -> Rect {
        Rect {
            x: x - size / 2.0,
            y: y - size / 2.0,
            width: size,
            height: size,
        }
    }
    /// The display split into quarters, like point scanning's grid, with the
    /// highlighted area filled and outlined.
    fn area_frame(&self, t: f64) -> Frame {
        let s = self.screen;
        let quarter = area(s, 0);
        let mut grid = outline(s, t).to_vec();
        grid.push(Rect {
            x: (s.x + quarter.width - t / 2.0).max(s.x),
            width: t,
            ..s
        });
        grid.push(Rect {
            y: (s.y + quarter.height - t / 2.0).max(s.y),
            height: t,
            ..s
        });
        let highlighted = area(s, self.index);
        Frame {
            fills: vec![highlighted],
            grid,
            strips: outline(highlighted, t * 2.0).to_vec(),
            ..Frame::default()
        }
    }
    fn beam(&self, t: f64) -> Vec<Rect> {
        let reach = self.reach();
        let dots = ((reach / (DOT_SPACING * self.units)).ceil() as usize).clamp(1, MAX_DOTS);
        let spacing = reach / dots as f64;
        // A chosen quarter stays outlined, showing how far the marker can go.
        let mut strips = if self.area == self.screen {
            vec![]
        } else {
            outline(self.area, t).to_vec()
        };
        strips.push(self.square(self.origin, t * 4.0));
        if self.phase == Phase::Angle {
            strips
                .extend((1..=dots).map(|dot| self.square(self.at(spacing * dot as f64), t * 2.0)));
        } else {
            // The beam grows out from the centre to its tip, where the pointer goes.
            strips.extend(
                (1..=dots)
                    .map(|dot| spacing * dot as f64)
                    // Leave room so no dot sits on the tip.
                    .take_while(|&along| along < self.distance - spacing / 2.0)
                    .map(|along| self.square(self.at(along), t * 2.0)),
            );
            strips.push(self.square(self.at(self.distance), t * 3.0));
        }
        strips
    }
    pub fn frame(&self) -> Frame {
        let t = 2.0 * self.units * self.options.thickness.scale();
        let mut frame = if self.phase == Phase::Area {
            self.area_frame(t)
        } else {
            Frame {
                strips: self.beam(t),
                ..Frame::default()
            }
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

    #[test]
    fn choosing_an_area_draws_the_quarters_and_highlights_one() {
        let mut r = Radar::new(Resolved::default(), 2, SCALED, 1.5);
        let frame = r.frame();
        assert_eq!(frame.label.as_ref().unwrap().text, "Radar · Select an area");
        assert_eq!(frame.fills, [area(SCALED, 0)]);
        assert_eq!(frame.strips.len(), 4, "the highlight is outlined");
        assert_eq!(
            frame.grid.len(),
            6,
            "the display edges and the cross between quarters"
        );
        for rect in frame.grid.iter().chain(&frame.strips) {
            assert!(rect.x >= SCALED.x && rect.y >= SCALED.y);
            assert!(rect.x + rect.width <= SCALED.x + SCALED.width + 1e-9);
            assert!(rect.y + rect.height <= SCALED.y + SCALED.height + 1e-9);
        }
        for _ in 0..4 {
            r.handle(Action::Next);
        }
        assert_eq!(r.frame().fills, [SCALED]);
    }

    #[test]
    fn frame_draws_a_bounded_beam_inside_the_display_and_labels_the_phase() {
        let mut r = chosen(SCREEN, 1.0, 4);
        r.angle = TAU / 8.0;
        let frame = r.frame();
        assert!(frame.strips.len() <= MAX_DOTS + 1);
        assert!(frame.fills.is_empty() && frame.grid.is_empty());
        assert_eq!(
            frame.label.as_ref().unwrap().text,
            "Radar · Select a direction"
        );
        let full = frame.strips.len();
        r.handle(Action::Select);
        let frame = r.frame();
        assert_eq!(
            frame.strips.len(),
            2,
            "the beam starts as the origin and its tip"
        );
        assert!(frame.fills.is_empty());
        assert_eq!(frame.label.unwrap().text, "Radar · Select a distance");

        // It grows with the distance, and its tip is where the pointer goes.
        r.distance = r.reach() / 2.0;
        let half = r.frame();
        assert!(half.strips.len() > 2 && half.strips.len() < full);
        let tip = half.strips.last().unwrap();
        let (x, y) = r.target();
        assert!((tip.x + tip.width / 2.0 - f64::from(x)).abs() <= 1.0);
        assert!((tip.y + tip.height / 2.0 - f64::from(y)).abs() <= 1.0);
        r.distance = r.reach();
        assert_eq!(
            r.frame().strips.len(),
            full,
            "the full beam ends in its tip"
        );
        for rect in &r.frame().strips {
            assert!(rect.x + rect.width / 2.0 >= SCREEN.x - 1e-6);
            assert!(rect.y + rect.height / 2.0 >= SCREEN.y - 1e-6);
        }
    }

    #[test]
    fn a_chosen_quarter_stays_outlined_around_the_beam() {
        let r = chosen(SCREEN, 1.0, 3);
        let frame = r.frame();
        assert_eq!(&frame.strips[..4], &outline(area(SCREEN, 3), 2.0));
        assert!(frame.strips.len() <= MAX_DOTS + 5);
    }
}
