use std::collections::HashMap;

use serde_json::Value;

use crate::state::AppSettings;

pub const INITIAL_SCALE: f64 = 0.25;

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

    pub fn scaled(self, scale: f64) -> Self {
        let scaled = |value: i32| (f64::from(value) * scale).round() as i32;
        match self {
            Self::Move { dx, dy } => Self::Move {
                dx: scaled(dx),
                dy: scaled(dy),
            },
            Self::Scroll { dx, dy } => Self::Scroll {
                dx: scaled(dx),
                dy: scaled(dy),
            },
        }
    }

    pub fn interval_ms(self, settings: &AppSettings) -> u32 {
        match self {
            Self::Move { .. } => settings.move_repeat_interval_ms,
            Self::Scroll { .. } => settings.scroll_repeat_interval_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveRepeat {
    pub generation: u64,
    pub command: RepeatCommand,
    pub started_at_ms: i64,
    pub acceleration_duration_ms: u32,
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
        now_ms: i64,
    ) -> ActiveRepeat {
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let active = ActiveRepeat {
            generation: self.next_generation,
            command,
            started_at_ms: now_ms,
            acceleration_duration_ms,
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

pub fn acceleration_scale(elapsed_ms: i64, duration_ms: u32) -> f64 {
    if duration_ms == 0 {
        return 1.0;
    }
    let progress = (elapsed_ms.max(0) as f64 / f64::from(duration_ms)).clamp(0.0, 1.0);
    let eased = progress * progress * (3.0 - 2.0 * progress);
    INITIAL_SCALE + (1.0 - INITIAL_SCALE) * eased
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        let first = controller.start("device".into(), RepeatCommand::Move { dx: 1, dy: 0 }, 0, 0);
        let second = controller.start(
            "device".into(),
            RepeatCommand::Scroll { dx: 0, dy: 1 },
            0,
            1,
        );
        assert!(!controller.stop_if_current("device", first.generation));
        assert!(controller.current("device", second.generation).is_some());
    }
}
