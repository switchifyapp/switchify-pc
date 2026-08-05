use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::protocol::{PendingPairingSummary, ProtocolEngine};
use crate::storage::{AppStorage, PersistedState};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub enum BluetoothState {
    Initializing,
    Advertising,
    Connected,
    PoweredOff,
    Unauthorized,
    #[cfg(target_os = "windows")]
    Conflict,
    Unsupported,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub enum AccessibilityState {
    Granted,
    Required,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ActivityKind {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub kind: ActivityKind,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub start_with_system: bool,
    pub pointer_scale_percent: u8,
    pub mouse_repeat_enabled: bool,
    pub move_repeat_interval_ms: u32,
    pub scroll_repeat_interval_ms: u32,
    #[serde(default = "default_mouse_repeat_acceleration")]
    pub mouse_repeat_acceleration_duration_ms: u32,
    pub cursor_overlay_enabled: bool,
    pub cursor_overlay_size: String,
    pub cursor_overlay_color: String,
    #[serde(default = "default_cursor_overlay_visibility")]
    pub cursor_overlay_visibility: String,
    pub cursor_crosshairs: bool,
    pub share_diagnostics: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            start_with_system: false,
            pointer_scale_percent: 100,
            mouse_repeat_enabled: true,
            move_repeat_interval_ms: 250,
            scroll_repeat_interval_ms: 250,
            mouse_repeat_acceleration_duration_ms: default_mouse_repeat_acceleration(),
            cursor_overlay_enabled: true,
            cursor_overlay_size: "medium".into(),
            cursor_overlay_color: "red".into(),
            cursor_overlay_visibility: default_cursor_overlay_visibility(),
            cursor_crosshairs: false,
            share_diagnostics: false,
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Result<Self, String> {
        if !(5..=225).contains(&self.pointer_scale_percent)
            || !self.pointer_scale_percent.is_multiple_of(5)
        {
            return Err("Pointer speed must be between 5 and 225 in steps of 5.".into());
        }
        if ![100, 250, 500, 1000].contains(&self.move_repeat_interval_ms)
            || ![100, 250, 500, 1000].contains(&self.scroll_repeat_interval_ms)
        {
            return Err("Mouse repeat interval is invalid.".into());
        }
        if ![0, 500, 1000, 2000].contains(&self.mouse_repeat_acceleration_duration_ms) {
            return Err("Mouse repeat acceleration is invalid.".into());
        }
        if !["small", "medium", "large"].contains(&self.cursor_overlay_size.as_str())
            || !["red", "green", "blue", "yellow", "white"]
                .contains(&self.cursor_overlay_color.as_str())
            || !["onInput", "whileControlling"].contains(&self.cursor_overlay_visibility.as_str())
        {
            return Err("Cursor overlay setting is invalid.".into());
        }
        self.cursor_overlay_size = self.cursor_overlay_size.to_lowercase();
        self.cursor_overlay_color = self.cursor_overlay_color.to_lowercase();
        Ok(self)
    }
}

fn default_cursor_overlay_visibility() -> String {
    "whileControlling".into()
}

fn default_mouse_repeat_acceleration() -> u32 {
    1000
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PairedDeviceView {
    pub device_id: String,
    pub device_name: String,
    pub paired_at: i64,
    pub last_seen_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    pub platform: String,
    pub grid3: bool,
    pub ui_access: bool,
    pub display_navigation: bool,
    pub cursor_overlay: bool,
}

impl PlatformCapabilities {
    fn current() -> Self {
        #[cfg(target_os = "windows")]
        return Self {
            platform: "windows".into(),
            grid3: true,
            ui_access: false,
            display_navigation: false,
            cursor_overlay: true,
        };
        #[cfg(target_os = "macos")]
        return Self {
            platform: "macos".into(),
            grid3: false,
            ui_access: false,
            display_navigation: false,
            cursor_overlay: true,
        };
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        Self {
            platform: "linux".into(),
            grid3: false,
            ui_access: false,
            display_navigation: false,
            cursor_overlay: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SwitchBinding {
    pub switch_id: u8,
    #[serde(rename = "type")]
    pub binding_type: String,
    pub value: Option<String>,
    pub keys: Option<Vec<String>>,
    pub click_count: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SwitchProfile {
    pub id: String,
    #[serde(default = "default_profile_version")]
    pub version: u32,
    pub name: String,
    pub provider: String,
    pub built_in: bool,
    pub bindings: Vec<SwitchBinding>,
}

fn default_profile_version() -> u32 {
    1
}

pub fn built_in_profiles(include_grid3: bool) -> Vec<SwitchProfile> {
    let none = |id| SwitchBinding {
        switch_id: id,
        binding_type: "none".into(),
        value: None,
        keys: None,
        click_count: None,
    };
    let mut profiles = vec![SwitchProfile {
        id: "builtin.keyboard".into(),
        version: 1,
        name: "Generic keyboard".into(),
        provider: "mapped".into(),
        built_in: true,
        bindings: (1..=8)
            .map(|id| match id {
                1 => SwitchBinding {
                    switch_id: id,
                    binding_type: "key".into(),
                    value: Some("Space".into()),
                    keys: None,
                    click_count: None,
                },
                2 => SwitchBinding {
                    switch_id: id,
                    binding_type: "key".into(),
                    value: Some("Enter".into()),
                    keys: None,
                    click_count: None,
                },
                _ => none(id),
            })
            .collect(),
    }];
    if include_grid3 {
        profiles.push(SwitchProfile {
            id: "builtin.grid3".into(),
            version: 1,
            name: "Grid 3".into(),
            provider: "grid3".into(),
            built_in: true,
            bindings: (1..=8).map(none).collect(),
        });
    }
    profiles
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub bluetooth: BluetoothState,
    pub accessibility: AccessibilityState,
    pub desktop_id: String,
    pub pending_pairing: Option<PendingPairingSummary>,
    pub paired_devices: Vec<PairedDeviceView>,
    pub connected_device_name: Option<String>,
    pub last_activity: Option<Activity>,
    pub settings: AppSettings,
    pub capabilities: PlatformCapabilities,
    pub version: String,
}

#[derive(Debug)]
pub struct ModelData {
    pub state: AppState,
    pub engine: ProtocolEngine,
    pub profiles: Vec<SwitchProfile>,
}
pub type SharedModel = Arc<Mutex<ModelData>>;

pub struct AppModel {
    pub shared: SharedModel,
    pub storage: AppStorage,
}

impl AppModel {
    pub fn new() -> Self {
        let storage = AppStorage::new();
        let saved = storage.load().unwrap_or_else(|_| PersistedState::default());
        let desktop_id = saved
            .desktop_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let capabilities = PlatformCapabilities::current();
        let mut profiles = built_in_profiles(capabilities.grid3);
        profiles.extend(
            saved
                .profiles
                .into_iter()
                .filter(|profile| !profile.built_in),
        );
        let mut engine = ProtocolEngine::new(desktop_id.clone());
        for device in &saved.paired_devices {
            if let Some(token) = storage.load_pairing_token(&device.device_id) {
                engine.set_paired_token(device.device_id.clone(), token);
            }
        }
        let shared = Arc::new(Mutex::new(ModelData {
            engine,
            profiles,
            state: AppState {
                bluetooth: BluetoothState::Initializing,
                accessibility: AccessibilityState::Required,
                desktop_id,
                pending_pairing: None,
                paired_devices: saved.paired_devices,
                connected_device_name: None,
                last_activity: None,
                settings: saved.settings,
                capabilities,
                version: env!("CARGO_PKG_VERSION").into(),
            },
        }));
        let model = Self { shared, storage };
        let _ = model.persist();
        model
    }

    pub fn snapshot(&self) -> AppState {
        snapshot(&self.shared)
    }
    pub fn persist(&self) -> Result<(), String> {
        self.storage.save(&self.persisted_state(None))
    }
    pub fn persist_settings(&self, settings: &AppSettings) -> Result<(), String> {
        self.storage.save(&self.persisted_state(Some(settings)))
    }
    fn persisted_state(&self, settings: Option<&AppSettings>) -> PersistedState {
        let data = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        PersistedState {
            schema_version: 1,
            desktop_id: Some(data.state.desktop_id.clone()),
            paired_devices: data.state.paired_devices.clone(),
            settings: settings
                .cloned()
                .unwrap_or_else(|| data.state.settings.clone()),
            profiles: data
                .profiles
                .iter()
                .filter(|profile| !profile.built_in)
                .cloned()
                .collect(),
        }
    }
}

pub fn snapshot(shared: &SharedModel) -> AppState {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .clone()
}
pub fn emit_state(app: &AppHandle, shared: &SharedModel) {
    let _ = app.emit("preview-state-changed", snapshot(shared));
}
pub fn set_activity(shared: &SharedModel, kind: ActivityKind, message: impl Into<String>) {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .last_activity = Some(Activity {
        kind,
        message: message.into(),
    });
}
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_default_to_a_persistent_cursor_overlay() {
        assert_eq!(
            AppSettings::default().cursor_overlay_visibility,
            "whileControlling"
        );
    }
    #[test]
    fn settings_reject_unsafe_values() {
        let value = AppSettings {
            pointer_scale_percent: 101,
            ..AppSettings::default()
        };
        assert!(value.normalized().is_err());
    }
    #[test]
    fn settings_reject_unknown_overlay_visibility() {
        let value = AppSettings {
            cursor_overlay_visibility: "always".into(),
            ..AppSettings::default()
        };
        assert!(value.normalized().is_err());
    }
    #[test]
    fn settings_default_repeat_acceleration_matches_shipping_app() {
        assert_eq!(
            AppSettings::default().mouse_repeat_acceleration_duration_ms,
            1000
        );
    }
    #[test]
    fn settings_reject_unknown_repeat_acceleration() {
        let value = AppSettings {
            mouse_repeat_acceleration_duration_ms: 750,
            ..AppSettings::default()
        };
        assert!(value.normalized().is_err());
    }
    #[test]
    fn mac_capability_shape_can_omit_grid3() {
        let profiles = built_in_profiles(false);
        assert!(profiles.iter().all(|profile| profile.provider != "grid3"));
    }
}
