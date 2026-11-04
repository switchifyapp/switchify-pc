//! Pure Mobile-style release/hold selection. Cancellation never becomes an action.
use crate::{
    scanning::Action,
    switches::{Binding, Settings},
};
use serde::Serialize;
use std::collections::HashSet;
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub switch_name: String,
    pub action: Action,
}
struct Press {
    binding: Binding,
    started: u64,
}
#[derive(Default)]
pub struct Gestures {
    held: HashSet<String>,
    press: Option<Press>,
    interval_ms: u64,
    accept_ms: u64,
    /// When the last press that counted was released.
    last_counted: Option<u64>,
}
impl Gestures {
    #[cfg(test)]
    pub fn pressed(&mut self, id: &str, now: u64, settings: &Settings) {
        self.pressed_for_scan(id, now, settings, false);
    }
    pub fn pressed_for_scan(&mut self, id: &str, now: u64, settings: &Settings, countdown: bool) {
        if !self.held.insert(id.into()) || self.held.len() != 1 {
            return;
        }
        // Too soon after the last press that counted: still held, so movement
        // freezes, but its release does nothing.
        if self
            .last_counted
            .is_some_and(|last| now.saturating_sub(last) < settings.ignore_repeat_ms)
        {
            return;
        }
        if let Some(binding) = settings.bindings.iter().find(|b| b.id == id) {
            let mut binding = binding.clone();
            if countdown
                && !binding.hold_actions.contains(&Action::OpenKeyboard)
                && !binding.hold_actions.contains(&Action::OpenPoint)
                && !binding.hold_actions.contains(&Action::OpenMouse)
                && !matches!(
                    binding.press_action,
                    Action::Stop
                        | Action::Pause
                        | Action::Cancel
                        | Action::OpenKeyboard
                        | Action::OpenPoint
                        | Action::OpenMouse
                )
            {
                binding.press_action = Action::Select;
                binding.hold_actions.clear();
            }
            self.interval_ms = settings.hold_interval_ms;
            self.accept_ms = settings.accept_ms;
            self.press = Some(Press {
                binding: binding.clone(),
                started: now,
            });
        }
    }
    fn candidate(&self, now: u64) -> Option<Action> {
        let p = self.press.as_ref()?;
        // Hold actions count from when the press counts, so the press action
        // always has a full interval of its own.
        let held = now.saturating_sub(p.started).checked_sub(self.accept_ms)?;
        if held < self.interval_ms || p.binding.hold_actions.is_empty() {
            return None;
        }
        let index =
            (held / self.interval_ms - 1).min(p.binding.hold_actions.len() as u64 - 1) as usize;
        Some(p.binding.hold_actions[index])
    }
    pub fn prompt(&self, now: u64) -> Option<Prompt> {
        Some(Prompt {
            switch_name: self.press.as_ref()?.binding.name.clone(),
            action: self.candidate(now)?,
        })
    }
    pub fn released(&mut self, id: &str, now: u64) -> Option<Action> {
        if !self.held.remove(id) || self.press.as_ref().is_none_or(|p| p.binding.id != id) {
            return None;
        }
        let action = self
            .candidate(now)
            .unwrap_or_else(|| self.press.as_ref().unwrap().binding.press_action);
        let started = self.press.take().unwrap().started;
        // Too short to count, such as a tremor or a bump.
        if now.saturating_sub(started) < self.accept_ms {
            return None;
        }
        self.last_counted = Some(now);
        Some(action)
    }
    pub fn held(&self) -> bool {
        !self.held.is_empty()
    }
    pub fn cancel(&mut self) {
        self.held.clear();
        self.press = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> Settings {
        Settings {
            bindings: vec![
                Binding {
                    id: "one".into(),
                    name: "Head switch".into(),
                    key: "Space".into(),
                    press_action: Action::Select,
                    hold_actions: vec![Action::Next, Action::Stop],
                },
                Binding {
                    id: "two".into(),
                    name: "Other".into(),
                    key: "Enter".into(),
                    press_action: Action::Back,
                    hold_actions: vec![],
                },
            ],
            ..Settings::default()
        }
    }
    #[test]
    fn countdown_preserves_direct_and_held_panel_actions() {
        let mut settings = Settings {
            bindings: vec![Binding {
                id: "keyboard".into(),
                name: "Keyboard".into(),
                key: "Space".into(),
                press_action: Action::OpenKeyboard,
                hold_actions: vec![],
            }],
            ..Default::default()
        };
        let mut gestures = Gestures::default();
        gestures.pressed_for_scan("keyboard", 0, &settings, true);
        assert_eq!(
            gestures.released("keyboard", 100),
            Some(Action::OpenKeyboard)
        );
        settings.bindings[0].press_action = Action::Select;
        settings.bindings[0].hold_actions = vec![Action::OpenKeyboard];
        gestures.pressed_for_scan("keyboard", 1000, &settings, true);
        assert_eq!(
            gestures.released("keyboard", 2100),
            Some(Action::OpenKeyboard)
        );
        settings.bindings[0].press_action = Action::OpenMouse;
        settings.bindings[0].hold_actions.clear();
        gestures.pressed_for_scan("keyboard", 3000, &settings, true);
        assert_eq!(gestures.released("keyboard", 3100), Some(Action::OpenMouse));
    }
    #[test]
    fn countdown_press_overrides_normal_actions_and_holds_once() {
        for id in ["one", "two"] {
            for duration in [10, 999, 2500] {
                let mut gestures = Gestures::default();
                gestures.pressed_for_scan(id, 0, &settings(), true);
                assert!(gestures.held());
                assert!(gestures.prompt(duration).is_none());
                assert_eq!(gestures.released(id, duration), Some(Action::Select));
                assert_eq!(gestures.released(id, duration), None);
            }
        }
    }
    #[test]
    fn countdown_preserves_safety_switches_and_cancellation() {
        for action in [Action::Stop, Action::Pause, Action::Cancel] {
            let mut settings = settings();
            settings.bindings[0].press_action = action;
            let mut gestures = Gestures::default();
            gestures.pressed_for_scan("one", 0, &settings, true);
            assert_eq!(gestures.released("one", 10), Some(action));
            gestures.pressed_for_scan("one", 20, &settings, true);
            gestures.cancel();
            assert_eq!(gestures.released("one", 30), None);
        }
    }
    #[test]
    fn overlapping_countdown_switches_do_not_select_a_menu_tile() {
        let mut gestures = Gestures::default();
        gestures.pressed_for_scan("one", 0, &settings(), true);
        gestures.pressed_for_scan("two", 10, &settings(), true);
        assert_eq!(gestures.released("one", 20), Some(Action::Select));
        assert_eq!(gestures.released("two", 30), None);
    }
    #[test]
    fn boundary_and_last_action_match_mobile() {
        for (duration, expected) in [
            (999, Action::Select),
            (1000, Action::Next),
            (1999, Action::Next),
            (2000, Action::Stop),
            (9000, Action::Stop),
        ] {
            let mut g = Gestures::default();
            g.pressed("one", 0, &settings());
            assert_eq!(g.released("one", duration), Some(expected));
            assert_eq!(g.released("one", duration), None);
        }
    }
    #[test]
    fn cancellation_discards_hold_and_release() {
        let mut g = Gestures::default();
        g.pressed("one", 0, &settings());
        assert_eq!(g.prompt(1000).unwrap().action, Action::Next);
        g.cancel();
        assert_eq!(g.released("one", 2000), None);
        assert!(g.prompt(2000).is_none());
    }
    #[test]
    fn a_press_shorter_than_the_hold_time_is_ignored() {
        let settings = Settings {
            accept_ms: 300,
            ..settings()
        };
        let mut g = Gestures::default();
        g.pressed("one", 0, &settings);
        assert!(g.held());
        assert_eq!(g.released("one", 299), None);
        assert!(!g.held());
        g.pressed("one", 1000, &settings);
        assert_eq!(g.released("one", 1300), Some(Action::Select));
        // Hold actions start once the press counts, so a hold time longer
        // than the hold interval still leaves room for the press action.
        let slow = Settings {
            accept_ms: 1500,
            ..settings
        };
        g.pressed("one", 5000, &slow);
        assert!(g.prompt(7499).is_none());
        assert_eq!(g.prompt(7500).unwrap().action, Action::Next);
        assert_eq!(g.prompt(8500).unwrap().action, Action::Stop);
        assert_eq!(g.released("one", 8500), Some(Action::Stop));
        for (release, expected) in [
            (10_499, None),
            (10_500, Some(Action::Select)),
            (11_499, Some(Action::Select)),
            (11_500, Some(Action::Next)),
        ] {
            g.pressed("one", 9000, &slow);
            assert_eq!(g.released("one", release), expected);
        }
    }
    #[test]
    fn presses_too_soon_after_one_that_counted_are_ignored() {
        let settings = Settings {
            ignore_repeat_ms: 500,
            ..settings()
        };
        let mut g = Gestures::default();
        g.pressed("one", 0, &settings);
        assert_eq!(g.released("one", 100), Some(Action::Select));
        // A bounce, and another switch, inside the window.
        g.pressed("one", 150, &settings);
        assert!(g.held());
        assert_eq!(g.released("one", 200), None);
        g.pressed("two", 599, &settings);
        assert_eq!(g.released("two", 2000), None);
        // An ignored press does not extend the window.
        g.pressed("two", 2100, &settings);
        assert_eq!(g.released("two", 2200), Some(Action::Back));
        // A press too short to count does not start one.
        let both = Settings {
            accept_ms: 300,
            ..settings
        };
        g.pressed("one", 3000, &both);
        assert_eq!(g.released("one", 3100), None);
        g.pressed("one", 3200, &both);
        assert_eq!(g.released("one", 3500), Some(Action::Select));
    }
    #[test]
    fn timing_off_counts_every_press() {
        let mut g = Gestures::default();
        for start in [0, 1, 2] {
            g.pressed("one", start * 10, &settings());
            assert_eq!(g.released("one", start * 10 + 1), Some(Action::Select));
        }
    }
    #[test]
    fn first_switch_owns_gesture_and_repeats_do_not_restart_it() {
        let mut g = Gestures::default();
        g.pressed("one", 0, &settings());
        g.pressed("one", 700, &settings());
        g.pressed("two", 800, &settings());
        assert_eq!(g.released("one", 1000), Some(Action::Next));
        assert!(g.held());
        assert_eq!(g.released("two", 1200), None);
        assert!(!g.held());
    }
}
