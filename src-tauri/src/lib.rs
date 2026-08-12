mod diagnostics;
mod display_navigation;
#[cfg(target_os = "windows")]
mod grid3;
mod input;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_overlay_window;
#[cfg(target_os = "macos")]
mod macos_relaunch;
mod modifier_overlay;
mod mouse_repeat;
mod overlay;
mod protocol;
mod state;
mod storage;
mod telemetry;
mod updater;
#[cfg(target_os = "windows")]
mod windows_runtime;
#[cfg(target_os = "windows")]
mod windows_security;
#[cfg(target_os = "windows")]
mod windows_startup;

use state::{
    snapshot, ActivityKind, AppModel, AppSettings, AppState, PairedDeviceView, SwitchProfile,
};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
#[cfg(not(target_os = "windows"))]
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_updater::UpdaterExt;
use telemetry::TelemetryConsent;
use updater::{
    download_with_cancel, DownloadResult, Operation as UpdateOperation, RetryAction,
    UpdateArtifact, UpdateManager, UpdateView,
};

#[tauri::command]
fn get_app_state(model: State<'_, AppModel>) -> AppState {
    model.snapshot()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileExitAction {
    Hide,
    Quit,
}

impl ProfileExitAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Hide => "hide",
            Self::Quit => "quit",
        }
    }
}

#[derive(Default)]
struct PendingProfileExit(Mutex<Option<ProfileExitAction>>);

impl PendingProfileExit {
    fn begin(&self, action: ProfileExitAction) -> bool {
        let mut pending = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if pending.is_some() {
            return false;
        }
        *pending = Some(action);
        true
    }

    fn take(&self) -> Option<ProfileExitAction> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }

    fn cancel(&self) {
        self.take();
    }
}

fn request_profile_exit(app: &AppHandle, action: ProfileExitAction) {
    if !app.state::<PendingProfileExit>().begin(action) {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("profile-exit-requested", action.as_str());
    }
}

const NAVIGATE_REQUESTED_EVENT: &str = "navigate-requested";

#[derive(Default)]
struct PendingNavigation(Mutex<Option<String>>);

impl PendingNavigation {
    fn set(&self, destination: &str) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(destination.to_owned());
    }

    fn take(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

fn show_main_window(app: &AppHandle, destination: Option<&str>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        if let Some(destination) = destination {
            app.state::<PendingNavigation>().set(destination);
            let _ = window.emit(NAVIGATE_REQUESTED_EVENT, destination);
        }
    }
}

#[tauri::command]
fn take_navigation_request(pending: State<'_, PendingNavigation>) -> Option<String> {
    pending.take()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraySnapshot {
    status: String,
    can_disconnect: bool,
}

impl TraySnapshot {
    fn from_state(state: &AppState) -> Self {
        let status = if let Some(device) = &state.connected_device_name {
            format!("Status: Connected to {device}")
        } else {
            let label = match state.bluetooth {
                state::BluetoothState::Initializing => "Starting Bluetooth",
                state::BluetoothState::Advertising => "Ready to connect",
                state::BluetoothState::Connected => "Device connected",
                state::BluetoothState::PoweredOff => "Bluetooth is off",
                state::BluetoothState::Unauthorized => "Bluetooth permission required",
                #[cfg(target_os = "windows")]
                state::BluetoothState::Conflict => "Another Switchify PC is running",
                state::BluetoothState::Unsupported => "Bluetooth unavailable",
                state::BluetoothState::Error => "Bluetooth error",
            };
            format!("Status: {label}")
        };
        Self {
            status,
            can_disconnect: state.connected_device_name.is_some()
                || state.bluetooth == state::BluetoothState::Connected,
        }
    }
}

struct TrayController {
    status: MenuItem<tauri::Wry>,
    disconnect: MenuItem<tauri::Wry>,
}

impl TrayController {
    fn sync(&self, state: &AppState) {
        let snapshot = TraySnapshot::from_state(state);
        let _ = self.status.set_text(snapshot.status);
        let _ = self.disconnect.set_enabled(snapshot.can_disconnect);
    }
}

pub(crate) fn sync_tray_state(app: &AppHandle, state: &AppState) {
    if let Some(tray) = app.try_state::<TrayController>() {
        tray.sync(state);
    }
}

fn finish_app_exit(app: &AppHandle) {
    let model = app.state::<AppModel>();
    let _ = platform_disconnect_all(app, &model.shared);
    app.state::<overlay::CursorOverlay>().end_session();
    app.state::<modifier_overlay::ModifierOverlay>()
        .end_session();
    app.exit(0);
}

#[tauri::command]
fn complete_profile_exit(
    app: AppHandle,
    pending: State<'_, PendingProfileExit>,
) -> Result<(), String> {
    match pending
        .take()
        .ok_or_else(|| "No window action is pending.".to_string())?
    {
        ProfileExitAction::Hide => app
            .get_webview_window("main")
            .ok_or_else(|| "The main window is unavailable.".to_string())?
            .hide()
            .map_err(|error| error.to_string()),
        ProfileExitAction::Quit => {
            finish_app_exit(&app);
            Ok(())
        }
    }
}

#[tauri::command]
fn cancel_profile_exit(pending: State<'_, PendingProfileExit>) {
    pending.cancel();
}

async fn on_main_thread<T, F>(app: AppHandle, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(operation());
    })
    .map_err(|error| error.to_string())?;
    receiver
        .await
        .map_err(|_| "The main-thread operation was cancelled.".to_string())?
}

#[tauri::command]
async fn check_accessibility(
    app: AppHandle,
    model: State<'_, AppModel>,
    prompt: bool,
) -> Result<AppState, String> {
    let shared = model.shared.clone();
    let operation_app = app.clone();
    on_main_thread(app, move || {
        platform_check_accessibility(&operation_app, &shared, prompt)?;
        Ok(snapshot(&shared))
    })
    .await
}

#[tauri::command]
async fn approve_pairing(
    app: AppHandle,
    model: State<'_, AppModel>,
    request_id: String,
) -> Result<AppState, String> {
    let pending = model
        .snapshot()
        .pending_pairings
        .into_iter()
        .find(|pending| pending.request_id == request_id)
        .ok_or_else(|| "Pairing request is no longer pending.".to_string())?;
    let shared = model.shared.clone();
    let operation_app = app.clone();
    on_main_thread(app, move || {
        platform_approve_pairing(&operation_app, &shared, &request_id)
    })
    .await?;
    let token = {
        let data = model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        data.engine
            .token_for(&pending.device_id)
            .ok_or_else(|| "Pairing token was not created.".to_string())?
            .to_owned()
    };
    model
        .storage
        .save_pairing_token(&pending.device_id, &token)?;
    {
        let mut data = model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        data.state
            .paired_devices
            .retain(|device| device.device_id != pending.device_id);
        data.state.paired_devices.push(PairedDeviceView {
            device_id: pending.device_id,
            device_name: pending.device_name,
            paired_at: state::now_ms(),
            last_seen_at: None,
        });
    }
    model.persist()?;
    Ok(model.snapshot())
}

#[tauri::command]
async fn reject_pairing(
    app: AppHandle,
    model: State<'_, AppModel>,
    request_id: String,
) -> Result<AppState, String> {
    let shared = model.shared.clone();
    let operation_app = app.clone();
    on_main_thread(app, move || {
        platform_reject_pairing(&operation_app, &shared, &request_id)
    })
    .await?;
    Ok(model.snapshot())
}

#[tauri::command]
async fn disconnect_all(app: AppHandle) -> Result<AppState, String> {
    disconnect_all_on_main_thread(app).await
}

async fn disconnect_all_on_main_thread(app: AppHandle) -> Result<AppState, String> {
    let shared = app.state::<AppModel>().shared.clone();
    let operation_app = app.clone();
    on_main_thread(app.clone(), move || {
        platform_disconnect_all(&operation_app, &shared)
    })
    .await?;

    let model = app.state::<AppModel>();
    let overlay = app.state::<overlay::CursorOverlay>();
    let modifier_overlay = app.state::<modifier_overlay::ModifierOverlay>();
    Ok(finish_disconnect(&app, &model, &overlay, &modifier_overlay))
}

fn disconnect_all_inner(
    app: &AppHandle,
    model: &AppModel,
    overlay: &overlay::CursorOverlay,
    modifier_overlay: &modifier_overlay::ModifierOverlay,
) -> Result<AppState, String> {
    platform_disconnect_all(app, &model.shared)?;
    Ok(finish_disconnect(app, model, overlay, modifier_overlay))
}

fn finish_disconnect(
    app: &AppHandle,
    model: &AppModel,
    overlay: &overlay::CursorOverlay,
    modifier_overlay: &modifier_overlay::ModifierOverlay,
) -> AppState {
    overlay.end_session();
    modifier_overlay.end_session();
    {
        let mut data = model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        data.state.connected_device_name = None;
    }
    state::set_activity(
        &model.shared,
        ActivityKind::Info,
        "All devices disconnected.",
    );
    state::emit_state(app, &model.shared);
    model.snapshot()
}

#[tauri::command]
fn modifier_overlay_ready(
    window: tauri::WebviewWindow,
    overlay: State<'_, modifier_overlay::ModifierOverlay>,
) -> Result<modifier_overlay::ModifierOverlaySnapshot, String> {
    overlay.ready(window.label())
}

#[tauri::command]
fn modifier_overlay_present(
    window: tauri::WebviewWindow,
    overlay: State<'_, modifier_overlay::ModifierOverlay>,
    revision: u64,
) -> Result<(), String> {
    overlay.present(window.label(), revision)
}

#[tauri::command]
fn forget_device(model: State<'_, AppModel>, device_id: String) -> Result<AppState, String> {
    {
        let mut data = model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        data.engine.forget_device(&device_id);
        data.state
            .paired_devices
            .retain(|device| device.device_id != device_id);
    }
    model.storage.delete_pairing_token(&device_id)?;
    model.persist()?;
    Ok(model.snapshot())
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    model: State<'_, AppModel>,
    overlay: State<'_, overlay::CursorOverlay>,
    settings: AppSettings,
) -> Result<AppState, String> {
    let settings = settings.normalized()?;
    let (previous_start_with_system, previous_share_diagnostics, previous_consent) = {
        let state = &model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state;
        (
            state.settings.start_with_system,
            state.settings.share_diagnostics,
            state.telemetry.consent,
        )
    };
    if settings.start_with_system != previous_start_with_system {
        update_startup_registration(&app, settings.start_with_system)?;
    }
    let next_consent = if settings.share_diagnostics != previous_share_diagnostics {
        if settings.share_diagnostics {
            TelemetryConsent::Enabled
        } else {
            TelemetryConsent::Disabled
        }
    } else {
        previous_consent
    };
    if next_consent == TelemetryConsent::Disabled {
        model.set_telemetry_consent(next_consent);
    }
    model.persist_settings_with_telemetry(&settings, next_consent)?;
    if next_consent != TelemetryConsent::Disabled {
        model.set_telemetry_consent(next_consent);
    }
    model
        .shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .settings = settings.clone();
    if !settings.mouse_repeat_enabled {
        platform_stop_mouse_repeat(&app);
    }
    overlay.apply_settings(settings);
    state::set_activity(&model.shared, ActivityKind::Success, "Settings saved.");
    Ok(model.snapshot())
}

#[tauri::command]
fn set_telemetry_consent(model: State<'_, AppModel>, enabled: bool) -> Result<AppState, String> {
    model.apply_telemetry_choice(enabled)?;
    state::set_activity(
        &model.shared,
        ActivityKind::Success,
        if enabled {
            "Anonymous diagnostics enabled."
        } else {
            "Anonymous diagnostics disabled."
        },
    );
    Ok(model.snapshot())
}

#[tauri::command]
fn mark_setup_shown(model: State<'_, AppModel>) -> Result<AppState, String> {
    model.mark_setup_shown()
}

#[tauri::command]
fn complete_setup(
    app: AppHandle,
    model: State<'_, AppModel>,
    start_with_system: bool,
    share_diagnostics: bool,
) -> Result<AppState, String> {
    if model.snapshot().paired_devices.is_empty() {
        return Err("Pair an Android device before finishing setup.".into());
    }
    let mut settings = model.snapshot().settings;
    let previous_start = settings.start_with_system;
    if previous_start != start_with_system {
        update_startup_registration(&app, start_with_system)?;
    }
    settings.start_with_system = start_with_system;
    settings.share_diagnostics = share_diagnostics;
    let consent = if share_diagnostics {
        TelemetryConsent::Enabled
    } else {
        TelemetryConsent::Disabled
    };
    match model.apply_setup_completion(settings, consent) {
        Ok(state) => Ok(state),
        Err(error) => {
            if previous_start != start_with_system {
                if let Err(rollback_error) = update_startup_registration(&app, previous_start) {
                    return Err(format!(
                        "{error}; startup registration could not be restored: {rollback_error}"
                    ));
                }
            }
            Err(error)
        }
    }
}

#[cfg(target_os = "windows")]
fn update_startup_registration(_app: &AppHandle, enabled: bool) -> Result<(), String> {
    windows_startup::apply(enabled)
}

#[cfg(not(target_os = "windows"))]
fn update_startup_registration(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable()
    } else {
        autostart.disable()
    }
    .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn platform_stop_mouse_repeat(app: &AppHandle) {
    macos::stop_mouse_repeat(app);
}

#[cfg(target_os = "windows")]
fn platform_stop_mouse_repeat(app: &AppHandle) {
    windows_runtime::stop_mouse_repeat(app);
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_stop_mouse_repeat(_app: &AppHandle) {}

#[tauri::command]
fn list_switch_profiles(model: State<'_, AppModel>) -> Vec<SwitchProfile> {
    model
        .shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .profiles
        .clone()
}

fn validate_profile(profile: &SwitchProfile) -> Result<(), String> {
    if profile.built_in {
        return Err("Custom profiles cannot be marked as built in.".into());
    }
    if profile.provider != "mapped" || uuid::Uuid::parse_str(&profile.id).is_err() {
        return Err("Custom profile identity is invalid.".into());
    }
    if profile.name.trim().is_empty() {
        return Err("Profile name is required.".into());
    }
    if profile.name.chars().count() > 50 {
        return Err("Profile name must use 50 characters or fewer.".into());
    }
    if profile.bindings.len() != 8
        || !profile
            .bindings
            .iter()
            .enumerate()
            .all(|(index, binding)| binding.switch_id as usize == index + 1)
    {
        return Err("Custom profiles must define switches 1 through 8.".into());
    }
    for binding in &profile.bindings {
        if !valid_binding(binding) {
            return Err(format!("Switch {} binding is invalid.", binding.switch_id));
        }
    }
    for (index, binding) in profile.bindings.iter().enumerate() {
        if binding.binding_type == "none" {
            continue;
        }
        if let Some(duplicate) = profile.bindings[..index]
            .iter()
            .find(|candidate| bindings_equivalent(candidate, binding))
        {
            return Err(format!(
                "Switch {} duplicates Switch {}.",
                binding.switch_id, duplicate.switch_id
            ));
        }
    }
    Ok(())
}

fn bindings_equivalent(left: &state::SwitchBinding, right: &state::SwitchBinding) -> bool {
    if left.binding_type != right.binding_type {
        return false;
    }
    match left.binding_type.as_str() {
        "shortcut" => left.keys.as_ref().is_some_and(|left_keys| {
            right.keys.as_ref().is_some_and(|right_keys| {
                left_keys.len() == right_keys.len()
                    && left_keys.iter().all(|key| right_keys.contains(key))
            })
        }),
        "mouseClick" => {
            left.value == right.value
                && left.click_count.unwrap_or(1) == right.click_count.unwrap_or(1)
        }
        _ => left.value == right.value,
    }
}

fn valid_binding(binding: &state::SwitchBinding) -> bool {
    let value = binding.value.as_deref().unwrap_or("");
    match binding.binding_type.as_str() {
        "none" => true,
        "key" => valid_key(value),
        "mouseButton" => matches!(value, "left" | "right" | "middle"),
        "shortcut" => binding.keys.as_ref().is_some_and(|keys| {
            (1..=4).contains(&keys.len())
                && keys.iter().all(|key| valid_key(key))
                && keys
                    .iter()
                    .any(|key| !matches!(key.as_str(), "Ctrl" | "Alt" | "Shift" | "Meta"))
                && keys.iter().collect::<std::collections::HashSet<_>>().len() == keys.len()
        }),
        "mouseClick" => {
            matches!(value, "left" | "right" | "middle")
                && matches!(binding.click_count.unwrap_or(1), 1 | 2)
        }
        "scroll" => matches!(value, "up" | "down" | "left" | "right"),
        "media" => matches!(
            value,
            "playPause" | "nextTrack" | "previousTrack" | "volumeUp" | "volumeDown" | "mute"
        ),
        _ => false,
    }
}

fn valid_key(key: &str) -> bool {
    matches!(
        key,
        "Space"
            | "Enter"
            | "Escape"
            | "Tab"
            | "Backspace"
            | "Delete"
            | "ArrowUp"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
            | "Ctrl"
            | "Alt"
            | "Shift"
            | "Meta"
            | "F1"
            | "F2"
            | "F3"
            | "F4"
            | "F5"
            | "F6"
            | "F7"
            | "F8"
            | "F9"
            | "F10"
            | "F11"
            | "F12"
    ) || key.len() == 1
        && key
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

#[tauri::command]
fn save_switch_profile(
    model: State<'_, AppModel>,
    mut profile: SwitchProfile,
) -> Result<Vec<SwitchProfile>, String> {
    validate_profile(&profile)?;
    {
        let mut data = model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let replacing = data
            .profiles
            .iter()
            .any(|candidate| candidate.id == profile.id && !candidate.built_in);
        if !replacing
            && data
                .profiles
                .iter()
                .filter(|candidate| !candidate.built_in)
                .count()
                >= 32
        {
            return Err("No more than 32 custom profiles can be saved.".into());
        }
        if data.profiles.iter().any(|candidate| {
            candidate.id != profile.id && candidate.name.eq_ignore_ascii_case(profile.name.trim())
        }) {
            return Err("Profile names must be unique.".into());
        }
        profile.name = profile.name.trim().into();
        profile.version = data
            .profiles
            .iter()
            .find(|candidate| candidate.id == profile.id && !candidate.built_in)
            .map_or(1, |candidate| candidate.version.saturating_add(1));
        data.profiles.retain(|existing| existing.id != profile.id);
        data.profiles.push(profile);
    }
    model.persist()?;
    Ok(list_switch_profiles(model))
}

#[tauri::command]
fn delete_switch_profile(
    model: State<'_, AppModel>,
    profile_id: String,
) -> Result<Vec<SwitchProfile>, String> {
    {
        let mut data = model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if data
            .profiles
            .iter()
            .any(|profile| profile.id == profile_id && profile.built_in)
        {
            return Err("Built-in profiles cannot be deleted.".into());
        }
        data.profiles.retain(|profile| profile.id != profile_id);
    }
    model.persist()?;
    Ok(list_switch_profiles(model))
}

fn publish_update(app: &AppHandle, model: &AppModel, update: UpdateView) -> AppState {
    model
        .shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .updater = update;
    state::emit_state(app, &model.shared);
    model.snapshot()
}

fn update_failure(
    app: &AppHandle,
    model: &AppModel,
    version: Option<String>,
    retry: RetryAction,
    context: &str,
    error: &str,
) -> AppState {
    let message = format!("{context}: {error}");
    state::set_activity(&model.shared, ActivityKind::Error, &message);
    model.record_updater("failed", Some(error));
    publish_update(app, model, UpdateView::failed(version, message, retry))
}

async fn check_for_updates_inner(app: &AppHandle) -> AppState {
    let model = app.state::<AppModel>();
    let manager = app.state::<UpdateManager>();
    if !updater_is_configured(app.config().plugins.0.get("updater")) {
        state::set_activity(
            &model.shared,
            ActivityKind::Info,
            "Updates are not configured for this build.",
        );
        model.record_updater(
            "unavailable",
            Some("update endpoint or public key is missing"),
        );
        return publish_update(app, &model, UpdateView::unconfigured());
    }
    if manager.has_download() {
        return model.snapshot();
    }
    if !manager.begin(UpdateOperation::Check) {
        return model.snapshot();
    }
    publish_update(app, &model, UpdateView::checking());
    let result = match app.updater() {
        Ok(updater) => updater.check().await.map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    };
    manager.finish(UpdateOperation::Check);
    match result {
        Ok(Some(update)) => {
            let version = update.version().to_owned();
            manager.replace_available(Some(update));
            state::set_activity(
                &model.shared,
                ActivityKind::Info,
                format!("Switchify PC {version} is available."),
            );
            model.record_updater("available", None);
            publish_update(app, &model, UpdateView::available(version))
        }
        Ok(None) => {
            manager.replace_available(None);
            state::set_activity(
                &model.shared,
                ActivityKind::Success,
                "Switchify PC is up to date.",
            );
            model.record_updater("current", None);
            publish_update(app, &model, UpdateView::current())
        }
        Err(error) => update_failure(
            app,
            &model,
            None,
            RetryAction::Check,
            "Update check failed",
            &error,
        ),
    }
}

#[tauri::command]
async fn check_for_updates(app: AppHandle) -> Result<AppState, String> {
    Ok(check_for_updates_inner(&app).await)
}

#[tauri::command]
async fn download_update(app: AppHandle) -> Result<AppState, String> {
    let model = app.state::<AppModel>();
    let manager = app.state::<UpdateManager>();
    if !manager.begin(UpdateOperation::Download) {
        return Ok(model.snapshot());
    }
    let Some(update) = manager.available() else {
        manager.finish(UpdateOperation::Download);
        return Ok(update_failure(
            &app,
            &model,
            None,
            RetryAction::Check,
            "Update download could not start",
            "check for an update first",
        ));
    };
    let version = update.version().to_owned();
    let (cancel_sender, cancel_receiver) = tokio::sync::watch::channel(false);
    manager.set_download_cancel(cancel_sender);
    state::set_activity(
        &model.shared,
        ActivityKind::Info,
        format!("Downloading Switchify PC {version}…"),
    );
    publish_update(&app, &model, UpdateView::downloading(version.clone()));

    let progress_app = app.clone();
    let progress_shared = model.shared.clone();
    let result = download_with_cancel(&update, cancel_receiver, move |chunk, total| {
        {
            let mut data = progress_shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            data.state.updater.add_progress(chunk, total);
        }
        state::emit_state(&progress_app, &progress_shared);
    });
    let result = result.await;
    manager.finish(UpdateOperation::Download);
    match result {
        DownloadResult::Complete(Ok(bytes)) => {
            let downloaded_bytes = bytes.len() as u64;
            let total_bytes = model.snapshot().updater.total_bytes;
            manager.store_download(bytes);
            state::set_activity(
                &model.shared,
                ActivityKind::Success,
                format!("Switchify PC {version} is ready to install."),
            );
            model.record_updater("ready", None);
            Ok(publish_update(
                &app,
                &model,
                UpdateView::ready(version, downloaded_bytes, total_bytes),
            ))
        }
        DownloadResult::Complete(Err(error)) => Ok(update_failure(
            &app,
            &model,
            Some(version),
            RetryAction::Download,
            "Update download failed",
            &error,
        )),
        DownloadResult::Cancelled => {
            state::set_activity(
                &model.shared,
                ActivityKind::Info,
                "Update download cancelled.",
            );
            model.record_updater("cancelled", None);
            Ok(publish_update(&app, &model, UpdateView::cancelled(version)))
        }
    }
}

#[tauri::command]
fn cancel_update_download(
    app: AppHandle,
    model: State<'_, AppModel>,
    manager: State<'_, UpdateManager>,
) -> AppState {
    if manager.cancel_download() {
        state::set_activity(
            &model.shared,
            ActivityKind::Info,
            "Cancelling update download…",
        );
        state::emit_state(&app, &model.shared);
    }
    model.snapshot()
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<AppState, String> {
    let model = app.state::<AppModel>();
    let manager = app.state::<UpdateManager>();
    if !manager.begin(UpdateOperation::Install) {
        return Ok(model.snapshot());
    }
    let Some(update) = manager.available() else {
        manager.finish(UpdateOperation::Install);
        return Ok(update_failure(
            &app,
            &model,
            None,
            RetryAction::Check,
            "Update installation could not start",
            "check for an update first",
        ));
    };
    let Some(bytes) = manager.take_download() else {
        manager.finish(UpdateOperation::Install);
        return Ok(update_failure(
            &app,
            &model,
            Some(update.version().to_owned()),
            RetryAction::Download,
            "Update installation could not start",
            "download the update first",
        ));
    };
    let version = update.version().to_owned();
    let downloaded_bytes = bytes.len() as u64;
    let total_bytes = model.snapshot().updater.total_bytes;
    if let Err(error) = disconnect_all_on_main_thread(app.clone()).await {
        manager.store_download(bytes);
        manager.finish(UpdateOperation::Install);
        return Ok(update_failure(
            &app,
            &model,
            Some(version),
            RetryAction::Install,
            "Update installation failed",
            &error,
        ));
    }
    state::set_activity(
        &model.shared,
        ActivityKind::Info,
        format!("Installing Switchify PC {version}…"),
    );
    publish_update(
        &app,
        &model,
        UpdateView::applying(version.clone(), downloaded_bytes, total_bytes),
    );
    if let Err(error) = UpdateArtifact::install(&update, &bytes) {
        manager.store_download(bytes);
        manager.finish(UpdateOperation::Install);
        return Ok(update_failure(
            &app,
            &model,
            Some(version),
            RetryAction::Install,
            "Update installation failed",
            &error,
        ));
    }
    model.record_updater("installed", None);
    #[cfg(target_os = "macos")]
    {
        if let Err(error) = macos_relaunch::spawn_after_update(&app) {
            manager.finish(UpdateOperation::Install);
            return Ok(update_failure(
                &app,
                &model,
                Some(version),
                RetryAction::Check,
                "Update installed but restart failed",
                &error,
            ));
        }
        app.exit(0);
        Ok(model.snapshot())
    }
    #[cfg(not(target_os = "macos"))]
    app.restart();
}

#[cfg(test)]
fn record_update_failure(model: &AppModel, error: &str) -> String {
    state::set_activity(
        &model.shared,
        ActivityKind::Error,
        format!("Update check failed: {error}"),
    );
    model.record_updater("failed", Some(error));
    error.to_owned()
}

fn updater_is_configured(config: Option<&serde_json::Value>) -> bool {
    let has_endpoints = config
        .and_then(|config| config.get("endpoints"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|endpoints| !endpoints.is_empty());
    let has_public_key = config
        .and_then(|config| config.get("pubkey"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|key| !key.trim().is_empty());
    has_endpoints && has_public_key
}

fn start_update_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        loop {
            let _ = check_for_updates_inner(&app).await;
            tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
        }
    });
}

#[tauri::command]
fn export_diagnostics(model: State<'_, AppModel>) -> Result<AppState, String> {
    let events = model.diagnostics.events();
    let diagnostics = {
        let data = model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        serde_json::json!({
            "schemaVersion": 1,
            "generatedAt": state::now_ms(),
            "appVersion": data.state.version,
            "platform": data.state.capabilities.platform,
            "bluetooth": data.state.bluetooth,
            "accessibility": data.state.accessibility,
            "pairedDeviceCount": data.state.paired_devices.len(),
            "connected": data.state.connected_device_name.is_some(),
            "customProfileCount": data.profiles.iter().filter(|profile| !profile.built_in).count(),
            "capabilities": data.state.capabilities,
            "telemetry": data.state.telemetry,
            "diagnosticHistorySchemaVersion": 1,
            "events": events,
        })
    };
    let path = model.storage.export_diagnostics(&diagnostics)?;
    state::set_activity(
        &model.shared,
        ActivityKind::Success,
        format!("Diagnostics exported to {}.", path.display()),
    );
    Ok(model.snapshot())
}

fn install_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Switchify PC", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Open settings", true, None::<&str>)?;
    let profiles = MenuItem::with_id(
        app,
        "profiles",
        "Switch control profiles",
        true,
        None::<&str>,
    )?;
    let status = MenuItem::with_id(
        app,
        "status",
        "Status: Starting Bluetooth",
        false,
        None::<&str>,
    )?;
    let disconnect =
        MenuItem::with_id(app, "disconnect", "Disconnect devices", false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let first_separator = PredefinedMenuItem::separator(app)?;
    let second_separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &settings,
            &profiles,
            &first_separator,
            &status,
            &disconnect,
            &second_separator,
            &quit,
        ],
    )?;
    let controller = TrayController { status, disconnect };
    controller.sync(&app.state::<AppModel>().snapshot());
    app.manage(controller);
    let mut builder = TrayIconBuilder::with_id("switchify")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Switchify PC");
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                show_main_window(app, None);
            }
            "settings" => show_main_window(app, Some("settings")),
            "profiles" => show_main_window(app, Some("profiles")),
            "disconnect" => {
                let model = app.state::<AppModel>();
                let overlay = app.state::<overlay::CursorOverlay>();
                let modifier_overlay = app.state::<modifier_overlay::ModifierOverlay>();
                if let Err(error) = disconnect_all_inner(app, &model, &overlay, &modifier_overlay) {
                    state::set_activity(
                        &model.shared,
                        ActivityKind::Error,
                        format!("Disconnect failed: {error}"),
                    );
                    state::emit_state(app, &model.shared);
                }
            }
            "quit" => {
                request_profile_exit(app, ProfileExitAction::Quit);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                show_main_window(tray.app_handle(), None);
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    #[cfg(target_os = "macos")]
    if macos_relaunch::run_from_args() {
        return;
    }
    let model = AppModel::new();
    #[cfg(target_os = "macos")]
    if let Some(error) = macos_relaunch::take_failure() {
        state::set_activity(
            &model.shared,
            ActivityKind::Error,
            format!("The installed update could not restart automatically: {error}"),
        );
        model.record_updater("failed", Some(&error));
    }
    let shared = model.shared.clone();
    let overlay_shared = shared.clone();
    let modifier_overlay_shared = shared.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if has_start_hidden_argument(&args) {
                return;
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--start-hidden"])
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(model)
        .manage(UpdateManager::<tauri_plugin_updater::Update>::default())
        .manage(PendingProfileExit::default())
        .manage(PendingNavigation::default())
        .setup(move |app| {
            install_tray(app)?;
            if updater_is_configured(app.config().plugins.0.get("updater")) {
                let model = app.state::<AppModel>();
                publish_update(app.handle(), &model, UpdateView::idle());
                start_update_scheduler(app.handle().clone());
            }
            {
                let model = app.state::<AppModel>();
                let state = model.snapshot();
                model
                    .telemetry
                    .start(&state.version, &state.capabilities.platform);
            }
            app.manage(overlay::CursorOverlay::install(
                app.handle().clone(),
                overlay_shared.clone(),
            ));
            app.manage(
                modifier_overlay::ModifierOverlay::install(
                    app.handle().clone(),
                    modifier_overlay_shared.clone(),
                )
                .map_err(std::io::Error::other)?,
            );
            platform_install(app.handle().clone(), shared.clone())
                .map_err(std::io::Error::other)?;
            #[cfg(target_os = "windows")]
            {
                let start_with_system = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .state
                    .settings
                    .start_with_system;
                match windows_startup::repair(start_with_system) {
                    Ok(repaired_enabled) if repaired_enabled != start_with_system => {
                        let settings = {
                            let mut data = shared
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            data.state.settings.start_with_system = repaired_enabled;
                            data.state.settings.clone()
                        };
                        if let Err(error) = app.state::<AppModel>().persist_settings(&settings) {
                            state::set_activity(
                                &shared,
                                ActivityKind::Error,
                                format!("Startup preference could not be migrated: {error}"),
                            );
                        }
                    }
                    Err(error) => state::set_activity(
                        &shared,
                        ActivityKind::Error,
                        format!("Startup registration could not be repaired: {error}"),
                    ),
                    _ => {}
                }
            }
            if has_start_hidden_argument(&std::env::args().collect::<Vec<_>>()) {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(target_os = "macos")]
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Focused(true)) {
                let model = window.app_handle().state::<AppModel>();
                let requires_access =
                    model.snapshot().accessibility == state::AccessibilityState::Required;
                if requires_access {
                    let _ = macos::check_accessibility(window.app_handle(), &model.shared, false);
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                request_profile_exit(window.app_handle(), ProfileExitAction::Hide);
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            check_accessibility,
            approve_pairing,
            reject_pairing,
            disconnect_all,
            modifier_overlay_ready,
            modifier_overlay_present,
            forget_device,
            save_settings,
            set_telemetry_consent,
            mark_setup_shown,
            complete_setup,
            list_switch_profiles,
            save_switch_profile,
            delete_switch_profile,
            complete_profile_exit,
            cancel_profile_exit,
            take_navigation_request,
            check_for_updates,
            download_update,
            cancel_update_download,
            install_update,
            export_diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("error while running Switchify PC");
}

fn has_start_hidden_argument(args: &[String]) -> bool {
    args.iter().any(|argument| argument == "--start-hidden")
}

#[cfg(target_os = "macos")]
fn platform_install(app: AppHandle, shared: state::SharedModel) -> Result<(), String> {
    macos::install(app, shared)
}
#[cfg(target_os = "macos")]
fn platform_check_accessibility(
    app: &AppHandle,
    shared: &state::SharedModel,
    prompt: bool,
) -> Result<(), String> {
    macos::check_accessibility(app, shared, prompt)
}
#[cfg(target_os = "macos")]
fn platform_approve_pairing(
    app: &AppHandle,
    shared: &state::SharedModel,
    request_id: &str,
) -> Result<(), String> {
    macos::approve_pairing(app, shared, request_id)
}
#[cfg(target_os = "macos")]
fn platform_reject_pairing(
    app: &AppHandle,
    shared: &state::SharedModel,
    request_id: &str,
) -> Result<(), String> {
    macos::reject_pairing(app, shared, request_id)
}
#[cfg(target_os = "macos")]
fn platform_disconnect_all(app: &AppHandle, shared: &state::SharedModel) -> Result<(), String> {
    macos::disconnect_all(app, shared)
}

#[cfg(target_os = "windows")]
fn platform_install(app: AppHandle, shared: state::SharedModel) -> Result<(), String> {
    windows_runtime::install(app, shared)
}
#[cfg(target_os = "windows")]
fn platform_check_accessibility(
    app: &AppHandle,
    shared: &state::SharedModel,
    prompt: bool,
) -> Result<(), String> {
    windows_runtime::check_accessibility(app, shared, prompt)
}
#[cfg(target_os = "windows")]
fn platform_approve_pairing(
    app: &AppHandle,
    shared: &state::SharedModel,
    request_id: &str,
) -> Result<(), String> {
    windows_runtime::approve_pairing(app, shared, request_id)
}
#[cfg(target_os = "windows")]
fn platform_reject_pairing(
    app: &AppHandle,
    shared: &state::SharedModel,
    request_id: &str,
) -> Result<(), String> {
    windows_runtime::reject_pairing(app, shared, request_id)
}
#[cfg(target_os = "windows")]
fn platform_disconnect_all(app: &AppHandle, shared: &state::SharedModel) -> Result<(), String> {
    windows_runtime::disconnect_all(app, shared)
}

#[cfg(test)]
mod tests {
    use super::{
        has_start_hidden_argument, record_update_failure, updater_is_configured, validate_profile,
        PendingNavigation, PendingProfileExit, ProfileExitAction, TraySnapshot,
        NAVIGATE_REQUESTED_EVENT,
    };
    use crate::state::{AppModel, BluetoothState, SwitchBinding, SwitchProfile};
    use crate::storage::AppStorage;
    use serde_json::json;

    fn custom_profile() -> SwitchProfile {
        SwitchProfile {
            id: "3a393675-6434-4e50-a62f-d85ac24bcdf5".into(),
            version: 1,
            name: "Accessible controls".into(),
            provider: "mapped".into(),
            built_in: false,
            bindings: (1..=8)
                .map(|switch_id| SwitchBinding {
                    switch_id,
                    binding_type: "none".into(),
                    value: None,
                    keys: None,
                    click_count: None,
                })
                .collect(),
        }
    }

    #[test]
    fn update_checks_require_a_configured_endpoint() {
        assert!(!updater_is_configured(None));
        assert!(!updater_is_configured(Some(&json!({
            "endpoints": [],
            "pubkey": ""
        }))));
        assert!(!updater_is_configured(Some(&json!({
            "endpoints": ["https://updates.example.com/latest.json"],
            "pubkey": ""
        }))));
        assert!(updater_is_configured(Some(&json!({
            "endpoints": ["https://updates.example.com/latest.json"],
            "pubkey": "test-key"
        }))));
    }

    #[test]
    fn update_failures_are_sanitized_and_persisted() {
        let root = std::env::temp_dir().join(format!("switchify-update-{}", uuid::Uuid::new_v4()));
        let model = AppModel::with_storage_for_test(AppStorage::at(root.join("state.json")));
        let error = "feed failed /Users/person/private token=secret";

        assert_eq!(record_update_failure(&model, error), error);
        let event = model
            .diagnostics
            .events()
            .into_iter()
            .rev()
            .find(|event| event.category == "updater")
            .unwrap();
        assert_eq!(event.status, "failed");
        assert_eq!(
            event.detail.as_deref(),
            Some("feed failed [redacted] [redacted]")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn hidden_start_requires_the_exact_argument() {
        assert!(has_start_hidden_argument(&[
            "switchify-pc.exe".into(),
            "--start-hidden".into()
        ]));
        assert!(!has_start_hidden_argument(&[
            "switchify-pc.exe".into(),
            "--start-hidden-now".into()
        ]));
    }

    #[test]
    fn profile_validation_identifies_invalid_and_duplicate_switches() {
        let mut profile = custom_profile();
        profile.bindings[0].binding_type = "shortcut".into();
        profile.bindings[0].keys = Some(vec!["Ctrl".into()]);
        assert_eq!(
            validate_profile(&profile),
            Err("Switch 1 binding is invalid.".into())
        );

        profile.bindings[0].keys = Some(vec!["Ctrl".into(), "K".into()]);
        profile.bindings[1].binding_type = "shortcut".into();
        profile.bindings[1].keys = Some(vec!["K".into(), "Ctrl".into()]);
        assert_eq!(
            validate_profile(&profile),
            Err("Switch 2 duplicates Switch 1.".into())
        );
    }

    #[test]
    fn profile_validation_identifies_the_name_field() {
        let mut profile = custom_profile();
        profile.name = "  ".into();
        assert_eq!(
            validate_profile(&profile),
            Err("Profile name is required.".into())
        );
    }

    #[test]
    fn profile_exit_requests_can_be_completed_or_cancelled() {
        let pending = PendingProfileExit::default();
        assert!(pending.begin(ProfileExitAction::Hide));
        assert!(!pending.begin(ProfileExitAction::Quit));
        assert_eq!(pending.take(), Some(ProfileExitAction::Hide));
        assert_eq!(pending.take(), None);

        assert!(pending.begin(ProfileExitAction::Quit));
        pending.cancel();
        assert_eq!(pending.take(), None);
    }

    #[test]
    fn tray_snapshot_tracks_connection_status_and_disconnect_availability() {
        let root = std::env::temp_dir().join(format!("switchify-tray-{}", uuid::Uuid::new_v4()));
        let model = AppModel::with_storage_for_test(AppStorage::at(root.join("state.json")));
        assert_eq!(
            TraySnapshot::from_state(&model.snapshot()),
            TraySnapshot {
                status: "Status: Starting Bluetooth".into(),
                can_disconnect: false,
            }
        );

        {
            let mut data = model.shared.lock().unwrap();
            data.state.bluetooth = BluetoothState::Advertising;
        }
        assert_eq!(
            TraySnapshot::from_state(&model.snapshot()),
            TraySnapshot {
                status: "Status: Ready to connect".into(),
                can_disconnect: false,
            }
        );

        {
            let mut data = model.shared.lock().unwrap();
            data.state.bluetooth = BluetoothState::Connected;
            data.state.connected_device_name = Some("Pixel".into());
        }
        assert_eq!(
            TraySnapshot::from_state(&model.snapshot()),
            TraySnapshot {
                status: "Status: Connected to Pixel".into(),
                can_disconnect: true,
            }
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn tray_navigation_uses_the_internal_navigation_event() {
        assert_eq!(NAVIGATE_REQUESTED_EVENT, "navigate-requested");
        let pending = PendingNavigation::default();
        pending.set("settings");
        pending.set("profiles");
        assert_eq!(pending.take().as_deref(), Some("profiles"));
        assert_eq!(pending.take(), None);
    }
}
