use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use corebluetooth::prelude::*;
use enigo::{Enigo, Settings};
use tauri::AppHandle;

use crate::input::DesktopInput;
use crate::protocol::{
    pointer_profile_response, DesktopCommand, EngineEvent, MouseButton, MouseClickCommand,
    MouseMoveCommand, OutboundQueue, PointerProfile, TextCommand, MAX_POINTER_DELTA,
};
use crate::state::{
    emit_state, now_ms, set_activity, AccessibilityState, ActivityKind, BluetoothState, SharedModel,
};

const SERVICE_UUID: &str = "7a78f7e8-1d6d-4d92-9ef0-1f89d3db21f4";
const RX_UUID: &str = "7a78f7e9-1d6d-4d92-9ef0-1f89d3db21f4";
const TX_UUID: &str = "7a78f7ea-1d6d-4d92-9ef0-1f89d3db21f4";
const STATUS_UUID: &str = "7a78f7eb-1d6d-4d92-9ef0-1f89d3db21f4";
const DISPLAY_NAME: &str = "Switchify Tauri POC";
const MAX_QUEUED_NOTIFICATIONS: usize = 512;

thread_local! {
    static RUNTIME: RefCell<Option<MacRuntime>> = const { RefCell::new(None) };
}

pub fn install(app: AppHandle, shared: SharedModel) -> Result<(), String> {
    let state_app = app.clone();
    let service_app = app.clone();
    let advertising_app = app.clone();
    let subscribe_app = app.clone();
    let unsubscribe_app = app.clone();
    let ready_app = app.clone();
    let read_app = app.clone();
    let write_app = app.clone();
    let callbacks = PeripheralManagerCallbacks::new()
        .on_state(move |state, _authorization| {
            dispatch_to_main(&state_app, move |runtime| {
                runtime.handle_manager_state(state)
            });
        })
        .on_add_service(move |_service, error| {
            dispatch_to_main(&service_app, move |runtime| {
                runtime.service_was_added(error.is_none())
            });
        })
        .on_start_advertising(move |error| {
            dispatch_to_main(&advertising_app, move |runtime| {
                runtime.advertising_did_start(error.is_none())
            });
        })
        .on_subscribe(move |central, characteristic| {
            let values = MainThreadBluetoothValue((central, characteristic));
            dispatch_to_main(&subscribe_app, move |runtime| {
                let (central, characteristic) = values.into_inner();
                runtime.central_subscribed(central, characteristic)
            });
        })
        .on_unsubscribe(move |central, characteristic| {
            let values = MainThreadBluetoothValue((central, characteristic));
            dispatch_to_main(&unsubscribe_app, move |runtime| {
                let (central, characteristic) = values.into_inner();
                runtime.central_unsubscribed(central, characteristic)
            });
        })
        .on_ready_to_update(move || {
            dispatch_to_main(&ready_app, MacRuntime::flush_outbound);
        })
        .on_read_request(move |request| {
            let request = MainThreadBluetoothValue(request);
            dispatch_to_main(&read_app, move |runtime| {
                runtime.handle_read(request.into_inner())
            });
        })
        .on_write_requests(move |requests| {
            let requests = MainThreadBluetoothValue(requests);
            dispatch_to_main(&write_app, move |runtime| {
                runtime.handle_writes(requests.into_inner())
            });
        });
    let manager =
        PeripheralManager::with_callbacks(callbacks).map_err(|error| error.to_string())?;
    let state = manager.state();
    RUNTIME.with(|slot| {
        *slot.borrow_mut() = Some(MacRuntime {
            app,
            shared,
            manager,
            service: None,
            tx_characteristic: None,
            status_value: Vec::new(),
            subscribers: HashMap::new(),
            outbound: OutboundQueue::default(),
            input: None,
        });
    });
    with_runtime(|runtime| runtime.handle_manager_state(state))
}

fn dispatch_to_main(
    app: &AppHandle,
    operation: impl FnOnce(&mut MacRuntime) -> Result<(), String> + Send + 'static,
) {
    let _ = app.run_on_main_thread(move || {
        let _ = with_runtime(operation);
    });
}

struct MainThreadBluetoothValue<T>(T);

impl<T> MainThreadBluetoothValue<T> {
    fn into_inner(self) -> T {
        self.0
    }
}

// SAFETY: callback values are owned retained CoreBluetooth references. Each value is moved exactly
// once from the library's serial callback queue and is not accessed until Tauri runs the receiving
// closure on the macOS main thread, where all application-side CoreBluetooth work is confined.
unsafe impl<T> Send for MainThreadBluetoothValue<T> {}

pub fn check_accessibility(
    app: &AppHandle,
    shared: &SharedModel,
    prompt: bool,
) -> Result<(), String> {
    with_runtime(|runtime| {
        let granted = runtime.refresh_accessibility(prompt);
        {
            let mut model = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            model.state.accessibility = if granted {
                AccessibilityState::Granted
            } else {
                AccessibilityState::Required
            };
        }
        if prompt && !granted {
            set_activity(
                shared,
                ActivityKind::Info,
                "Grant Accessibility access in System Settings, then return to this window.",
            );
        }
        emit_state(app, shared);
        Ok(())
    })
}

pub fn approve_pairing(
    app: &AppHandle,
    shared: &SharedModel,
    request_id: &str,
) -> Result<(), String> {
    with_runtime(|runtime| {
        let response = {
            let mut model = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let response = model.engine.approve_pairing(request_id, now_ms());
            let pending = model.engine.pending_pairing();
            model.state.pending_pairing = pending;
            response
        }?;
        runtime.enqueue_message(&response)?;
        set_activity(
            shared,
            ActivityKind::Success,
            "Device paired for this app session.",
        );
        emit_state(app, shared);
        Ok(())
    })
}

pub fn reject_pairing(
    app: &AppHandle,
    shared: &SharedModel,
    request_id: &str,
) -> Result<(), String> {
    with_runtime(|runtime| {
        let response = {
            let mut model = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let response = model.engine.reject_pairing(request_id);
            let pending = model.engine.pending_pairing();
            model.state.pending_pairing = pending;
            response
        }?;
        runtime.enqueue_message(&response)?;
        set_activity(shared, ActivityKind::Info, "Pairing request rejected.");
        emit_state(app, shared);
        Ok(())
    })
}

fn with_runtime<T>(
    operation: impl FnOnce(&mut MacRuntime) -> Result<T, String>,
) -> Result<T, String> {
    RUNTIME.with(|slot| {
        let mut slot = slot.borrow_mut();
        let runtime = slot
            .as_mut()
            .ok_or_else(|| "The macOS runtime is not initialized.".to_string())?;
        operation(runtime)
    })
}

struct MacRuntime {
    app: AppHandle,
    shared: SharedModel,
    manager: PeripheralManager,
    service: Option<MutableService>,
    tx_characteristic: Option<MutableCharacteristic>,
    status_value: Vec<u8>,
    subscribers: HashMap<String, usize>,
    outbound: OutboundQueue,
    input: Option<DesktopInput<Enigo>>,
}

impl MacRuntime {
    fn handle_manager_state(&mut self, state: PeripheralManagerState) -> Result<(), String> {
        match state {
            PeripheralManagerState::PoweredOn => self.configure_service()?,
            PeripheralManagerState::PoweredOff => {
                self.reset_gatt();
                self.set_bluetooth(BluetoothState::PoweredOff);
            }
            PeripheralManagerState::Unauthorized => {
                self.reset_gatt();
                self.set_bluetooth(BluetoothState::Unauthorized);
            }
            PeripheralManagerState::Unsupported => {
                self.reset_gatt();
                self.set_bluetooth(BluetoothState::Error);
                set_activity(
                    &self.shared,
                    ActivityKind::Error,
                    "This Mac does not support the Bluetooth peripheral role.",
                );
            }
            PeripheralManagerState::Unknown | PeripheralManagerState::Resetting => {
                self.set_bluetooth(BluetoothState::Initializing);
            }
        }
        emit_state(&self.app, &self.shared);
        Ok(())
    }

    fn configure_service(&mut self) -> Result<(), String> {
        if self.service.is_some() {
            return Ok(());
        }
        self.set_bluetooth(BluetoothState::Initializing);
        let service_uuid =
            BluetoothUuid::from_string(SERVICE_UUID).map_err(|error| error.to_string())?;
        let rx_uuid = BluetoothUuid::from_string(RX_UUID).map_err(|error| error.to_string())?;
        let tx_uuid = BluetoothUuid::from_string(TX_UUID).map_err(|error| error.to_string())?;
        let status_uuid =
            BluetoothUuid::from_string(STATUS_UUID).map_err(|error| error.to_string())?;

        let rx_properties = CharacteristicProperties::from_bits(
            CharacteristicProperties::WRITE.bits()
                | CharacteristicProperties::WRITE_WITHOUT_RESPONSE.bits(),
        );
        let rx = MutableCharacteristic::new(
            &rx_uuid,
            rx_properties,
            None,
            AttributePermissions::WRITEABLE,
        )
        .map_err(|error| error.to_string())?;
        let tx = MutableCharacteristic::new(
            &tx_uuid,
            CharacteristicProperties::NOTIFY,
            None,
            AttributePermissions::from_bits(0),
        )
        .map_err(|error| error.to_string())?;

        let desktop_id = {
            self.shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .engine
                .desktop_id()
                .to_owned()
        };
        self.status_value = serde_json::to_vec(&serde_json::json!({
            "protocolVersion": 1,
            "displayName": DISPLAY_NAME,
            "desktopId": desktop_id
        }))
        .map_err(|error| error.to_string())?;
        let status = MutableCharacteristic::new(
            &status_uuid,
            CharacteristicProperties::READ,
            Some(&self.status_value),
            AttributePermissions::READABLE,
        )
        .map_err(|error| error.to_string())?;

        let mut service =
            MutableService::new(&service_uuid, true).map_err(|error| error.to_string())?;
        service
            .set_characteristics(&[&rx, &tx, &status])
            .map_err(|error| error.to_string())?;
        self.manager
            .add_service(&service)
            .map_err(|error| error.to_string())?;
        self.tx_characteristic = Some(tx);
        self.service = Some(service);
        emit_state(&self.app, &self.shared);
        Ok(())
    }

    fn service_was_added(&mut self, succeeded: bool) -> Result<(), String> {
        if !succeeded {
            self.set_bluetooth(BluetoothState::Error);
            set_activity(
                &self.shared,
                ActivityKind::Error,
                "The Bluetooth service could not be published.",
            );
            emit_state(&self.app, &self.shared);
            return Ok(());
        }
        let service_uuid =
            BluetoothUuid::from_string(SERVICE_UUID).map_err(|error| error.to_string())?;
        let advertisement = AdvertisementData::new()
            .with_local_name(DISPLAY_NAME)
            .with_service_uuid(service_uuid);
        self.manager
            .start_advertising(&advertisement)
            .map_err(|error| error.to_string())
    }

    fn advertising_did_start(&mut self, succeeded: bool) -> Result<(), String> {
        if succeeded {
            self.set_bluetooth(BluetoothState::Advertising);
            set_activity(
                &self.shared,
                ActivityKind::Info,
                "Advertising to nearby Switchify Android devices.",
            );
        } else {
            self.set_bluetooth(BluetoothState::Error);
            set_activity(
                &self.shared,
                ActivityKind::Error,
                "Bluetooth advertising failed.",
            );
        }
        emit_state(&self.app, &self.shared);
        Ok(())
    }

    fn central_subscribed(
        &mut self,
        central: Central,
        characteristic: Characteristic,
    ) -> Result<(), String> {
        if characteristic.uuid().eq_ignore_ascii_case(TX_UUID) {
            let maximum_notification_bytes = central.maximum_update_value_length();
            self.subscribers
                .insert(central.identifier(), maximum_notification_bytes);
            self.set_bluetooth(BluetoothState::Connected);
            set_activity(
                &self.shared,
                ActivityKind::Info,
                format!(
                    "Android connected. Notification limit: {maximum_notification_bytes} bytes."
                ),
            );
            self.flush_outbound()?;
            emit_state(&self.app, &self.shared);
        }
        Ok(())
    }

    fn central_unsubscribed(
        &mut self,
        central: Central,
        characteristic: Characteristic,
    ) -> Result<(), String> {
        if characteristic.uuid().eq_ignore_ascii_case(TX_UUID) {
            self.subscribers.remove(&central.identifier());
            if self.subscribers.is_empty() {
                self.outbound.clear();
                self.set_bluetooth(BluetoothState::Advertising);
            }
            emit_state(&self.app, &self.shared);
        }
        Ok(())
    }

    fn handle_read(&mut self, mut request: AttRequest) -> Result<(), String> {
        let uuid = request.characteristic().uuid();
        let offset = request.offset();
        let result = if !uuid.eq_ignore_ascii_case(STATUS_UUID) {
            AttError::ReadNotPermitted
        } else if offset > self.status_value.len() {
            AttError::InvalidOffset
        } else {
            request.set_value(Some(&self.status_value[offset..]));
            AttError::Success
        };
        self.manager
            .respond_to_request(&request, result)
            .map_err(|error| error.to_string())
    }

    fn handle_writes(&mut self, requests: Vec<AttRequest>) -> Result<(), String> {
        for request in requests {
            let uuid = request.characteristic().uuid();
            let result = if !uuid.eq_ignore_ascii_case(RX_UUID) {
                AttError::WriteNotPermitted
            } else if request.offset() != 0 {
                AttError::InvalidOffset
            } else {
                match request.value() {
                    Ok(Some(value)) => {
                        self.handle_frame(&value);
                        AttError::Success
                    }
                    _ => AttError::InvalidPdu,
                }
            };
            self.manager
                .respond_to_request(&request, result)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn handle_frame(&mut self, bytes: &[u8]) {
        let event = {
            self.shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .engine
                .receive_frame(bytes, now_ms())
        };
        match event {
            Ok(None) => {}
            Ok(Some(EngineEvent::PendingPairing(pending))) => {
                let request_id = pending.request_id.clone();
                let delay_ms = pending.expires_at.saturating_sub(now_ms()) as u64;
                self.shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .state
                    .pending_pairing = Some(pending);
                set_activity(
                    &self.shared,
                    ActivityKind::Info,
                    "Review the pairing code before approving this device.",
                );
                self.schedule_pairing_expiration(request_id, delay_ms);
                emit_state(&self.app, &self.shared);
            }
            Ok(Some(EngineEvent::Response(response))) => {
                if let Err(error) = self.enqueue_message(&response) {
                    self.report_error(error);
                }
            }
            Ok(Some(EngineEvent::PointerProfile(id))) => self.handle_pointer_profile(&id),
            Ok(Some(EngineEvent::MouseMove(command))) => self.handle_mouse_move(command),
            Ok(Some(EngineEvent::MouseClick(command))) => self.handle_mouse_click(command),
            Ok(Some(EngineEvent::Text(command))) => self.handle_text(command),
            Ok(Some(EngineEvent::Desktop(command))) => self.handle_desktop(command),
            Err(reason) => self.report_error(format!("Rejected Bluetooth message: {reason}.")),
        }
    }

    fn handle_pointer_profile(&mut self, id: &str) {
        let response = pointer_profile_response(id, &self.pointer_profile());
        if let Err(error) = self.enqueue_message(&response) {
            self.report_error(error);
        }
    }

    fn handle_mouse_move(&mut self, command: MouseMoveCommand) {
        let dx = command.dx.round() as i32;
        let dy = command.dy.round() as i32;
        let injection = self.inject_pointer_move(dx, dy);
        let response = {
            self.shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .engine
                .complete_mouse_move_command(
                    &command,
                    injection
                        .as_ref()
                        .map(|_| ())
                        .map_err(|error| error.as_str()),
                )
        };
        match injection {
            Ok(()) => set_activity(
                &self.shared,
                ActivityKind::Success,
                format!("Moved the pointer by ({dx}, {dy})."),
            ),
            Err(error) => set_activity(&self.shared, ActivityKind::Error, error),
        }
        if let Some(response) = response {
            if let Err(error) = self.enqueue_message(&response) {
                self.report_error(error);
            }
        }
        emit_state(&self.app, &self.shared);
    }

    fn handle_mouse_click(&mut self, command: MouseClickCommand) {
        let injection = self.inject_pointer_click(command.button, command.click_count);
        let response = {
            self.shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .engine
                .complete_mouse_click_command(
                    &command,
                    injection
                        .as_ref()
                        .map(|_| ())
                        .map_err(|error| error.as_str()),
                )
        };
        match injection {
            Ok(()) => {
                let button = match command.button {
                    MouseButton::Left => "left",
                    MouseButton::Middle => "middle",
                    MouseButton::Right => "right",
                };
                let action = if command.click_count == 2 {
                    "Double-clicked"
                } else {
                    "Clicked"
                };
                set_activity(
                    &self.shared,
                    ActivityKind::Success,
                    format!("{action} the {button} mouse button."),
                );
            }
            Err(error) => set_activity(&self.shared, ActivityKind::Error, error),
        }
        if let Some(response) = response {
            if let Err(error) = self.enqueue_message(&response) {
                self.report_error(error);
            }
        }
        emit_state(&self.app, &self.shared);
    }

    fn handle_text(&mut self, command: TextCommand) {
        let character_count = command.text.chars().count();
        let injection = self.inject_text(&command.text);
        let response = {
            self.shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .engine
                .complete_text_command(
                    &command,
                    injection
                        .as_ref()
                        .map(|_| ())
                        .map_err(|error| error.as_str()),
                )
        };
        match injection {
            Ok(()) => set_activity(
                &self.shared,
                ActivityKind::Success,
                format!("Typed {character_count} characters into the focused app."),
            ),
            Err(error) => set_activity(&self.shared, ActivityKind::Error, error),
        }
        if let Some(response) = response {
            if let Err(error) = self.enqueue_message(&response) {
                self.report_error(error);
            }
        }
        emit_state(&self.app, &self.shared);
    }

    fn handle_desktop(&mut self, command: DesktopCommand) {
        let injection = if self.input.is_none() && !self.refresh_accessibility(false) {
            Err("Accessibility permission is required before input can be controlled.".to_string())
        } else {
            self.input
                .as_mut()
                .ok_or_else(|| {
                    "Accessibility permission is required before input can be controlled."
                        .to_string()
                })
                .and_then(|input| input.execute(&command.command_type, &command.payload))
        };
        let response = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .engine
            .complete_desktop_command(
                &command,
                injection.as_ref().map(|_| ()).map_err(String::as_str),
            );
        match injection {
            Ok(()) => set_activity(
                &self.shared,
                ActivityKind::Success,
                format!("Handled {}.", command.command_type),
            ),
            Err(error) => set_activity(&self.shared, ActivityKind::Error, error),
        }
        if let Some(response) = response {
            if let Err(error) = self.enqueue_message(&response) {
                self.report_error(error);
            }
        }
        emit_state(&self.app, &self.shared);
    }

    fn inject_text(&mut self, text: &str) -> Result<(), String> {
        if self.input.is_none() && !self.refresh_accessibility(false) {
            return Err("Accessibility permission is required before text can be typed.".into());
        }
        self.input
            .as_mut()
            .ok_or_else(|| {
                "Accessibility permission is required before text can be typed.".to_string()
            })?
            .type_text(text)
    }

    fn inject_pointer_move(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        if self.input.is_none() && !self.refresh_accessibility(false) {
            return Err("Accessibility permission is required before the pointer can move.".into());
        }
        self.input
            .as_mut()
            .ok_or_else(|| {
                "Accessibility permission is required before the pointer can move.".to_string()
            })?
            .move_pointer(dx, dy)
    }

    fn inject_pointer_click(&mut self, button: MouseButton, click_count: u8) -> Result<(), String> {
        if self.input.is_none() && !self.refresh_accessibility(false) {
            return Err(
                "Accessibility permission is required before the pointer can click.".into(),
            );
        }
        self.input
            .as_mut()
            .ok_or_else(|| {
                "Accessibility permission is required before the pointer can click.".to_string()
            })?
            .click_pointer(button, click_count)
    }

    fn pointer_profile(&self) -> PointerProfile {
        let monitor = self
            .app
            .cursor_position()
            .ok()
            .and_then(|position| {
                self.app
                    .monitor_from_point(position.x, position.y)
                    .ok()
                    .flatten()
            })
            .or_else(|| self.app.primary_monitor().ok().flatten());
        let Some(monitor) = monitor else {
            return pointer_profile_for_display("fallback", 1.0, 0, 0, 1920, 1080);
        };
        let scale_factor = monitor.scale_factor();
        let position = monitor.position();
        let size = monitor.size();
        pointer_profile_for_display(
            monitor.name().map(String::as_str).unwrap_or("display"),
            scale_factor,
            position.x,
            position.y,
            size.width,
            size.height,
        )
    }

    fn refresh_accessibility(&mut self, prompt: bool) -> bool {
        let settings = Settings {
            open_prompt_to_get_permissions: prompt,
            ..Settings::default()
        };
        self.input = Enigo::new(&settings).ok().map(DesktopInput::new);
        let granted = self.input.is_some();
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .accessibility = if granted {
            AccessibilityState::Granted
        } else {
            AccessibilityState::Required
        };
        granted
    }

    fn schedule_pairing_expiration(&self, request_id: String, delay_ms: u64) {
        let app = self.app.clone();
        let shared = self.shared.clone();
        let _task = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            let callback_app = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = expire_pairing(&callback_app, &shared, &request_id);
            });
        });
    }

    fn enqueue_message(&mut self, message: &str) -> Result<(), String> {
        let maximum_notification_bytes = self
            .subscribers
            .values()
            .copied()
            .min()
            .ok_or_else(|| "No Android device is subscribed for notifications.".to_string())?;
        self.outbound.push_notification_message(
            message,
            MAX_QUEUED_NOTIFICATIONS,
            maximum_notification_bytes,
        )?;
        self.flush_outbound()
    }

    fn flush_outbound(&mut self) -> Result<(), String> {
        let Some(characteristic) = self.tx_characteristic.as_ref() else {
            return Ok(());
        };
        self.outbound.flush(|frame| {
            self.manager
                .update_value(frame, characteristic, None)
                .map_err(|error| error.to_string())
        })
    }

    fn set_bluetooth(&self, state: BluetoothState) {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .bluetooth = state;
    }

    fn report_error(&self, message: String) {
        set_activity(&self.shared, ActivityKind::Error, message);
        emit_state(&self.app, &self.shared);
    }

    fn reset_gatt(&mut self) {
        self.manager.stop_advertising();
        self.service = None;
        self.tx_characteristic = None;
        self.subscribers.clear();
        self.outbound.clear();
    }
}

fn pointer_profile_for_display(
    name: &str,
    scale_factor: f64,
    physical_x: i32,
    physical_y: i32,
    physical_width: u32,
    physical_height: u32,
) -> PointerProfile {
    let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    let logical_x = (f64::from(physical_x) / scale_factor).round() as i32;
    let logical_y = (f64::from(physical_y) / scale_factor).round() as i32;
    let logical_width = (f64::from(physical_width) / scale_factor)
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32;
    let logical_height = (f64::from(physical_height) / scale_factor)
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32;
    let short_edge = logical_width.min(logical_height);
    let recommended_delta = |fraction: f64| {
        (f64::from(short_edge) * fraction)
            .round()
            .clamp(1.0, MAX_POINTER_DELTA) as u32
    };

    PointerProfile {
        display_id: format!(
            "{name}:{logical_x}:{logical_y}:{logical_width}:{logical_height}:{scale_factor}"
        ),
        scale_factor,
        x: logical_x,
        y: logical_y,
        width: logical_width,
        height: logical_height,
        small_delta: recommended_delta(0.0225),
        medium_delta: recommended_delta(0.06),
        large_delta: recommended_delta(0.13),
    }
}

fn expire_pairing(app: &AppHandle, shared: &SharedModel, request_id: &str) -> Result<(), String> {
    with_runtime(|runtime| {
        let response = {
            let mut model = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let response = model.engine.expire_pairing(request_id, now_ms());
            let pending = model.engine.pending_pairing();
            model.state.pending_pairing = pending;
            response
        };
        let Some(response) = response else {
            return Ok(());
        };
        runtime.enqueue_message(&response)?;
        set_activity(shared, ActivityKind::Info, "Pairing request expired.");
        emit_state(app, shared);
        Ok(())
    })
}

impl Drop for MacRuntime {
    fn drop(&mut self) {
        self.manager.stop_advertising();
        self.manager.remove_all_services();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_profile_uses_logical_bounds_and_reduced_movement_steps() {
        let profile = pointer_profile_for_display("Retina", 2.0, 0, 0, 3840, 2160);
        assert_eq!((profile.width, profile.height), (1920, 1080));
        assert_eq!(
            (
                profile.small_delta,
                profile.medium_delta,
                profile.large_delta
            ),
            (24, 65, 140)
        );
        assert!(profile.display_id.starts_with("Retina:0:0:1920:1080:2"));
    }

    #[test]
    fn pointer_profile_caps_recommended_steps_at_protocol_maximum() {
        let profile = pointer_profile_for_display("Large", 1.0, 0, 0, 10_000, 10_000);
        assert_eq!(profile.large_delta, MAX_POINTER_DELTA as u32);
    }
}
