//! Persisted switch assignments, independent of capture and scanning techniques.
use crate::scanning::Action;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub id: String,
    pub name: String,
    pub key: String,
    pub press_action: Action,
    pub hold_actions: Vec<Action>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub hold_interval_ms: u64,
    pub bindings: Vec<Binding>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            hold_interval_ms: 1000,
            bindings: vec![],
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || !(250..=5000).contains(&self.hold_interval_ms)
            || self.bindings.len() > 128
        {
            return Err("Unsupported switch settings or hold interval.".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut keys = std::collections::HashSet::new();
        for b in &self.bindings {
            if b.id.is_empty()
                || b.id.len() > 128
                || !ids.insert(&b.id)
                || b.name.trim().is_empty()
                || b.name.chars().count() > 64
                || b.hold_actions.len() > 32
            {
                return Err(
                    "Each switch needs a name, a unique ID, and at most 32 hold actions.".into(),
                );
            }
            let key = crate::switch_input::normalize_key(&b.key)
                .ok_or("Choose a supported switch key.")?;
            if key == "Escape" || !keys.insert(key) {
                return Err(
                    "Each switch needs a different key. Escape is reserved for disable.".into(),
                );
            }
            if std::iter::once(&b.press_action)
                .chain(&b.hold_actions)
                .any(|a| *a == Action::Cancel)
            {
                return Err("Choose a scanning action for each switch.".into());
            }
        }
        Ok(())
    }
    pub fn validate_actions(&self, automatic: bool) -> Result<(), String> {
        self.validate()?;
        let has = |action| {
            self.bindings
                .iter()
                .any(|b| b.press_action == action || b.hold_actions.contains(&action))
        };
        if !has(Action::Select) {
            return Err(
                "Scanning starts once a switch has the Select action. Add one in Settings → Switches."
                    .into(),
            );
        }
        if !automatic && (!has(Action::Next) || !has(Action::Back)) {
            return Err(
                "Manual scanning starts once switches cover Select, Next and Previous.".into(),
            );
        }
        Ok(())
    }
    pub fn escape_ms(&self) -> u64 {
        let n = self
            .bindings
            .iter()
            .map(|b| b.hold_actions.len())
            .max()
            .unwrap_or(0) as u64;
        if n == 0 {
            4000
        } else {
            4000.max((n + 2) * self.hold_interval_ms)
        }
    }
    pub fn migrate(config: &crate::point_scan::Config) -> Self {
        Self {
            bindings: [
                ("select", &config.select_key, Action::Select),
                ("next", &config.next_key, Action::Next),
                ("previous", &config.back_key, Action::Back),
                ("pause", &config.pause_key, Action::Pause),
            ]
            .into_iter()
            .map(|(id, key, action)| Binding {
                id: format!("legacy-{id}"),
                name: action.label().into(),
                key: key.clone(),
                press_action: action,
                hold_actions: vec![],
            })
            .collect(),
            ..Self::default()
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_bindings_migrate_without_losing_keys() {
        let c = crate::point_scan::Config::default();
        let s = Settings::migrate(&c);
        assert_eq!(s.bindings[2].key, "Backspace");
        assert_eq!(s.bindings[3].key, "F8");
        s.validate_actions(false).unwrap();
        assert_eq!(
            serde_json::from_value::<Settings>(serde_json::to_value(&s).unwrap()).unwrap(),
            s
        );
    }
    #[test]
    fn hold_actions_satisfy_requirements_and_extend_escape() {
        let mut s = Settings::migrate(&crate::point_scan::Config::default());
        s.bindings.truncate(1);
        s.bindings[0].hold_actions = vec![Action::Next, Action::Back, Action::Stop];
        s.validate_actions(false).unwrap();
        assert_eq!(s.escape_ms(), 5000);
        s.bindings[0].hold_actions.clear();
        assert!(s.validate_actions(false).is_err());
    }
    #[test]
    fn duplicate_keys_and_reserved_actions_are_rejected() {
        let mut s = Settings::migrate(&crate::point_scan::Config::default());
        s.bindings[1].key = "Space".into();
        assert!(s.validate().is_err());
        s.bindings[1].key = "Escape".into();
        assert!(s.validate().is_err());
    }
}
