#[cfg(target_os = "windows")]
mod grid3;
mod input;
#[cfg(target_os = "macos")]
mod macos;
mod modifier_overlay;
mod mouse_repeat;
mod overlay;
mod protocol;
mod state;
mod storage;
#[cfg(target_os = "windows")]
mod windows_runtime;
#[cfg(target_os = "windows")]
mod windows_security;
#[cfg(target_os = "windows")]
mod windows_startup;

use state::{
    snapshot, ActivityKind, AppModel, AppSettings, AppState, PairedDeviceView, SwitchProfile,
};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, State};
#[cfg(not(target_os = "windows"))]
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_updater::UpdaterExt;

#[tauri::command]
fn get_app_state(model: State<'_, AppModel>) -> AppState {
    model.snapshot()
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
fn disconnect_all(
    app: AppHandle,
    model: State<'_, AppModel>,
    overlay: State<'_, overlay::CursorOverlay>,
    modifier_overlay: State<'_, modifier_overlay::ModifierOverlay>,
) -> Result<AppState, String> {
    platform_disconnect_all(&app, &model.shared)?;
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
    Ok(model.snapshot())
}

#[tauri::command]
fn modifier_overlay_ready(
    window: tauri::WebviewWindow,
    overlay: State<'_, modifier_overlay::ModifierOverlay>,
) -> Result<modifier_overlay::ModifierOverlaySnapshot, String> {
    overlay.ready(window.label())
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
    let previous_start_with_system = model
        .shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .settings
        .start_with_system;
    if settings.start_with_system != previous_start_with_system {
        update_startup_registration(&app, settings.start_with_system)?;
    }
    model.persist_settings(&settings)?;
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

#[tauri::command]
async fn check_for_updates(app: AppHandle, model: State<'_, AppModel>) -> Result<AppState, String> {
    if !updater_has_endpoints(app.config().plugins.0.get("updater")) {
        state::set_activity(
            &model.shared,
            ActivityKind::Info,
            "Updates are not configured for this build.",
        );
        return Ok(model.snapshot());
    }
    let update = app
        .updater()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;
    let message = update.map_or_else(
        || "Switchify PC is up to date.".to_string(),
        |update| format!("Switchify PC {} is available.", update.version),
    );
    state::set_activity(&model.shared, ActivityKind::Info, message);
    Ok(model.snapshot())
}

fn updater_has_endpoints(config: Option<&serde_json::Value>) -> bool {
    config
        .and_then(|config| config.get("endpoints"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|endpoints| !endpoints.is_empty())
}

#[tauri::command]
fn export_diagnostics(model: State<'_, AppModel>) -> Result<AppState, String> {
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
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut builder = TrayIconBuilder::with_id("switchify")
        .menu(&menu)
        .tooltip("Switchify PC");
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                let model = app.state::<AppModel>();
                let _ = platform_disconnect_all(app, &model.shared);
                app.state::<overlay::CursorOverlay>().end_session();
                app.state::<modifier_overlay::ModifierOverlay>()
                    .end_session();
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    let model = AppModel::new();
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
        .setup(move |app| {
            install_tray(app)?;
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
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            check_accessibility,
            approve_pairing,
            reject_pairing,
            disconnect_all,
            modifier_overlay_ready,
            forget_device,
            save_settings,
            list_switch_profiles,
            save_switch_profile,
            delete_switch_profile,
            check_for_updates,
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
    use super::{has_start_hidden_argument, updater_has_endpoints, validate_profile};
    use crate::state::{SwitchBinding, SwitchProfile};
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
        assert!(!updater_has_endpoints(None));
        assert!(!updater_has_endpoints(Some(&json!({
            "endpoints": [],
            "pubkey": ""
        }))));
        assert!(updater_has_endpoints(Some(&json!({
            "endpoints": ["https://updates.example.com/latest.json"],
            "pubkey": "test-key"
        }))));
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
}
