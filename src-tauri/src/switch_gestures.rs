//! Pure Android-style release/hold selection. Cancellation never becomes an action.
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
}
impl Gestures {
    pub fn pressed(&mut self, id: &str, now: u64, settings: &Settings) {
        if !self.held.insert(id.into()) || self.held.len() != 1 {
            return;
        }
        if let Some(binding) = settings.bindings.iter().find(|b| b.id == id) {
            self.interval_ms = settings.hold_interval_ms;
            self.press = Some(Press {
                binding: binding.clone(),
                started: now,
            });
        }
    }
    fn candidate(&self, now: u64) -> Option<Action> {
        let p = self.press.as_ref()?;
        let elapsed = now.saturating_sub(p.started);
        if elapsed < self.interval_ms || p.binding.hold_actions.is_empty() {
            return None;
        }
        let index =
            (elapsed / self.interval_ms - 1).min(p.binding.hold_actions.len() as u64 - 1) as usize;
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
        self.press = None;
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
    fn boundary_and_last_action_match_android() {
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
