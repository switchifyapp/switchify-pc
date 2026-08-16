use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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

use crate::display_navigation::{self, NavigationError};
use crate::dwell::DwellController;
use crate::input::{
    execute_desktop_command, execute_dwell_click, AndroidTypingRoute, DesktopCommandOutcome,
    DesktopInput, PointerFeedback,
};
use crate::modifier_overlay::ModifierOverlay;
use crate::mouse_repeat::{MouseRepeatController, RepeatCommand, MOVE_TICK_INTERVAL_MS};
use crate::overlay::CursorOverlay;
use crate::protocol::{
    bluetooth_status_payload, create_notification_frames, pointer_profile_response,
    switch_profile_catalog_response, DesktopCommand, EngineEvent, MouseClickCommand,
    MouseMoveCommand, PointerProfile, TextCommand,
};
use crate::state::{
    emit_state, set_activity, AccessibilityState, ActivityKind, AppModel, BluetoothState,
    SharedModel,
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
            let subscriber_count = sender
                .as_ref()
                .and_then(|value| value.SubscribedClients().ok())
                .and_then(|clients| clients.Size().ok());
            let Some(subscriber_count) = subscriber_count else {
                eprintln!(
                    "Switchify BLE subscriber count was unavailable; preserving connection state."
                );
                return Ok(());
            };
            let connected = subscriber_count > 0;
            let cancelled = {
                let mut model = subscribe_shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let cancelled = if should_cancel_pending_pairings(Some(subscriber_count)) {
                    model.engine.cancel_all_pairings()
                } else {
                    0
                };
                if cancelled > 0 {
                    model.state.pending_pairings = model.engine.pending_pairings();
                }
                model.state.bluetooth = if connected {
                    BluetoothState::Connected
                } else {
                    BluetoothState::Advertising
                };
                model.state.connected_device_name = connected.then(|| "Bluetooth device".into());
                cancelled
            };
            set_activity(
                &subscribe_shared,
                ActivityKind::Info,
                if cancelled > 0 {
                    "Pairing request cancelled."
                } else if connected {
                    "Android device connected."
                } else {
                    "Android device disconnected."
                },
            );
            if !connected {
                subscribe_app
                    .state::<DwellController>()
                    .cancel(&subscribe_app);
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

fn should_cancel_pending_pairings(subscriber_count: Option<u32>) -> bool {
    subscriber_count == Some(0)
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
        Some(EngineEvent::PendingPairing {
            request,
            replaced_response,
        }) => {
            let request_id = request.request_id.clone();
            let delay_ms = request.expires_at.saturating_sub(crate::state::now_ms()) as u64;
            {
                let mut model = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                model.state.pending_pairings = model.engine.pending_pairings();
            }
            set_activity(
                shared,
                ActivityKind::Info,
                "Review the pairing code before approving this device.",
            );
            schedule_pairing_expiration(app, shared, request_id, delay_ms);
            replaced_response
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
                &current_pointer_profile(app),
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
        app.state::<DwellController>().arm(app);
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
    app.state::<DwellController>().cancel(app);
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
    let typing_route = AndroidTypingRoute::for_text(&command.text);
    typing_route.prepare(
        || app.state::<DwellController>().cancel(app),
        || stop_all_repeats(app),
    );
    let result = with_runtime_input(|input| input.type_text(&command.text));
    typing_route.finish(result.is_ok(), || {
        app.state::<CursorOverlay>().hide_for_typing()
    });
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
    if matches!(
        command.command_type.as_str(),
        "mouse.scroll"
            | "mouse.dragStart"
            | "mouse.dragEnd"
            | "mouse.click"
            | "mouse.doubleClick"
            | "mouse.rightClick"
            | "switch.session.start"
            | "connection.disconnecting"
    ) {
        app.state::<DwellController>().cancel(app);
    }
    let typing_route = AndroidTypingRoute::for_command(&command.command_type);
    let profiles = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .profiles
        .clone();
    if command.command_type == "switch.profile.list" {
        stop_all_repeats(app);
        return Some(switch_profile_catalog_response(&command.id, &profiles));
    }
    let (result, error_code) = if command.command_type == "pointer.display.move" {
        let direction = command.payload["direction"].as_str().unwrap_or_default();
        let mut context = (app, shared);
        match display_navigation::run_navigation_command(
            &mut context,
            direction,
            |(app, _)| stop_all_repeats(app),
            |(app, _), direction| navigate_display(app, direction),
            |(app, shared), feedback| {
                app.state::<CursorOverlay>()
                    .show(feedback, overlay_settings(shared));
            },
        ) {
            Ok(feedback) => (
                Ok(DesktopCommandOutcome {
                    pointer_feedback: Some(feedback),
                    typing_injected: false,
                }),
                "input_failed",
            ),
            Err(error) => (Err(error.message), error.code),
        }
    } else {
        if typing_route.is_eligible() {
            typing_route.prepare(
                || app.state::<DwellController>().cancel(app),
                || stop_all_repeats(app),
            );
        } else {
            stop_all_repeats(app);
        }
        let model = app.state::<AppModel>();
        (
            with_runtime_input(|input| {
                execute_desktop_command(
                    input,
                    &model,
                    &command.device_id,
                    &command.command_type,
                    &command.payload,
                    &profiles,
                )
            }),
            "input_failed",
        )
    };
    if command.command_type != "pointer.display.move" {
        if let Ok(outcome) = &result {
            let settings = overlay_settings(shared);
            let overlay = app.state::<CursorOverlay>();
            typing_route.finish(outcome.typing_injected, || overlay.hide_for_typing());
            if !outcome.typing_injected {
                if let Some(feedback) = &outcome.pointer_feedback {
                    overlay.show(*feedback, settings);
                } else {
                    overlay.mark_control_active(settings);
                }
            }
            if command.command_type == "connection.disconnecting" {
                overlay.end_session();
            }
        }
    }
    if command.command_type == "pointer.display.move" && result.is_ok() {
        app.state::<DwellController>().arm(app);
    }
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .engine
        .complete_desktop_command_with_error(
            &command,
            result
                .as_ref()
                .map(|_| ())
                .map_err(|error| (error_code, error.as_str())),
        )
}

fn navigate_display(app: &AppHandle, direction: &str) -> Result<PointerFeedback, NavigationError> {
    let mut guard = runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let input = &mut guard
        .as_mut()
        .ok_or_else(|| NavigationError {
            code: "adapter_failure",
            message: "Bluetooth runtime is not ready.".into(),
        })?
        .input;
    if input.has_active_switch_session() {
        return Err(NavigationError {
            code: "input_failed",
            message: "Stop PC Switch Control before using other PC control commands.".into(),
        });
    }
    if input.has_active_drag() {
        return Err(NavigationError {
            code: "drag_active",
            message: "End the active drag before moving to another monitor.".into(),
        });
    }
    let (cursor, displays) = display_navigation::displays(app)?;
    let source =
        display_navigation::current_display(cursor, &displays).ok_or_else(|| NavigationError {
            code: "adapter_failure",
            message: "The active monitor could not be resolved.".into(),
        })?;
    let (x, y) = display_navigation::target_center(source, &displays, direction)?;
    display_navigation::map_injection(input.move_pointer_absolute(x, y))
}

fn complete_repeat_start(
    app: &AppHandle,
    shared: &SharedModel,
    command: DesktopCommand,
) -> Option<String> {
    app.state::<DwellController>().cancel(app);
    let settings = overlay_settings(shared);
    let repeat_command = RepeatCommand::parse(&command.payload);
    let result = repeat_command.and_then(|repeat_command| {
        if !settings.mouse_repeat_enabled {
            stop_repeat_for_device(app, &command.device_id);
            return Err("Mouse repeat is disabled in settings.".into());
        }
        stop_repeat_for_device(app, &command.device_id);
        let (active, initial_feedback) = {
            let mut guard = runtime()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let runtime = guard
                .as_mut()
                .ok_or_else(|| "Bluetooth runtime is not ready.".to_string())?;
            let active = runtime.repeats.start(
                command.device_id.clone(),
                repeat_command,
                settings.mouse_repeat_acceleration_duration_ms,
                Instant::now(),
            );
            let initial = match repeat_command {
                RepeatCommand::Move { .. } => {
                    let (dx, dy) = runtime
                        .repeats
                        .initial_move(&command.device_id, active.generation)
                        .unwrap_or((0, 0));
                    if dx == 0 && dy == 0 {
                        Ok(runtime.input.pointer_feedback_for_move())
                    } else {
                        runtime.input.move_pointer_pixels(dx, dy)
                    }
                }
                RepeatCommand::Scroll { dx, dy } => runtime.input.execute_repeat_scroll(dx, dy),
            };
            match initial {
                Ok(feedback) => (active, feedback),
                Err(error) => {
                    runtime
                        .repeats
                        .stop_if_current(&command.device_id, active.generation);
                    return Err(error);
                }
            }
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
    let stopped = stop_repeat_for_device(app, &command.device_id);
    if stopped.is_some_and(|active| matches!(active.command, RepeatCommand::Move { .. })) {
        app.state::<DwellController>().arm(app);
    }
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
            let delay_ms = match active.command {
                RepeatCommand::Move { .. } => MOVE_TICK_INTERVAL_MS,
                RepeatCommand::Scroll { .. } => u64::from(active.command.interval_ms(&settings)),
            };
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
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
                    match active.command {
                        RepeatCommand::Move { .. } => {
                            let Some((dx, dy)) = runtime.repeats.advance_move(
                                &device_id,
                                generation,
                                Instant::now(),
                                settings.move_repeat_interval_ms,
                                settings.pointer_scale_percent,
                            ) else {
                                return;
                            };
                            if dx == 0 && dy == 0 {
                                Ok(())
                            } else {
                                runtime.input.move_pointer_pixels(dx, dy).map(|_| ())
                            }
                        }
                        RepeatCommand::Scroll { dx, dy } => {
                            runtime.input.execute_repeat_scroll(dx, dy).map(|_| ())
                        }
                    }
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

fn stop_repeat_for_device(
    app: &AppHandle,
    device_id: &str,
) -> Option<crate::mouse_repeat::ActiveRepeat> {
    let active = runtime()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_mut()
        .and_then(|runtime| runtime.repeats.stop(device_id));
    if let Some(active) = active {
        app.state::<CursorOverlay>().end_repeat(active.generation);
    }
    active
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

pub fn dwell_click(app: &AppHandle) -> Result<(), String> {
    stop_all_repeats(app);
    with_runtime_input(execute_dwell_click).map(|_| ())
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

fn current_pointer_profile(app: &AppHandle) -> PointerProfile {
    let Ok((cursor, displays)) = display_navigation::displays(app) else {
        return fallback_pointer_profile();
    };
    let Some(display) = display_navigation::current_display(cursor, &displays) else {
        return fallback_pointer_profile();
    };
    pointer_profile_for_display(display, display_navigation::display_count(displays.len()))
}

fn pointer_profile_for_display(
    display: &display_navigation::Display,
    display_count: u8,
) -> PointerProfile {
    let scale_factor = if display.scale_factor.is_finite() && display.scale_factor > 0.0 {
        display.scale_factor
    } else {
        1.0
    };
    let x = (f64::from(display.x) / scale_factor).round() as i32;
    let y = (f64::from(display.y) / scale_factor).round() as i32;
    let width = (f64::from(display.width) / scale_factor)
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32;
    let height = (f64::from(display.height) / scale_factor)
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32;
    let short_edge = width.min(height);
    let delta = |fraction: f64| {
        (f64::from(short_edge) * fraction)
            .round()
            .clamp(1.0, crate::protocol::MAX_POINTER_DELTA) as u32
    };
    PointerProfile {
        display_id: format!("{}:{x}:{y}:{width}:{height}:{scale_factor}", display.name),
        scale_factor,
        x,
        y,
        width,
        height,
        small_delta: delta(0.0225),
        medium_delta: delta(0.06),
        large_delta: delta(0.13),
        display_navigation_supported: true,
        display_count,
    }
}

fn fallback_pointer_profile() -> PointerProfile {
    pointer_profile_for_display(
        &display_navigation::Display {
            name: "fallback".into(),
            scale_factor: 1.0,
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        1,
    )
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
    app: &AppHandle,
    shared: &SharedModel,
    request_id: &str,
) -> Result<(), String> {
    let response = {
        let mut model = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let response = model
            .engine
            .approve_pairing(request_id, crate::state::now_ms());
        model.state.pending_pairings = model.engine.pending_pairings();
        response
    }?;
    notify(response)?;
    emit_state(app, shared);
    Ok(())
}

pub fn reject_pairing(
    app: &AppHandle,
    shared: &SharedModel,
    request_id: &str,
) -> Result<(), String> {
    let response = {
        let mut model = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let response = model.engine.reject_pairing(request_id)?;
        model.state.pending_pairings = model.engine.pending_pairings();
        response
    };
    notify(response)?;
    emit_state(app, shared);
    Ok(())
}

fn schedule_pairing_expiration(
    app: &AppHandle,
    shared: &SharedModel,
    request_id: String,
    delay_ms: u64,
) {
    let app = app.clone();
    let shared = shared.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        let callback_app = app.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = expire_pairing(&callback_app, &shared, &request_id);
        });
    });
}

fn expire_pairing(app: &AppHandle, shared: &SharedModel, request_id: &str) -> Result<(), String> {
    let response = {
        let mut model = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let response = model
            .engine
            .expire_pairing(request_id, crate::state::now_ms());
        model.state.pending_pairings = model.engine.pending_pairings();
        response
    };
    let Some(response) = response else {
        return Ok(());
    };
    notify(response)?;
    set_activity(shared, ActivityKind::Info, "Pairing request expired.");
    emit_state(app, shared);
    Ok(())
}

pub fn disconnect_all(app: &AppHandle, shared: &SharedModel) -> Result<(), String> {
    app.state::<DwellController>().cancel(app);
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
    model.engine.cancel_all_pairings();
    model.state.pending_pairings.clear();
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

#[cfg(test)]
mod tests {
    use super::{
        pointer_profile_for_display, should_cancel_pending_pairings,
        tasklist_has_other_switchify_process,
    };
    use crate::display_navigation::Display;
    use crate::input::AndroidTypingRoute;
    use std::sync::Mutex;

    #[test]
    fn windows_typing_route_orders_cleanup_before_successful_overlay_hiding() {
        let events = Mutex::new(Vec::new());
        let route = AndroidTypingRoute::for_text("Hello");
        route.prepare(
            || events.lock().unwrap().push("cancel dwell"),
            || events.lock().unwrap().push("stop repeats"),
        );
        route.finish(true, || events.lock().unwrap().push("hide overlay"));
        assert_eq!(
            *events.lock().unwrap(),
            ["cancel dwell", "stop repeats", "hide overlay"]
        );

        let events = Mutex::new(Vec::new());
        let failed = AndroidTypingRoute::for_text("Hello");
        failed.prepare(
            || events.lock().unwrap().push("cancel dwell"),
            || events.lock().unwrap().push("stop repeats"),
        );
        failed.finish(false, || events.lock().unwrap().push("hide overlay"));
        assert_eq!(*events.lock().unwrap(), ["cancel dwell", "stop repeats"]);

        let events = Mutex::new(Vec::new());
        let empty = AndroidTypingRoute::for_text("");
        empty.prepare(
            || events.lock().unwrap().push("cancel dwell"),
            || events.lock().unwrap().push("stop repeats"),
        );
        empty.finish(true, || events.lock().unwrap().push("hide overlay"));
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn pending_pairings_clear_only_after_the_final_subscriber_leaves() {
        assert!(should_cancel_pending_pairings(Some(0)));
        assert!(!should_cancel_pending_pairings(Some(1)));
        assert!(!should_cancel_pending_pairings(Some(2)));
        assert!(!should_cancel_pending_pairings(None));
    }

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

    #[test]
    fn pointer_profile_uses_the_active_display_scale_and_live_count() {
        let profile = pointer_profile_for_display(
            &Display {
                name: "Retina".into(),
                scale_factor: 2.0,
                x: -3840,
                y: 0,
                width: 3840,
                height: 2160,
            },
            3,
        );
        assert_eq!(profile.display_id, "Retina:-1920:0:1920:1080:2");
        assert_eq!(
            (profile.x, profile.y, profile.width, profile.height),
            (-1920, 0, 1920, 1080)
        );
        assert_eq!(profile.display_count, 3);
        assert_eq!(
            (
                profile.small_delta,
                profile.medium_delta,
                profile.large_delta
            ),
            (24, 65, 140)
        );
    }
}
