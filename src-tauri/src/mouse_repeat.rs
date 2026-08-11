use std::collections::HashMap;
use std::time::Instant;

use serde_json::Value;

use crate::state::AppSettings;

pub const INITIAL_SCALE: f64 = 0.25;
pub const MOVE_TICK_INTERVAL_MS: u64 = 8;
const MAX_MOVE_ELAPSED_MS: f64 = 16.0;
const PIXEL_EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatCommand {
    Move { dx: i32, dy: i32 },
    Scroll { dx: i32, dy: i32 },
}

impl RepeatCommand {
    pub fn parse(payload: &Value) -> Result<Self, String> {
        let command = payload
            .get("command")
            .and_then(Value::as_object)
            .ok_or_else(|| "Mouse repeat command is invalid.".to_string())?;
        let command_type = command
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| "Mouse repeat command is invalid.".to_string())?;
        let nested = command
            .get("payload")
            .ok_or_else(|| "Mouse repeat command is invalid.".to_string())?;
        let value = |name: &str, maximum: i32| {
            nested
                .get(name)
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && value.abs() <= f64::from(maximum))
                .map(|value| value.round() as i32)
                .ok_or_else(|| "Mouse repeat command is invalid.".to_string())
        };
        match command_type {
            "mouse.move" => Ok(Self::Move {
                dx: value("dx", 500)?,
                dy: value("dy", 500)?,
            }),
            "mouse.scroll" => Ok(Self::Scroll {
                dx: value("dx", 50)?,
                dy: value("dy", 50)?,
            }),
            _ => Err("Mouse repeat command is invalid.".into()),
        }
    }

    pub fn interval_ms(self, settings: &AppSettings) -> u32 {
        match self {
            Self::Move { .. } => settings.move_repeat_interval_ms,
            Self::Scroll { .. } => settings.scroll_repeat_interval_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveRepeat {
    pub generation: u64,
    pub command: RepeatCommand,
    pub acceleration_duration_ms: u32,
    movement: Option<MoveRepeatState>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MoveRepeatState {
    started_at: Instant,
    last_tick_at: Instant,
    residual_x: f64,
    residual_y: f64,
    initial_emitted: bool,
}

impl MoveRepeatState {
    fn new(now: Instant) -> Self {
        Self {
            started_at: now,
            last_tick_at: now,
            residual_x: 0.0,
            residual_y: 0.0,
            initial_emitted: false,
        }
    }

    fn initial_delta(&mut self, dx: i32, dy: i32) -> (i32, i32) {
        if self.initial_emitted {
            return (0, 0);
        }
        self.initial_emitted = true;
        let dominant = dx.abs().max(dy.abs());
        if dominant == 0 {
            return (0, 0);
        }
        let step_x = (f64::from(dx) / f64::from(dominant)).round() as i32;
        let step_y = (f64::from(dy) / f64::from(dominant)).round() as i32;
        self.residual_x -= f64::from(step_x);
        self.residual_y -= f64::from(step_y);
        (step_x, step_y)
    }

    fn advance(
        &mut self,
        now: Instant,
        dx: i32,
        dy: i32,
        move_interval_ms: u32,
        pointer_scale_percent: u8,
        acceleration_duration_ms: u32,
    ) -> (i32, i32) {
        let elapsed_ms = now
            .checked_duration_since(self.last_tick_at)
            .unwrap_or_default()
            .as_secs_f64()
            * 1000.0;
        self.last_tick_at = now;
        let credited_ms = elapsed_ms.min(MAX_MOVE_ELAPSED_MS);
        if credited_ms <= 0.0 {
            return (0, 0);
        }
        let total_elapsed_ms = now
            .checked_duration_since(self.started_at)
            .unwrap_or_default()
            .as_secs_f64()
            * 1000.0;
        let weighted_ms = acceleration_integral(
            total_elapsed_ms - credited_ms,
            total_elapsed_ms,
            acceleration_duration_ms,
        );
        let interval_ms = f64::from(move_interval_ms.max(1));
        let pointer_scale = f64::from(pointer_scale_percent) / 100.0;
        self.residual_x += f64::from(dx) * pointer_scale * weighted_ms / interval_ms;
        self.residual_y += f64::from(dy) * pointer_scale * weighted_ms / interval_ms;
        let whole_pixels = |value: f64| {
            if value >= 0.0 {
                (value + PIXEL_EPSILON).floor() as i32
            } else {
                (value - PIXEL_EPSILON).ceil() as i32
            }
        };
        let emitted_x = whole_pixels(self.residual_x);
        let emitted_y = whole_pixels(self.residual_y);
        self.residual_x -= f64::from(emitted_x);
        self.residual_y -= f64::from(emitted_y);
        (emitted_x, emitted_y)
    }
}

#[derive(Debug, Default)]
pub struct MouseRepeatController {
    next_generation: u64,
    active: HashMap<String, ActiveRepeat>,
}

impl MouseRepeatController {
    pub fn start(
        &mut self,
        device_id: String,
        command: RepeatCommand,
        acceleration_duration_ms: u32,
        now: Instant,
    ) -> ActiveRepeat {
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let active = ActiveRepeat {
            generation: self.next_generation,
            command,
            acceleration_duration_ms,
            movement: matches!(command, RepeatCommand::Move { .. })
                .then(|| MoveRepeatState::new(now)),
        };
        self.active.insert(device_id, active);
        active
    }

    pub fn current(&self, device_id: &str, generation: u64) -> Option<ActiveRepeat> {
        self.active
            .get(device_id)
            .copied()
            .filter(|active| active.generation == generation)
    }

    pub fn initial_move(&mut self, device_id: &str, generation: u64) -> Option<(i32, i32)> {
        let active = self.active.get_mut(device_id)?;
        if active.generation != generation {
            return None;
        }
        let RepeatCommand::Move { dx, dy } = active.command else {
            return None;
        };
        active
            .movement
            .as_mut()
            .map(|movement| movement.initial_delta(dx, dy))
    }

    pub fn advance_move(
        &mut self,
        device_id: &str,
        generation: u64,
        now: Instant,
        move_interval_ms: u32,
        pointer_scale_percent: u8,
    ) -> Option<(i32, i32)> {
        let active = self.active.get_mut(device_id)?;
        if active.generation != generation {
            return None;
        }
        let RepeatCommand::Move { dx, dy } = active.command else {
            return None;
        };
        active.movement.as_mut().map(|movement| {
            movement.advance(
                now,
                dx,
                dy,
                move_interval_ms,
                pointer_scale_percent,
                active.acceleration_duration_ms,
            )
        })
    }

    pub fn stop(&mut self, device_id: &str) -> Option<ActiveRepeat> {
        self.active.remove(device_id)
    }

    pub fn stop_if_current(&mut self, device_id: &str, generation: u64) -> bool {
        if self.current(device_id, generation).is_some() {
            self.active.remove(device_id);
            true
        } else {
            false
        }
    }

    pub fn stop_all(&mut self) -> Vec<ActiveRepeat> {
        self.active.drain().map(|(_, active)| active).collect()
    }
}

#[cfg(test)]
fn acceleration_scale(elapsed_ms: i64, duration_ms: u32) -> f64 {
    if duration_ms == 0 {
        return 1.0;
    }
    let progress = (elapsed_ms.max(0) as f64 / f64::from(duration_ms)).clamp(0.0, 1.0);
    let eased = progress * progress * (3.0 - 2.0 * progress);
    INITIAL_SCALE + (1.0 - INITIAL_SCALE) * eased
}

fn acceleration_integral(from_ms: f64, to_ms: f64, duration_ms: u32) -> f64 {
    fn integral_at(elapsed_ms: f64, duration_ms: u32) -> f64 {
        let elapsed_ms = elapsed_ms.max(0.0);
        if duration_ms == 0 {
            return elapsed_ms;
        }
        let duration = f64::from(duration_ms);
        let ramp_elapsed = elapsed_ms.min(duration);
        let progress = ramp_elapsed / duration;
        let ramp = duration
            * (INITIAL_SCALE * progress
                + (1.0 - INITIAL_SCALE) * (progress.powi(3) - 0.5 * progress.powi(4)));
        ramp + (elapsed_ms - duration).max(0.0)
    }

    integral_at(to_ms, duration_ms) - integral_at(from_ms, duration_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn parses_only_bounded_move_and_scroll_commands() {
        assert_eq!(
            RepeatCommand::parse(
                &json!({"command":{"type":"mouse.move","payload":{"dx":12,"dy":-6}}})
            ),
            Ok(RepeatCommand::Move { dx: 12, dy: -6 })
        );
        assert!(RepeatCommand::parse(
            &json!({"command":{"type":"mouse.move","payload":{"dx":501,"dy":0}}})
        )
        .is_err());
        assert!(
            RepeatCommand::parse(&json!({"command":{"type":"mouse.click","payload":{}}})).is_err()
        );
    }

    #[test]
    fn acceleration_uses_smoothstep_from_quarter_to_full_speed() {
        assert_eq!(acceleration_scale(0, 1000), 0.25);
        assert!((acceleration_scale(500, 1000) - 0.625).abs() < f64::EPSILON);
        assert_eq!(acceleration_scale(1000, 1000), 1.0);
        assert_eq!(acceleration_scale(0, 0), 1.0);
    }

    #[test]
    fn stale_generations_cannot_stop_replacements() {
        let mut controller = MouseRepeatController::default();
        let now = Instant::now();
        let first = controller.start(
            "device".into(),
            RepeatCommand::Move { dx: 1, dy: 0 },
            0,
            now,
        );
        let second = controller.start(
            "device".into(),
            RepeatCommand::Scroll { dx: 0, dy: 1 },
            0,
            now + Duration::from_millis(1),
        );
        assert!(!controller.stop_if_current("device", first.generation));
        assert!(controller.current("device", second.generation).is_some());
    }

    #[test]
    fn move_repeat_starts_with_a_directional_pixel_and_debits_it() {
        let now = Instant::now();
        let mut controller = MouseRepeatController::default();
        let active = controller.start(
            "device".into(),
            RepeatCommand::Move { dx: 10, dy: 4 },
            0,
            now,
        );

        assert_eq!(
            controller.initial_move("device", active.generation),
            Some((1, 0))
        );
        assert_eq!(
            controller.initial_move("device", active.generation),
            Some((0, 0))
        );
        assert_eq!(
            controller.advance_move(
                "device",
                active.generation,
                now + Duration::from_millis(10),
                100,
                100,
            ),
            Some((0, 0))
        );
        assert_eq!(
            controller.advance_move(
                "device",
                active.generation,
                now + Duration::from_millis(20),
                100,
                100,
            ),
            Some((1, 0))
        );
    }

    #[test]
    fn move_repeat_accumulates_fractional_scaled_displacement() {
        let now = Instant::now();
        let mut controller = MouseRepeatController::default();
        let active = controller.start(
            "device".into(),
            RepeatCommand::Move { dx: 4, dy: -2 },
            0,
            now,
        );
        assert_eq!(
            controller.initial_move("device", active.generation),
            Some((1, -1))
        );

        let mut emitted = (1, -1);
        for tick in 1..=25 {
            let next = controller
                .advance_move(
                    "device",
                    active.generation,
                    now + Duration::from_millis(tick * MOVE_TICK_INTERVAL_MS),
                    200,
                    50,
                )
                .unwrap();
            emitted.0 += next.0;
            emitted.1 += next.1;
        }
        assert_eq!(emitted, (2, -1));
    }

    #[test]
    fn move_repeat_caps_stall_catch_up() {
        let now = Instant::now();
        let mut controller = MouseRepeatController::default();
        let active = controller.start(
            "device".into(),
            RepeatCommand::Move { dx: 100, dy: 0 },
            0,
            now,
        );
        assert_eq!(
            controller.initial_move("device", active.generation),
            Some((1, 0))
        );
        assert_eq!(
            controller.advance_move(
                "device",
                active.generation,
                now + Duration::from_secs(10),
                100,
                100,
            ),
            Some((15, 0))
        );
    }

    #[test]
    fn move_repeat_acceleration_integrates_from_quarter_speed() {
        let now = Instant::now();
        let mut controller = MouseRepeatController::default();
        let active = controller.start(
            "device".into(),
            RepeatCommand::Move { dx: 1000, dy: 0 },
            1000,
            now,
        );
        assert_eq!(
            controller.initial_move("device", active.generation),
            Some((1, 0))
        );
        let next = controller
            .advance_move(
                "device",
                active.generation,
                now + Duration::from_millis(8),
                1000,
                100,
            )
            .unwrap();
        assert_eq!(next, (1, 0));
    }

    #[test]
    fn initial_move_handles_axes_diagonals_and_zero_vectors() {
        let now = Instant::now();
        for (index, (command, expected)) in [
            (RepeatCommand::Move { dx: 0, dy: -10 }, (0, -1)),
            (RepeatCommand::Move { dx: -10, dy: 0 }, (-1, 0)),
            (RepeatCommand::Move { dx: 10, dy: 10 }, (1, 1)),
            (RepeatCommand::Move { dx: -10, dy: 5 }, (-1, 1)),
            (RepeatCommand::Move { dx: 0, dy: 0 }, (0, 0)),
        ]
        .into_iter()
        .enumerate()
        {
            let device_id = format!("device-{index}");
            let mut controller = MouseRepeatController::default();
            let active = controller.start(device_id.clone(), command, 0, now);
            assert_eq!(
                controller.initial_move(&device_id, active.generation),
                Some(expected)
            );
        }
    }

    #[test]
    fn acceleration_integral_preserves_the_full_ramp_distance() {
        let now = Instant::now();
        let mut controller = MouseRepeatController::default();
        let active = controller.start(
            "device".into(),
            RepeatCommand::Move { dx: 1000, dy: 0 },
            1000,
            now,
        );
        let mut emitted_x = controller
            .initial_move("device", active.generation)
            .unwrap()
            .0;
        for tick in 1..=125 {
            emitted_x += controller
                .advance_move(
                    "device",
                    active.generation,
                    now + Duration::from_millis(tick * MOVE_TICK_INTERVAL_MS),
                    1000,
                    100,
                )
                .unwrap()
                .0;
        }
        assert_eq!(emitted_x, 625);
    }

    #[test]
    fn stale_generations_cannot_advance_replacement_movement() {
        let now = Instant::now();
        let mut controller = MouseRepeatController::default();
        let first = controller.start(
            "device".into(),
            RepeatCommand::Move { dx: 10, dy: 0 },
            0,
            now,
        );
        let second = controller.start(
            "device".into(),
            RepeatCommand::Move { dx: 0, dy: 10 },
            0,
            now,
        );

        assert_eq!(controller.initial_move("device", first.generation), None);
        assert_eq!(
            controller.advance_move(
                "device",
                first.generation,
                now + Duration::from_millis(8),
                100,
                100,
            ),
            None
        );
        assert_eq!(
            controller.initial_move("device", second.generation),
            Some((0, 1))
        );
    }
}
