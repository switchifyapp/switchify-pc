//! The portable settings document synced between a user's desktops.
//!
//! Only preferences that mean the same thing on another machine are included.
//! Device identity, pairing, telemetry consent, setup progress and startup
//! registration never leave this install. Incoming documents are fully
//! validated before anything is applied, and each section is applied through
//! the same path as a local save so runtime state and overlays stay in step.

use crate::{
    custom_panels::{self, CustomPanel},
    point_scan, remote_scan,
    state::{AppSettings, SwitchProfile},
    switches,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use tauri::{AppHandle, Manager};

pub const SCHEMA_VERSION: u32 = 12;
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
    /// Added in version 7: user-built panels, merged one by one like profiles.
    /// The default only fills in a merge base saved by an older build;
    /// documents must still carry it (see `parse`).
    #[serde(default)]
    pub panels: Vec<CustomPanel>,
    /// Added in version 8: the built-in menus the user rearranged, by menu
    /// name. The same default rule as `panels`.
    #[serde(default)]
    pub menu_overrides: BTreeMap<String, CustomPanel>,
    /// Added in version 9 as rows of tiles on the mouse panel and the
    /// keyboard, by panel; version 10 holds rows and columns. The same
    /// default rule as `panels`.
    #[serde(default)]
    pub panel_extras: crate::panel_extras::Extras,
}

/// `AppSettings` without `start_with_system` (registered with this OS),
/// `share_diagnostics` (consent must be given on each install),
/// `auto_approve_account_devices` (pairing is per install) and
/// `learn_word_prediction` (learned words stay on this computer).
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
    /// Added in version 2. The default only fills in a merge base saved by an
    /// older build; documents must still carry it (see `parse`).
    #[serde(default)]
    pub scan_key_repeat_enabled: bool,
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
            scan_key_repeat_enabled: settings.scan_key_repeat_enabled,
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
            auto_approve_account_devices: local.auto_approve_account_devices,
            learn_word_prediction: local.learn_word_prediction,
            pointer_scale_percent: self.pointer_scale_percent,
            mouse_repeat_enabled: self.mouse_repeat_enabled,
            move_repeat_interval_ms: self.move_repeat_interval_ms,
            scroll_repeat_interval_ms: self.scroll_repeat_interval_ms,
            mouse_repeat_acceleration_duration_ms: self.mouse_repeat_acceleration_duration_ms,
            key_repeat_enabled: self.key_repeat_enabled,
            key_repeat_interval_ms: self.key_repeat_interval_ms,
            key_repeat_initial_delay_ms: self.key_repeat_initial_delay_ms,
            scan_key_repeat_enabled: self.scan_key_repeat_enabled,
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
    pub panels: Vec<CustomPanel>,
    pub menu_overrides: BTreeMap<String, CustomPanel>,
    pub panel_extras: crate::panel_extras::Extras,
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
            panels: custom_panels::list(app),
            menu_overrides: custom_panels::overrides(app),
            panel_extras: custom_panels::extras(app),
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
            panels: self.panels.clone(),
            menu_overrides: self.menu_overrides.clone(),
            panel_extras: self.panel_extras.clone(),
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

pub fn parse(mut value: Value, schema_version: u32) -> Result<Document, ParseError> {
    if schema_version > SCHEMA_VERSION {
        return Err(ParseError::Newer(schema_version));
    }
    if schema_version == 0 {
        return Err(ParseError::Invalid("unsupported version 0".into()));
    }
    // Each step takes a document of one version to the next.
    for version in schema_version..SCHEMA_VERSION {
        value = match version {
            1 => upgrade_v1(value)?,
            2 => upgrade_v2(value)?,
            3 => upgrade_v3(value)?,
            4 => upgrade_v4(value)?,
            5 => upgrade_v5(value)?,
            6 => upgrade_v6(value)?,
            7 => upgrade_v7(value)?,
            8 => upgrade_v8(value)?,
            9 => upgrade_v9(value)?,
            10 => upgrade_v10(value)?,
            _ => upgrade_v11(value)?,
        };
    }
    let document: Document = serde_json::from_value(value.clone())
        .map_err(|error| ParseError::Invalid(error.to_string()))?;
    if document.schema_version != SCHEMA_VERSION {
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

/// Version 1 had no scanned key repeat, which is off unless chosen.
fn upgrade_v1(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(1)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let app = value
        .get_mut("app")
        .and_then(Value::as_object_mut)
        .filter(|app| !app.contains_key("scanKeyRepeatEnabled"))
        .ok_or_else(|| ParseError::Invalid("it does not match version 1 settings".into()))?;
    app.insert("scanKeyRepeatEnabled".into(), Value::Bool(false));
    value["schemaVersion"] = Value::from(2);
    Ok(value)
}

/// Version 2 had no row groups, which are off unless chosen, and an area
/// without its own choice follows the shared one.
fn upgrade_v2(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(2)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let invalid = || ParseError::Invalid("it does not match version 2 settings".into());
    let preferences = value
        .pointer_mut("/pointScan/scanPreferences")
        .and_then(Value::as_object_mut)
        .filter(|preferences| !preferences.contains_key("rowGroups"))
        .ok_or_else(invalid)?;
    preferences.insert("rowGroups".into(), Value::Bool(false));
    for area in ["point", "menu", "keyboard", "mouse"] {
        preferences
            .get_mut(area)
            .and_then(Value::as_object_mut)
            .filter(|overrides| !overrides.contains_key("rowGroups"))
            .ok_or_else(invalid)?
            .insert("rowGroups".into(), Value::Null);
    }
    value["schemaVersion"] = Value::from(3);
    Ok(value)
}

/// Version 3 had no radar speed, so the radar follows the line speed.
fn upgrade_v3(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(3)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    value
        .get_mut("pointScan")
        .and_then(Value::as_object_mut)
        .filter(|point_scan| !point_scan.contains_key("radarSpeed"))
        .ok_or_else(|| ParseError::Invalid("it does not match version 3 settings".into()))?
        .insert("radarSpeed".into(), Value::Null);
    value["schemaVersion"] = Value::from(4);
    Ok(value)
}

/// Version 4 had no panel theme, so the panels follow the computer.
fn upgrade_v4(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(4)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    value
        .pointer_mut("/pointScan/scanPreferences")
        .and_then(Value::as_object_mut)
        .filter(|preferences| !preferences.contains_key("panelTheme"))
        .ok_or_else(|| ParseError::Invalid("it does not match version 4 settings".into()))?
        .insert("panelTheme".into(), Value::from("system"));
    value["schemaVersion"] = Value::from(5);
    Ok(value)
}

/// Version 5 documents have the same shape, but a version 5 install rejects
/// gamepad buttons as switch keys. The bump makes such an install report
/// that it must update instead of refusing the synced switches.
fn upgrade_v5(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(5)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    value["schemaVersion"] = Value::from(6);
    Ok(value)
}

/// Version 6 had no user-built panels.
fn upgrade_v6(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(6)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let document = value
        .as_object_mut()
        .filter(|document| !document.contains_key("panels"))
        .ok_or_else(|| ParseError::Invalid("it does not match version 6 settings".into()))?;
    document.insert("panels".into(), Value::Array(Vec::new()));
    value["schemaVersion"] = Value::from(7);
    Ok(value)
}

/// Version 7 had no rearranged menus.
fn upgrade_v7(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(7)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let document = value
        .as_object_mut()
        .filter(|document| !document.contains_key("menuOverrides"))
        .ok_or_else(|| ParseError::Invalid("it does not match version 7 settings".into()))?;
    document.insert("menuOverrides".into(), Value::Object(Default::default()));
    value["schemaVersion"] = Value::from(8);
    Ok(value)
}

/// Version 8 had no extra rows on the mouse panel and the keyboard.
fn upgrade_v8(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(8)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let document = value
        .as_object_mut()
        .filter(|document| !document.contains_key("panelExtras"))
        .ok_or_else(|| ParseError::Invalid("it does not match version 8 settings".into()))?;
    document.insert("panelExtras".into(), Value::Object(Default::default()));
    value["schemaVersion"] = Value::from(9);
    Ok(value)
}

/// Version 9 held each panel's extra tiles as its rows alone; version 10
/// holds rows and columns.
fn upgrade_v9(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(9)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let invalid = || ParseError::Invalid("it does not match version 9 settings".into());
    let extras = value
        .get_mut("panelExtras")
        .and_then(Value::as_object_mut)
        .ok_or_else(invalid)?;
    for rows in extras.values_mut() {
        if !rows.is_array() {
            return Err(invalid());
        }
        let old = rows.take();
        *rows = serde_json::json!({ "rows": old, "columns": [] });
    }
    value["schemaVersion"] = Value::from(10);
    Ok(value)
}

/// Version 12 adds panel-opening switch actions. Existing assignments keep
/// their serialized form; the version gates older sync clients.
fn upgrade_v11(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(11)) {
        return Err(ParseError::Invalid(
            "it does not match version 11 settings".into(),
        ));
    }
    value["schemaVersion"] = Value::from(SCHEMA_VERSION);
    Ok(value)
}

/// Version 10 had no switch timing or first-item and in-group scan timing.
fn upgrade_v10(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(10)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let invalid = || ParseError::Invalid("it does not match version 10 settings".into());
    value
        .get("switches")
        .and_then(Value::as_object)
        .filter(|s| !s.contains_key("acceptMs") && !s.contains_key("ignoreRepeatMs"))
        .ok_or_else(invalid)?;
    let preferences = value
        .pointer_mut("/pointScan/scanPreferences")
        .and_then(Value::as_object_mut)
        .filter(|p| !p.contains_key("firstItemMs") && !p.contains_key("groupRatePercent"))
        .ok_or_else(invalid)?;
    preferences.insert("firstItemMs".into(), Value::from(0));
    preferences.insert("groupRatePercent".into(), Value::from(100));
    value["schemaVersion"] = Value::from(11);
    Ok(value)
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
    pub panels: Option<Vec<CustomPanel>>,
    pub menu_overrides: Option<BTreeMap<String, CustomPanel>>,
    pub panel_extras: Option<crate::panel_extras::Extras>,
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
    let panels = custom_panels::plan_replacement(&local.panels, incoming.panels)?;
    let menu_overrides =
        custom_panels::plan_override_replacement(&local.menu_overrides, incoming.menu_overrides)?;
    let panel_extras =
        crate::panel_extras::plan_replacement(&local.panel_extras, incoming.panel_extras)?;

    Ok(Plan {
        settings: (settings != local.settings).then_some(settings),
        profiles,
        switches: (incoming.switches != local.switches).then_some(incoming.switches),
        point_scan: (point_scan != local.point_scan).then_some(point_scan),
        keyboard_layout: (keyboard_layout != local.point_scan.keyboard_layout)
            .then_some(keyboard_layout),
        remote: (remote.slots != local.remote.slots).then_some(remote),
        panels,
        menu_overrides,
        panel_extras,
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
    pub panels: bool,
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
    if let Some(panels) = plan.panels {
        custom_panels::replace(app, panels)?;
        applied.panels = true;
    }
    if let Some(overrides) = plan.menu_overrides {
        custom_panels::replace_overrides(app, overrides)?;
        applied.panels = true;
    }
    if let Some(extras) = plan.panel_extras {
        custom_panels::replace_extras(app, extras)?;
        applied.panels = true;
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

/// A current document as an older version wrote it.
#[cfg(test)]
pub(crate) fn written_by(mut value: Value, version: u32) -> Value {
    if version < 11 {
        let switches = value["switches"].as_object_mut().unwrap();
        switches.remove("acceptMs");
        switches.remove("ignoreRepeatMs");
        let preferences = value
            .pointer_mut("/pointScan/scanPreferences")
            .and_then(Value::as_object_mut)
            .unwrap();
        preferences.remove("firstItemMs");
        preferences.remove("groupRatePercent");
    }
    if version < 10 {
        if let Some(extras) = value.get_mut("panelExtras").and_then(Value::as_object_mut) {
            for panel in extras.values_mut() {
                *panel = panel["rows"].take();
            }
        }
    }
    if version < 9 {
        value.as_object_mut().unwrap().remove("panelExtras");
    }
    if version < 8 {
        value.as_object_mut().unwrap().remove("menuOverrides");
    }
    if version < 7 {
        value.as_object_mut().unwrap().remove("panels");
    }
    if version < 5 {
        value
            .pointer_mut("/pointScan/scanPreferences")
            .and_then(Value::as_object_mut)
            .unwrap()
            .remove("panelTheme");
    }
    if version < 4 {
        value["pointScan"]
            .as_object_mut()
            .unwrap()
            .remove("radarSpeed");
    }
    if version < 3 {
        let preferences = value
            .pointer_mut("/pointScan/scanPreferences")
            .and_then(Value::as_object_mut)
            .unwrap();
        preferences.remove("rowGroups");
        for area in ["point", "menu", "keyboard", "mouse"] {
            preferences[area]
                .as_object_mut()
                .unwrap()
                .remove("rowGroups");
        }
    }
    if version < 2 {
        value["app"]
            .as_object_mut()
            .unwrap()
            .remove("scanKeyRepeatEnabled");
    }
    value["schemaVersion"] = Value::from(version);
    value
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
                auto_approve_account_devices: false,
                learn_word_prediction: false,
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
            panels: vec![custom_panels::sample("greetings", "Greetings")],
            menu_overrides: BTreeMap::new(),
            panel_extras: BTreeMap::new(),
        }
    }

    #[test]
    fn version_eight_documents_read_with_no_extra_rows() {
        let hello = custom_panels::Cell {
            label: "Hello".into(),
            action: crate::panel_actions::PanelAction::Text {
                text: "Hello".into(),
            },
            span: 1,
        };
        let mut local = local();
        local.panel_extras.insert(
            "keyboard".into(),
            crate::panel_extras::PanelExtras::rows(vec![vec![hello.clone()]]),
        );
        let mut cloud = local.clone();
        cloud.panel_extras.clear();
        let version_eight = written_by(serde_json::to_value(cloud.document()).unwrap(), 8);
        assert!(version_eight.get("panelExtras").is_none());
        assert_eq!(version_eight["schemaVersion"], json!(8));
        let parsed = parse(version_eight.clone(), 8).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert!(parsed.panel_extras.is_empty());
        // Applying it removes this computer's extra rows, as a removal elsewhere would.
        assert_eq!(
            plan(&local, parsed).unwrap().panel_extras,
            Some(BTreeMap::new())
        );
        let mut early = version_eight.clone();
        early["panelExtras"] = json!({});
        assert!(matches!(parse(early, 8), Err(ParseError::Invalid(_))));
        let mut missing = version_eight.clone();
        missing["schemaVersion"] = json!(SCHEMA_VERSION);
        assert!(matches!(
            parse(missing, SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));
        // A version 1 document still climbs every step.
        let oldest = written_by(serde_json::to_value(cloud.document()).unwrap(), 1);
        assert_eq!(parse(oldest, 1).unwrap().schema_version, SCHEMA_VERSION);
        // Extra rows sync, and a panel that takes none refuses the document.
        let value = serde_json::to_value(local.document()).unwrap();
        assert_eq!(
            value["panelExtras"]["keyboard"]["rows"][0][0]["label"],
            json!("Hello")
        );
        let mut bad = local.document();
        bad.panel_extras.insert(
            "home".into(),
            crate::panel_extras::PanelExtras::rows(vec![vec![hello]]),
        );
        assert!(plan(&local, bad).is_err());
    }

    #[test]
    fn version_nine_documents_keep_their_rows_and_gain_no_columns() {
        let hello = custom_panels::Cell {
            label: "Hello".into(),
            action: crate::panel_actions::PanelAction::Text {
                text: "Hello".into(),
            },
            span: 1,
        };
        let mut local = local();
        local.panel_extras.insert(
            "mouse".into(),
            crate::panel_extras::PanelExtras {
                rows: vec![vec![hello.clone()]],
                columns: vec![vec![hello.clone()]],
            },
        );
        let version_nine = written_by(serde_json::to_value(local.document()).unwrap(), 9);
        assert_eq!(
            version_nine["panelExtras"]["mouse"][0][0]["label"],
            json!("Hello")
        );
        let parsed = parse(version_nine.clone(), 9).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert_eq!(parsed.panel_extras["mouse"].rows, vec![vec![hello]]);
        assert!(parsed.panel_extras["mouse"].columns.is_empty());
        // Applying it drops this computer's columns, as a removal elsewhere would.
        let planned = plan(&local, parsed).unwrap().panel_extras.unwrap();
        assert!(planned["mouse"].columns.is_empty());
        let mut early = version_nine.clone();
        early["panelExtras"]["mouse"] = json!({ "rows": [], "columns": [] });
        assert!(matches!(parse(early, 9), Err(ParseError::Invalid(_))));
    }

    #[test]
    fn version_eleven_assignments_upgrade_and_panel_actions_round_trip() {
        let original = local().document();
        let old = written_by(serde_json::to_value(&original).unwrap(), 11);
        assert_eq!(parse(old, 11).unwrap(), original);
        let mut document = original;
        document.remote_switches[0].press_action = Some(crate::scanning::Action::OpenMenu(
            crate::scan_menu::Kind::Editing,
        ));
        document.remote_switches[0].hold_actions =
            vec![crate::scanning::Action::OpenCustomPanel("greetings".into())];
        let value = serde_json::to_value(&document).unwrap();
        assert_eq!(parse(value, SCHEMA_VERSION).unwrap(), document);
    }

    #[test]
    fn version_ten_documents_read_with_every_timing_off() {
        let mut local = local();
        local.switches.accept_ms = 300;
        local.switches.ignore_repeat_ms = 1000;
        local.point_scan.scan_preferences.first_item_ms = 1500;
        local.point_scan.scan_preferences.group_rate_percent = 200;
        let current = serde_json::to_value(local.document()).unwrap();
        assert_eq!(current["switches"]["acceptMs"], json!(300));
        assert_eq!(current["switches"]["ignoreRepeatMs"], json!(1000));
        let parsed = parse(current.clone(), SCHEMA_VERSION).unwrap();
        assert_eq!(parsed.switches.accept_ms, 300);
        assert_eq!(parsed.point_scan.scan_preferences.group_rate_percent, 200);
        let version_ten = written_by(current, 10);
        let parsed = parse(version_ten.clone(), 10).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert_eq!(parsed.switches.accept_ms, 0);
        assert_eq!(parsed.switches.ignore_repeat_ms, 0);
        let preferences = &parsed.point_scan.scan_preferences;
        assert_eq!(preferences.first_item_ms, 0);
        assert_eq!(preferences.group_rate_percent, 100);
        // Switch timing that is off is left out, as version 10 wrote it.
        let off = serde_json::to_value(&parsed).unwrap();
        assert!(off["switches"].get("acceptMs").is_none());
        assert_eq!(off["switches"], version_ten["switches"]);
        for (pointer, field) in [
            ("/switches", "acceptMs"),
            ("/pointScan/scanPreferences", "firstItemMs"),
        ] {
            let mut early = version_ten.clone();
            early.pointer_mut(pointer).unwrap()[field] = json!(0);
            assert!(matches!(parse(early, 10), Err(ParseError::Invalid(_))));
        }
        // Timing a build does not allow is refused, not reset.
        let mut lossy = serde_json::to_value(local.document()).unwrap();
        lossy["pointScan"]["scanPreferences"]["groupRatePercent"] = json!(175);
        assert!(parse(lossy, SCHEMA_VERSION).is_err());
    }

    #[test]
    fn document_excludes_device_only_values() {
        let document = local().document();
        let value = serde_json::to_value(&document).unwrap();
        let text = value.to_string();
        for field in [
            "startWithSystem",
            "shareDiagnostics",
            "autoApproveAccountDevices",
            "learnWordPrediction",
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
            parse(json!({"schemaVersion": SCHEMA_VERSION}), SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));
        let mut extra = serde_json::to_value(local().document()).unwrap();
        extra["app"]["startWithSystem"] = json!(true);
        assert!(matches!(
            parse(extra, SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));
    }

    /// A document as written by version 1, before scanned key repeat.
    fn version_one() -> Value {
        written_by(serde_json::to_value(local().document()).unwrap(), 1)
    }

    #[test]
    fn gamepad_switch_keys_sync_as_version_six() {
        // Version 5 installs reject these keys, which is why the schema moved
        // to 6: they see a newer document and ask for an update instead.
        let local = local();
        let mut cloud = local.clone();
        cloud.switches.bindings = vec![
            binding("pad", "Gamepad1", Action::Select),
            binding("next", "Gamepad8", Action::Next),
        ];
        let value = serde_json::to_value(cloud.document()).unwrap();
        assert_eq!(value["schemaVersion"], json!(SCHEMA_VERSION));
        assert_eq!(value["switches"]["bindings"][0]["key"], json!("Gamepad1"));
        let parsed = parse(value, SCHEMA_VERSION).unwrap();
        let switches = plan(&local, parsed).unwrap().switches.unwrap();
        assert_eq!(switches, cloud.switches);
        // Button 9 is not a switch, so a document naming it is refused.
        let mut bad = cloud.clone();
        bad.switches.bindings[0].key = "Gamepad9".into();
        let parsed = parse(
            serde_json::to_value(bad.document()).unwrap(),
            SCHEMA_VERSION,
        )
        .unwrap();
        assert!(plan(&local, parsed).is_err());
    }

    #[test]
    fn version_seven_documents_read_with_no_rearranged_menus() {
        use crate::builtin_panels::{default_document, key};
        use crate::scan_menu::Kind;
        let mut local = local();
        let mut home = default_document(Kind::Home);
        home.rows.reverse();
        local.menu_overrides.insert(key(Kind::Home), home.clone());
        let mut cloud = local.clone();
        cloud.menu_overrides.clear();
        let version_seven = written_by(serde_json::to_value(cloud.document()).unwrap(), 7);
        assert!(version_seven.get("menuOverrides").is_none());
        let parsed = parse(version_seven.clone(), 7).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert!(parsed.menu_overrides.is_empty());
        // Applying it resets this computer's menus, as a reset elsewhere would.
        assert_eq!(
            plan(&local, parsed).unwrap().menu_overrides,
            Some(BTreeMap::new())
        );
        let mut early = version_seven.clone();
        early["menuOverrides"] = json!({});
        assert!(matches!(parse(early, 7), Err(ParseError::Invalid(_))));
        let mut missing = version_seven.clone();
        missing["schemaVersion"] = json!(SCHEMA_VERSION);
        assert!(matches!(
            parse(missing, SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));
        // A rearranged menu syncs, and a menu that may not change refuses the document.
        let value = serde_json::to_value(local.document()).unwrap();
        assert_eq!(value["menuOverrides"]["home"]["id"], json!("builtin-home"));
        let parsed = parse(value, SCHEMA_VERSION).unwrap();
        assert!(plan(&local, parsed).unwrap().is_empty());
        let mut bad = local.clone();
        bad.menu_overrides
            .insert("shutDown".into(), default_document(Kind::ShutDown));
        let parsed = parse(
            serde_json::to_value(bad.document()).unwrap(),
            SCHEMA_VERSION,
        )
        .unwrap();
        assert!(plan(&local, parsed).is_err());
    }

    #[test]
    fn version_six_documents_read_with_no_custom_panels() {
        let local = local();
        let mut cloud = local.clone();
        cloud.panels.clear();
        let version_six = written_by(serde_json::to_value(cloud.document()).unwrap(), 6);
        assert!(version_six.get("panels").is_none());
        let parsed = parse(version_six.clone(), 6).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert!(parsed.panels.is_empty());
        // Applying it removes this computer's panels, as a deletion elsewhere would.
        let planned = plan(&local, parsed).unwrap();
        assert_eq!(planned.panels, Some(Vec::new()));

        // A version 6 document never carried panels, and a version 7 one must.
        let mut early = version_six.clone();
        early["panels"] = json!([]);
        assert!(matches!(parse(early, 6), Err(ParseError::Invalid(_))));
        let mut missing = version_six.clone();
        missing["schemaVersion"] = json!(SCHEMA_VERSION);
        assert!(matches!(
            parse(missing, SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));

        // Panels sync with their tiles, and an invalid panel refuses the document.
        let value = serde_json::to_value(local.document()).unwrap();
        assert_eq!(
            value["panels"][0]["rows"][0][1]["action"]["type"],
            json!("text")
        );
        let parsed = parse(value, SCHEMA_VERSION).unwrap();
        assert!(plan(&local, parsed).unwrap().is_empty());
        let mut bad = local.clone();
        bad.panels[0].rows.clear();
        let parsed = parse(
            serde_json::to_value(bad.document()).unwrap(),
            SCHEMA_VERSION,
        )
        .unwrap();
        assert!(plan(&local, parsed).is_err());
    }

    #[test]
    fn version_four_documents_read_with_panels_matching_the_system() {
        use crate::scan_preferences::PanelTheme;
        let mut local = local();
        local.point_scan.scan_preferences.panel_theme = PanelTheme::Light;
        let mut cloud = local.clone();
        cloud.point_scan.scan_preferences.panel_theme = PanelTheme::System;
        let version_four = written_by(serde_json::to_value(cloud.document()).unwrap(), 4);
        assert!(version_four["pointScan"]["scanPreferences"]
            .get("panelTheme")
            .is_none());
        let parsed = parse(version_four.clone(), 4).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert_eq!(
            parsed.point_scan.scan_preferences.panel_theme,
            PanelTheme::System
        );
        let point_scan = plan(&local, parsed).unwrap().point_scan.unwrap();
        assert_eq!(point_scan.scan_preferences.panel_theme, PanelTheme::System);

        // Version 4 never had a panel theme, and a version 5 document needs it.
        let mut early = version_four.clone();
        early["pointScan"]["scanPreferences"]["panelTheme"] = json!("light");
        assert!(matches!(parse(early, 4), Err(ParseError::Invalid(_))));
        let mut missing = version_four.clone();
        missing["schemaVersion"] = json!(SCHEMA_VERSION);
        assert!(matches!(
            parse(missing, SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));
        // Every older version still climbs to the current one.
        for version in 1..SCHEMA_VERSION {
            let old = written_by(serde_json::to_value(cloud.document()).unwrap(), version);
            let parsed = parse(old, version).unwrap();
            assert_eq!(parsed.schema_version, SCHEMA_VERSION);
            assert_eq!(
                parsed.point_scan.scan_preferences.panel_theme,
                PanelTheme::System
            );
        }
        // A merge base saved by version 4 still loads.
        let base: Document = serde_json::from_value(written_by(
            serde_json::to_value(local.document()).unwrap(),
            4,
        ))
        .unwrap();
        assert_eq!(
            base.point_scan.scan_preferences.panel_theme,
            PanelTheme::System
        );

        // A version 5 document climbs unchanged: nothing was added in 6.
        let version_five = written_by(serde_json::to_value(cloud.document()).unwrap(), 5);
        let parsed = parse(version_five.clone(), 5).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert_eq!(parsed.switches, cloud.switches);
        assert!(matches!(
            parse(version_five, SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));

        // A chosen theme syncs, and an unknown one is refused rather than reset.
        let mut chosen = serde_json::to_value(cloud.document()).unwrap();
        chosen["pointScan"]["scanPreferences"]["panelTheme"] = json!("dark");
        let parsed = parse(chosen.clone(), SCHEMA_VERSION).unwrap();
        assert_eq!(
            parsed.point_scan.scan_preferences.panel_theme,
            PanelTheme::Dark
        );
        chosen["pointScan"]["scanPreferences"]["panelTheme"] = json!("sepia");
        assert!(matches!(
            parse(chosen, SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));
    }

    #[test]
    fn version_three_documents_read_with_the_radar_following_line_speed() {
        let mut local = local();
        local.point_scan.radar_speed = Some(4);
        let mut cloud = local.clone();
        cloud.point_scan.radar_speed = None;
        let version_three = written_by(serde_json::to_value(cloud.document()).unwrap(), 3);
        let parsed = parse(version_three.clone(), 3).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert_eq!(parsed.point_scan.radar_speed, None);
        let point_scan = plan(&local, parsed).unwrap().point_scan.unwrap();
        assert_eq!(point_scan.radar_speed, None);

        // Version 3 never had a radar speed, and a version 4 document needs it.
        let mut early = version_three.clone();
        early["pointScan"]["radarSpeed"] = json!(4);
        assert!(matches!(parse(early, 3), Err(ParseError::Invalid(_))));
        let mut missing = version_three;
        missing["schemaVersion"] = json!(SCHEMA_VERSION);
        assert!(matches!(
            parse(missing, SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));
        // A merge base saved by version 3 still loads.
        let base: Document = serde_json::from_value(written_by(
            serde_json::to_value(local.document()).unwrap(),
            3,
        ))
        .unwrap();
        assert_eq!(base.point_scan.radar_speed, None);

        // A chosen radar speed syncs, and an invalid one is refused.
        let mut chosen = serde_json::to_value(cloud.document()).unwrap();
        chosen["pointScan"]["radarSpeed"] = json!(0);
        let parsed = parse(chosen.clone(), SCHEMA_VERSION).unwrap();
        assert_eq!(parsed.point_scan.radar_speed, Some(0));
        chosen["pointScan"]["radarSpeed"] = json!(5);
        let parsed = parse(chosen, SCHEMA_VERSION).unwrap();
        assert!(plan(&local, parsed).is_err());
    }

    #[test]
    fn version_two_documents_read_with_row_groups_off_and_inherited() {
        let mut local = local();
        local.point_scan.scan_preferences.row_groups = true;
        local.point_scan.scan_preferences.keyboard.row_groups = Some(true);
        let mut cloud = local.clone();
        cloud.point_scan.scan_preferences = Default::default();
        let version_two = written_by(serde_json::to_value(cloud.document()).unwrap(), 2);
        let parsed = parse(version_two.clone(), 2).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        let preferences = &parsed.point_scan.scan_preferences;
        assert!(!preferences.row_groups);
        for area in [
            &preferences.point,
            &preferences.menu,
            &preferences.keyboard,
            &preferences.mouse,
        ] {
            assert_eq!(area.row_groups, None);
        }
        let point_scan = plan(&local, parsed).unwrap().point_scan.unwrap();
        assert_eq!(point_scan.scan_preferences, Default::default());

        // Version 2 never had row groups, and a version 3 document needs them.
        let mut early = version_two.clone();
        early["pointScan"]["scanPreferences"]["menu"]["rowGroups"] = json!(true);
        assert!(matches!(parse(early, 2), Err(ParseError::Invalid(_))));
        let mut missing = version_two;
        missing["schemaVersion"] = json!(SCHEMA_VERSION);
        assert!(matches!(
            parse(missing, SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));
        // A merge base saved by version 2 still loads.
        let base: Document = serde_json::from_value(written_by(
            serde_json::to_value(local.document()).unwrap(),
            2,
        ))
        .unwrap();
        assert!(!base.point_scan.scan_preferences.row_groups);
    }

    #[test]
    fn synced_scan_preferences_are_deliberate() {
        // Changing scan_preferences::Preferences changes what syncs. Update
        // this list and bump SCHEMA_VERSION together.
        let value = serde_json::to_value(local().document()).unwrap();
        let preferences = &value["pointScan"]["scanPreferences"];
        let keys = |value: &Value| {
            let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
            keys.sort_unstable();
            keys
        };
        assert_eq!(
            keys(preferences),
            [
                "direction",
                "firstItemMs",
                "groupRatePercent",
                "keyboard",
                "menu",
                "mouse",
                "nextScan",
                "panelSize",
                "panelTheme",
                "passLimit",
                "pattern",
                "point",
                "rowGroups",
                "startFrom",
                "thickness",
            ]
        );
        for area in ["point", "menu", "keyboard", "mouse"] {
            assert_eq!(
                keys(&preferences[area]),
                [
                    "automatic",
                    "color",
                    "direction",
                    "intervalMs",
                    "nextScan",
                    "passLimit",
                    "pattern",
                    "rowGroups",
                    "startFrom",
                    "thickness",
                ]
            );
        }
    }

    #[test]
    fn version_one_documents_read_with_scanned_key_repeat_off() {
        let mut local = local();
        local.settings.scan_key_repeat_enabled = true;
        let parsed = parse(version_one(), 1).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert!(!parsed.app.scan_key_repeat_enabled);
        let settings = plan(&local, parsed).unwrap().settings.unwrap();
        assert!(!settings.scan_key_repeat_enabled);

        // Version 1 never had the field, and a version 2 document needs it.
        let mut early = version_one();
        early["app"]["scanKeyRepeatEnabled"] = json!(true);
        assert!(matches!(parse(early, 1), Err(ParseError::Invalid(_))));
        let mut missing = version_one();
        missing["schemaVersion"] = json!(SCHEMA_VERSION);
        assert!(matches!(
            parse(missing.clone(), SCHEMA_VERSION),
            Err(ParseError::Invalid(_))
        ));
        assert!(matches!(parse(missing, 1), Err(ParseError::Invalid(_))));
    }

    #[test]
    fn a_merge_base_saved_by_version_one_still_loads() {
        let base: Document = serde_json::from_value(version_one()).unwrap();
        assert!(!base.app.scan_key_repeat_enabled);
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
        assert!(!settings.auto_approve_account_devices);
        assert!(!settings.learn_word_prediction);
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
                matches!(
                    parse(value.clone(), SCHEMA_VERSION),
                    Err(ParseError::Invalid(_))
                ),
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
                "radarSpeed",
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
