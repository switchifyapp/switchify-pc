//! The portable settings document synced between a user's desktops.
//!
//! Only preferences that mean the same thing on another machine are included.
//! Device identity, pairing, telemetry consent, setup progress and startup
//! registration never leave this install. Incoming documents are fully
//! validated before anything is applied, and each section is applied through
//! the same path as a local save so runtime state and overlays stay in step.

use crate::{
    point_scan, remote_scan,
    state::{AppSettings, SwitchProfile},
    switches,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use tauri::{AppHandle, Manager};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_CUSTOM_PROFILES: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Document {
    pub schema_version: u32,
    pub app: PortableAppSettings,
    pub profiles: Vec<SwitchProfile>,
    pub switches: switches::Settings,
    pub point_scan: point_scan::Config,
    pub remote_switches: Vec<remote_scan::Slot>,
}

/// `AppSettings` without `start_with_system` (registered with this OS) and
/// `share_diagnostics` (consent must be given on each install).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableAppSettings {
    pub pointer_scale_percent: u16,
    pub mouse_repeat_enabled: bool,
    pub move_repeat_interval_ms: u32,
    pub scroll_repeat_interval_ms: u32,
    pub mouse_repeat_acceleration_duration_ms: u32,
    pub key_repeat_enabled: bool,
    pub key_repeat_interval_ms: u32,
    pub key_repeat_initial_delay_ms: u32,
    pub dwell_click_enabled: bool,
    pub dwell_click_delay_ms: u32,
    pub cursor_overlay_enabled: bool,
    pub cursor_overlay_size: String,
    pub cursor_overlay_color: String,
    pub cursor_overlay_visibility: String,
    pub cursor_crosshairs: bool,
}

impl PortableAppSettings {
    fn from_settings(settings: &AppSettings) -> Self {
        Self {
            pointer_scale_percent: settings.pointer_scale_percent,
            mouse_repeat_enabled: settings.mouse_repeat_enabled,
            move_repeat_interval_ms: settings.move_repeat_interval_ms,
            scroll_repeat_interval_ms: settings.scroll_repeat_interval_ms,
            mouse_repeat_acceleration_duration_ms: settings.mouse_repeat_acceleration_duration_ms,
            key_repeat_enabled: settings.key_repeat_enabled,
            key_repeat_interval_ms: settings.key_repeat_interval_ms,
            key_repeat_initial_delay_ms: settings.key_repeat_initial_delay_ms,
            dwell_click_enabled: settings.dwell_click_enabled,
            dwell_click_delay_ms: settings.dwell_click_delay_ms,
            cursor_overlay_enabled: settings.cursor_overlay_enabled,
            cursor_overlay_size: settings.cursor_overlay_size.clone(),
            cursor_overlay_color: settings.cursor_overlay_color.clone(),
            cursor_overlay_visibility: settings.cursor_overlay_visibility.clone(),
            cursor_crosshairs: settings.cursor_crosshairs,
        }
    }

    /// Overlays the portable fields onto this install's settings.
    fn onto(self, local: &AppSettings) -> AppSettings {
        AppSettings {
            start_with_system: local.start_with_system,
            share_diagnostics: local.share_diagnostics,
            pointer_scale_percent: self.pointer_scale_percent,
            mouse_repeat_enabled: self.mouse_repeat_enabled,
            move_repeat_interval_ms: self.move_repeat_interval_ms,
            scroll_repeat_interval_ms: self.scroll_repeat_interval_ms,
            mouse_repeat_acceleration_duration_ms: self.mouse_repeat_acceleration_duration_ms,
            key_repeat_enabled: self.key_repeat_enabled,
            key_repeat_interval_ms: self.key_repeat_interval_ms,
            key_repeat_initial_delay_ms: self.key_repeat_initial_delay_ms,
            dwell_click_enabled: self.dwell_click_enabled,
            dwell_click_delay_ms: self.dwell_click_delay_ms,
            cursor_overlay_enabled: self.cursor_overlay_enabled,
            cursor_overlay_size: self.cursor_overlay_size,
            cursor_overlay_color: self.cursor_overlay_color,
            cursor_overlay_visibility: self.cursor_overlay_visibility,
            cursor_crosshairs: self.cursor_crosshairs,
        }
    }
}

/// Everything the document is built from and applied onto.
#[derive(Debug, Clone)]
pub struct Local {
    pub settings: AppSettings,
    /// Built-in and custom profiles, as held by the model.
    pub profiles: Vec<SwitchProfile>,
    pub switches: switches::Settings,
    pub point_scan: point_scan::Config,
    pub remote: remote_scan::Config,
}

impl Local {
    pub fn read(app: &AppHandle) -> Self {
        let model = app.state::<crate::state::AppModel>();
        let (settings, profiles) = {
            let data = model
                .shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (data.state.settings.clone(), data.profiles.clone())
        };
        Self {
            settings,
            profiles,
            switches: app.state::<crate::switch_runtime::Controller>().settings(),
            point_scan: app
                .state::<crate::point_scan_runtime::Controller>()
                .view()
                .config,
            remote: remote_scan::config(app),
        }
    }

    pub fn document(&self) -> Document {
        Document {
            schema_version: SCHEMA_VERSION,
            app: PortableAppSettings::from_settings(&self.settings),
            profiles: self
                .profiles
                .iter()
                .filter(|profile| !profile.built_in)
                .cloned()
                .collect(),
            switches: self.switches.clone(),
            point_scan: portable_point_scan(&self.point_scan),
            remote_switches: self.remote.slots.clone(),
        }
    }
}

/// The scan mode in use and the legacy key fields (superseded by switch
/// bindings) are runtime state, not preferences, so they are left out.
fn portable_point_scan(config: &point_scan::Config) -> point_scan::Config {
    let defaults = point_scan::Config::default();
    point_scan::Config {
        control_mode: defaults.control_mode,
        select_key: defaults.select_key,
        next_key: defaults.next_key,
        back_key: defaults.back_key,
        pause_key: defaults.pause_key,
        ..config.clone()
    }
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    /// Written by a newer Switchify PC; this install must update to use it.
    Newer(u32),
    Invalid(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Newer(version) => write!(
                f,
                "Synced settings use version {version}. Update Switchify PC to use them."
            ),
            Self::Invalid(reason) => write!(f, "Synced settings are invalid: {reason}"),
        }
    }
}

pub fn parse(value: Value, schema_version: u32) -> Result<Document, ParseError> {
    if schema_version > SCHEMA_VERSION {
        return Err(ParseError::Newer(schema_version));
    }
    if schema_version != SCHEMA_VERSION {
        return Err(ParseError::Invalid(format!(
            "unsupported version {schema_version}"
        )));
    }
    let document: Document = serde_json::from_value(value.clone())
        .map_err(|error| ParseError::Invalid(error.to_string()))?;
    if document.schema_version != schema_version {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    // Some sections deserialize leniently (defaults for missing fields,
    // ignored unknown fields, scan preferences falling back to defaults).
    // Requiring an exact round trip means a partial document, or one with
    // fields this build does not know, is rejected instead of silently
    // resetting settings. Any change to a synced type needs a schema bump.
    let canonical =
        serde_json::to_value(&document).map_err(|error| ParseError::Invalid(error.to_string()))?;
    if canonical != value {
        return Err(ParseError::Invalid(
            "it does not match this version's settings".into(),
        ));
    }
    Ok(document)
}

/// The sections of a document that differ from this install, already merged
/// with local-only values and validated. `None` leaves a section untouched.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub settings: Option<AppSettings>,
    pub profiles: Option<Vec<SwitchProfile>>,
    pub switches: Option<switches::Settings>,
    pub point_scan: Option<point_scan::Config>,
    pub keyboard_layout: Option<point_scan::KeyboardLayout>,
    pub remote: Option<remote_scan::Config>,
}

impl Plan {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Validates every section of `incoming` before anything is applied.
pub fn plan(local: &Local, incoming: Document) -> Result<Plan, String> {
    let settings = incoming.app.onto(&local.settings).normalized()?;

    incoming.switches.validate()?;

    let remote = remote_scan::Config {
        slots: incoming.remote_switches,
        ..local.remote.clone()
    };
    remote.validate()?;

    let keyboard_layout = incoming.point_scan.keyboard_layout;
    let point_scan = point_scan::Config {
        control_mode: local.point_scan.control_mode,
        keyboard_layout: local.point_scan.keyboard_layout,
        select_key: local.point_scan.select_key.clone(),
        next_key: local.point_scan.next_key.clone(),
        back_key: local.point_scan.back_key.clone(),
        pause_key: local.point_scan.pause_key.clone(),
        ..incoming.point_scan
    };
    point_scan.validate()?;

    let profiles = merge_profiles(&local.profiles, incoming.profiles)?;

    Ok(Plan {
        settings: (settings != local.settings).then_some(settings),
        profiles,
        switches: (incoming.switches != local.switches).then_some(incoming.switches),
        point_scan: (point_scan != local.point_scan).then_some(point_scan),
        keyboard_layout: (keyboard_layout != local.point_scan.keyboard_layout)
            .then_some(keyboard_layout),
        remote: (remote.slots != local.remote.slots).then_some(remote),
    })
}

/// Replaces the custom profiles with the incoming set, keeping built-ins.
/// A changed profile gets a version above both copies so connected phones
/// notice the new bindings; an unchanged one keeps the higher of the two.
/// Returns `None` when nothing would change.
pub(crate) fn merge_profiles(
    local: &[SwitchProfile],
    mut incoming: Vec<SwitchProfile>,
) -> Result<Option<Vec<SwitchProfile>>, String> {
    if incoming.len() > MAX_CUSTOM_PROFILES {
        return Err("No more than 32 custom profiles can be saved.".into());
    }
    for profile in &mut incoming {
        crate::validate_profile(profile)?;
        profile.name = profile.name.trim().into();
    }
    // A document from an install saved before built-in names were reserved
    // on every platform may hold e.g. a custom "Grid 3" from a Mac. Rename it
    // (as that install does on load) rather than rejecting the document.
    crate::state::rename_reserved_profiles(&mut incoming);
    let mut ids = HashSet::new();
    // Compared like save_switch_profile does (ASCII case-insensitive).
    let mut names = HashSet::new();
    let mut merged = Vec::with_capacity(incoming.len());
    for mut profile in incoming {
        if !ids.insert(profile.id.clone()) {
            return Err("Custom profile identity is invalid.".into());
        }
        if !names.insert(profile.name.to_ascii_lowercase()) {
            return Err("Profile names must be unique.".into());
        }
        if let Some(existing) = local
            .iter()
            .find(|candidate| candidate.id == profile.id && !candidate.built_in)
        {
            // Same content: never lower the version, so computers that synced
            // the same profile converge on one version instead of trading
            // their own back and forth.
            profile.version = if same_profile(existing, &profile) {
                existing.version.max(profile.version)
            } else {
                profile.version.max(existing.version.saturating_add(1))
            };
        }
        merged.push(profile);
    }
    let current: Vec<&SwitchProfile> = local.iter().filter(|profile| !profile.built_in).collect();
    if current.len() == merged.len() && current.iter().zip(&merged).all(|(a, b)| *a == b) {
        return Ok(None);
    }
    Ok(Some(
        local
            .iter()
            .filter(|profile| profile.built_in)
            .cloned()
            .chain(merged)
            .collect(),
    ))
}

fn same_profile(left: &SwitchProfile, right: &SwitchProfile) -> bool {
    left.name == right.name && left.provider == right.provider && left.bindings == right.bindings
}

/// What an apply changed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub settings: bool,
    pub profiles: bool,
    pub switches: bool,
    pub point_scan: bool,
    pub remote: bool,
}

/// Reads this install, plans the incoming document against it and applies
/// the differences. Must run on the main thread, like the settings commands
/// it shares paths with (dispatch with `run_on_main_thread`).
///
/// Validation happens up front, but a later step can still fail (a file
/// write, an active switch capture). Sections applied before the failure
/// stay applied; because each call re-plans from current state, retrying
/// the same document converges.
pub fn apply(app: &AppHandle, incoming: Document) -> Result<Applied, String> {
    if crate::switch_practice::active(app) {
        return Err("Finish practice before synced settings can be applied.".into());
    }
    let plan = plan(&Local::read(app), incoming)?;
    let mut applied = Applied::default();
    if let Some(settings) = plan.switches {
        crate::point_scan_runtime::pause(app);
        app.state::<crate::switch_runtime::Controller>()
            .save(app, settings)?;
        remote_scan::apply(app);
        applied.switches = true;
    }
    if let Some(config) = plan.remote {
        remote_scan::save(app, config)?;
        applied.remote = true;
    }
    if let Some(config) = plan.point_scan {
        crate::point_scan_runtime::configure(app, config)?;
        applied.point_scan = true;
    }
    if let Some(layout) = plan.keyboard_layout {
        crate::scanning_runtime::update_point_setting(
            app,
            crate::scan_menu::Setting::KeyboardLayout(layout),
        )?;
        applied.point_scan = true;
    }
    if let Some(settings) = plan.settings {
        crate::apply_app_settings(app, settings)?;
        applied.settings = true;
    }
    let model = app.state::<crate::state::AppModel>();
    if let Some(profiles) = plan.profiles {
        model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .profiles = profiles;
        model.persist()?;
        applied.profiles = true;
    }
    if applied.settings || applied.profiles {
        crate::state::emit_state(app, &model.shared);
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanning::Action;
    use crate::state::{built_in_profiles, SwitchBinding};
    use serde_json::json;

    fn binding(id: &str, key: &str, action: Action) -> switches::Binding {
        switches::Binding {
            id: id.into(),
            name: action.label().into(),
            key: key.into(),
            press_action: action,
            hold_actions: vec![],
        }
    }

    fn custom_profile(id: &str, name: &str, first_key: &str) -> SwitchProfile {
        SwitchProfile {
            id: id.into(),
            version: 1,
            name: name.into(),
            provider: "mapped".into(),
            built_in: false,
            bindings: (1..=8)
                .map(|switch_id| SwitchBinding {
                    switch_id,
                    binding_type: if switch_id == 1 { "key" } else { "none" }.into(),
                    value: (switch_id == 1).then(|| first_key.into()),
                    keys: None,
                    click_count: None,
                })
                .collect(),
        }
    }

    const PROFILE_A: &str = "6f1c1a52-6a0e-4c2b-9d0f-2f8a7b9e4c11";
    const PROFILE_B: &str = "0b7e2d4a-1c3f-4e5a-8b9c-7d6e5f4a3b22";

    fn local() -> Local {
        let mut profiles = built_in_profiles(false);
        profiles.push(custom_profile(PROFILE_A, "Desk", "Space"));
        Local {
            settings: AppSettings {
                start_with_system: true,
                share_diagnostics: true,
                ..AppSettings::default()
            },
            profiles,
            switches: switches::Settings {
                bindings: vec![
                    binding("a", "Space", Action::Select),
                    binding("b", "Enter", Action::Next),
                ],
                ..switches::Settings::default()
            },
            point_scan: point_scan::Config {
                control_mode: point_scan::ControlMode::Mouse,
                select_key: "F1".into(),
                ..point_scan::Config::default()
            },
            remote: remote_scan::Config {
                revision: 7,
                ..remote_scan::Config::default()
            },
        }
    }

    #[test]
    fn document_excludes_device_only_values() {
        let document = local().document();
        let value = serde_json::to_value(&document).unwrap();
        let text = value.to_string();
        for field in [
            "startWithSystem",
            "shareDiagnostics",
            "telemetry",
            "desktopId",
            "pairedDevices",
            "setupShown",
            "setupCompleted",
            "revision",
        ] {
            assert!(!text.contains(field), "document leaked {field}");
        }
        assert!(document.profiles.iter().all(|profile| !profile.built_in));
        assert_eq!(document.profiles.len(), 1);
        assert_eq!(
            document.point_scan.control_mode,
            point_scan::Config::default().control_mode
        );
        assert_eq!(document.point_scan.select_key, "Space");
    }

    #[test]
    fn document_keeps_switch_keys() {
        let document = local().document();
        let keys: Vec<_> = document
            .switches
            .bindings
            .iter()
            .map(|b| b.key.as_str())
            .collect();
        assert_eq!(keys, ["Space", "Enter"]);
    }

    #[test]
    fn document_round_trips_and_plans_nothing() {
        let local = local();
        let value = serde_json::to_value(local.document()).unwrap();
        let parsed = parse(value, SCHEMA_VERSION).unwrap();
        assert_eq!(parsed, local.document());
        assert!(plan(&local, parsed).unwrap().is_empty());
    }

    #[test]
    fn parse_rejects_newer_and_malformed_documents() {
        let value = serde_json::to_value(local().document()).unwrap();
        assert_eq!(
            parse(value.clone(), SCHEMA_VERSION + 1),
            Err(ParseError::Newer(SCHEMA_VERSION + 1))
        );
        assert!(matches!(parse(value, 0), Err(ParseError::Invalid(_))));
        assert!(matches!(
            parse(json!({"schemaVersion": 1}), 1),
            Err(ParseError::Invalid(_))
        ));
        let mut extra = serde_json::to_value(local().document()).unwrap();
        extra["app"]["startWithSystem"] = json!(true);
        assert!(matches!(parse(extra, 1), Err(ParseError::Invalid(_))));
    }

    #[test]
    fn plan_keeps_local_only_settings() {
        let local = local();
        let mut incoming = local.document();
        incoming.app.dwell_click_enabled = true;
        let plan = plan(&local, incoming).unwrap();
        let settings = plan.settings.unwrap();
        assert!(settings.dwell_click_enabled);
        assert!(settings.start_with_system);
        assert!(settings.share_diagnostics);
        assert!(plan.switches.is_none() && plan.point_scan.is_none() && plan.remote.is_none());
    }

    #[test]
    fn plan_applies_switch_keys() {
        let local = local();
        let mut incoming = local.document();
        incoming.switches.bindings[0].key = "F5".into();
        let plan = plan(&local, incoming).unwrap();
        assert_eq!(plan.switches.unwrap().bindings[0].key, "F5");
    }

    #[test]
    fn plan_keeps_runtime_point_scan_state() {
        let local = local();
        let mut incoming = local.document();
        incoming.point_scan.speed = 4;
        let config = plan(&local, incoming).unwrap().point_scan.unwrap();
        assert_eq!(config.speed, 4);
        assert_eq!(config.control_mode, point_scan::ControlMode::Mouse);
        assert_eq!(config.select_key, "F1");
    }

    #[test]
    fn plan_separates_keyboard_layout() {
        let local = local();
        let mut incoming = local.document();
        incoming.point_scan.keyboard_layout = point_scan::KeyboardLayout::SimpleQwerty;
        let plan = plan(&local, incoming).unwrap();
        assert_eq!(
            plan.keyboard_layout,
            Some(point_scan::KeyboardLayout::SimpleQwerty)
        );
        assert!(plan.point_scan.is_none());
    }

    #[test]
    fn plan_keeps_local_remote_revision() {
        let local = local();
        let mut incoming = local.document();
        incoming.remote_switches[6].press_action = Some(Action::Select);
        let remote = plan(&local, incoming).unwrap().remote.unwrap();
        assert_eq!(remote.revision, 7);
        assert_eq!(remote.slots[6].press_action, Some(Action::Select));
    }

    #[test]
    fn plan_rejects_any_invalid_section() {
        let local = local();
        let invalid: [fn(&mut Document); 6] = [
            |d| d.app.pointer_scale_percent = 3,
            |d| d.app.cursor_overlay_color = "purple".into(),
            |d| d.switches.bindings[1].key = "Space".into(),
            |d| d.switches.hold_interval_ms = 10,
            |d| d.point_scan.grid_size = 99,
            |d| d.remote_switches.truncate(3),
        ];
        for corrupt in invalid {
            let mut incoming = local.document();
            corrupt(&mut incoming);
            assert!(plan(&local, incoming).is_err());
        }
    }

    #[test]
    fn plan_rejects_invalid_profiles() {
        let local = local();
        let invalid: [fn(&mut Document); 4] = [
            |d| d.profiles[0].built_in = true,
            |d| d.profiles[0].id = "not-a-uuid".into(),
            |d| {
                let mut other = custom_profile(PROFILE_B, "DESK", "Tab");
                other.version = 1;
                d.profiles.push(other);
            },
            |d| {
                let copy = d.profiles[0].clone();
                d.profiles.push(copy);
            },
        ];
        for corrupt in invalid {
            let mut incoming = local.document();
            corrupt(&mut incoming);
            assert!(plan(&local, incoming).is_err());
        }
        let mut incoming = local.document();
        incoming.profiles = (0..33)
            .map(|_| custom_profile(&uuid::Uuid::new_v4().to_string(), "x", "Space"))
            .enumerate()
            .map(|(index, mut profile)| {
                profile.name = format!("Profile {index}");
                profile
            })
            .collect();
        assert!(plan(&local, incoming).is_err());
    }

    #[test]
    fn changed_profiles_get_a_newer_version() {
        let local = local();
        let mut incoming = local.document();
        incoming.profiles[0].bindings[0].value = Some("Enter".into());
        incoming.profiles[0].version = 1;
        incoming
            .profiles
            .push(custom_profile(PROFILE_B, "Laptop", "Tab"));
        let profiles = plan(&local, incoming).unwrap().profiles.unwrap();
        assert!(profiles[0].built_in, "built-ins stay first");
        let desk = profiles.iter().find(|p| p.id == PROFILE_A).unwrap();
        assert_eq!(desk.version, 2);
        assert_eq!(desk.bindings[0].value.as_deref(), Some("Enter"));
        assert!(profiles.iter().any(|p| p.id == PROFILE_B));
    }

    #[test]
    fn unchanged_profiles_keep_their_version() {
        let mut local = local();
        local.profiles[1].version = 5;
        let mut incoming = local.document();
        incoming.profiles[0].version = 2;
        assert!(plan(&local, incoming).unwrap().profiles.is_none());
    }

    #[test]
    fn removed_profiles_are_dropped() {
        let local = local();
        let mut incoming = local.document();
        incoming.profiles.clear();
        let profiles = plan(&local, incoming).unwrap().profiles.unwrap();
        assert!(profiles.iter().all(|profile| profile.built_in));
        assert!(!profiles.is_empty());
    }

    #[test]
    fn reserved_names_cover_every_platform_built_in() {
        let mut reserved: Vec<_> = crate::state::reserved_profile_names().collect();
        let mut built_in: Vec<_> = built_in_profiles(true)
            .into_iter()
            .map(|profile| profile.name)
            .collect();
        reserved.sort_unstable();
        built_in.sort_unstable();
        assert_eq!(reserved, built_in);
    }

    #[test]
    fn profile_named_after_another_platforms_built_in_is_renamed() {
        // A Mac has no built-in "Grid 3", so a custom one could exist there.
        let local = local();
        let mut incoming = local.document();
        incoming.profiles[0].name = "grid 3".into();
        let profiles = plan(&local, incoming).unwrap().profiles.unwrap();
        let desk = profiles.iter().find(|p| p.id == PROFILE_A).unwrap();
        assert_eq!(desk.name, "grid 3 (custom)");
    }

    #[test]
    fn reserved_rename_skips_names_already_in_use() {
        // Both were valid custom names on a Mac before names were reserved.
        let local = local();
        let mut incoming = local.document();
        incoming.profiles[0].name = "Grid 3".into();
        incoming
            .profiles
            .push(custom_profile(PROFILE_B, "Grid 3 (custom)", "Tab"));
        let profiles = plan(&local, incoming).unwrap().profiles.unwrap();
        let name = |id| &profiles.iter().find(|p| p.id == id).unwrap().name;
        assert_eq!(name(PROFILE_A), "Grid 3 (custom 2)");
        assert_eq!(name(PROFILE_B), "Grid 3 (custom)");
    }

    #[test]
    fn load_and_merge_rename_the_same_way() {
        // An install renames on load; its next document must then plan
        // nothing on another install that renamed the same profile.
        let mut renamed = local();
        renamed.profiles[1].name = "Grid 3".into();
        crate::state::rename_reserved_profiles(&mut renamed.profiles);
        assert_eq!(renamed.profiles[1].name, "Grid 3 (custom)");
        assert!(renamed.profiles[0].built_in && renamed.profiles[0].name == "Generic keyboard");

        let mut legacy = renamed.document();
        legacy.profiles[0].name = "Grid 3".into();
        assert!(plan(&renamed, legacy).unwrap().profiles.is_none());
    }

    #[test]
    fn profile_names_compare_like_local_saves() {
        // save_switch_profile compares ASCII case-insensitively, so both of
        // these can exist on one install and must sync.
        let local = local();
        let mut incoming = local.document();
        incoming.profiles[0].name = "Été".into();
        let mut second = custom_profile(PROFILE_B, "été", "Tab");
        second.version = 1;
        incoming.profiles.push(second);
        assert!(plan(&local, incoming).unwrap().profiles.is_some());
    }

    #[test]
    fn parse_rejects_lossy_point_scan_sections() {
        let document = serde_json::to_value(local().document()).unwrap();
        let lossy: [fn(&mut Value); 4] = [
            // Partial section: missing fields would silently become defaults.
            |v| v["pointScan"] = json!({}),
            // A field from a newer build would be dropped without a trace.
            |v| v["pointScan"]["futureSetting"] = json!(true),
            // An unknown scan preference value resets all scan preferences.
            |v| v["pointScan"]["scanPreferences"] = json!("garbage"),
            |v| v["pointScan"]["scanPreferences"]["pattern"] = json!("spiral"),
        ];
        for corrupt in lossy {
            let mut value = document.clone();
            corrupt(&mut value);
            assert!(
                matches!(parse(value.clone(), 1), Err(ParseError::Invalid(_))),
                "accepted {value}"
            );
        }
    }

    #[test]
    fn synced_point_scan_fields_are_deliberate() {
        // Changing point_scan::Config changes what syncs. Update this list
        // and bump SCHEMA_VERSION together.
        let value = serde_json::to_value(local().document()).unwrap();
        let mut fields: Vec<_> = value["pointScan"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        fields.sort_unstable();
        assert_eq!(
            fields,
            [
                "autoSelectDelayMs",
                "autoSelectEnabled",
                "automatic",
                "backKey",
                "blockIntervalMs",
                "controlMode",
                "enhancedWordPrediction",
                "gridSize",
                "keyboardLayout",
                "keyboardWaitAfterTyping",
                "mode",
                "mouseRepeatStopEdge",
                "nextKey",
                "panelAvoidsPointer",
                "pauseKey",
                "scanPreferences",
                "scannerColor",
                "selectKey",
                "speed",
                "startWith",
                "wordPrediction",
            ]
        );
    }
}
