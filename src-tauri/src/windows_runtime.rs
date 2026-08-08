use std::process::Command;
use std::sync::{Mutex, OnceLock};

use enigo::{Enigo, Settings};
use tauri::{AppHandle, Manager};
use windows::core::{IInspectable, Ref, GUID, HSTRING};
use windows::Devices::Bluetooth::BluetoothError;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristicProperties, GattLocalCharacteristic, GattLocalCharacteristicParameters,
    GattProtectionLevel, GattReadRequestedEventArgs, GattServiceProvider,
    GattServiceProviderAdvertisementStatus, GattServiceProviderAdvertisementStatusChangedEventArgs,
    GattServiceProviderAdvertisingParameters, GattWriteOption, GattWriteRequestedEventArgs,
};
use windows::Foundation::{Deferral, TypedEventHandler};
use windows::Security::Cryptography::CryptographicBuffer;

use crate::input::{DesktopInput, PointerFeedback};
use crate::modifier_overlay::ModifierOverlay;
use crate::mouse_repeat::{
    acceleration_scale, MouseRepeatController, RepeatCommand, INITIAL_SCALE,
};
use crate::overlay::CursorOverlay;
use crate::protocol::{
    bluetooth_status_payload, create_notification_frames, pointer_profile_response,
    switch_profile_catalog_response, DesktopCommand, EngineEvent, MouseClickCommand,
    MouseMoveCommand, PointerProfile, TextCommand,
};
use crate::state::{
    emit_state, set_activity, AccessibilityState, ActivityKind, BluetoothState, SharedModel,
};

const SERVICE_UUID: GUID = GUID::from_u128(0x7a78f7e8_1d6d_4d92_9ef0_1f89d3db21f4);
const RX_UUID: GUID = GUID::from_u128(0x7a78f7e9_1d6d_4d92_9ef0_1f89d3db21f4);
const TX_UUID: GUID = GUID::from_u128(0x7a78f7ea_1d6d_4d92_9ef0_1f89d3db21f4);
const STATUS_UUID: GUID = GUID::from_u128(0x7a78f7eb_1d6d_4d92_9ef0_1f89d3db21f4);
const NOTIFICATION_BYTES: usize = 160;

struct WindowsRuntime {
    _provider: GattServiceProvider,
    _rx: GattLocalCharacteristic,
    tx: GattLocalCharacteristic,
    _status: GattLocalCharacteristic,
    input: DesktopInput<Enigo>,
    repeats: MouseRepeatController,
}

static RUNTIME: OnceLock<Mutex<Option<WindowsRuntime>>> = OnceLock::new();
fn runtime() -> &'static Mutex<Option<WindowsRuntime>> {
    RUNTIME.get_or_init(|| Mutex::new(None))
}

fn tasklist_has_other_switchify_process(output: &str, current_pid: u32) -> bool {
    output.lines().any(|line| {
        let mut fields = line.split(',');
        let image_name = fields.next().map(|field| field.trim_matches('"'));
        let process_id = fields
            .next()
            .and_then(|field| field.trim_matches('"').parse::<u32>().ok());
        image_name.is_some_and(|name| name.eq_ignore_ascii_case("Switchify PC.exe"))
            && process_id.is_some_and(|pid| pid != current_pid)
    })
}

fn another_switchify_app_is_running() -> bool {
    Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq Switchify PC.exe", "/FO", "CSV", "/NH"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|output| tasklist_has_other_switchify_process(&output, std::process::id()))
}

pub fn install(app: AppHandle, shared: SharedModel) -> Result<(), String> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .accessibility = AccessibilityState::Granted;
    if another_switchify_app_is_running() {
        shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .bluetooth = BluetoothState::Conflict;
        set_activity(
            &shared,
            ActivityKind::Info,
            "Close the other Switchify PC app to use Bluetooth.",
        );
        emit_state(&app, &shared);
        return Ok(());
    }
    tauri::async_runtime::spawn(async move {
        if let Err(error) = start_gatt(app.clone(), shared.clone()).await {
            shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .state
                .bluetooth = BluetoothState::Error;
            set_activity(
                &shared,
                ActivityKind::Error,
                format!("Bluetooth could not start: {error}"),
            );
            emit_state(&app, &shared);
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::tasklist_has_other_switchify_process;

    #[test]
    fn conflict_check_ignores_the_current_process() {
        let output = r#""Switchify PC.exe","4242","Console","1","20,000 K""#;
        assert!(!tasklist_has_other_switchify_process(output, 4242));
    }

    #[test]
    fn conflict_check_detects_another_switchify_process() {
        let output = concat!(
            r#""Switchify PC.exe","4242","Console","1","20,000 K""#,
            "\r\n",
            r#""SWITCHIFY PC.EXE","7331","Console","1","18,000 K""#,
        );
        assert!(tasklist_has_other_switchify_process(output, 4242));
    }

    #[test]
    fn conflict_check_ignores_unrelated_and_malformed_rows() {
        let output = concat!(
            r#""Other App.exe","7331","Console","1","18,000 K""#,
            "\r\n",
            r#""Switchify PC.exe","not-a-pid","Console","1","20,000 K""#,
        );
        assert!(!tasklist_has_other_switchify_process(output, 4242));
    }
}

async fn create_characteristic(
    provider: &GattServiceProvider,
    uuid: GUID,
    properties: GattCharacteristicProperties,
    read: GattProtectionLevel,
    write: GattProtectionLevel,
    description: &str,
) -> Result<GattLocalCharacteristic, String> {
    let parameters = GattLocalCharacteristicParameters::new().map_err(|error| error.to_string())?;
    parameters
        .SetCharacteristicProperties(properties)
        .map_err(|error| error.to_string())?;
    parameters
        .SetReadProtectionLevel(read)
        .map_err(|error| error.to_string())?;
    parameters
        .SetWriteProtectionLevel(write)
        .map_err(|error| error.to_string())?;
    parameters
        .SetUserDescription(&HSTRING::from(description))
        .map_err(|error| error.to_string())?;
    let result = provider
        .Service()
        .map_err(|error| error.to_string())?
        .CreateCharacteristicAsync(uuid, &parameters)
        .map_err(|error| error.to_string())?
        .await
        .map_err(|error| error.to_string())?;
    if result.Error().map_err(|error| error.to_string())? != BluetoothError::Success {
        return Err(format!("Could not create {description}."));
    }
    result.Characteristic().map_err(|error| error.to_string())
}

async fn start_gatt(app: AppHandle, shared: SharedModel) -> Result<(), String> {
    let result = GattServiceProvider::CreateAsync(SERVICE_UUID)
        .map_err(|error| error.to_string())?
        .await
        .map_err(|error| error.to_string())?;
    if result.Error().map_err(|error| error.to_string())? != BluetoothError::Success {
        return Err("The Bluetooth adapter does not support the peripheral role.".into());
    }
    let provider = result
        .ServiceProvider()
        .map_err(|error| error.to_string())?;
    let rx = create_characteristic(
        &provider,
        RX_UUID,
        GattCharacteristicProperties::Write | GattCharacteristicProperties::WriteWithoutResponse,
        GattProtectionLevel::Plain,
        GattProtectionLevel::Plain,
        "Switchify RX",
    )
    .await?;
    let tx = create_characteristic(
        &provider,
        TX_UUID,
        GattCharacteristicProperties::Notify,
        GattProtectionLevel::Plain,
        GattProtectionLevel::Plain,
        "Switchify TX",
    )
    .await?;
    let status = create_characteristic(
        &provider,
        STATUS_UUID,
        GattCharacteristicProperties::Read,
        GattProtectionLevel::Plain,
        GattProtectionLevel::Plain,
        "Switchify status",
    )
    .await?;

    let write_app = app.clone();
    let write_shared = shared.clone();
    rx.WriteRequested(&TypedEventHandler::new(
        move |_: Ref<'_, GattLocalCharacteristic>, args: Ref<'_, GattWriteRequestedEventArgs>| {
            if let Some(args) = args.cloned() {
                let deferral = args.GetDeferral()?;
                let callback_app = write_app.clone();
                let callback_shared = write_shared.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = handle_write(
                        callback_app.clone(),
                        callback_shared.clone(),
                        args,
                        deferral,
                    )
                    .await
                    {
                        set_activity(
                            &callback_shared,
                            ActivityKind::Error,
                            format!("Bluetooth write failed: {error}"),
                        );
                        emit_state(&callback_app, &callback_shared);
                    }
                });
            }
            Ok(())
        },
    ))
    .map_err(|error| error.to_string())?;

    let subscribe_app = app.clone();
    let subscribe_shared = shared.clone();
    tx.SubscribedClientsChanged(
        &TypedEventHandler::<GattLocalCharacteristic, IInspectable>::new(move |sender, _| {
            let connected = sender
                .as_ref()
                .and_then(|value| value.SubscribedClients().ok())
                .and_then(|clients| clients.Size().ok())
                .is_some_and(|size| size > 0);
            {
                let mut model = subscribe_shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                model.state.bluetooth = if connected {
                    BluetoothState::Connected
                } else {
                    BluetoothState::Advertising
                };
                model.state.connected_device_name = connected.then(|| "Bluetooth device".into());
            }
            set_activity(
                &subscribe_shared,
                ActivityKind::Info,
                if connected {
                    "Android device connected."
                } else {
                    "Android device disconnected."
                },
            );
            if !connected {
                stop_all_repeats(&subscribe_app);
                release_input_session();
                subscribe_app.state::<CursorOverlay>().end_session();
                subscribe_app.state::<ModifierOverlay>().end_session();
            }
            emit_state(&subscribe_app, &subscribe_shared);
            Ok(())
        }),
    )
    .map_err(|error| error.to_string())?;

    let read_shared = shared.clone();
    status
        .ReadRequested(&TypedEventHandler::new(
            move |_: Ref<'_, GattLocalCharacteristic>,
                  args: Ref<'_, GattReadRequestedEventArgs>| {
                if let Some(args) = args.cloned() {
                    let deferral = args.GetDeferral()?;
                    let status_shared = read_shared.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = handle_status_read(status_shared, args, deferral).await
                        {
                            eprintln!("Switchify BLE status read failed: {error}");
                        }
                    });
                }
                Ok(())
            },
        ))
        .map_err(|error| error.to_string())?;

    let status_app = app.clone();
    let status_shared = shared.clone();
    provider
        .AdvertisementStatusChanged(&TypedEventHandler::<
            GattServiceProvider,
            GattServiceProviderAdvertisementStatusChangedEventArgs,
        >::new(move |_, args| {
            if let Some(args) = args.cloned() {
                let status = args.Status()?;
                let error = args.Error()?;
                update_advertisement_status(&status_app, &status_shared, status, error);
            }
            Ok(())
        }))
        .map_err(|error| error.to_string())?;

    let input = Enigo::new(&Settings::default())
        .map_err(|_| "Windows input injection could not initialize.".to_string())?;
    *runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(WindowsRuntime {
        _provider: provider.clone(),
        _rx: rx,
        tx,
        _status: status,
        input: DesktopInput::with_modifier_overlay(
            input,
            app.state::<ModifierOverlay>().notifier(),
        ),
        repeats: MouseRepeatController::default(),
    });
    let advertising =
        GattServiceProviderAdvertisingParameters::new().map_err(|error| error.to_string())?;
    advertising
        .SetIsConnectable(true)
        .map_err(|error| error.to_string())?;
    advertising
        .SetIsDiscoverable(true)
        .map_err(|error| error.to_string())?;
    provider
        .StartAdvertisingWithParameters(&advertising)
        .map_err(|error| error.to_string())?;
    let status = provider
        .AdvertisementStatus()
        .map_err(|error| error.to_string())?;
    update_advertisement_status(&app, &shared, status, BluetoothError::Success);
    Ok(())
}

fn update_advertisement_status(
    app: &AppHandle,
    shared: &SharedModel,
    status: GattServiceProviderAdvertisementStatus,
    error: BluetoothError,
) {
    eprintln!("Switchify BLE advertisement status: {status:?}, error: {error:?}");
    let (bluetooth, kind, message) = if status == GattServiceProviderAdvertisementStatus::Started {
        (
            BluetoothState::Advertising,
            ActivityKind::Info,
            "Advertising to nearby Switchify Android devices.".to_string(),
        )
    } else if status == GattServiceProviderAdvertisementStatus::StartedWithoutAllAdvertisementData {
        (
            BluetoothState::Error,
            ActivityKind::Error,
            "Bluetooth started without the Switchify service identifier. Restart Bluetooth and try again."
                .to_string(),
        )
    } else if status == GattServiceProviderAdvertisementStatus::Aborted {
        (
            BluetoothState::Error,
            ActivityKind::Error,
            format!("Bluetooth advertising stopped: {error:?}"),
        )
    } else {
        (
            BluetoothState::Initializing,
            ActivityKind::Info,
            "Starting Bluetooth advertising...".to_string(),
        )
    };
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .bluetooth = bluetooth;
    set_activity(shared, kind, message);
    emit_state(app, shared);
}

async fn handle_write(
    app: AppHandle,
    shared: SharedModel,
    args: GattWriteRequestedEventArgs,
    deferral: Deferral,
) -> Result<(), String> {
    let result = async {
        let request = args
            .GetRequestAsync()
            .map_err(|error| error.to_string())?
            .await
            .map_err(|error| error.to_string())?;
        let mut bytes = windows::core::Array::<u8>::new();
        CryptographicBuffer::CopyToByteArray(
            &request.Value().map_err(|error| error.to_string())?,
            &mut bytes,
        )
        .map_err(|error| error.to_string())?;
        if request.Option().map_err(|error| error.to_string())?
            == GattWriteOption::WriteWithResponse
        {
            request.Respond().map_err(|error| error.to_string())?;
        }
        if let Some(response) = process_frame(&app, &shared, &bytes)? {
            notify(response)?;
        }
        Ok(())
    }
    .await;
    deferral.Complete().map_err(|error| error.to_string())?;
    result
}

async fn handle_status_read(
    shared: SharedModel,
    args: GattReadRequestedEventArgs,
    deferral: Deferral,
) -> Result<(), String> {
    eprintln!("Switchify BLE status read requested.");
    let result = async {
        let request = args
            .GetRequestAsync()
            .map_err(|error| error.to_string())?
            .await
            .map_err(|error| error.to_string())?;
        let model = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let payload = bluetooth_status_payload(
            "Switchify PC",
            &model.state.desktop_id,
            &model.state.capabilities.platform,
        )?;
        drop(model);
        let buffer = CryptographicBuffer::CreateFromByteArray(&payload)
            .map_err(|error| error.to_string())?;
        request
            .RespondWithValue(&buffer)
            .map_err(|error| error.to_string())?;
        eprintln!("Switchify BLE status read completed.");
        Ok(())
    }
    .await;
    deferral.Complete().map_err(|error| error.to_string())?;
    result
}

fn process_frame(
    app: &AppHandle,
    shared: &SharedModel,
    bytes: &[u8],
) -> Result<Option<String>, String> {
    let event = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .receive_frame(bytes, crate::state::now_ms())?;
    let response = match event {
        None => None,
        Some(EngineEvent::PendingPairing(pending)) => {
            shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .state
                .pending_pairing = Some(pending);
            set_activity(
                shared,
                ActivityKind::Info,
                "Review the pairing code before approving this device.",
            );
            None
        }
        Some(EngineEvent::Response(response)) => Some(response),
        Some(EngineEvent::PointerProfile(id)) => {
            let settings = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .state
                .settings
                .clone();
            Some(pointer_profile_response(
                &id,
                &default_pointer_profile(),
                &settings,
            ))
        }
        Some(EngineEvent::MouseMove(command)) => complete_mouse_move(app, shared, command),
        Some(EngineEvent::MouseClick(command)) => complete_mouse_click(app, shared, command),
        Some(EngineEvent::Text(command)) => complete_text(app, shared, command),
        Some(EngineEvent::Desktop(command)) => complete_desktop(app, shared, command),
    };
    emit_state(app, shared);
    Ok(response)
}

fn with_runtime_input<T>(
    operation: impl FnOnce(&mut DesktopInput<Enigo>) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime = guard
        .as_mut()
        .ok_or_else(|| "Bluetooth runtime is not ready.".to_string())?;
    operation(&mut runtime.input)
}

fn complete_mouse_move(
    app: &AppHandle,
    shared: &SharedModel,
    command: MouseMoveCommand,
) -> Option<String> {
    stop_all_repeats(app);
    let scale = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .settings
        .pointer_scale_percent;
    let result = with_runtime_input(|input| {
        input.set_pointer_scale_percent(scale);
        input.move_pointer(command.dx.round() as i32, command.dy.round() as i32)?;
        Ok(input.pointer_feedback_for_move())
    });
    if let Ok(feedback) = &result {
        show_overlay(app, shared, *feedback);
    }
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .complete_mouse_move_command(
            &command,
            result.as_ref().map(|_| ()).map_err(String::as_str),
        )
}
fn complete_mouse_click(
    app: &AppHandle,
    shared: &SharedModel,
    command: MouseClickCommand,
) -> Option<String> {
    stop_all_repeats(app);
    let result =
        with_runtime_input(|input| input.click_pointer(command.button, command.click_count));
    if result.is_ok() {
        show_overlay(
            app,
            shared,
            PointerFeedback::Click {
                button: command.button,
                count: command.click_count,
            },
        );
    }
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .complete_mouse_click_command(
            &command,
            result.as_ref().map(|_| ()).map_err(String::as_str),
        )
}
fn complete_text(app: &AppHandle, shared: &SharedModel, command: TextCommand) -> Option<String> {
    stop_all_repeats(app);
    let result = with_runtime_input(|input| input.type_text(&command.text));
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .complete_text_command(
            &command,
            result.as_ref().map(|_| ()).map_err(String::as_str),
        )
}
fn complete_desktop(
    app: &AppHandle,
    shared: &SharedModel,
    command: DesktopCommand,
) -> Option<String> {
    if command.command_type == "mouse.repeat.start" {
        return complete_repeat_start(app, shared, command);
    }
    if command.command_type == "mouse.repeat.stop" {
        return complete_repeat_stop(app, shared, command);
    }
    stop_all_repeats(app);
    let profiles = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .profiles
        .clone();
    if command.command_type == "switch.profile.list" {
        return Some(switch_profile_catalog_response(&command.id, &profiles));
    }
    let result = with_runtime_input(|input| {
        input.execute(
            &command.device_id,
            &command.command_type,
            &command.payload,
            &profiles,
        )
    });
    if let Ok(feedback) = &result {
        let settings = overlay_settings(shared);
        let overlay = app.state::<CursorOverlay>();
        if let Some(feedback) = feedback {
            overlay.show(*feedback, settings);
        } else {
            overlay.mark_control_active(settings);
        }
        if command.command_type == "connection.disconnecting" {
            overlay.end_session();
        }
    }
    if result.is_ok() && command.command_type == "pointer.speed.set" {
        if let Some(scale) = command
            .payload
            .get("scalePercent")
            .and_then(serde_json::Value::as_f64)
        {
            shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .state
                .settings
                .pointer_scale_percent = ((scale / 5.0).round() * 5.0).clamp(5.0, 225.0) as u8;
        }
    }
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .complete_desktop_command(
            &command,
            result.as_ref().map(|_| ()).map_err(String::as_str),
        )
}

fn complete_repeat_start(
    app: &AppHandle,
    shared: &SharedModel,
    command: DesktopCommand,
) -> Option<String> {
    let settings = overlay_settings(shared);
    let repeat_command = RepeatCommand::parse(&command.payload);
    let result = repeat_command.and_then(|repeat_command| {
        if !settings.mouse_repeat_enabled {
            stop_repeat_for_device(app, &command.device_id);
            return Err("Mouse repeat is disabled in settings.".into());
        }
        stop_repeat_for_device(app, &command.device_id);
        let initial_scale = if matches!(repeat_command, RepeatCommand::Move { .. })
            && settings.mouse_repeat_acceleration_duration_ms > 0
        {
            INITIAL_SCALE
        } else {
            1.0
        };
        let (active, initial_feedback) = {
            let mut guard = runtime()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let runtime = guard
                .as_mut()
                .ok_or_else(|| "Bluetooth runtime is not ready.".to_string())?;
            runtime
                .input
                .set_pointer_scale_percent(settings.pointer_scale_percent);
            let initial_feedback = runtime
                .input
                .execute_repeat(repeat_command, initial_scale)?;
            (
                runtime.repeats.start(
                    command.device_id.clone(),
                    repeat_command,
                    settings.mouse_repeat_acceleration_duration_ms,
                    crate::state::now_ms(),
                ),
                initial_feedback,
            )
        };
        app.state::<CursorOverlay>().begin_repeat(
            active.generation,
            repeat_command,
            settings.mouse_repeat_acceleration_duration_ms > 0,
            matches!(initial_feedback, PointerFeedback::Drag),
            settings,
        );
        spawn_repeat_loop(
            app.clone(),
            shared.clone(),
            command.device_id.clone(),
            active.generation,
        );
        Ok(())
    });
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .complete_desktop_command(
            &command,
            result.as_ref().map(|_| ()).map_err(String::as_str),
        )
}

fn complete_repeat_stop(
    app: &AppHandle,
    shared: &SharedModel,
    command: DesktopCommand,
) -> Option<String> {
    stop_repeat_for_device(app, &command.device_id);
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .complete_desktop_command(&command, Ok(()))
}

fn spawn_repeat_loop(app: AppHandle, shared: SharedModel, device_id: String, generation: u64) {
    tauri::async_runtime::spawn(async move {
        loop {
            let settings = overlay_settings(&shared);
            let active = {
                let guard = runtime()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard
                    .as_ref()
                    .and_then(|runtime| runtime.repeats.current(&device_id, generation))
            };
            let Some(active) = active else { return };
            if !settings.mouse_repeat_enabled {
                stop_repeat_if_current(&app, &device_id, generation);
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(u64::from(
                active.command.interval_ms(&settings),
            )))
            .await;
            let settings = overlay_settings(&shared);
            let result = {
                let mut guard = runtime()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let Some(runtime) = guard.as_mut() else {
                    return;
                };
                let Some(active) = runtime.repeats.current(&device_id, generation) else {
                    return;
                };
                if !settings.mouse_repeat_enabled {
                    Err("Mouse repeat was disabled.".to_string())
                } else {
                    runtime
                        .input
                        .set_pointer_scale_percent(settings.pointer_scale_percent);
                    let scale = if matches!(active.command, RepeatCommand::Move { .. }) {
                        acceleration_scale(
                            crate::state::now_ms() - active.started_at_ms,
                            active.acceleration_duration_ms,
                        )
                    } else {
                        1.0
                    };
                    runtime
                        .input
                        .execute_repeat(active.command, scale)
                        .map(|_| ())
                }
            };
            if let Err(error) = result {
                if stop_repeat_if_current(&app, &device_id, generation) {
                    set_activity(
                        &shared,
                        ActivityKind::Error,
                        format!("Mouse repeat stopped: {error}"),
                    );
                    emit_state(&app, &shared);
                }
                return;
            }
        }
    });
}

fn stop_repeat_if_current(app: &AppHandle, device_id: &str, generation: u64) -> bool {
    let stopped = runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_mut()
        .is_some_and(|runtime| runtime.repeats.stop_if_current(device_id, generation));
    if stopped {
        app.state::<CursorOverlay>().end_repeat(generation);
    }
    stopped
}

fn stop_repeat_for_device(app: &AppHandle, device_id: &str) {
    let active = runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_mut()
        .and_then(|runtime| runtime.repeats.stop(device_id));
    if let Some(active) = active {
        app.state::<CursorOverlay>().end_repeat(active.generation);
    }
}

fn stop_all_repeats(app: &AppHandle) {
    let active = runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_mut()
        .map_or_else(Vec::new, |runtime| runtime.repeats.stop_all());
    for repeat in active {
        app.state::<CursorOverlay>().end_repeat(repeat.generation);
    }
}

pub fn stop_mouse_repeat(app: &AppHandle) {
    stop_all_repeats(app);
}

fn overlay_settings(shared: &SharedModel) -> crate::state::AppSettings {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .settings
        .clone()
}

fn show_overlay(app: &AppHandle, shared: &SharedModel, feedback: PointerFeedback) {
    let settings = overlay_settings(shared);
    let overlay = app.state::<CursorOverlay>();
    overlay.show(feedback, settings);
}

fn default_pointer_profile() -> PointerProfile {
    PointerProfile {
        display_id: "primary".into(),
        scale_factor: 1.0,
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        small_delta: 19,
        medium_delta: 96,
        large_delta: 288,
    }
}

fn notify(message: String) -> Result<(), String> {
    let tx = runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .map(|runtime| runtime.tx.clone())
        .ok_or_else(|| "Bluetooth runtime is not ready.".to_string())?;
    for frame in create_notification_frames(&message, NOTIFICATION_BYTES)? {
        let buffer =
            CryptographicBuffer::CreateFromByteArray(&frame).map_err(|error| error.to_string())?;
        let _operation = tx
            .NotifyValueAsync(&buffer)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn check_accessibility(
    app: &AppHandle,
    shared: &SharedModel,
    _prompt: bool,
) -> Result<(), String> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .accessibility = AccessibilityState::Granted;
    emit_state(app, shared);
    Ok(())
}

pub fn approve_pairing(
    _app: &AppHandle,
    shared: &SharedModel,
    request_id: &str,
) -> Result<(), String> {
    let response = {
        let mut model = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let response = model
            .engine
            .approve_pairing(request_id, crate::state::now_ms())?;
        model.state.pending_pairing = None;
        response
    };
    notify(response)
}

pub fn reject_pairing(
    _app: &AppHandle,
    shared: &SharedModel,
    request_id: &str,
) -> Result<(), String> {
    let response = {
        let mut model = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let response = model.engine.reject_pairing(request_id)?;
        model.state.pending_pairing = None;
        response
    };
    notify(response)
}

pub fn disconnect_all(app: &AppHandle, shared: &SharedModel) -> Result<(), String> {
    stop_all_repeats(app);
    if let Some(runtime) = runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_mut()
    {
        let _ = runtime.input.release_all();
        runtime.input.end_control_session();
    }
    let mut model = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    model.state.bluetooth = BluetoothState::Advertising;
    model.state.connected_device_name = None;
    drop(model);
    emit_state(app, shared);
    Ok(())
}

fn release_input_session() {
    if let Some(runtime) = runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_mut()
    {
        let _ = runtime.input.release_all();
        runtime.input.end_control_session();
    }
}
