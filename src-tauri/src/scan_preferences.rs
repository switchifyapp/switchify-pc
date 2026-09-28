use crate::scanning::ScannerColor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    #[default]
    Forward,
    Reverse,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pattern {
    #[default]
    Grouped,
    Linear,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Thickness {
    Thin,
    #[default]
    Standard,
    Thick,
}
impl Thickness {
    pub fn scale(self) -> f64 {
        match self {
            Self::Thin => 0.5,
            Self::Standard => 1.0,
            Self::Thick => 2.0,
        }
    }
}
/// Whether scanning moves on by itself after a selection that performed
/// an action.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NextScan {
    /// What the scanner has always done.
    #[default]
    Standard,
    Automatic,
    Wait,
}
/// Where scanning starts after a selection that performed an action.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartFrom {
    /// What the scanner has always done.
    #[default]
    Standard,
    Beginning,
    Selection,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Point,
    Menu,
    Keyboard,
    Mouse,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Overrides {
    pub automatic: Option<bool>,
    pub interval_ms: Option<u64>,
    pub direction: Option<Direction>,
    pub pass_limit: Option<usize>,
    pub pattern: Option<Pattern>,
    pub color: Option<ScannerColor>,
    pub thickness: Option<Thickness>,
    pub next_scan: Option<NextScan>,
    pub start_from: Option<StartFrom>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub direction: Direction,
    pub pass_limit: usize,
    pub pattern: Pattern,
    pub thickness: Thickness,
    pub next_scan: NextScan,
    pub start_from: StartFrom,
    pub point: Overrides,
    pub menu: Overrides,
    pub keyboard: Overrides,
    pub mouse: Overrides,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            direction: Direction::Forward,
            pass_limit: 3,
            pattern: Pattern::Grouped,
            thickness: Thickness::Standard,
            next_scan: NextScan::Standard,
            start_from: StartFrom::Standard,
            point: Default::default(),
            menu: Default::default(),
            keyboard: Default::default(),
            mouse: Default::default(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolved {
    pub automatic: bool,
    pub interval_ms: u64,
    pub direction: Direction,
    pub pass_limit: usize,
    pub pattern: Pattern,
    pub color: ScannerColor,
    pub thickness: Thickness,
    pub next_scan: NextScan,
    pub start_from: StartFrom,
}
impl Default for Resolved {
    fn default() -> Self {
        Preferences::default().resolve(Area::Point, true, 1000, ScannerColor::default())
    }
}
impl Resolved {
    pub fn exhausted(self, passes: usize) -> bool {
        self.pass_limit != 0 && passes >= self.pass_limit
    }
}
fn valid_passes(value: usize) -> bool {
    matches!(value, 0 | 1 | 2 | 3 | 5)
}
impl Preferences {
    pub fn resolve(
        &self,
        area: Area,
        automatic: bool,
        interval_ms: u64,
        color: ScannerColor,
    ) -> Resolved {
        let local = match area {
            Area::Point => self.point,
            Area::Menu => self.menu,
            Area::Keyboard => self.keyboard,
            Area::Mouse => self.mouse,
        };
        Resolved {
            automatic: local.automatic.unwrap_or(automatic),
            interval_ms: local
                .interval_ms
                .filter(|v| (100..=10000).contains(v))
                .unwrap_or(interval_ms),
            direction: local.direction.unwrap_or(self.direction),
            pass_limit: local.pass_limit.filter(|v| valid_passes(*v)).unwrap_or(
                if valid_passes(self.pass_limit) {
                    self.pass_limit
                } else {
                    3
                },
            ),
            pattern: if area == Area::Point {
                Pattern::Grouped
            } else {
                local.pattern.unwrap_or(self.pattern)
            },
            color: local.color.unwrap_or(color),
            thickness: local.thickness.unwrap_or(self.thickness),
            next_scan: local.next_scan.unwrap_or(self.next_scan),
            start_from: local.start_from.unwrap_or(self.start_from),
        }
    }
}
pub fn deserialize_preferences<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Preferences, D::Error> {
    let mut value = serde_json::Value::deserialize(deserializer)?;
    // A choice this version does not know must not cost the other settings.
    forget_unknown_choices(&mut value);
    for area in ["point", "menu", "keyboard", "mouse"] {
        if let Some(local) = value.get_mut(area) {
            forget_unknown_choices(local);
        }
    }
    let mut preferences: Preferences = serde_json::from_value(value).unwrap_or_default();
    if !valid_passes(preferences.pass_limit) {
        preferences.pass_limit = 3;
    }
    for local in [
        &mut preferences.point,
        &mut preferences.menu,
        &mut preferences.keyboard,
        &mut preferences.mouse,
    ] {
        local.interval_ms = local.interval_ms.filter(|v| (100..=10000).contains(v));
        local.pass_limit = local.pass_limit.filter(|v| valid_passes(*v));
    }
    Ok(preferences)
}
fn forget_unknown_choices(value: &mut serde_json::Value) {
    let Some(fields) = value.as_object_mut() else {
        return;
    };
    fields.retain(|name, choice| match name.as_str() {
        "nextScan" => choice.is_null() || NextScan::deserialize(&*choice).is_ok(),
        "startFrom" => choice.is_null() || StartFrom::deserialize(&*choice).is_ok(),
        _ => true,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read(value: serde_json::Value) -> Preferences {
        deserialize_preferences(value).unwrap()
    }
    #[test]
    fn after_selection_choices_default_to_standard_and_round_trip() {
        let saved = read(serde_json::json!({"direction":"reverse"}));
        assert_eq!(saved.next_scan, NextScan::Standard);
        assert_eq!(saved.start_from, StartFrom::Standard);
        for area in [Area::Point, Area::Menu, Area::Keyboard, Area::Mouse] {
            let resolved = saved.resolve(area, true, 1000, ScannerColor::Blue);
            assert_eq!(resolved.next_scan, NextScan::Standard);
            assert_eq!(resolved.start_from, StartFrom::Standard);
        }
        let mut chosen = Preferences {
            next_scan: NextScan::Wait,
            start_from: StartFrom::Selection,
            ..Default::default()
        };
        chosen.menu.next_scan = Some(NextScan::Automatic);
        chosen.mouse.start_from = Some(StartFrom::Beginning);
        let stored = serde_json::to_value(&chosen).unwrap();
        assert_eq!(stored["nextScan"], "wait");
        assert_eq!(stored["startFrom"], "selection");
        assert_eq!(stored["menu"]["nextScan"], "automatic");
        assert_eq!(stored["mouse"]["startFrom"], "beginning");
        assert_eq!(read(stored), chosen);
        let menu = chosen.resolve(Area::Menu, true, 1000, ScannerColor::Blue);
        assert_eq!(menu.next_scan, NextScan::Automatic);
        assert_eq!(menu.start_from, StartFrom::Selection);
        let mouse = chosen.resolve(Area::Mouse, true, 1000, ScannerColor::Blue);
        assert_eq!(mouse.next_scan, NextScan::Wait);
        assert_eq!(mouse.start_from, StartFrom::Beginning);
    }
    #[test]
    fn an_unknown_choice_falls_back_alone() {
        let saved = read(serde_json::json!({
            "direction": "reverse",
            "passLimit": 5,
            "nextScan": "sometimes",
            "startFrom": 7,
            "keyboard": {"intervalMs": 250, "nextScan": "later", "startFrom": "selection"},
            "menu": {"nextScan": "wait", "startFrom": null},
        }));
        assert_eq!(saved.direction, Direction::Reverse);
        assert_eq!(saved.pass_limit, 5);
        assert_eq!(saved.next_scan, NextScan::Standard);
        assert_eq!(saved.start_from, StartFrom::Standard);
        assert_eq!(saved.keyboard.interval_ms, Some(250));
        assert_eq!(saved.keyboard.next_scan, None);
        assert_eq!(saved.keyboard.start_from, Some(StartFrom::Selection));
        assert_eq!(saved.menu.next_scan, Some(NextScan::Wait));
        assert_eq!(saved.menu.start_from, None);
    }
    #[test]
    fn areas_inherit_and_override_independently() {
        let mut settings = Preferences::default();
        settings.keyboard.automatic = Some(false);
        settings.keyboard.interval_ms = Some(250);
        settings.keyboard.pass_limit = Some(0);
        settings.keyboard.color = Some(ScannerColor::Red);
        let keyboard = settings.resolve(Area::Keyboard, true, 1500, ScannerColor::Green);
        assert!(!keyboard.automatic);
        assert_eq!(keyboard.interval_ms, 250);
        assert!(!keyboard.exhausted(10000));
        assert_eq!(keyboard.color, ScannerColor::Red);
        for area in [Area::Point, Area::Menu] {
            let inherited = settings.resolve(area, true, 1500, ScannerColor::Green);
            assert!(inherited.automatic);
            assert_eq!(inherited.interval_ms, 1500);
            assert_eq!(inherited.color, ScannerColor::Green);
        }
    }
    #[test]
    fn invalid_numeric_values_use_validated_defaults() {
        let settings = Preferences {
            pass_limit: 99,
            menu: Overrides {
                interval_ms: Some(0),
                pass_limit: Some(9),
                ..Default::default()
            },
            ..Default::default()
        };
        let resolved = settings.resolve(Area::Menu, true, 1000, ScannerColor::Blue);
        assert_eq!(resolved.interval_ms, 1000);
        assert_eq!(resolved.pass_limit, 3);
    }
}
