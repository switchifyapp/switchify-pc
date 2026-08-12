use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use corebluetooth::prelude::*;
use enigo::{Enigo, Settings};
use objc2_app_kit::NSWorkspace;
#[allow(deprecated)]
use objc2_foundation::{NSHost, NSString, NSURL};
use tauri::{AppHandle, Manager};

use crate::display_navigation::{self, NavigationError};
use crate::input::{DesktopInput, PointerFeedback};
use crate::modifier_overlay::ModifierOverlay;
use crate::mouse_repeat::{MouseRepeatController, RepeatCommand, MOVE_TICK_INTERVAL_MS};
use crate::overlay::CursorOverlay;
use crate::protocol::{
    bluetooth_status_payload, pointer_profile_response, switch_profile_catalog_response,
    DesktopCommand, EngineEvent, MouseButton, MouseClickCommand, MouseMoveCommand, OutboundQueue,
    PointerProfile, TextCommand, MAX_POINTER_DELTA,
};
use crate::state::{
    emit_state, now_ms, set_activity, AccessibilityState, ActivityKind, BluetoothState, SharedModel,
};

const SERVICE_UUID: &str = "7a78f7e8-1d6d-4d92-9ef0-1f89d3db21f4";
const RX_UUID: &str = "7a78f7e9-1d6d-4d92-9ef0-1f89d3db21f4";
const TX_UUID: &str = "7a78f7ea-1d6d-4d92-9ef0-1f89d3db21f4";
const STATUS_UUID: &str = "7a78f7eb-1d6d-4d92-9ef0-1f89d3db21f4";
const FALLBACK_DISPLAY_NAME: &str = "Switchify PC";
const MAX_QUEUED_NOTIFICATIONS: usize = 512;
const ACCESSIBILITY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

trait AccessibilityAdapter {
    type Input;

    fn create_input(&mut self) -> Result<Self::Input, String>;
    fn request_prompt(&mut self);
    fn open_settings(&mut self) -> Result<(), String>;
}

struct SystemAccessibilityAdapter;

impl AccessibilityAdapter for SystemAccessibilityAdapter {
    type Input = Enigo;

    fn create_input(&mut self) -> Result<Self::Input, String> {
        Enigo::new(&Settings {
            open_prompt_to_get_permissions: false,
            ..Settings::default()
        })
        .map_err(|error| error.to_string())
    }

    fn request_prompt(&mut self) {
        let _ = Enigo::new(&Settings {
            open_prompt_to_get_permissions: true,
            ..Settings::default()
        });
    }

    fn open_settings(&mut self) -> Result<(), String> {
        let url = NSURL::URLWithString(&NSString::from_str(ACCESSIBILITY_SETTINGS_URL))
            .ok_or_else(|| "Could not create the Accessibility Settings URL.".to_string())?;
        if NSWorkspace::sharedWorkspace().openURL(&url) {
            Ok(())
        } else {
            Err("Could not open Privacy & Security → Accessibility.".to_string())
        }
    }
}

#[derive(Debug)]
enum AccessibilityCheck<I> {
    Granted(I),
    Required,
}

fn evaluate_accessibility<A: AccessibilityAdapter>(
    adapter: &mut A,
    prompt: bool,
) -> Result<AccessibilityCheck<A::Input>, String> {
    match adapter.create_input() {
        Ok(input) => Ok(AccessibilityCheck::Granted(input)),
        Err(_) if !prompt => Ok(AccessibilityCheck::Required),
        Err(_) => {
            // Apple's prompt is asynchronous: requesting it cannot make this check succeed.
            adapter.request_prompt();
            adapter.open_settings()?;
            Ok(AccessibilityCheck::Required)
        }
    }
}

fn resolved_display_name(localized_name: Option<&str>) -> String {
    localized_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(FALLBACK_DISPLAY_NAME)
        .to_owned()
}

#[allow(deprecated)]
fn system_display_name() -> String {
    let localized_name = NSHost::currentHost()
        .localizedName()
        .map(|name| name.to_string());
    resolved_display_name(localized_name.as_deref())
}

thread_local! {
    static RUNTIME: RefCell<Option<MacRuntime>> = const { RefCell::new(None) };
}

pub fn install(app: AppHandle, shared: SharedModel) -> Result<(), String> {
    let display_name = system_display_name();
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
            display_name,
            manager,
            service: None,
            tx_characteristic: None,
            status_value: Vec::new(),
            subscribers: HashMap::new(),
            pairing_centrals: PairingCentralRegistry::default(),
            outbound: OutboundQueue::default(),
            input: None,
            repeats: MouseRepeatController::default(),
        });
    });
    with_runtime(|runtime| {
        runtime.refresh_accessibility(false)?;
        runtime.handle_manager_state(state)
    })
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
        let was_required = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .accessibility
            == AccessibilityState::Required;
        let granted = match runtime.refresh_accessibility(prompt) {
            Ok(granted) => granted,
            Err(error) => {
                set_activity(shared, ActivityKind::Error, error.clone());
                emit_state(app, shared);
                return Err(error);
            }
        };
        if granted && was_required {
            set_activity(
                shared,
                ActivityKind::Success,
                "Accessibility access is ready.",
            );
        } else if prompt && !granted {
            set_activity(
                shared,
                ActivityKind::Info,
                "Enable Switchify PC in Accessibility, then return here. If it is already enabled but access is still required, remove the stale row and reopen Accessibility Settings from Switchify.",
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
            model.state.pending_pairings = model.engine.pending_pairings();
            response
        }?;
        runtime.pairing_centrals.remove(request_id);
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
            model.state.pending_pairings = model.engine.pending_pairings();
            response
        }?;
        runtime.pairing_centrals.remove(request_id);
        runtime.enqueue_message(&response)?;
        set_activity(shared, ActivityKind::Info, "Pairing request rejected.");
        emit_state(app, shared);
        Ok(())
    })
}

pub fn disconnect_all(app: &AppHandle, shared: &SharedModel) -> Result<(), String> {
    with_runtime(|runtime| {
        runtime.stop_all_repeats();
        let release = if let Some(input) = runtime.input.as_mut() {
            let release = input.release_all();
            input.end_control_session();
            release
        } else {
            Ok(())
        };
        runtime.subscribers.clear();
        runtime.pairing_centrals.clear();
        runtime.outbound.clear();
        {
            let mut model = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            model.engine.cancel_all_pairings();
            model.state.pending_pairings.clear();
            model.state.bluetooth = BluetoothState::Advertising;
            model.state.connected_device_name = None;
        }
        emit_state(app, shared);
        release
    })
}

pub fn stop_mouse_repeat(app: &AppHandle) {
    dispatch_to_main(app, |runtime| {
        runtime.stop_all_repeats();
        Ok(())
    });
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
    display_name: String,
    manager: PeripheralManager,
    service: Option<MutableService>,
    tx_characteristic: Option<MutableCharacteristic>,
    status_value: Vec<u8>,
    subscribers: HashMap<String, usize>,
    pairing_centrals: PairingCentralRegistry,
    outbound: OutboundQueue,
    input: Option<DesktopInput<Enigo>>,
    repeats: MouseRepeatController,
}

#[derive(Debug, Default)]
struct PairingCentralRegistry {
    by_request: HashMap<String, String>,
}

impl PairingCentralRegistry {
    fn associate(&mut self, request_id: String, central_id: String) {
        self.by_request.insert(request_id, central_id);
    }

    fn retain_pending(&mut self, pending_request_ids: &HashSet<String>) {
        self.by_request
            .retain(|request_id, _| pending_request_ids.contains(request_id));
    }

    fn remove(&mut self, request_id: &str) {
        self.by_request.remove(request_id);
    }

    fn take_for_central(&mut self, central_id: &str) -> Vec<String> {
        let request_ids = self
            .by_request
            .iter()
            .filter(|(_, associated_central)| associated_central.as_str() == central_id)
            .map(|(request_id, _)| request_id.clone())
            .collect::<Vec<_>>();
        self.by_request
            .retain(|_, associated_central| associated_central != central_id);
        request_ids
    }

    fn clear(&mut self) {
        self.by_request.clear();
    }
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
                self.set_bluetooth(BluetoothState::Unsupported);
                self.shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .state
                    .accessibility = AccessibilityState::Unavailable;
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

        let (desktop_id, platform) = {
            let model = self
                .shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            (
                model.engine.desktop_id().to_owned(),
                model.state.capabilities.platform.clone(),
            )
        };
        self.status_value = bluetooth_status_payload(&self.display_name, &desktop_id, &platform)?;
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
            .with_local_name(FALLBACK_DISPLAY_NAME)
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
            let central_id = central.identifier();
            self.subscribers.remove(&central_id);
            let request_ids = self.pairing_centrals.take_for_central(&central_id);
            let cancelled = if request_ids.is_empty() {
                0
            } else {
                let mut model = self
                    .shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let cancelled = request_ids
                    .iter()
                    .filter(|request_id| model.engine.cancel_pairing(request_id))
                    .count();
                model.state.pending_pairings = model.engine.pending_pairings();
                cancelled
            };
            if self.subscribers.is_empty() {
                self.stop_all_repeats();
                self.outbound.clear();
                if let Some(input) = self.input.as_mut() {
                    let _ = input.release_all();
                    input.end_control_session();
                }
                self.app.state::<CursorOverlay>().end_session();
                self.app.state::<ModifierOverlay>().end_session();
                self.set_bluetooth(BluetoothState::Advertising);
            }
            if cancelled > 0 {
                set_activity(
                    &self.shared,
                    ActivityKind::Info,
                    "Pairing request cancelled.",
                );
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
            let central_id = request.central().identifier();
            let result = if !uuid.eq_ignore_ascii_case(RX_UUID) {
                AttError::WriteNotPermitted
            } else if request.offset() != 0 {
                AttError::InvalidOffset
            } else {
                match request.value() {
                    Ok(Some(value)) => {
                        self.handle_frame(&central_id, &value);
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

    fn handle_frame(&mut self, central_id: &str, bytes: &[u8]) {
        let event = {
            self.shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .engine
                .receive_frame(bytes, now_ms())
        };
        match event {
            Ok(None) => {}
            Ok(Some(EngineEvent::PendingPairing {
                request,
                replaced_response,
            })) => {
                let request_id = request.request_id.clone();
                let delay_ms = request.expires_at.saturating_sub(now_ms()) as u64;
                let pending_request_ids = {
                    let mut model = self
                        .shared
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    model.state.pending_pairings = model.engine.pending_pairings();
                    model
                        .state
                        .pending_pairings
                        .iter()
                        .map(|pending| pending.request_id.clone())
                        .collect::<HashSet<_>>()
                };
                self.pairing_centrals.retain_pending(&pending_request_ids);
                self.pairing_centrals
                    .associate(request_id.clone(), central_id.to_string());
                if let Some(response) = replaced_response {
                    if let Err(error) = self.enqueue_message(&response) {
                        self.report_error(error);
                    }
                }
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
        let settings = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .settings
            .clone();
        let response = pointer_profile_response(id, &self.pointer_profile(), &settings);
        if let Err(error) = self.enqueue_message(&response) {
            self.report_error(error);
        }
    }

    fn handle_mouse_move(&mut self, command: MouseMoveCommand) {
        self.stop_all_repeats();
        let dx = command.dx.round() as i32;
        let dy = command.dy.round() as i32;
        let injection = self.inject_pointer_move(dx, dy);
        if injection.is_ok() {
            let feedback = self
                .input
                .as_ref()
                .map(DesktopInput::pointer_feedback_for_move)
                .unwrap_or(PointerFeedback::Move);
            self.show_overlay(feedback);
        }
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
        self.stop_all_repeats();
        let injection = self.inject_pointer_click(command.button, command.click_count);
        if injection.is_ok() {
            self.show_overlay(PointerFeedback::Click {
                button: command.button,
                count: command.click_count,
            });
        }
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
        self.stop_all_repeats();
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
        if command.command_type == "mouse.repeat.start" {
            self.handle_repeat_start(command);
            return;
        }
        if command.command_type == "mouse.repeat.stop" {
            self.handle_repeat_stop(command);
            return;
        }
        self.stop_all_repeats();
        let profiles = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .profiles
            .clone();
        if command.command_type == "switch.profile.list" {
            if let Err(error) =
                self.enqueue_message(&switch_profile_catalog_response(&command.id, &profiles))
            {
                self.report_error(error);
            }
            return;
        }
        let (injection, error_code) = if command.command_type == "pointer.display.move" {
            let direction = command.payload["direction"].as_str().unwrap_or_default();
            match self.navigate_display(direction) {
                Ok(feedback) => (Ok(Some(feedback)), "input_failed"),
                Err(error) => (Err(error.message), error.code),
            }
        } else if self.input.is_none() && !self.refresh_accessibility(false).unwrap_or(false) {
            (
                Err(
                    "Accessibility permission is required before input can be controlled."
                        .to_string(),
                ),
                "input_failed",
            )
        } else {
            (
                self.input
                    .as_mut()
                    .ok_or_else(|| {
                        "Accessibility permission is required before input can be controlled."
                            .to_string()
                    })
                    .and_then(|input| {
                        input.execute(
                            &command.device_id,
                            &command.command_type,
                            &command.payload,
                            &profiles,
                        )
                    }),
                "input_failed",
            )
        };
        let response = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .engine
            .complete_desktop_command_with_error(
                &command,
                injection
                    .as_ref()
                    .map(|_| ())
                    .map_err(|error| (error_code, error.as_str())),
            );
        if let Ok(feedback) = &injection {
            let settings = self.overlay_settings();
            let overlay = self.app.state::<CursorOverlay>();
            if let Some(feedback) = feedback {
                overlay.show(*feedback, settings);
            } else {
                overlay.mark_control_active(settings);
            }
            if command.command_type == "connection.disconnecting" {
                overlay.end_session();
            }
        }
        if injection.is_ok() && command.command_type == "pointer.speed.set" {
            if let Some(scale) = command
                .payload
                .get("scalePercent")
                .and_then(serde_json::Value::as_f64)
            {
                self.shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .state
                    .settings
                    .pointer_scale_percent = ((scale / 5.0).round() * 5.0).clamp(5.0, 225.0) as u8;
            }
        }
        match injection {
            Ok(_) => set_activity(
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

    fn handle_repeat_start(&mut self, command: DesktopCommand) {
        let settings = self.overlay_settings();
        let parsed = RepeatCommand::parse(&command.payload);
        let injection = parsed.and_then(|repeat_command| {
            if !settings.mouse_repeat_enabled {
                self.stop_repeat_for_device(&command.device_id);
                return Err("Mouse repeat is disabled in settings.".into());
            }
            self.stop_repeat_for_device(&command.device_id);
            if self.input.is_none() && !self.refresh_accessibility(false)? {
                return Err(
                    "Accessibility permission is required before the pointer can move.".into(),
                );
            }
            let active = self.repeats.start(
                command.device_id.clone(),
                repeat_command,
                settings.mouse_repeat_acceleration_duration_ms,
                Instant::now(),
            );
            let initial = match repeat_command {
                RepeatCommand::Move { .. } => {
                    let (dx, dy) = self
                        .repeats
                        .initial_move(&command.device_id, active.generation)
                        .unwrap_or((0, 0));
                    match self.input.as_mut() {
                        Some(input) if dx == 0 && dy == 0 => Ok(input.pointer_feedback_for_move()),
                        Some(input) => input.move_pointer_pixels(dx, dy),
                        None => Err(
                            "Accessibility permission is required before the pointer can move."
                                .to_string(),
                        ),
                    }
                }
                RepeatCommand::Scroll { dx, dy } => match self.input.as_mut() {
                    Some(input) => input.execute_repeat_scroll(dx, dy),
                    None => Err(
                        "Accessibility permission is required before input can be controlled."
                            .to_string(),
                    ),
                },
            };
            let initial_feedback = match initial {
                Ok(feedback) => feedback,
                Err(error) => {
                    self.repeats
                        .stop_if_current(&command.device_id, active.generation);
                    return Err(error);
                }
            };
            self.app.state::<CursorOverlay>().begin_repeat(
                active.generation,
                repeat_command,
                settings.mouse_repeat_acceleration_duration_ms > 0,
                matches!(initial_feedback, PointerFeedback::Drag),
                settings.clone(),
            );
            self.schedule_repeat_tick(
                command.device_id.clone(),
                active.generation,
                match repeat_command {
                    RepeatCommand::Move { .. } => MOVE_TICK_INTERVAL_MS,
                    RepeatCommand::Scroll { .. } => {
                        u64::from(repeat_command.interval_ms(&settings))
                    }
                },
            );
            Ok(())
        });
        self.complete_repeat_command(command, injection);
    }

    fn handle_repeat_stop(&mut self, command: DesktopCommand) {
        self.stop_repeat_for_device(&command.device_id);
        self.complete_repeat_command(command, Ok(()));
    }

    fn complete_repeat_command(&mut self, command: DesktopCommand, result: Result<(), String>) {
        let response = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .engine
            .complete_desktop_command(
                &command,
                result.as_ref().map(|_| ()).map_err(String::as_str),
            );
        match result {
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

    fn schedule_repeat_tick(&self, device_id: String, generation: u64, interval_ms: u64) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
            let callback_app = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = with_runtime(|runtime| runtime.repeat_tick(device_id, generation));
                drop(callback_app);
            });
        });
    }

    fn repeat_tick(&mut self, device_id: String, generation: u64) -> Result<(), String> {
        let settings = self.overlay_settings();
        let Some(active) = self.repeats.current(&device_id, generation) else {
            return Ok(());
        };
        if !settings.mouse_repeat_enabled {
            self.stop_repeat_if_current(&device_id, generation);
            return Ok(());
        }
        let movement = match active.command {
            RepeatCommand::Move { .. } => self.repeats.advance_move(
                &device_id,
                generation,
                Instant::now(),
                settings.move_repeat_interval_ms,
                settings.pointer_scale_percent,
            ),
            RepeatCommand::Scroll { .. } => None,
        };
        let result = self
            .input
            .as_mut()
            .ok_or_else(|| {
                "Accessibility permission is required before input can be controlled.".to_string()
            })
            .and_then(|input| match active.command {
                RepeatCommand::Move { .. } => match movement {
                    Some((0, 0)) => Ok(()),
                    Some((dx, dy)) => input.move_pointer_pixels(dx, dy).map(|_| ()),
                    None => Ok(()),
                },
                RepeatCommand::Scroll { dx, dy } => input.execute_repeat_scroll(dx, dy).map(|_| ()),
            });
        if let Err(error) = result {
            if self.stop_repeat_if_current(&device_id, generation) {
                self.report_error(format!("Mouse repeat stopped: {error}"));
            }
            return Ok(());
        }
        let delay_ms = match active.command {
            RepeatCommand::Move { .. } => MOVE_TICK_INTERVAL_MS,
            RepeatCommand::Scroll { .. } => u64::from(active.command.interval_ms(&settings)),
        };
        self.schedule_repeat_tick(device_id, generation, delay_ms);
        Ok(())
    }

    fn stop_repeat_if_current(&mut self, device_id: &str, generation: u64) -> bool {
        let stopped = self.repeats.stop_if_current(device_id, generation);
        if stopped {
            self.app.state::<CursorOverlay>().end_repeat(generation);
        }
        stopped
    }

    fn stop_repeat_for_device(&mut self, device_id: &str) {
        if let Some(active) = self.repeats.stop(device_id) {
            self.app
                .state::<CursorOverlay>()
                .end_repeat(active.generation);
        }
    }

    fn stop_all_repeats(&mut self) {
        for active in self.repeats.stop_all() {
            self.app
                .state::<CursorOverlay>()
                .end_repeat(active.generation);
        }
    }

    fn inject_text(&mut self, text: &str) -> Result<(), String> {
        if self.input.is_none() && !self.refresh_accessibility(false)? {
            return Err("Accessibility permission is required before text can be typed.".into());
        }
        self.input
            .as_mut()
            .ok_or_else(|| {
                "Accessibility permission is required before text can be typed.".to_string()
            })?
            .type_text(text)
    }

    fn overlay_settings(&self) -> crate::state::AppSettings {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .settings
            .clone()
    }

    fn show_overlay(&self, feedback: PointerFeedback) {
        let settings = self.overlay_settings();
        let overlay = self.app.state::<CursorOverlay>();
        overlay.show(feedback, settings);
    }

    fn inject_pointer_move(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        if self.input.is_none() && !self.refresh_accessibility(false)? {
            return Err("Accessibility permission is required before the pointer can move.".into());
        }
        let scale = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .settings
            .pointer_scale_percent;
        let input = self.input.as_mut().ok_or_else(|| {
            "Accessibility permission is required before the pointer can move.".to_string()
        })?;
        input.set_pointer_scale_percent(scale);
        input.move_pointer(dx, dy)
    }

    fn inject_pointer_click(&mut self, button: MouseButton, click_count: u8) -> Result<(), String> {
        if self.input.is_none() && !self.refresh_accessibility(false)? {
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
        let Ok((cursor, displays)) = display_navigation::displays(&self.app) else {
            return pointer_profile_for_display("fallback", 1.0, 0, 0, 1920, 1080, 1);
        };
        let Some(monitor) = display_navigation::current_display(cursor, &displays) else {
            return pointer_profile_for_display("fallback", 1.0, 0, 0, 1920, 1080, 1);
        };
        pointer_profile_for_display(
            &monitor.name,
            monitor.scale_factor,
            monitor.x,
            monitor.y,
            monitor.width,
            monitor.height,
            displays.len().clamp(1, 64) as u8,
        )
    }

    fn navigate_display(&mut self, direction: &str) -> Result<PointerFeedback, NavigationError> {
        if self.input.is_none() && !self.refresh_accessibility(false).unwrap_or(false) {
            return Err(NavigationError {
                code: "adapter_failure",
                message: "Accessibility permission is required before the pointer can move.".into(),
            });
        }
        let input = self.input.as_mut().ok_or_else(|| NavigationError {
            code: "adapter_failure",
            message: "Accessibility permission is required before the pointer can move.".into(),
        })?;
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
        let (cursor, displays) = display_navigation::displays(&self.app)?;
        let source = display_navigation::current_display(cursor, &displays).ok_or_else(|| {
            NavigationError {
                code: "adapter_failure",
                message: "The active monitor could not be resolved.".into(),
            }
        })?;
        let (x, y) = display_navigation::target_center(source, &displays, direction)?;
        input
            .move_pointer_absolute(x, y)
            .map_err(|_| NavigationError {
                code: "adapter_failure",
                message: "The operating system could not move the pointer to another monitor."
                    .into(),
            })
    }

    fn refresh_accessibility(&mut self, prompt: bool) -> Result<bool, String> {
        if let Some(input) = self.input.as_mut() {
            let release = input.release_all();
            input.end_control_session();
            release?;
        }
        let mut adapter = SystemAccessibilityAdapter;
        let check = evaluate_accessibility(&mut adapter, prompt);
        self.input = match check {
            Ok(AccessibilityCheck::Granted(input)) => Some(DesktopInput::with_modifier_overlay(
                input,
                self.app.state::<ModifierOverlay>().notifier(),
            )),
            Ok(AccessibilityCheck::Required) => None,
            Err(error) => {
                self.set_accessibility_state(AccessibilityState::Required);
                return Err(error);
            }
        };
        let granted = self.input.is_some();
        self.set_accessibility_state(if granted {
            AccessibilityState::Granted
        } else {
            AccessibilityState::Required
        });
        Ok(granted)
    }

    fn set_accessibility_state(&self, state: AccessibilityState) {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
            .accessibility = state;
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
        self.stop_all_repeats();
        self.manager.stop_advertising();
        self.service = None;
        self.tx_characteristic = None;
        self.subscribers.clear();
        self.pairing_centrals.clear();
        self.outbound.clear();
        {
            let mut model = self
                .shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            model.engine.cancel_all_pairings();
            model.state.pending_pairings.clear();
        }
        if let Some(input) = self.input.as_mut() {
            let _ = input.release_all();
            input.end_control_session();
        }
        self.app.state::<CursorOverlay>().end_session();
        self.app.state::<ModifierOverlay>().end_session();
    }
}

fn pointer_profile_for_display(
    name: &str,
    scale_factor: f64,
    physical_x: i32,
    physical_y: i32,
    physical_width: u32,
    physical_height: u32,
    display_count: u8,
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
        display_count,
    }
}

fn expire_pairing(app: &AppHandle, shared: &SharedModel, request_id: &str) -> Result<(), String> {
    with_runtime(|runtime| {
        let response = {
            let mut model = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let response = model.engine.expire_pairing(request_id, now_ms());
            model.state.pending_pairings = model.engine.pending_pairings();
            response
        };
        let Some(response) = response else {
            runtime.pairing_centrals.remove(request_id);
            return Ok(());
        };
        runtime.pairing_centrals.remove(request_id);
        runtime.enqueue_message(&response)?;
        set_activity(shared, ActivityKind::Info, "Pairing request expired.");
        emit_state(app, shared);
        Ok(())
    })
}

impl Drop for MacRuntime {
    fn drop(&mut self) {
        if let Some(input) = self.input.as_mut() {
            let _ = input.release_all();
            input.end_control_session();
        }
        self.manager.stop_advertising();
        self.manager.remove_all_services();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeAccessibilityAdapter {
        input_available: bool,
        prompt_requests: usize,
        settings_opens: usize,
        settings_error: Option<String>,
    }

    impl AccessibilityAdapter for FakeAccessibilityAdapter {
        type Input = ();

        fn create_input(&mut self) -> Result<Self::Input, String> {
            if self.input_available {
                Ok(())
            } else {
                Err("not trusted".into())
            }
        }

        fn request_prompt(&mut self) {
            self.prompt_requests += 1;
        }

        fn open_settings(&mut self) -> Result<(), String> {
            self.settings_opens += 1;
            match self.settings_error.as_ref() {
                Some(error) => Err(error.clone()),
                None => Ok(()),
            }
        }
    }

    #[test]
    fn startup_and_focus_checks_are_silent_when_access_is_required() {
        let mut adapter = FakeAccessibilityAdapter::default();
        assert!(matches!(
            evaluate_accessibility(&mut adapter, false).unwrap(),
            AccessibilityCheck::Required
        ));
        assert_eq!(adapter.prompt_requests, 0);
        assert_eq!(adapter.settings_opens, 0);

        adapter.input_available = true;
        assert!(matches!(
            evaluate_accessibility(&mut adapter, false).unwrap(),
            AccessibilityCheck::Granted(())
        ));
        assert_eq!(adapter.prompt_requests, 0);
        assert_eq!(adapter.settings_opens, 0);
    }

    #[test]
    fn prompt_checks_trust_before_requesting_and_opening_settings() {
        let mut trusted = FakeAccessibilityAdapter {
            input_available: true,
            ..Default::default()
        };
        assert!(matches!(
            evaluate_accessibility(&mut trusted, true).unwrap(),
            AccessibilityCheck::Granted(())
        ));
        assert_eq!((trusted.prompt_requests, trusted.settings_opens), (0, 0));

        let mut required = FakeAccessibilityAdapter::default();
        assert!(matches!(
            evaluate_accessibility(&mut required, true).unwrap(),
            AccessibilityCheck::Required
        ));
        assert_eq!((required.prompt_requests, required.settings_opens), (1, 1));
    }

    #[test]
    fn settings_open_failures_do_not_report_access_as_granted() {
        let mut adapter = FakeAccessibilityAdapter {
            settings_error: Some("settings unavailable".into()),
            ..Default::default()
        };
        assert_eq!(
            evaluate_accessibility(&mut adapter, true).unwrap_err(),
            "settings unavailable"
        );
        assert_eq!((adapter.prompt_requests, adapter.settings_opens), (1, 1));
    }

    #[test]
    fn localized_computer_name_is_trimmed_and_preserves_unicode() {
        assert_eq!(
            resolved_display_name(Some("  Owen’s Mac Studio  ")),
            "Owen’s Mac Studio"
        );
    }

    #[test]
    fn missing_or_blank_computer_name_uses_product_fallback() {
        assert_eq!(resolved_display_name(None), FALLBACK_DISPLAY_NAME);
        assert_eq!(resolved_display_name(Some(" \n\t ")), FALLBACK_DISPLAY_NAME);
    }

    #[test]
    fn bluetooth_status_uses_resolved_computer_name() {
        let display_name = resolved_display_name(Some("Owen’s Mac Studio"));
        let payload = bluetooth_status_payload(&display_name, "desktop-1", "macos").unwrap();
        let status: serde_json::Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(status["displayName"], "Owen’s Mac Studio");
        assert_eq!(status["desktopId"], "desktop-1");
        assert_eq!(status["platform"], "macos");
    }

    #[test]
    fn pointer_profile_uses_logical_bounds_and_reduced_movement_steps() {
        let profile = pointer_profile_for_display("Retina", 2.0, 0, 0, 3840, 2160, 2);
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
        let profile = pointer_profile_for_display("Large", 1.0, 0, 0, 10_000, 10_000, 1);
        assert_eq!(profile.large_delta, MAX_POINTER_DELTA as u32);
    }

    #[test]
    fn pairing_central_registry_cancels_only_the_disconnected_central() {
        let mut registry = PairingCentralRegistry::default();
        registry.associate("pair-a-old".into(), "central-a".into());
        registry.associate("pair-a".into(), "central-a".into());
        registry.associate("pair-b".into(), "central-b".into());

        registry.retain_pending(&HashSet::from(["pair-a".to_string(), "pair-b".to_string()]));
        let mut cancelled = registry.take_for_central("central-a");
        cancelled.sort();

        assert_eq!(cancelled, ["pair-a"]);
        assert!(registry.take_for_central("unknown").is_empty());
        assert_eq!(registry.take_for_central("central-b"), ["pair-b"]);
    }
}
