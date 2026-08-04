mod input;
#[cfg(target_os = "macos")]
mod macos;
mod protocol;
mod state;
mod storage;
#[cfg(target_os = "windows")]
mod windows_runtime;

use state::{
    snapshot, ActivityKind, AppModel, AppSettings, AppState, PairedDeviceView, SwitchProfile,
};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, State};
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
        .pending_pairing
        .ok_or_else(|| "Pairing request is no longer pending.".to_string())?;
    let shared = model.shared.clone();
    let operation_app = app.clone();
    on_main_thread(app, move || {
        platform_approve_pairing(&operation_app, &shared, &request_id)
    })
    .await?;
    {
        let mut data = model
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let token = data
            .engine
            .token_for(&pending.device_id)
            .ok_or_else(|| "Pairing token was not created.".to_string())?
            .to_owned();
        model
            .storage
            .save_pairing_token(&pending.device_id, &token)?;
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
fn disconnect_all(app: AppHandle, model: State<'_, AppModel>) -> Result<AppState, String> {
    platform_disconnect_all(&app, &model.shared)?;
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
    settings: AppSettings,
) -> Result<AppState, String> {
    let settings = settings.normalized()?;
    let autostart = app.autolaunch();
    if settings.start_with_system {
        autostart.enable()
    } else {
        autostart.disable()
    }
    .map_err(|error| error.to_string())?;
    model
        .shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .settings = settings;
    model.persist()?;
    state::set_activity(&model.shared, ActivityKind::Success, "Settings saved.");
    Ok(model.snapshot())
}

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
    if profile.built_in
        || profile.provider != "mapped"
        || profile.name.trim().is_empty()
        || profile.name.chars().count() > 50
    {
        return Err("Custom profile metadata is invalid.".into());
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
    Ok(())
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
    let update = app
        .updater()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;
    let message = update.map_or_else(
        || "Switchify PC Preview is up to date.".to_string(),
        |update| format!("Switchify PC Preview {} is available.", update.version),
    );
    state::set_activity(&model.shared, ActivityKind::Info, message);
    Ok(model.snapshot())
}

fn install_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Switchify PC Preview", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut builder = TrayIconBuilder::with_id("switchify-preview")
        .menu(&menu)
        .tooltip("Switchify PC Preview");
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
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    let model = AppModel::new();
    let shared = model.shared.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
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
            platform_install(app.handle().clone(), shared.clone())
                .map_err(std::io::Error::other)?;
            Ok(())
        })
        .on_window_event(|window, event| {
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
            forget_device,
            save_settings,
            list_switch_profiles,
            save_switch_profile,
            delete_switch_profile,
            check_for_updates
        ])
        .run(tauri::generate_context!())
        .expect("error while running Switchify PC Preview");
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
