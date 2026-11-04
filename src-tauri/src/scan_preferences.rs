pub use crate::panel_theme::PanelTheme;
use crate::scanning::ScannerColor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    #[default]
    Forward,
    Reverse,
}
/// Row groups are saved as a separate `rowGroups` choice, which only applies to
/// `Grouped`, so versions without them still read the pattern.
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
/// Shared visual size for panels; never changes point scanning or pointer markers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PanelSize {
    Small,
    #[default]
    Medium,
    Large,
}
impl PanelSize {
    pub fn scale(self) -> f64 {
        match self {
            Self::Small => 0.8,
            Self::Medium => 1.0,
            Self::Large => 1.2,
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
    /// Missing beside an area's own pattern means no row groups, as an area's
    /// pattern chosen before row groups existed meant.
    pub row_groups: Option<bool>,
    #[serde(deserialize_with = "deserialize_color_override")]
    pub color: Option<ScannerColor>,
    pub thickness: Option<Thickness>,
    pub next_scan: Option<NextScan>,
    pub start_from: Option<StartFrom>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub panel_size: PanelSize,
    /// Shared by every panel, like `panel_size`.
    pub panel_theme: PanelTheme,
    pub direction: Direction,
    pub pass_limit: usize,
    pub pattern: Pattern,
    pub row_groups: bool,
    pub thickness: Thickness,
    pub next_scan: NextScan,
    pub start_from: StartFrom,
    /// Extra time on the first item after scanning starts or enters a group,
    /// shared by menus, the keyboard and the mouse panel. 0 is off.
    pub first_item_ms: u64,
    /// How long each step inside a group takes, as a percentage of the scan
    /// rate. 100 is the same rate.
    pub group_rate_percent: u16,
    pub point: Overrides,
    pub menu: Overrides,
    pub keyboard: Overrides,
    pub mouse: Overrides,
}
pub const MAX_FIRST_ITEM_MS: u64 = 5000;
fn valid_group_rate(value: u16) -> bool {
    matches!(value, 100 | 150 | 200)
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            panel_size: PanelSize::Medium,
            panel_theme: PanelTheme::System,
            direction: Direction::Forward,
            pass_limit: 3,
            pattern: Pattern::Grouped,
            row_groups: false,
            thickness: Thickness::Standard,
            next_scan: NextScan::Standard,
            start_from: StartFrom::Standard,
            first_item_ms: 0,
            group_rate_percent: 100,
            point: Default::default(),
            menu: Default::default(),
            keyboard: Default::default(),
            mouse: Default::default(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolved {
    pub panel_size: PanelSize,
    pub panel_theme: PanelTheme,
    pub automatic: bool,
    pub interval_ms: u64,
    pub direction: Direction,
    pub pass_limit: usize,
    pub pattern: Pattern,
    /// Long rows are split into groups. Only used with `Pattern::Grouped`.
    pub row_groups: bool,
    pub color: ScannerColor,
    pub thickness: Thickness,
    pub next_scan: NextScan,
    pub start_from: StartFrom,
    pub first_item_ms: u64,
    pub group_rate_percent: u16,
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
            panel_size: self.panel_size,
            panel_theme: self.panel_theme,
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
            row_groups: area != Area::Point
                && local
                    .row_groups
                    .unwrap_or(local.pattern.is_none() && self.row_groups),
            color: scanning_color(local.color.unwrap_or(color)),
            thickness: local.thickness.unwrap_or(self.thickness),
            next_scan: local.next_scan.unwrap_or(self.next_scan),
            start_from: local.start_from.unwrap_or(self.start_from),
            first_item_ms: self.first_item_ms.min(MAX_FIRST_ITEM_MS),
            group_rate_percent: if valid_group_rate(self.group_rate_percent) {
                self.group_rate_percent
            } else {
                100
            },
        }
    }
}
// White remains a valid cursor-overlay colour. Legacy scanning settings use
// blue instead, without discarding the rest of their saved preferences.
fn scanning_color(color: ScannerColor) -> ScannerColor {
    if color == ScannerColor::White {
        ScannerColor::Blue
    } else {
        color
    }
}
pub fn deserialize_scanner_color<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<ScannerColor, D::Error> {
    ScannerColor::deserialize(deserializer).map(scanning_color)
}
fn deserialize_color_override<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<ScannerColor>, D::Error> {
    Option::<ScannerColor>::deserialize(deserializer).map(|color| color.map(scanning_color))
}
pub fn deserialize_preferences<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Preferences, D::Error> {
    let mut value = serde_json::Value::deserialize(deserializer)?;
    // A choice this version does not know must not cost the other settings.
    forget_unknown_choices(&mut value, false);
    for area in ["point", "menu", "keyboard", "mouse"] {
        if let Some(local) = value.get_mut(area) {
            forget_unknown_choices(local, true);
        }
    }
    let mut preferences: Preferences = serde_json::from_value(value).unwrap_or_default();
    if !valid_passes(preferences.pass_limit) {
        preferences.pass_limit = 3;
    }
    if preferences.first_item_ms > MAX_FIRST_ITEM_MS {
        preferences.first_item_ms = 0;
    }
    if !valid_group_rate(preferences.group_rate_percent) {
        preferences.group_rate_percent = 100;
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
/// Only a scanner's own choice may be absent, which is saved as null.
fn forget_unknown_choices(value: &mut serde_json::Value, optional: bool) {
    let Some(fields) = value.as_object_mut() else {
        return;
    };
    fields.retain(|name, choice| match name.as_str() {
        "panelSize" => !optional && PanelSize::deserialize(&*choice).is_ok(),
        "panelTheme" => !optional && PanelTheme::deserialize(&*choice).is_ok(),
        "nextScan" => (optional && choice.is_null()) || NextScan::deserialize(&*choice).is_ok(),
        "startFrom" => (optional && choice.is_null()) || StartFrom::deserialize(&*choice).is_ok(),
        "pattern" => (optional && choice.is_null()) || Pattern::deserialize(&*choice).is_ok(),
        "rowGroups" => (optional && choice.is_null()) || choice.is_boolean(),
        "firstItemMs" | "groupRatePercent" => {
            !optional && choice.as_u64().is_some_and(|v| v <= u64::from(u16::MAX))
        }
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
    fn shared_panel_size_defaults_round_trips_and_ignores_local_overrides() {
        for value in [
            serde_json::json!({}),
            serde_json::json!({"panelSize":"future", "direction":"reverse"}),
        ] {
            let preferences = read(value.clone());
            assert_eq!(preferences.panel_size, PanelSize::Medium);
            if value.get("direction").is_some() {
                assert_eq!(preferences.direction, Direction::Reverse);
            }
        }
        for size in [PanelSize::Small, PanelSize::Medium, PanelSize::Large] {
            let preferences = Preferences {
                panel_size: size,
                ..Default::default()
            };
            let mut value = serde_json::to_value(&preferences).unwrap();
            value["mouse"] = serde_json::json!({"panelSize":"small"});
            let restored = read(value);
            for area in [Area::Menu, Area::Mouse, Area::Keyboard] {
                assert_eq!(
                    restored
                        .resolve(area, false, 1000, ScannerColor::Blue)
                        .panel_size,
                    size
                );
            }
        }
    }

    #[test]
    fn panel_theme_defaults_to_system_and_is_shared_by_every_panel() {
        for value in [
            serde_json::json!({}),
            serde_json::json!({"panelTheme":"sepia", "direction":"reverse"}),
            serde_json::json!({"panelTheme":null}),
        ] {
            let preferences = read(value.clone());
            assert_eq!(preferences.panel_theme, PanelTheme::System);
            if value.get("direction").is_some() {
                assert_eq!(preferences.direction, Direction::Reverse);
            }
        }
        for theme in [PanelTheme::System, PanelTheme::Light, PanelTheme::Dark] {
            let preferences = Preferences {
                panel_theme: theme,
                ..Default::default()
            };
            let mut value = serde_json::to_value(&preferences).unwrap();
            assert!(value["panelTheme"].is_string());
            // An area never has a theme of its own.
            value["keyboard"] = serde_json::json!({"panelTheme":"light"});
            let restored = read(value);
            for area in [Area::Point, Area::Menu, Area::Mouse, Area::Keyboard] {
                assert_eq!(
                    restored
                        .resolve(area, false, 1000, ScannerColor::Blue)
                        .panel_theme,
                    theme
                );
            }
        }
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
            "nextScan": null,
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
        let saved = read(serde_json::json!({"passLimit": 5, "nextScan": "sometimes"}));
        assert_eq!(saved.pass_limit, 5);
        assert_eq!(saved.next_scan, NextScan::Standard);
    }
    #[test]
    fn row_groups_are_saved_beside_the_pattern_older_versions_read() {
        // Saved before row groups existed.
        let saved =
            read(serde_json::json!({"pattern": "grouped", "keyboard": {"pattern": "grouped"}}));
        assert!(!saved.row_groups);
        for area in [Area::Menu, Area::Keyboard, Area::Mouse] {
            assert!(
                !saved
                    .resolve(area, true, 1000, ScannerColor::Blue)
                    .row_groups
            );
        }
        let mut chosen = Preferences {
            row_groups: true,
            ..Default::default()
        };
        chosen.menu.pattern = Some(Pattern::Linear);
        chosen.mouse.pattern = Some(Pattern::Grouped);
        let stored = serde_json::to_value(&chosen).unwrap();
        // The pattern stays one that versions without row groups know.
        assert_eq!(stored["pattern"], "grouped");
        assert_eq!(stored["rowGroups"], true);
        assert_eq!(read(stored), chosen);
        let resolve =
            |chosen: &Preferences, area| chosen.resolve(area, true, 1000, ScannerColor::Blue);
        // The keyboard inherits row groups with the shared pattern.
        assert!(resolve(&chosen, Area::Keyboard).row_groups);
        assert_eq!(resolve(&chosen, Area::Menu).pattern, Pattern::Linear);
        // An area's own pattern, saved without row groups, has none.
        assert!(!resolve(&chosen, Area::Mouse).row_groups);
        chosen.mouse.row_groups = Some(true);
        assert!(resolve(&chosen, Area::Mouse).row_groups);
        chosen.row_groups = false;
        assert!(resolve(&chosen, Area::Mouse).row_groups);
        assert!(!resolve(&chosen, Area::Keyboard).row_groups);
        // Point scanning has no rows of items.
        chosen.point.row_groups = Some(true);
        assert!(!resolve(&chosen, Area::Point).row_groups);
        // A value this version does not know is dropped alone.
        let saved = read(serde_json::json!({
            "passLimit": 5,
            "pattern": "spiral",
            "rowGroups": "yes",
            "keyboard": {"pattern": "spiral", "rowGroups": 1, "intervalMs": 250},
        }));
        assert_eq!(saved.pass_limit, 5);
        assert_eq!(saved.pattern, Pattern::Grouped);
        assert!(!saved.row_groups);
        assert_eq!(saved.keyboard.pattern, None);
        assert_eq!(saved.keyboard.row_groups, None);
        assert_eq!(saved.keyboard.interval_ms, Some(250));
    }
    #[test]
    fn first_item_and_group_timing_default_off_round_trip_and_are_shared() {
        let saved = read(serde_json::json!({"direction": "reverse"}));
        assert_eq!(saved.first_item_ms, 0);
        assert_eq!(saved.group_rate_percent, 100);
        let chosen = Preferences {
            first_item_ms: 1500,
            group_rate_percent: 200,
            ..Default::default()
        };
        let mut stored = serde_json::to_value(&chosen).unwrap();
        assert_eq!(stored["firstItemMs"], 1500);
        assert_eq!(stored["groupRatePercent"], 200);
        // An area never has timing of its own.
        stored["menu"] = serde_json::json!({"firstItemMs": 100, "groupRatePercent": 150});
        let restored = read(stored);
        assert_eq!(restored, chosen);
        for area in [Area::Menu, Area::Keyboard, Area::Mouse] {
            let resolved = restored.resolve(area, true, 1000, ScannerColor::Blue);
            assert_eq!(resolved.first_item_ms, 1500);
            assert_eq!(resolved.group_rate_percent, 200);
        }
        // Values this version does not know fall back alone.
        for (first, rate) in [
            (serde_json::json!(9000), serde_json::json!(175)),
            (serde_json::json!("slow"), serde_json::json!(-1)),
            (serde_json::json!(null), serde_json::json!(1e9)),
        ] {
            let saved = read(serde_json::json!({
                "passLimit": 5, "firstItemMs": first, "groupRatePercent": rate,
            }));
            assert_eq!(saved.pass_limit, 5);
            assert_eq!(saved.first_item_ms, 0);
            assert_eq!(saved.group_rate_percent, 100);
        }
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
