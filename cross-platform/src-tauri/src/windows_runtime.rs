use std::process::Command;
use std::sync::{Mutex, OnceLock};

use enigo::{Enigo, Settings};
use tauri::AppHandle;
use windows::core::{IInspectable, Ref, GUID, HSTRING};
use windows::Devices::Bluetooth::BluetoothError;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristicProperties, GattLocalCharacteristic, GattLocalCharacteristicParameters,
    GattProtectionLevel, GattReadRequestedEventArgs, GattServiceProvider,
    GattServiceProviderAdvertisingParameters, GattWriteOption, GattWriteRequestedEventArgs,
};
use windows::Foundation::TypedEventHandler;
use windows::Security::Cryptography::CryptographicBuffer;

use crate::input::DesktopInput;
use crate::protocol::{
    create_notification_frames, pointer_profile_response, switch_profile_catalog_response,
    DesktopCommand, EngineEvent, MouseClickCommand, MouseMoveCommand, PointerProfile, TextCommand,
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
    tx: GattLocalCharacteristic,
    input: DesktopInput<Enigo>,
}

static RUNTIME: OnceLock<Mutex<Option<WindowsRuntime>>> = OnceLock::new();
fn runtime() -> &'static Mutex<Option<WindowsRuntime>> {
    RUNTIME.get_or_init(|| Mutex::new(None))
}

fn shipping_app_is_running() -> bool {
    Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq Switchify PC.exe", "/FO", "CSV", "/NH"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|output| output.contains("Switchify PC.exe"))
}

pub fn install(app: AppHandle, shared: SharedModel) -> Result<(), String> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .accessibility = AccessibilityState::Granted;
    if shipping_app_is_running() {
        shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .bluetooth = BluetoothState::Conflict;
        set_activity(
            &shared,
            ActivityKind::Info,
            "Close the current Switchify PC app to test preview Bluetooth.",
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
                let callback_app = write_app.clone();
                let callback_shared = write_shared.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) =
                        handle_write(callback_app.clone(), callback_shared.clone(), args).await
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
                    let status_shared = read_shared.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = handle_status_read(status_shared, args).await;
                    });
                }
                Ok(())
            },
        ))
        .map_err(|error| error.to_string())?;

    let input = Enigo::new(&Settings::default())
        .map_err(|_| "Windows input injection could not initialize.".to_string())?;
    *runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(WindowsRuntime {
        _provider: provider.clone(),
        tx,
        input: DesktopInput::new(input),
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
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .bluetooth = BluetoothState::Advertising;
    set_activity(
        &shared,
        ActivityKind::Info,
        "Advertising to nearby Switchify Android devices.",
    );
    emit_state(&app, &shared);
    Ok(())
}

async fn handle_write(
    app: AppHandle,
    shared: SharedModel,
    args: GattWriteRequestedEventArgs,
) -> Result<(), String> {
    let deferral = args.GetDeferral().map_err(|error| error.to_string())?;
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
) -> Result<(), String> {
    let deferral = args.GetDeferral().map_err(|error| error.to_string())?;
    let request = args
        .GetRequestAsync()
        .map_err(|error| error.to_string())?
        .await
        .map_err(|error| error.to_string())?;
    let model = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let payload = serde_json::to_vec(&serde_json::json!({"protocolVersion":1,"displayName":"Switchify PC Preview","desktopId":model.state.desktop_id})).map_err(|error| error.to_string())?;
    drop(model);
    let buffer =
        CryptographicBuffer::CreateFromByteArray(&payload).map_err(|error| error.to_string())?;
    request
        .RespondWithValue(&buffer)
        .map_err(|error| error.to_string())?;
    deferral.Complete().map_err(|error| error.to_string())
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
            let scale = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .state
                .settings
                .pointer_scale_percent;
            Some(pointer_profile_response(
                &id,
                &default_pointer_profile(),
                scale,
            ))
        }
        Some(EngineEvent::MouseMove(command)) => complete_mouse_move(shared, command),
        Some(EngineEvent::MouseClick(command)) => complete_mouse_click(shared, command),
        Some(EngineEvent::Text(command)) => complete_text(shared, command),
        Some(EngineEvent::Desktop(command)) => complete_desktop(shared, command),
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

fn complete_mouse_move(shared: &SharedModel, command: MouseMoveCommand) -> Option<String> {
    let scale = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .state
        .settings
        .pointer_scale_percent;
    let result = with_runtime_input(|input| {
        input.set_pointer_scale_percent(scale);
        input.move_pointer(command.dx.round() as i32, command.dy.round() as i32)
    });
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .complete_mouse_move_command(
            &command,
            result.as_ref().map(|_| ()).map_err(String::as_str),
        )
}
fn complete_mouse_click(shared: &SharedModel, command: MouseClickCommand) -> Option<String> {
    let result =
        with_runtime_input(|input| input.click_pointer(command.button, command.click_count));
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .complete_mouse_click_command(
            &command,
            result.as_ref().map(|_| ()).map_err(String::as_str),
        )
}
fn complete_text(shared: &SharedModel, command: TextCommand) -> Option<String> {
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
fn complete_desktop(shared: &SharedModel, command: DesktopCommand) -> Option<String> {
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
    if let Some(runtime) = runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_mut()
    {
        let _ = runtime.input.release_all();
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
