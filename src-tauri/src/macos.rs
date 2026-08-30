use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use block2::RcBlock;
use corebluetooth::prelude::*;
use enigo::{Enigo, Settings};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillSleepNotification,
};
#[allow(deprecated)]
use objc2_foundation::{NSHost, NSNotification, NSNotificationCenter, NSString, NSURL};
use tauri::{AppHandle, Manager};

use crate::ble_lifecycle::{RecoveryCoordinator, RECOVERY_DELAYS};
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
    bluetooth_status_payload, pointer_profile_response, switch_profile_catalog_response,
    DesktopCommand, EngineEvent, MouseButton, MouseClickCommand, MouseMoveCommand, OutboundQueue,
    PointerProfile, TextCommand, MAX_POINTER_DELTA,
};
use crate::state::{
    emit_state, now_ms, set_activity, AccessibilityState, ActivityKind, AppModel, BluetoothState,
    SharedModel,
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
    let lifecycle = Arc::new(Mutex::new(RecoveryCoordinator::default()));
    lifecycle
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .begin_initial();
    let display_name = system_display_name();
    let manager_generation = 1;
    let manager = create_peripheral_manager(&app, lifecycle.clone(), manager_generation)?;
    let state = manager.state();
    RUNTIME.with(|slot| {
        *slot.borrow_mut() = Some(MacRuntime {
            app,
            shared,
            display_name,
            manager,
            manager_generation,
            service: None,
            rx_characteristic: None,
            tx_characteristic: None,
            status_value: Vec::new(),
            subscribers: HashMap::new(),
            pairing_centrals: PairingCentralRegistry::default(),
            outbound: OutboundQueue::default(),
            input: None,
            repeats: MouseRepeatController::default(),
            pending_repeat_moves: HashMap::new(),
            lifecycle,
            service_generation: None,
            notification_center: None,
            power_observers: Vec::new(),
        });
    });
    with_runtime(|runtime| {
        runtime.install_power_observers();
        runtime.refresh_accessibility(false)?;
        runtime.handle_manager_state(state)?;
        let generation = runtime
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .current_generation();
        if runtime
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .should_retry(generation)
        {
            runtime.schedule_recovery(generation);
        }
        Ok(())
    })
}

fn create_peripheral_manager(
    app: &AppHandle,
    lifecycle: Arc<Mutex<RecoveryCoordinator>>,
    manager_generation: u64,
) -> Result<PeripheralManager, String> {
    let state_app = app.clone();
    let service_app = app.clone();
    let advertising_app = app.clone();
    let subscribe_app = app.clone();
    let unsubscribe_app = app.clone();
    let ready_app = app.clone();
    let read_app = app.clone();
    let write_app = app.clone();
    let write_lifecycle = lifecycle;
    let callbacks = PeripheralManagerCallbacks::new()
        .on_state(move |state, _authorization| {
            dispatch_to_main(&state_app, move |runtime| {
                if runtime.manager_generation == manager_generation {
                    runtime.handle_manager_state(state)
                } else {
                    Ok(())
                }
            });
        })
        .on_add_service(move |service, error| {
            let service = MainThreadBluetoothValue(service);
            dispatch_to_main(&service_app, move |runtime| {
                let service = service.into_inner();
                if runtime.manager_generation == manager_generation {
                    runtime.service_was_added(&service, error.is_none())
                } else {
                    Ok(())
                }
            });
        })
        .on_start_advertising(move |error| {
            dispatch_to_main(&advertising_app, move |runtime| {
                if runtime.manager_generation == manager_generation {
                    runtime.advertising_did_start(error.is_none())
                } else {
                    Ok(())
                }
            });
        })
        .on_subscribe(move |central, characteristic| {
            let values = MainThreadBluetoothValue((central, characteristic));
            dispatch_to_main(&subscribe_app, move |runtime| {
                let (central, characteristic) = values.into_inner();
                if runtime.manager_generation == manager_generation {
                    runtime.central_subscribed(central, characteristic)
                } else {
                    Ok(())
                }
            });
        })
        .on_unsubscribe(move |central, characteristic| {
            let values = MainThreadBluetoothValue((central, characteristic));
            dispatch_to_main(&unsubscribe_app, move |runtime| {
                let (central, characteristic) = values.into_inner();
                if runtime.manager_generation == manager_generation {
                    runtime.central_unsubscribed(central, characteristic)
                } else {
                    Ok(())
                }
            });
        })
        .on_ready_to_update(move || {
            dispatch_to_main(&ready_app, move |runtime| {
                if runtime.manager_generation == manager_generation {
                    runtime.flush_outbound()
                } else {
                    Ok(())
                }
            });
        })
        .on_read_request(move |request| {
            let request = MainThreadBluetoothValue(request);
            dispatch_to_main(&read_app, move |runtime| {
                if runtime.manager_generation == manager_generation {
                    runtime.handle_read(request.into_inner())
                } else {
                    Ok(())
                }
            });
        })
        .on_write_requests(move |requests| {
            let generation = write_lifecycle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .current_generation();
            let requests = MainThreadBluetoothValue(requests);
            dispatch_to_main(&write_app, move |runtime| {
                if runtime.manager_generation == manager_generation {
                    runtime.handle_writes(requests.into_inner(), generation)
                } else {
                    Ok(())
                }
            });
        });
    PeripheralManager::with_callbacks(callbacks).map_err(|error| error.to_string())
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
    app.state::<DwellController>().cancel(app);
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

pub fn dwell_click(_app: &AppHandle) -> Result<(), String> {
    with_runtime(|runtime| {
        let input = runtime.input.as_mut().ok_or_else(|| {
            "Accessibility permission is required before the pointer can click.".to_string()
        })?;
        execute_dwell_click(input).map(|_| ())
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
    display_name: String,
    manager: PeripheralManager,
    manager_generation: u64,
    service: Option<MutableService>,
    rx_characteristic: Option<MutableCharacteristic>,
    tx_characteristic: Option<MutableCharacteristic>,
    status_value: Vec<u8>,
    subscribers: HashMap<String, usize>,
    pairing_centrals: PairingCentralRegistry,
    outbound: OutboundQueue,
    input: Option<DesktopInput<Enigo>>,
    repeats: MouseRepeatController,
    pending_repeat_moves: HashMap<u64, PendingRepeatMove>,
    lifecycle: Arc<Mutex<RecoveryCoordinator>>,
    service_generation: Option<u64>,
    notification_center: Option<Retained<NSNotificationCenter>>,
    power_observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

pub fn shutdown(_app: &AppHandle, _shared: &SharedModel) {
    RUNTIME.with(|slot| {
        slot.borrow_mut().take();
    });
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingRepeatMove {
    before: (f64, f64),
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
    fn install_power_observers(&mut self) {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let sleep_app = self.app.clone();
        let sleep_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            dispatch_to_main(&sleep_app, |runtime| runtime.handle_system_suspend());
        });
        let wake_app = self.app.clone();
        let wake_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            dispatch_to_main(&wake_app, |runtime| runtime.handle_system_resume());
        });
        let sleep_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceWillSleepNotification),
                None,
                None,
                &sleep_block,
            )
        };
        let wake_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceDidWakeNotification),
                None,
                None,
                &wake_block,
            )
        };
        self.notification_center = Some(center);
        self.power_observers = vec![sleep_observer, wake_observer];
    }

    fn handle_system_suspend(&mut self) -> Result<(), String> {
        self.lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .suspend();
        self.reset_gatt();
        self.set_bluetooth(BluetoothState::Initializing);
        set_activity(
            &self.shared,
            ActivityKind::Info,
            "Bluetooth paused while the Mac sleeps.",
        );
        emit_state(&self.app, &self.shared);
        Ok(())
    }

    fn handle_system_resume(&mut self) -> Result<(), String> {
        let generation = self
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .resume(Instant::now());
        let Some(generation) = generation else {
            return Ok(());
        };
        self.reset_gatt();
        self.set_bluetooth(BluetoothState::Initializing);
        emit_state(&self.app, &self.shared);
        self.schedule_recovery(generation);
        Ok(())
    }

    fn schedule_recovery(&self, generation: u64) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let started_at = tokio::time::Instant::now();
            for delay in RECOVERY_DELAYS {
                if !delay.is_zero() {
                    tokio::time::sleep_until(started_at + delay).await;
                }
                let callback_app = app.clone();
                let (sender, receiver) = tokio::sync::oneshot::channel();
                if app
                    .run_on_main_thread(move || {
                        let result = with_runtime(|runtime| runtime.recovery_attempt(generation));
                        let _ = sender.send(result);
                    })
                    .is_err()
                {
                    return;
                }
                match receiver.await {
                    Ok(Ok(true)) => {}
                    Ok(Ok(false)) | Err(_) => return,
                    Ok(Err(error)) => {
                        let _ = callback_app.run_on_main_thread(move || {
                            let _ =
                                with_runtime(|runtime| runtime.recovery_failed(generation, error));
                        });
                        return;
                    }
                }
            }
            // Give the final fresh CoreBluetooth request one event-loop turn to report
            // success before declaring the bounded recovery sequence exhausted.
            tokio::time::sleep(Duration::from_secs(1)).await;
            let (sender, receiver) = tokio::sync::oneshot::channel();
            if app
                .run_on_main_thread(move || {
                    let result = with_runtime(|runtime| runtime.recovery_timed_out(generation));
                    let _ = sender.send(result);
                })
                .is_ok()
            {
                let _ = receiver.await;
            }
        });
    }

    fn recovery_attempt(&mut self, generation: u64) -> Result<bool, String> {
        if !self
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .should_retry(generation)
        {
            return Ok(false);
        }
        let state = self.rebuild_peripheral_manager()?;
        match state {
            PeripheralManagerState::PoweredOn => {
                self.configure_service()?;
                Ok(true)
            }
            PeripheralManagerState::PoweredOff => {
                self.recovery_terminal(generation, BluetoothState::PoweredOff);
                Ok(false)
            }
            PeripheralManagerState::Unauthorized => {
                self.recovery_terminal(generation, BluetoothState::Unauthorized);
                Ok(false)
            }
            PeripheralManagerState::Unsupported => {
                self.recovery_terminal(generation, BluetoothState::Unsupported);
                Ok(false)
            }
            PeripheralManagerState::Unknown | PeripheralManagerState::Resetting => Ok(true),
        }
    }

    fn rebuild_peripheral_manager(&mut self) -> Result<PeripheralManagerState, String> {
        self.reset_gatt();
        let generation = self.manager_generation.wrapping_add(1).max(1);
        let manager = create_peripheral_manager(&self.app, self.lifecycle.clone(), generation)?;
        let state = manager.state();
        self.manager = manager;
        self.manager_generation = generation;
        Ok(state)
    }

    fn recovery_timed_out(&mut self, generation: u64) -> Result<bool, String> {
        if !self
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .should_retry(generation)
        {
            return Ok(false);
        }
        self.reset_gatt();
        self.recovery_failed(
            generation,
            "the Bluetooth stack did not become ready within 30 seconds".to_string(),
        )?;
        Ok(false)
    }

    fn recovery_failed(&mut self, generation: u64, error: String) -> Result<(), String> {
        if self
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .mark_terminal(generation)
        {
            self.set_bluetooth(BluetoothState::Error);
            set_activity(
                &self.shared,
                ActivityKind::Error,
                format!("Bluetooth could not recover after resume: {error}"),
            );
            emit_state(&self.app, &self.shared);
        }
        Ok(())
    }

    fn recovery_terminal(&self, generation: u64, state: BluetoothState) {
        if self
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .mark_terminal(generation)
        {
            self.set_bluetooth(state);
            emit_state(&self.app, &self.shared);
        }
    }

    fn handle_manager_state(&mut self, state: PeripheralManagerState) -> Result<(), String> {
        if manager_state_invalidates_gatt(state) {
            self.reset_gatt();
        }
        match state {
            PeripheralManagerState::PoweredOn => {
                let (generation, recovering) = {
                    let mut lifecycle = self
                        .lifecycle
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let generation = lifecycle.recover_from_terminal();
                    let current = lifecycle.current_generation();
                    (generation, lifecycle.should_retry(current))
                };
                if let Some(generation) = generation {
                    self.reset_gatt();
                    self.schedule_recovery(generation);
                } else if recovering && self.service.is_none() {
                    self.configure_service()?;
                }
            }
            PeripheralManagerState::PoweredOff => {
                self.lifecycle
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .interrupt_terminal();
                self.set_bluetooth(BluetoothState::PoweredOff);
            }
            PeripheralManagerState::Unauthorized => {
                self.lifecycle
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .interrupt_terminal();
                self.set_bluetooth(BluetoothState::Unauthorized);
            }
            PeripheralManagerState::Unsupported => {
                self.lifecycle
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .interrupt_terminal();
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
                self.lifecycle
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .interrupt_terminal();
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
        self.service_generation = Some(
            self.lifecycle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .current_generation(),
        );
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
        self.rx_characteristic = Some(rx);
        self.tx_characteristic = Some(tx);
        self.service = Some(service);
        emit_state(&self.app, &self.shared);
        Ok(())
    }

    fn service_was_added(&mut self, service: &Service, succeeded: bool) -> Result<(), String> {
        if !self
            .service
            .as_ref()
            .is_some_and(|current| current.is_same_service(service))
        {
            return Ok(());
        }
        if !succeeded {
            self.reset_gatt();
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
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn advertising_did_start(&mut self, succeeded: bool) -> Result<(), String> {
        let Some(generation) = self.service_generation else {
            return Ok(());
        };
        if !self
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_current(generation)
        {
            return Ok(());
        }
        if succeeded {
            self.lifecycle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .mark_active(generation);
            self.set_bluetooth(BluetoothState::Advertising);
            set_activity(
                &self.shared,
                ActivityKind::Info,
                "Advertising to nearby Switchify Android devices.",
            );
        } else {
            self.reset_gatt();
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
                self.app.state::<DwellController>().cancel(&self.app);
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

    fn handle_writes(&mut self, requests: Vec<AttRequest>, generation: u64) -> Result<(), String> {
        for request in requests {
            let characteristic = request.characteristic();
            let uuid = characteristic.uuid();
            let central_id = request.central().identifier();
            let is_current = self
                .lifecycle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_current(generation)
                && self
                    .rx_characteristic
                    .as_ref()
                    .is_some_and(|current| current.is_same_characteristic(&characteristic));
            let result = if !is_current {
                AttError::UnlikelyError
            } else if !uuid.eq_ignore_ascii_case(RX_UUID) {
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
            Ok(Some(EngineEvent::AuthenticatedConnection(connection))) => {
                let model = self.app.state::<AppModel>();
                let saved = model
                    .record_authenticated_connection(
                        &connection.device_id,
                        connection.device_name.as_deref(),
                        connection.connected_at,
                        connection.received_order,
                    )
                    .unwrap_or(false);
                if !saved {
                    set_activity(
                        &self.shared,
                        ActivityKind::Error,
                        "The device connection details could not be saved.",
                    );
                }
                let response = self
                    .shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .engine
                    .complete_authenticated_connection(&connection, saved);
                if let Err(error) = self.enqueue_message(&response) {
                    self.report_error(error);
                }
                emit_state(&self.app, &self.shared);
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
            self.app.state::<DwellController>().arm(&self.app);
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
        self.app.state::<DwellController>().cancel(&self.app);
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
        let typing_route = AndroidTypingRoute::for_text(&command.text);
        let app = self.app.clone();
        typing_route.prepare(
            || app.state::<DwellController>().cancel(&app),
            || self.stop_all_repeats(),
        );
        let character_count = command.text.chars().count();
        let injection = self.inject_text(&command.text);
        typing_route.finish(injection.is_ok(), || {
            self.app.state::<CursorOverlay>().hide_for_typing()
        });
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
            self.app.state::<DwellController>().cancel(&self.app);
        }
        let typing_route = AndroidTypingRoute::for_command(&command.command_type);
        let profiles = self
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .profiles
            .clone();
        if command.command_type == "switch.profile.list" {
            self.stop_all_repeats();
            if let Err(error) =
                self.enqueue_message(&switch_profile_catalog_response(&command.id, &profiles))
            {
                self.report_error(error);
            }
            return;
        }
        let accessibility_unavailable = if command.command_type == "pointer.display.move" {
            false
        } else {
            if typing_route.is_eligible() {
                let app = self.app.clone();
                typing_route.prepare(
                    || app.state::<DwellController>().cancel(&app),
                    || self.stop_all_repeats(),
                );
            } else {
                self.stop_all_repeats();
            }
            self.input.is_none() && !self.refresh_accessibility(false).unwrap_or(false)
        };
        let (injection, error_code) = if command.command_type == "pointer.display.move" {
            let direction = command.payload["direction"].as_str().unwrap_or_default();
            match display_navigation::run_navigation_command(
                self,
                direction,
                MacRuntime::stop_all_repeats,
                MacRuntime::navigate_display,
                |runtime, feedback| {
                    runtime
                        .app
                        .state::<CursorOverlay>()
                        .show(feedback, runtime.overlay_settings());
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
        } else if accessibility_unavailable {
            (
                Err(
                    "Accessibility permission is required before input can be controlled."
                        .to_string(),
                ),
                "input_failed",
            )
        } else {
            let model = self.app.state::<AppModel>();
            (
                self.input
                    .as_mut()
                    .ok_or_else(|| {
                        "Accessibility permission is required before input can be controlled."
                            .to_string()
                    })
                    .and_then(|input| {
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
        if command.command_type != "pointer.display.move" {
            if let Ok(outcome) = &injection {
                let settings = self.overlay_settings();
                let overlay = self.app.state::<CursorOverlay>();
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
        if command.command_type == "pointer.display.move" && injection.is_ok() {
            self.app.state::<DwellController>().arm(&self.app);
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
        self.app.state::<DwellController>().cancel(&self.app);
        let settings = self.overlay_settings();
        let parsed = RepeatCommand::parse(&command.payload);
        let injection = parsed.and_then(|repeat_command| {
            if repeat_move_is_stationary(repeat_command) {
                self.stop_repeat_for_device(&command.device_id);
                return Ok(());
            }
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
                    self.execute_repeat_move(active.generation, dx, dy)
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
        let stopped = self.stop_repeat_for_device(&command.device_id);
        if stopped.is_some_and(|active| matches!(active.command, RepeatCommand::Move { .. })) {
            self.app.state::<DwellController>().arm(&self.app);
        }
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
            self.stop_repeat_if_current(&device_id, generation, false);
            return Ok(());
        }
        if matches!(active.command, RepeatCommand::Move { .. }) {
            match self.pending_repeat_made_progress(generation, active.command) {
                Ok(true) => {}
                Ok(false) => {
                    self.stop_repeat_if_current(&device_id, generation, true);
                    return Ok(());
                }
                Err(error) => {
                    if self.stop_repeat_if_current(&device_id, generation, false) {
                        self.report_error(format!("Mouse repeat stopped: {error}"));
                    }
                    return Ok(());
                }
            }
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
        let result = match active.command {
            RepeatCommand::Move { .. } => match movement {
                Some((dx, dy)) => self.execute_repeat_move(generation, dx, dy).map(|_| ()),
                None => Ok(()),
            },
            RepeatCommand::Scroll { dx, dy } => match self.input.as_mut() {
                Some(input) => input.execute_repeat_scroll(dx, dy).map(|_| ()),
                None => Err(
                    "Accessibility permission is required before input can be controlled."
                        .to_string(),
                ),
            },
        };
        if let Err(error) = result {
            if self.stop_repeat_if_current(&device_id, generation, false) {
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

    fn execute_repeat_move(
        &mut self,
        generation: u64,
        dx: i32,
        dy: i32,
    ) -> Result<PointerFeedback, String> {
        let input = self.input.as_mut().ok_or_else(|| {
            "Accessibility permission is required before the pointer can move.".to_string()
        })?;
        if dx == 0 && dy == 0 {
            return Ok(input.pointer_feedback_for_move());
        }
        let (before, displays) =
            display_navigation::displays(&self.app).map_err(|error| error.message)?;
        let target = display_navigation::clamped_pointer_target(before, dx, dy, &displays)
            .ok_or_else(|| "No active display could be resolved.".to_string())?;
        let feedback = input.move_pointer_pixels_absolute(target.0, target.1)?;
        self.pending_repeat_moves
            .insert(generation, PendingRepeatMove { before });
        Ok(feedback)
    }

    fn pending_repeat_made_progress(
        &mut self,
        generation: u64,
        command: RepeatCommand,
    ) -> Result<bool, String> {
        let Some(pending) = self.pending_repeat_moves.remove(&generation) else {
            return Ok(true);
        };
        let RepeatCommand::Move { dx, dy } = command else {
            return Ok(true);
        };
        let after = display_navigation::cursor_position().map_err(|error| error.message)?;
        if repeat_move_made_progress(dx, dy, pending.before, after) {
            return Ok(true);
        }
        let (_, displays) =
            display_navigation::displays(&self.app).map_err(|error| error.message)?;
        Ok(repeat_direction_has_available_space(
            dx, dy, after, &displays,
        ))
    }

    fn stop_repeat_if_current(
        &mut self,
        device_id: &str,
        generation: u64,
        arm_dwell: bool,
    ) -> bool {
        let was_move = self
            .repeats
            .current(device_id, generation)
            .is_some_and(|active| matches!(active.command, RepeatCommand::Move { .. }));
        let stopped = self.repeats.stop_if_current(device_id, generation);
        if stopped {
            self.pending_repeat_moves.remove(&generation);
            self.app.state::<CursorOverlay>().end_repeat(generation);
            if arm_dwell && was_move {
                self.app.state::<DwellController>().arm(&self.app);
            }
        }
        stopped
    }

    fn stop_repeat_for_device(
        &mut self,
        device_id: &str,
    ) -> Option<crate::mouse_repeat::ActiveRepeat> {
        let active = self.repeats.stop(device_id);
        if let Some(active) = active {
            self.pending_repeat_moves.remove(&active.generation);
            self.app
                .state::<CursorOverlay>()
                .end_repeat(active.generation);
        }
        active
    }

    fn stop_all_repeats(&mut self) {
        for active in self.repeats.stop_all() {
            self.pending_repeat_moves.remove(&active.generation);
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
        let (dx, dy) = input.scaled_pointer_delta(dx, dy);
        let (cursor, displays) =
            display_navigation::displays(&self.app).map_err(|error| error.message)?;
        let target = display_navigation::clamped_pointer_target(cursor, dx, dy, &displays)
            .ok_or_else(|| "No active display could be resolved.".to_string())?;
        input
            .move_pointer_pixels_absolute(target.0, target.1)
            .map(|_| ())
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
            display_navigation::display_count(displays.len()),
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
                message: "Stop Switch Forwarding before using other PC control commands.".into(),
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
        display_navigation::map_injection(input.move_pointer_absolute(x, y))
    }

    fn refresh_accessibility(&mut self, prompt: bool) -> Result<bool, String> {
        self.app.state::<DwellController>().cancel(&self.app);
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
        if !granted {
            self.app.state::<DwellController>().cancel(&self.app);
        }
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
        self.app.state::<DwellController>().cancel(&self.app);
        self.stop_all_repeats();
        self.manager.stop_advertising();
        self.manager.remove_all_services();
        self.service = None;
        self.service_generation = None;
        self.rx_characteristic = None;
        self.tx_characteristic = None;
        self.subscribers.clear();
        self.pairing_centrals.clear();
        self.outbound.clear();
        {
            let mut model = self
                .shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            model.engine.reset_transport_session();
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

fn manager_state_invalidates_gatt(state: PeripheralManagerState) -> bool {
    matches!(
        state,
        PeripheralManagerState::PoweredOff
            | PeripheralManagerState::Unauthorized
            | PeripheralManagerState::Unsupported
            | PeripheralManagerState::Unknown
            | PeripheralManagerState::Resetting
    )
}

fn pointer_profile_for_display(
    name: &str,
    scale_factor: f64,
    logical_x: i32,
    logical_y: i32,
    logical_width: u32,
    logical_height: u32,
    display_count: u8,
) -> PointerProfile {
    let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
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
        display_navigation_supported: true,
        display_count,
    }
}

fn repeat_move_made_progress(dx: i32, dy: i32, before: (f64, f64), after: (f64, f64)) -> bool {
    const POSITION_EPSILON: f64 = 0.01;
    let axis_progress = |requested: i32, before: f64, after: f64| match requested.cmp(&0) {
        std::cmp::Ordering::Greater => after > before + POSITION_EPSILON,
        std::cmp::Ordering::Less => after < before - POSITION_EPSILON,
        std::cmp::Ordering::Equal => false,
    };
    axis_progress(dx, before.0, after.0) || axis_progress(dy, before.1, after.1)
}

fn repeat_move_is_stationary(command: RepeatCommand) -> bool {
    matches!(command, RepeatCommand::Move { dx: 0, dy: 0 })
}

fn repeat_direction_has_available_space(
    dx: i32,
    dy: i32,
    cursor: (f64, f64),
    displays: &[display_navigation::Display],
) -> bool {
    let contains = |point: (f64, f64)| {
        displays.iter().any(|display| {
            point.0 >= f64::from(display.x)
                && point.0 < f64::from(display.x) + f64::from(display.width)
                && point.1 >= f64::from(display.y)
                && point.1 < f64::from(display.y) + f64::from(display.height)
        })
    };
    let step = |value: i32| f64::from(value.signum());
    (dx != 0 && contains((cursor.0 + step(dx), cursor.1)))
        || (dy != 0 && contains((cursor.0, cursor.1 + step(dy))))
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
        if let Some(center) = self.notification_center.as_ref() {
            for observer in self.power_observers.drain(..) {
                let observer: &ProtocolObject<dyn NSObjectProtocol> = &observer;
                let observer: &AnyObject = AsRef::<AnyObject>::as_ref(observer);
                unsafe { center.removeObserver(observer) };
            }
        }
        self.app.state::<DwellController>().cancel(&self.app);
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
    use std::sync::Mutex;

    #[test]
    fn resetting_and_unknown_manager_states_invalidate_cached_gatt_objects() {
        assert!(manager_state_invalidates_gatt(
            PeripheralManagerState::Resetting
        ));
        assert!(manager_state_invalidates_gatt(
            PeripheralManagerState::Unknown
        ));
        assert!(!manager_state_invalidates_gatt(
            PeripheralManagerState::PoweredOn
        ));
    }

    #[test]
    fn mac_typing_route_orders_cleanup_before_successful_overlay_hiding() {
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
    fn repeated_move_stops_when_the_requested_axis_cannot_move() {
        assert!(!repeat_move_made_progress(
            12,
            0,
            (1919.0, 540.0),
            (1919.0, 540.0)
        ));
        assert!(!repeat_move_made_progress(
            0,
            -12,
            (960.0, 0.0),
            (960.0, 0.0)
        ));
        assert!(!repeat_move_made_progress(
            12,
            12,
            (1919.0, 1079.0),
            (1919.0, 1079.0)
        ));
    }

    #[test]
    fn repeated_diagonal_move_continues_along_a_free_axis() {
        assert!(repeat_move_made_progress(
            12,
            12,
            (1919.0, 500.0),
            (1919.0, 512.0)
        ));
        assert!(repeat_move_made_progress(
            -12,
            -12,
            (0.0, 500.0),
            (0.0, 488.0)
        ));
    }

    #[test]
    fn repeated_move_requires_progress_in_the_requested_direction() {
        assert!(repeat_move_made_progress(
            12,
            0,
            (100.0, 100.0),
            (112.0, 100.0)
        ));
        assert!(repeat_move_made_progress(
            0,
            -12,
            (100.0, 100.0),
            (100.0, 88.0)
        ));
        assert!(!repeat_move_made_progress(
            12,
            0,
            (100.0, 100.0),
            (99.0, 100.0)
        ));
        assert!(!repeat_move_made_progress(
            0,
            0,
            (100.0, 100.0),
            (112.0, 112.0)
        ));
    }

    fn repeat_display(x: i32, y: i32, width: u32, height: u32) -> display_navigation::Display {
        display_navigation::Display {
            name: "test".into(),
            scale_factor: 1.0,
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn uneven_diagonal_can_continue_on_a_free_minor_axis() {
        let displays = [repeat_display(0, 0, 1920, 1080)];
        assert!(repeat_direction_has_available_space(
            10,
            1,
            (1919.0, 500.0),
            &displays
        ));
        assert!(!repeat_direction_has_available_space(
            10,
            1,
            (1919.0, 1079.0),
            &displays
        ));
    }

    #[test]
    fn repeat_direction_crosses_an_adjacent_monitor_boundary() {
        let displays = [
            repeat_display(0, 0, 1920, 1080),
            repeat_display(1920, 0, 1280, 1024),
        ];
        assert!(repeat_direction_has_available_space(
            1,
            0,
            (1919.0, 500.0),
            &displays
        ));
        assert!(!repeat_direction_has_available_space(
            1,
            0,
            (3199.0, 500.0),
            &displays
        ));
    }

    #[test]
    fn zero_vector_has_no_available_repeat_direction() {
        let displays = [repeat_display(0, 0, 1920, 1080)];
        assert!(repeat_move_is_stationary(RepeatCommand::Move {
            dx: 0,
            dy: 0
        }));
        assert!(!repeat_move_is_stationary(RepeatCommand::Move {
            dx: 0,
            dy: 1
        }));
        assert!(!repeat_move_is_stationary(RepeatCommand::Scroll {
            dx: 0,
            dy: 0
        }));
        assert!(!repeat_direction_has_available_space(
            0,
            0,
            (960.0, 540.0),
            &displays
        ));
    }

    #[test]
    fn pointer_profile_uses_logical_bounds_and_reduced_movement_steps() {
        let profile = pointer_profile_for_display("Retina", 2.0, 0, 0, 1920, 1080, 2);
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
