use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::diagnostics::{DiagnosticHistory, DiagnosticSummary};
use crate::protocol::{PendingPairingSummary, ProtocolEngine};
use crate::storage::{AppStorage, PersistedState};
use crate::telemetry::{TelemetryConsent, TelemetryService, TelemetryView};
use crate::updater::UpdateView;

pub const APP_STATE_EVENT: &str = "app-state-changed";

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
    #[serde(default)]
    pub dwell_click_enabled: bool,
    #[serde(default = "default_dwell_click_delay")]
    pub dwell_click_delay_ms: u32,
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
            dwell_click_enabled: false,
            dwell_click_delay_ms: default_dwell_click_delay(),
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
        if ![500, 1000, 1500, 2000, 3000, 4000, 5000, 6000, 7000, 8000]
            .contains(&self.dwell_click_delay_ms)
        {
            return Err("Dwell click delay is invalid.".into());
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

pub fn normalize_pointer_scale_percent(scale_percent: f64) -> Result<u8, String> {
    if !scale_percent.is_finite() || scale_percent <= 0.0 {
        return Err("Pointer speed is invalid.".into());
    }
    Ok(((scale_percent / 5.0).round() * 5.0).clamp(5.0, 225.0) as u8)
}

fn default_cursor_overlay_visibility() -> String {
    "whileControlling".into()
}

fn default_mouse_repeat_acceleration() -> u32 {
    1000
}

fn default_dwell_click_delay() -> u32 {
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
            ui_access: crate::windows_security::has_ui_access(),
            display_navigation: true,
            cursor_overlay: true,
        };
        #[cfg(target_os = "macos")]
        return Self {
            platform: "macos".into(),
            grid3: false,
            ui_access: false,
            display_navigation: true,
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
    pub pending_pairings: Vec<PendingPairingSummary>,
    pub paired_devices: Vec<PairedDeviceView>,
    pub connected_device_name: Option<String>,
    pub last_activity: Option<Activity>,
    pub settings: AppSettings,
    pub capabilities: PlatformCapabilities,
    pub version: String,
    pub diagnostics: DiagnosticSummary,
    pub telemetry: TelemetryView,
    pub setup: SetupState,
    pub updater: UpdateView,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetupState {
    pub shown: bool,
    pub completed: bool,
    pub auto_open_eligible: bool,
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
    pub diagnostics: DiagnosticHistory,
    pub telemetry: TelemetryService,
    emission_lock: Mutex<()>,
    persistence_lock: Mutex<()>,
    preserved_paired_devices: Vec<PairedDeviceView>,
}

impl AppModel {
    pub fn new() -> Self {
        let storage = AppStorage::new();
        Self::with_storage(storage)
    }

    fn with_storage(storage: AppStorage) -> Self {
        let diagnostics = DiagnosticHistory::new(storage.diagnostic_history_path());
        let mut saved = storage.load().unwrap_or_else(|_| PersistedState::default());
        let telemetry_consent = match saved.telemetry_consent {
            Some(true) => TelemetryConsent::Enabled,
            Some(false) => TelemetryConsent::Disabled,
            None if saved.settings.share_diagnostics => TelemetryConsent::Enabled,
            None => TelemetryConsent::Undecided,
        };
        saved.settings.share_diagnostics = telemetry_consent == TelemetryConsent::Enabled;
        let telemetry = TelemetryService::new(storage.telemetry_path(), telemetry_consent);
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
        let saved_pairing_count = saved.paired_devices.len();
        let saved_paired_devices = saved.paired_devices;
        let restored_devices = restore_paired_devices(
            saved_paired_devices.clone(),
            |device_id| storage.load_pairing_token(device_id),
            &mut engine,
        );
        let (paired_devices, preserved_paired_devices, pairing_storage_error) =
            match restored_devices {
                Ok(devices) => (devices, Vec::new(), None),
                Err(error) => (Vec::new(), saved_paired_devices, Some(error)),
            };
        let discarded_legacy_pairing =
            pairing_storage_error.is_none() && paired_devices.len() < saved_pairing_count;
        let setup_auto_open_eligible = pairing_storage_error.is_none()
            && saved_pairing_count == 0
            && !saved.setup_shown
            && !saved.setup_completed;
        let shared = Arc::new(Mutex::new(ModelData {
            engine,
            profiles,
            state: AppState {
                bluetooth: BluetoothState::Initializing,
                accessibility: AccessibilityState::Required,
                desktop_id,
                pending_pairings: Vec::new(),
                paired_devices,
                connected_device_name: None,
                last_activity: pairing_storage_error
                    .map(|error| Activity {
                        kind: ActivityKind::Error,
                        message: format!("Pairing storage could not be read: {error}"),
                    })
                    .or_else(|| {
                        discarded_legacy_pairing.then(|| Activity {
                            kind: ActivityKind::Info,
                            message:
                                "Pair Android again once to finish the secure-storage upgrade."
                                    .into(),
                        })
                    }),
                settings: saved.settings,
                capabilities,
                version: env!("CARGO_PKG_VERSION").into(),
                diagnostics: DiagnosticSummary::default(),
                telemetry: telemetry.view(),
                setup: SetupState {
                    shown: saved.setup_shown,
                    completed: saved.setup_completed,
                    auto_open_eligible: setup_auto_open_eligible,
                },
                updater: UpdateView::default(),
            },
        }));
        let model = Self {
            shared,
            storage,
            diagnostics,
            telemetry,
            emission_lock: Mutex::new(()),
            persistence_lock: Mutex::new(()),
            preserved_paired_devices,
        };
        let _ = model.persist();
        let summary = model.diagnostics.startup(now_ms());
        model.set_diagnostic_summary(summary);
        let state = model.snapshot();
        let summary = model.diagnostics.observe(&state, now_ms());
        model.set_diagnostic_summary(summary);
        model
    }

    #[cfg(test)]
    pub(crate) fn with_storage_for_test(storage: AppStorage) -> Self {
        Self::with_storage(storage)
    }

    pub fn snapshot(&self) -> AppState {
        snapshot(&self.shared)
    }
    pub fn persist(&self) -> Result<(), String> {
        let _transaction = self
            .persistence_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.persist_unlocked()
    }
    fn persist_unlocked(&self) -> Result<(), String> {
        self.storage.save(&self.persisted_state(None, None, None))
    }
    pub fn apply_remote_name(&self, device_id: &str, device_name: &str) -> Result<bool, String> {
        let _transaction = self
            .persistence_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = {
            let mut data = self
                .shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(device) = data
                .state
                .paired_devices
                .iter_mut()
                .find(|device| device.device_id == device_id)
            else {
                return Ok(false);
            };
            let previous_name = std::mem::replace(&mut device.device_name, device_name.to_owned());
            let previous_connected_name = data.state.connected_device_name.clone();
            if previous_connected_name.is_some() {
                data.state.connected_device_name = Some(device_name.to_owned());
            }
            (previous_name, previous_connected_name)
        };
        if let Err(error) = self.persist_unlocked() {
            let mut data = self
                .shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(device) = data
                .state
                .paired_devices
                .iter_mut()
                .find(|device| device.device_id == device_id)
            {
                device.device_name = previous.0;
            }
            data.state.connected_device_name = previous.1;
            return Err(error);
        }
        Ok(true)
    }
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn persist_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let _transaction = self
            .persistence_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.persist_settings_unlocked(settings, None)
    }
    fn persist_settings_unlocked(
        &self,
        settings: &AppSettings,
        consent: Option<TelemetryConsent>,
    ) -> Result<(), String> {
        self.storage
            .save(&self.persisted_state(Some(settings), consent, None))
    }
    pub fn apply_pointer_scale_percent(&self, scale_percent: u8) -> Result<(), String> {
        let _transaction = self
            .persistence_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut settings = self.snapshot().settings;
        settings.pointer_scale_percent = scale_percent;
        let settings = settings.normalized()?;
        self.persist_settings_unlocked(&settings, None)?;
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .settings = settings;
        Ok(())
    }
    pub fn apply_settings_with_telemetry(
        &self,
        settings: AppSettings,
        consent: TelemetryConsent,
    ) -> Result<(), String> {
        // Opting out takes effect immediately even if the following save fails.
        if consent == TelemetryConsent::Disabled {
            self.set_telemetry_consent(consent);
        }
        let _transaction = self
            .persistence_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.apply_settings_with_telemetry_unlocked(settings, consent)
    }
    fn apply_settings_with_telemetry_unlocked(
        &self,
        settings: AppSettings,
        consent: TelemetryConsent,
    ) -> Result<(), String> {
        self.persist_settings_unlocked(&settings, Some(consent))?;
        self.telemetry.set_consent(consent);
        let telemetry = self.telemetry.view();
        let mut data = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        data.state.settings = settings;
        data.state.telemetry = telemetry;
        Ok(())
    }
    pub fn record_updater(&self, status: &str, detail: Option<&str>) {
        let _emission = self
            .emission_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let summary = self.diagnostics.updater(now_ms(), status, detail);
        self.set_diagnostic_summary(summary);
    }
    fn set_diagnostic_summary(&self, summary: DiagnosticSummary) {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .diagnostics = summary;
    }
    pub fn set_telemetry_consent(&self, consent: TelemetryConsent) {
        self.telemetry.set_consent(consent);
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .telemetry = self.telemetry.view();
    }
    pub fn apply_telemetry_choice(&self, enabled: bool) -> Result<AppState, String> {
        let consent = if enabled {
            TelemetryConsent::Enabled
        } else {
            TelemetryConsent::Disabled
        };
        if !enabled {
            self.set_telemetry_consent(consent);
        }
        let _transaction = self
            .persistence_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut settings = self.snapshot().settings;
        settings.share_diagnostics = enabled;
        self.apply_settings_with_telemetry_unlocked(settings, consent)?;
        Ok(self.snapshot())
    }
    fn persisted_state(
        &self,
        settings: Option<&AppSettings>,
        telemetry_consent: Option<TelemetryConsent>,
        setup: Option<&SetupState>,
    ) -> PersistedState {
        let data = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut paired_devices = self
            .preserved_paired_devices
            .iter()
            .filter(|preserved| {
                !data
                    .state
                    .paired_devices
                    .iter()
                    .any(|active| active.device_id == preserved.device_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        paired_devices.extend(data.state.paired_devices.clone());
        PersistedState {
            schema_version: 1,
            desktop_id: Some(data.state.desktop_id.clone()),
            paired_devices,
            settings: settings
                .cloned()
                .unwrap_or_else(|| data.state.settings.clone()),
            profiles: data
                .profiles
                .iter()
                .filter(|profile| !profile.built_in)
                .cloned()
                .collect(),
            telemetry_consent: match telemetry_consent.unwrap_or(data.state.telemetry.consent) {
                TelemetryConsent::Enabled => Some(true),
                TelemetryConsent::Disabled => Some(false),
                TelemetryConsent::Undecided => None,
            },
            setup_shown: setup.unwrap_or(&data.state.setup).shown,
            setup_completed: setup.unwrap_or(&data.state.setup).completed,
        }
    }

    pub fn mark_setup_shown(&self) -> Result<AppState, String> {
        let _transaction = self
            .persistence_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = {
            let mut data = self
                .shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let previous = data.state.setup.clone();
            data.state.setup.shown = true;
            data.state.setup.auto_open_eligible = false;
            previous
        };
        if let Err(error) = self.persist_unlocked() {
            self.shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .state
                .setup = previous;
            return Err(error);
        }
        Ok(self.snapshot())
    }

    pub fn apply_setup_completion(
        &self,
        settings: AppSettings,
        consent: TelemetryConsent,
    ) -> Result<AppState, String> {
        let _transaction = self
            .persistence_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.snapshot().paired_devices.is_empty() {
            return Err("Pair an Android device before finishing setup.".into());
        }
        let completed = SetupState {
            shown: true,
            completed: true,
            auto_open_eligible: false,
        };
        self.storage.save(&self.persisted_state(
            Some(&settings),
            Some(consent),
            Some(&completed),
        ))?;
        self.telemetry.set_consent(consent);
        let mut data = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        data.state.settings = settings;
        data.state.telemetry = self.telemetry.view();
        data.state.setup = completed;
        Ok(data.state.clone())
    }
}

fn restore_paired_devices(
    devices: Vec<PairedDeviceView>,
    mut load_token: impl FnMut(&str) -> Result<Option<String>, String>,
    engine: &mut ProtocolEngine,
) -> Result<Vec<PairedDeviceView>, String> {
    let restored = devices
        .into_iter()
        .map(|device| {
            let Some(token) = load_token(&device.device_id)? else {
                return Ok(None);
            };
            Ok(Some((device, token)))
        })
        .filter_map(Result::transpose)
        .collect::<Result<Vec<_>, String>>()?;
    Ok(restored
        .into_iter()
        .map(|(device, token)| {
            engine.set_paired_token(device.device_id.clone(), token);
            device
        })
        .collect())
}

pub fn snapshot(shared: &SharedModel) -> AppState {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .clone()
}
pub fn emit_state(app: &AppHandle, shared: &SharedModel) {
    if let Some(model) = app.try_state::<AppModel>() {
        let _emission = model
            .emission_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current = snapshot(shared);
        let summary = model.diagnostics.observe(&current, now_ms());
        model.set_diagnostic_summary(summary);
        model.telemetry.observe(&current);
        let state = snapshot(shared);
        crate::sync_tray_state(app, &state);
        let _ = app.emit(APP_STATE_EVENT, state);
        return;
    }
    let state = snapshot(shared);
    crate::sync_tray_state(app, &state);
    let _ = app.emit(APP_STATE_EVENT, state);
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
    use std::fs;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn state_updates_use_the_promoted_event_name() {
        assert_eq!(APP_STATE_EVENT, "app-state-changed");
    }

    #[test]
    fn diagnostic_emission_transactions_cannot_overlap() {
        let root = std::env::temp_dir().join(format!("switchify-emit-{}", uuid::Uuid::new_v4()));
        let model = Arc::new(AppModel::with_storage(AppStorage::at(
            root.join("state.json"),
        )));
        let (first_locked_tx, first_locked_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (second_locked_tx, second_locked_rx) = mpsc::channel();

        let first = model.clone();
        let first_thread = thread::spawn(move || {
            let _guard = first.emission_lock.lock().unwrap();
            first_locked_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        first_locked_rx.recv().unwrap();

        let second = model.clone();
        let second_thread = thread::spawn(move || {
            let _guard = second.emission_lock.lock().unwrap();
            second_locked_tx.send(()).unwrap();
        });
        assert!(second_locked_rx
            .recv_timeout(Duration::from_millis(50))
            .is_err());
        release_tx.send(()).unwrap();
        second_locked_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        first_thread.join().unwrap();
        second_thread.join().unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fresh_storage_starts_with_a_new_unpaired_identity() {
        let root = std::env::temp_dir().join(format!("switchify-fresh-{}", uuid::Uuid::new_v4()));
        let model = AppModel::with_storage(AppStorage::at(root.join("state.json")));
        let state = model.snapshot();

        assert!(uuid::Uuid::parse_str(&state.desktop_id).is_ok());
        assert!(state.paired_devices.is_empty());
        assert_eq!(state.settings, AppSettings::default());
        assert_eq!(state.telemetry.consent, TelemetryConsent::Undecided);
        assert!(!model.storage.telemetry_path().exists());
        assert_eq!(
            state.setup,
            SetupState {
                shown: false,
                completed: false,
                auto_open_eligible: true
            }
        );
        let saved = model.storage.load().unwrap();
        assert_eq!(saved.desktop_id.as_deref(), Some(state.desktop_id.as_str()));
        assert!(saved.paired_devices.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn setup_dismissal_and_completion_persist() {
        let root = std::env::temp_dir().join(format!("switchify-setup-{}", Uuid::new_v4()));
        let path = root.join("state.json");
        let model = AppModel::with_storage_for_test(AppStorage::at(path.clone()));
        assert!(model.mark_setup_shown().unwrap().setup.shown);

        let dismissed = AppModel::with_storage_for_test(AppStorage::at(path.clone()));
        assert!(dismissed.snapshot().setup.shown);
        assert!(!dismissed.snapshot().setup.completed);
        assert_eq!(
            dismissed.apply_setup_completion(AppSettings::default(), TelemetryConsent::Undecided),
            Err("Pair an Android device before finishing setup.".into())
        );

        dismissed
            .shared
            .lock()
            .unwrap()
            .state
            .paired_devices
            .push(PairedDeviceView {
                device_id: "phone-1".into(),
                device_name: "Pixel".into(),
                paired_at: 1,
                last_seen_at: None,
            });
        let chosen_settings = AppSettings {
            start_with_system: true,
            ..AppSettings::default()
        };
        assert!(
            dismissed
                .apply_setup_completion(chosen_settings.clone(), TelemetryConsent::Disabled)
                .unwrap()
                .setup
                .completed
        );
        let completed = AppModel::with_storage_for_test(AppStorage::at(path));
        assert!(completed.snapshot().setup.completed);
        assert_eq!(completed.snapshot().settings, chosen_settings);
        assert_eq!(
            completed.snapshot().telemetry.consent,
            TelemetryConsent::Disabled
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn setup_completion_does_not_change_memory_when_the_atomic_save_fails() {
        let root = std::env::temp_dir().join(format!("switchify-setup-fail-{}", Uuid::new_v4()));
        let state_path = root.join("state.json");
        let model = AppModel::with_storage_for_test(AppStorage::at(state_path.clone()));
        model
            .shared
            .lock()
            .unwrap()
            .state
            .paired_devices
            .push(PairedDeviceView {
                device_id: "phone-1".into(),
                device_name: "Pixel".into(),
                paired_at: 1,
                last_seen_at: None,
            });
        fs::remove_file(&state_path).unwrap();
        fs::create_dir(&state_path).unwrap();
        let chosen = AppSettings {
            start_with_system: true,
            ..AppSettings::default()
        };

        assert!(model
            .apply_setup_completion(chosen, TelemetryConsent::Disabled)
            .is_err());
        let state = model.snapshot();
        assert_eq!(state.settings, AppSettings::default());
        assert!(!state.setup.completed);
        assert_eq!(state.telemetry.consent, TelemetryConsent::Undecided);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn remote_name_updates_persist_and_refresh_connected_state() {
        let root = std::env::temp_dir().join(format!("switchify-remote-name-{}", Uuid::new_v4()));
        let state_path = root.join("state.json");
        let model = AppModel::with_storage_for_test(AppStorage::at(state_path));
        {
            let mut data = model.shared.lock().unwrap();
            data.state.paired_devices.push(PairedDeviceView {
                device_id: "remote-1".into(),
                device_name: "Old name".into(),
                paired_at: 1,
                last_seen_at: None,
            });
            data.state.connected_device_name = Some("Old name".into());
        }
        model.persist().unwrap();

        assert!(model
            .apply_remote_name("remote-1", "Kitchen Remote")
            .unwrap());
        let state = model.snapshot();
        assert_eq!(state.paired_devices[0].device_name, "Kitchen Remote");
        assert_eq!(
            state.connected_device_name.as_deref(),
            Some("Kitchen Remote")
        );
        assert_eq!(
            model.storage.load().unwrap().paired_devices[0].device_name,
            "Kitchen Remote"
        );
        assert!(!model.apply_remote_name("missing", "Unknown").unwrap());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn remote_name_update_rolls_back_when_persistence_fails() {
        let root =
            std::env::temp_dir().join(format!("switchify-remote-name-fail-{}", Uuid::new_v4()));
        let state_path = root.join("state.json");
        let model = AppModel::with_storage_for_test(AppStorage::at(state_path.clone()));
        {
            let mut data = model.shared.lock().unwrap();
            data.state.paired_devices.push(PairedDeviceView {
                device_id: "remote-1".into(),
                device_name: "Old name".into(),
                paired_at: 1,
                last_seen_at: None,
            });
            data.state.connected_device_name = Some("Old name".into());
        }
        model.persist().unwrap();
        fs::remove_file(&state_path).unwrap();
        fs::create_dir(&state_path).unwrap();

        assert!(model.apply_remote_name("remote-1", "New name").is_err());
        let state = model.snapshot();
        assert_eq!(state.paired_devices[0].device_name, "Old name");
        assert_eq!(state.connected_device_name.as_deref(), Some("Old name"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_true_setting_becomes_explicit_opt_in_but_false_stays_undecided() {
        let root = std::env::temp_dir().join(format!("switchify-consent-{}", uuid::Uuid::new_v4()));
        let storage = AppStorage::at(root.join("state.json"));
        let mut saved = PersistedState::default();
        saved.settings.share_diagnostics = true;
        storage.save(&saved).unwrap();

        let model = AppModel::with_storage(storage);
        assert_eq!(
            model.snapshot().telemetry.consent,
            TelemetryConsent::Enabled
        );
        assert!(model.storage.telemetry_path().exists());

        let settings = AppSettings::default();
        model.set_telemetry_consent(TelemetryConsent::Disabled);
        model
            .apply_settings_with_telemetry(settings, TelemetryConsent::Disabled)
            .unwrap();
        assert!(!model.storage.telemetry_path().exists());
        let reloaded = AppModel::with_storage(AppStorage::at(root.join("state.json")));
        assert_eq!(
            reloaded.snapshot().telemetry.consent,
            TelemetryConsent::Disabled
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn paired_devices_without_accessible_tokens_are_removed_at_startup() {
        let available = PairedDeviceView {
            device_id: "available".into(),
            device_name: "Available phone".into(),
            paired_at: 1,
            last_seen_at: None,
        };
        let legacy = PairedDeviceView {
            device_id: "legacy".into(),
            device_name: "Legacy phone".into(),
            paired_at: 2,
            last_seen_at: None,
        };
        let mut engine = ProtocolEngine::new("desktop".into());
        let devices = restore_paired_devices(
            vec![available.clone(), legacy],
            |device_id| Ok((device_id == "available").then(|| "token".into())),
            &mut engine,
        )
        .unwrap();

        assert_eq!(devices, vec![available]);
        assert_eq!(engine.token_for("available"), Some("token"));
        assert_eq!(engine.token_for("legacy"), None);
    }

    #[test]
    fn pairing_restoration_is_transactional_when_storage_fails() {
        let first = PairedDeviceView {
            device_id: "first".into(),
            device_name: "First phone".into(),
            paired_at: 1,
            last_seen_at: None,
        };
        let second = PairedDeviceView {
            device_id: "second".into(),
            device_name: "Second phone".into(),
            paired_at: 2,
            last_seen_at: None,
        };
        let mut engine = ProtocolEngine::new("desktop".into());
        let result = restore_paired_devices(
            vec![first, second],
            |device_id| {
                if device_id == "first" {
                    Ok(Some("token".into()))
                } else {
                    Err("unreadable token store".into())
                }
            },
            &mut engine,
        );

        assert_eq!(result.unwrap_err(), "unreadable token store");
        assert_eq!(engine.token_for("first"), None);
        assert_eq!(engine.token_for("second"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn startup_storage_failure_preserves_persisted_pairing_metadata() {
        let root = std::env::temp_dir().join(format!(
            "switchify-startup-pairing-error-{}",
            uuid::Uuid::new_v4()
        ));
        let state_path = root.join("state.json");
        let storage = AppStorage::at(state_path);
        let saved = PersistedState {
            paired_devices: vec![PairedDeviceView {
                device_id: "android-1".into(),
                device_name: "Android phone".into(),
                paired_at: 1,
                last_seen_at: None,
            }],
            ..PersistedState::default()
        };
        storage.save(&saved).unwrap();
        fs::write(root.join("pairing-tokens.json"), "not json").unwrap();

        let model = AppModel::with_storage(storage);

        assert!(model.snapshot().paired_devices.is_empty());
        assert!(!model.snapshot().setup.auto_open_eligible);
        model.persist_settings(&AppSettings::default()).unwrap();
        assert_eq!(model.storage.load().unwrap().paired_devices.len(), 1);

        model
            .shared
            .lock()
            .unwrap()
            .state
            .paired_devices
            .push(PairedDeviceView {
                device_id: "android-1".into(),
                device_name: "Re-paired phone".into(),
                paired_at: 2,
                last_seen_at: None,
            });
        model.persist().unwrap();
        let repaired = model.storage.load().unwrap();
        assert_eq!(repaired.paired_devices.len(), 1);
        assert_eq!(repaired.paired_devices[0].device_name, "Re-paired phone");
        let _ = fs::remove_dir_all(root);
    }
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
    fn pointer_speed_normalization_rounds_and_clamps_to_supported_steps() {
        assert_eq!(normalize_pointer_scale_percent(1.0), Ok(5));
        assert_eq!(normalize_pointer_scale_percent(122.0), Ok(120));
        assert_eq!(normalize_pointer_scale_percent(123.0), Ok(125));
        assert_eq!(normalize_pointer_scale_percent(500.0), Ok(225));
        assert!(normalize_pointer_scale_percent(0.0).is_err());
        assert!(normalize_pointer_scale_percent(f64::NAN).is_err());
    }
    #[test]
    fn pointer_speed_transaction_preserves_a_serialized_settings_change() {
        let root = std::env::temp_dir().join(format!(
            "switchify-pointer-settings-race-{}",
            uuid::Uuid::new_v4()
        ));
        let state_path = root.join("state.json");
        let model = Arc::new(AppModel::with_storage_for_test(AppStorage::at(
            state_path.clone(),
        )));
        let transaction = model.persistence_lock.lock().unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let pointer_model = model.clone();
        let pointer_thread = thread::spawn(move || {
            started_tx.send(()).unwrap();
            pointer_model.apply_pointer_scale_percent(150)
        });
        started_rx.recv().unwrap();

        let mut desktop_settings = model.snapshot().settings;
        desktop_settings.cursor_crosshairs = true;
        model
            .persist_settings_unlocked(&desktop_settings, None)
            .unwrap();
        model.shared.lock().unwrap().state.settings = desktop_settings;
        drop(transaction);
        pointer_thread.join().unwrap().unwrap();

        let state = model.snapshot();
        assert_eq!(state.settings.pointer_scale_percent, 150);
        assert!(state.settings.cursor_crosshairs);
        let restored = AppModel::with_storage_for_test(AppStorage::at(state_path));
        assert_eq!(restored.snapshot().settings, state.settings);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn telemetry_opt_in_and_pointer_speed_remain_durable_when_concurrent() {
        let root = std::env::temp_dir().join(format!(
            "switchify-telemetry-pointer-race-{}",
            uuid::Uuid::new_v4()
        ));
        let state_path = root.join("state.json");
        let model = Arc::new(AppModel::with_storage_for_test(AppStorage::at(
            state_path.clone(),
        )));
        let transaction = model.persistence_lock.lock().unwrap();
        let (started_tx, started_rx) = mpsc::channel();

        let telemetry_model = model.clone();
        let telemetry_started = started_tx.clone();
        let telemetry_thread = thread::spawn(move || {
            telemetry_started.send(()).unwrap();
            telemetry_model.apply_telemetry_choice(true)
        });
        let pointer_model = model.clone();
        let pointer_thread = thread::spawn(move || {
            started_tx.send(()).unwrap();
            pointer_model.apply_pointer_scale_percent(150)
        });
        started_rx.recv().unwrap();
        started_rx.recv().unwrap();
        drop(transaction);

        telemetry_thread.join().unwrap().unwrap();
        pointer_thread.join().unwrap().unwrap();

        let state = model.snapshot();
        assert!(state.settings.share_diagnostics);
        assert_eq!(state.settings.pointer_scale_percent, 150);
        assert_eq!(state.telemetry.consent, TelemetryConsent::Enabled);
        let restored = AppModel::with_storage_for_test(AppStorage::at(state_path));
        assert!(restored.snapshot().settings.share_diagnostics);
        assert_eq!(restored.snapshot().settings.pointer_scale_percent, 150);
        assert_eq!(
            restored.snapshot().telemetry.consent,
            TelemetryConsent::Enabled
        );
        let _ = fs::remove_dir_all(root);
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
    fn dwell_click_defaults_are_safe_and_delays_are_bounded() {
        let defaults = AppSettings::default();
        assert!(!defaults.dwell_click_enabled);
        assert_eq!(defaults.dwell_click_delay_ms, 1000);
        for delay in [500, 1000, 1500, 2000, 3000, 4000, 5000, 6000, 7000, 8000] {
            assert!(AppSettings {
                dwell_click_delay_ms: delay,
                ..AppSettings::default()
            }
            .normalized()
            .is_ok());
        }
        for delay in [0, 499, 750, 3001, 8001, u32::MAX] {
            assert!(AppSettings {
                dwell_click_delay_ms: delay,
                ..AppSettings::default()
            }
            .normalized()
            .is_err());
        }
    }
    #[test]
    fn mac_capability_shape_can_omit_grid3() {
        let profiles = built_in_profiles(false);
        assert!(profiles.iter().all(|profile| profile.provider != "grid3"));
    }
}
