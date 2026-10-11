//! Radar pointer targeting for the mouse panel. A beam turns around the
//! pointer, then a marker travels out along it. Pure geometry; no OS input.
use crate::{
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Angle,
    Distance,
}

pub struct Radar {
    options: Resolved,
    /// Logical pixels per second, the same as point scanning.
    speed: f64,
    screen: Rect,
    units: f64,
    origin: (f64, f64),
    pub phase: Phase,
    /// Radians clockwise from straight up.
    angle: f64,
    /// Display units from the origin along the beam.
    distance: f64,
    direction: f64,
    /// Full automatic passes of the current phase since it began.
    cycles: usize,
    /// Time since the last counted pass, so a short beam is not used up at once.
    pass_ms: u64,
}

impl Radar {
    /// `speed` is the point scan speed index. The origin is kept on the display.
    pub fn new(
        options: Resolved,
        speed: usize,
        screen: Rect,
        units: f64,
        origin: (f64, f64),
    ) -> Self {
        let mut radar = Self {
            options,
            speed: [45.0, 75.0, 120.0, 180.0, 270.0][speed.min(4)],
            screen,
            units,
            origin: (
                origin.0.clamp(screen.x, screen.x + screen.width - 1.0),
                origin.1.clamp(screen.y, screen.y + screen.height - 1.0),
            ),
            phase: Phase::Angle,
            angle: 0.0,
            distance: 0.0,
            direction: 1.0,
            cycles: 0,
            pass_ms: 0,
        };
        radar.direction = radar.initial_direction();
        radar
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
    /// How far the beam reaches before leaving the display.
    pub fn reach(&self) -> f64 {
        let (dx, dy) = self.unit();
        let (x, y) = self.origin;
        let right = self.screen.x + self.screen.width - 1.0;
        let bottom = self.screen.y + self.screen.height - 1.0;
        let along = |position: f64, delta: f64, low: f64, high: f64| {
            if delta > 1e-9 {
                (high - position) / delta
            } else if delta < -1e-9 {
                (low - position) / delta
            } else {
                f64::INFINITY
            }
        };
        along(x, dx, self.screen.x, right)
            .min(along(y, dy, self.screen.y, bottom))
            .max(0.0)
    }
    fn at(&self, distance: f64) -> (f64, f64) {
        let (dx, dy) = self.unit();
        (self.origin.0 + dx * distance, self.origin.1 + dy * distance)
    }
    pub fn target(&self) -> (i32, i32) {
        let (x, y) = self.at(self.distance);
        (
            x.round()
                .clamp(self.screen.x, self.screen.x + self.screen.width - 1.0) as i32,
            y.round()
                .clamp(self.screen.y, self.screen.y + self.screen.height - 1.0) as i32,
        )
    }
    pub fn exhausted(&self) -> bool {
        self.options.exhausted(self.cycles)
    }
    /// Select fixes the angle, then returns the chosen point.
    pub fn handle(&mut self, action: Action) -> Option<(i32, i32)> {
        match action {
            Action::Select => {
                self.cycles = 0;
                self.pass_ms = 0;
                self.direction = self.initial_direction();
                match self.phase {
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
    pub fn frame(&self) -> Frame {
        let t = 2.0 * self.units * self.options.thickness.scale();
        let reach = self.reach();
        let dots = ((reach / (DOT_SPACING * self.units)).ceil() as usize).clamp(1, MAX_DOTS);
        let beam: Vec<Rect> = (1..=dots)
            .map(|dot| self.square(self.at(reach * dot as f64 / dots as f64), t * 2.0))
            .collect();
        let origin = self.square(self.origin, t * 4.0);
        let mut strips = vec![];
        let mut fills = vec![];
        match self.phase {
            Phase::Angle => {
                strips.push(origin);
                strips.extend(beam);
            }
            Phase::Distance => {
                // The chosen direction stays faint while the marker moves.
                fills.push(origin);
                fills.extend(beam);
                strips.extend(crate::scan_menu::outline(
                    self.square(self.at(self.distance), 20.0 * self.units),
                    t,
                ));
            }
        }
        let width = (480.0 * self.units).min(self.screen.width);
        Frame {
            fills,
            strips,
            label: Some(FrameLabel {
                text: match self.phase {
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
            }),
            ..Frame::default()
        }
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

    fn radar(origin: (f64, f64)) -> Radar {
        Radar::new(Resolved::default(), 2, SCREEN, 1.0, origin)
    }

    #[test]
    fn beam_reaches_the_display_edge_in_each_direction() {
        let mut r = radar((-500.0, 500.0));
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
    fn select_fixes_the_angle_then_returns_the_marker_on_the_display() {
        let mut r = radar((-500.0, 500.0));
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
    fn origin_off_the_display_is_brought_onto_it() {
        let r = radar((50.0, -20.0));
        assert_eq!(r.origin, (-1.0, 100.0));
        assert_eq!(r.target(), (-1, 100));
    }

    #[test]
    fn automatic_passes_count_towards_the_limit_but_manual_steps_do_not() {
        let mut r = radar((-500.0, 500.0));
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
        // On the top edge, straight up reaches nowhere.
        let mut r = radar((-500.0, 100.0));
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
            (-500.0, 500.0),
        );
        r.advance(100);
        assert!(r.angle > TAU * 0.9);
        assert_eq!(r.cycles, 0, "leaving straight up is not a pass");
        r.handle(Action::Select);
        assert!((r.distance - r.reach()).abs() < 1e-9);
        r.advance(100);
        assert!(r.distance < r.reach());
    }

    #[test]
    fn frame_draws_a_bounded_beam_inside_the_display_and_labels_the_phase() {
        let mut r = radar((-500.0, 500.0));
        r.angle = TAU / 8.0;
        let frame = r.frame();
        assert!(frame.strips.len() <= MAX_DOTS + 1);
        assert!(frame.fills.is_empty());
        assert_eq!(
            frame.label.as_ref().unwrap().text,
            "Radar · Select a direction"
        );
        r.handle(Action::Select);
        let frame = r.frame();
        assert_eq!(frame.strips.len(), 4, "marker outline");
        assert!(!frame.fills.is_empty());
        assert_eq!(frame.label.unwrap().text, "Radar · Select a distance");
        for rect in frame.fills.iter().chain(&frame.strips) {
            assert!(rect.x + rect.width / 2.0 >= SCREEN.x - 1e-6);
            assert!(rect.y + rect.height / 2.0 >= SCREEN.y - 1e-6);
        }
    }
}
