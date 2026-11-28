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

pub const SCHEMA_VERSION: u32 = 22;
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
/// `auto_approve_account_devices` (pairing is per install),
/// `sync_switches` (each computer can have its own switch interface) and
/// `text_size_percent` (screens differ between computers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableAppSettings {
    pub pointer_scale_percent: u16,
    pub mouse_repeat_enabled: bool,
    pub move_repeat_interval_ms: u32,
    pub scroll_repeat_interval_ms: u32,
    /// Added in version 14, with the same merge-base default as above.
    #[serde(default)]
    pub mouse_movement: crate::mouse_repeat::MouseMovement,
    #[serde(default)]
    pub mouse_block_size: crate::mouse_repeat::BlockSize,
    /// Added in version 15, with the same merge-base default as above.
    #[serde(default)]
    pub mouse_block_interval_ms: u32,
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
            mouse_movement: settings.mouse_movement,
            mouse_block_size: settings.mouse_block_size,
            mouse_block_interval_ms: settings.mouse_block_interval_ms,
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
            sync_switches: local.sync_switches,
            text_size_percent: local.text_size_percent,
            pointer_scale_percent: self.pointer_scale_percent,
            mouse_repeat_enabled: self.mouse_repeat_enabled,
            move_repeat_interval_ms: self.move_repeat_interval_ms,
            scroll_repeat_interval_ms: self.scroll_repeat_interval_ms,
            mouse_movement: self.mouse_movement,
            mouse_block_size: self.mouse_block_size,
            mouse_block_interval_ms: self.mouse_block_interval_ms,
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
            11 => upgrade_v11(value)?,
            12 => upgrade_v12(value)?,
            13 => upgrade_v13(value)?,
            14 => upgrade_v14(value)?,
            15 => upgrade_v15(value)?,
            16 => upgrade_v16(value)?,
            17 => upgrade_v17(value)?,
            18 => upgrade_v18(value)?,
            19 => upgrade_v19(value)?,
            20 => upgrade_v20(value)?,
            _ => upgrade_v21(value)?,
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
    value["schemaVersion"] = Value::from(12);
    Ok(value)
}

/// Version 13 drops the old prediction engine's options. Version 12 always
/// wrote `enhancedWordPrediction`, and wrote the nearby-key and abbreviation
/// options only when they were on.
fn upgrade_v12(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(12)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let invalid = || ParseError::Invalid("it does not match version 12 settings".into());
    let point_scan = value
        .get_mut("pointScan")
        .and_then(Value::as_object_mut)
        .ok_or_else(invalid)?;
    point_scan
        .remove("enhancedWordPrediction")
        .filter(Value::is_boolean)
        .ok_or_else(invalid)?;
    for optional in ["nearbyKeyPredictions", "abbreviationPredictions"] {
        if let Some(on) = point_scan.remove(optional) {
            if on != Value::Bool(true) {
                return Err(invalid());
            }
        }
    }
    value["schemaVersion"] = Value::from(13);
    Ok(value)
}

/// Version 13 had no block movement: the mouse panel's arrows glided, and
/// a block would have been Medium.
fn upgrade_v13(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(13)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let app = value
        .get_mut("app")
        .and_then(Value::as_object_mut)
        .filter(|app| !app.contains_key("mouseMovement") && !app.contains_key("mouseBlockSize"))
        .ok_or_else(|| ParseError::Invalid("it does not match version 13 settings".into()))?;
    app.insert("mouseMovement".into(), Value::from("smooth"));
    app.insert("mouseBlockSize".into(), Value::from("medium"));
    value["schemaVersion"] = Value::from(14);
    Ok(value)
}

/// Version 14 had no block speed: repeating blocks hopped at the mouse
/// panel's scan speed.
fn upgrade_v14(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(14)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let app = value
        .get_mut("app")
        .and_then(Value::as_object_mut)
        .filter(|app| !app.contains_key("mouseBlockIntervalMs"))
        .ok_or_else(|| ParseError::Invalid("it does not match version 14 settings".into()))?;
    app.insert("mouseBlockIntervalMs".into(), Value::from(0));
    value["schemaVersion"] = Value::from(15);
    Ok(value)
}

/// Version 15 offered only these block speeds. Version 16 takes any tenth
/// of a second from 0.1 to 10 seconds, which version 15 would reject, so
/// those documents carry a newer version.
const VERSION_15_BLOCK_INTERVALS_MS: [u64; 6] = [0, 500, 1000, 1500, 2000, 3000];

/// Requires the number at `pointer` to be one of the choices a version
/// offered. A later version takes any value in a range, which the earlier
/// version would reject, so a document holding one carries the later version.
fn require_choice(
    value: &Value,
    pointer: &str,
    choices: &[u64],
    version: u32,
) -> Result<(), ParseError> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .filter(|ms| choices.contains(ms))
        .map(|_| ())
        .ok_or_else(|| ParseError::Invalid(format!("it does not match version {version} settings")))
}

fn upgrade_v15(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(15)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    require_choice(
        &value,
        "/app/mouseBlockIntervalMs",
        &VERSION_15_BLOCK_INTERVALS_MS,
        15,
    )?;
    value["schemaVersion"] = Value::from(16);
    Ok(value)
}

/// Version 16 offered fixed choices for the dwell wait and key repeat timing.
/// Version 17 sets them with steppers.
fn upgrade_v16(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(16)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    require_choice(
        &value,
        "/app/dwellClickDelayMs",
        &[500, 1000, 1500, 2000, 3000, 4000, 5000, 6000, 7000, 8000],
        16,
    )?;
    require_choice(
        &value,
        "/app/keyRepeatInitialDelayMs",
        &[0, 250, 500, 1000],
        16,
    )?;
    require_choice(
        &value,
        "/app/keyRepeatIntervalMs",
        &[100, 250, 500, 1000],
        16,
    )?;
    value["schemaVersion"] = Value::from(17);
    Ok(value)
}

/// Version 17 offered fixed choices for the mouse repeat intervals and
/// acceleration. Version 18 sets them with steppers.
fn upgrade_v17(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(17)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    for pointer in ["/app/moveRepeatIntervalMs", "/app/scrollRepeatIntervalMs"] {
        require_choice(&value, pointer, &[100, 250, 500, 1000], 17)?;
    }
    require_choice(
        &value,
        "/app/mouseRepeatAccelerationDurationMs",
        &[0, 500, 1000, 2000],
        17,
    )?;
    value["schemaVersion"] = Value::from(18);
    Ok(value)
}

/// Version 18 offered fixed choices for press timing. Version 19 sets them
/// with steppers. Press timing that is off is left out of the document.
fn upgrade_v18(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(18)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    for (pointer, choices) in [
        ("/switches/acceptMs", &[0, 100, 200, 300, 500, 1000][..]),
        ("/switches/ignoreRepeatMs", &[0, 250, 500, 1000, 2000][..]),
    ] {
        if value.pointer(pointer).is_some() {
            require_choice(&value, pointer, choices, 18)?;
        }
    }
    value["schemaVersion"] = Value::from(19);
    Ok(value)
}

/// Version 19 offered fixed choices for the extra time on the first item.
/// Version 20 sets it with a stepper. Saved scan preferences fall back to
/// off for an unknown time, but a synced document must round-trip exactly,
/// so version 19 rejected any other time and so does this step.
fn upgrade_v19(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(19)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    require_choice(
        &value,
        "/pointScan/scanPreferences/firstItemMs",
        &[0, 500, 1000, 1500, 2000],
        19,
    )?;
    value["schemaVersion"] = Value::from(20);
    Ok(value)
}

/// Version 20 tiles had no colour. Version 21 lets a tile on a panel, a
/// rearranged menu or around the mouse panel's and the keyboard's keys carry
/// one, left out when unset, so the tiles themselves need no change.
fn upgrade_v20(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(20)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    let panels = value
        .get("panels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    let menus = value
        .get("menuOverrides")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|menus| menus.values());
    let extras = value
        .get("panelExtras")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|extras| extras.values());
    let coloured = panels
        .chain(menus)
        .filter_map(|panel| panel.get("rows"))
        .chain(
            extras
                .flat_map(|panel| [panel.get("rows"), panel.get("columns")])
                .flatten(),
        )
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_array)
        .flatten()
        .any(|tile| tile.get("color").is_some());
    if coloured {
        return Err(ParseError::Invalid(
            "it does not match version 20 settings".into(),
        ));
    }
    value["schemaVersion"] = Value::from(21);
    Ok(value)
}

/// Version 21 had no choice about built-in panel colours. Version 22 adds
/// it, on, as a new install has it.
fn upgrade_v21(mut value: Value) -> Result<Value, ParseError> {
    if value.get("schemaVersion") != Some(&Value::from(21)) {
        return Err(ParseError::Invalid("version mismatch".into()));
    }
    value
        .pointer_mut("/pointScan/scanPreferences")
        .and_then(Value::as_object_mut)
        .filter(|preferences| !preferences.contains_key("panelColors"))
        .ok_or_else(|| ParseError::Invalid("it does not match version 21 settings".into()))?
        .insert("panelColors".into(), Value::Bool(true));
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
    if version < 22 {
        value
            .pointer_mut("/pointScan/scanPreferences")
            .and_then(Value::as_object_mut)
            .unwrap()
            .remove("panelColors");
    }
    if version < 15 {
        value["app"]
            .as_object_mut()
            .unwrap()
            .remove("mouseBlockIntervalMs");
    }
    if version < 14 {
        let app = value["app"].as_object_mut().unwrap();
        app.remove("mouseMovement");
        app.remove("mouseBlockSize");
    }
    if version < 13 {
        value["pointScan"]["enhancedWordPrediction"] = Value::Bool(false);
    }
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
mod tests;
