use core::ffi::{c_char, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{from_swift, BluetoothErrorInfo, CoreBluetoothError};
use crate::ffi;
use crate::peripheral::Peripheral;
use crate::private::{encode_json, encode_string_slice, take_retained_pointer_array, to_cstring};
use crate::uuid::BluetoothUuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[repr(i32)]
/// Mirrors `CBCentralManagerState`.
pub enum CentralManagerState {
    /// Corresponds to the matching `CBCentralManagerState` case.
    Unknown = 0,
    /// Corresponds to the matching `CBCentralManagerState` case.
    Resetting = 1,
    /// Corresponds to the matching `CBCentralManagerState` case.
    Unsupported = 2,
    /// Corresponds to the matching `CBCentralManagerState` case.
    Unauthorized = 3,
    /// Corresponds to the matching `CBCentralManagerState` case.
    PoweredOff = 4,
    /// Corresponds to the matching `CBCentralManagerState` case.
    PoweredOn = 5,
}

impl CentralManagerState {
    /// Converts a raw `CBCentralManagerState` value into `CentralManagerState`.
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            1 => Self::Resetting,
            2 => Self::Unsupported,
            3 => Self::Unauthorized,
            4 => Self::PoweredOff,
            5 => Self::PoweredOn,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[repr(i32)]
/// Mirrors `CBManagerAuthorization`.
pub enum ManagerAuthorization {
    /// Corresponds to the matching `CBManagerAuthorization` case.
    NotDetermined = 0,
    /// Corresponds to the matching `CBManagerAuthorization` case.
    Restricted = 1,
    /// Corresponds to the matching `CBManagerAuthorization` case.
    Denied = 2,
    /// Corresponds to the matching `CBManagerAuthorization` case.
    AllowedAlways = 3,
}

impl ManagerAuthorization {
    /// Converts a raw `CBManagerAuthorization` value into `ManagerAuthorization`.
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            1 => Self::Restricted,
            2 => Self::Denied,
            3 => Self::AllowedAlways,
            _ => Self::NotDetermined,
        }
    }

    /// Returns whether `CoreBluetooth` currently reports `AllowedAlways` authorization.
    pub const fn is_authorized(self) -> bool {
        matches!(self, Self::AllowedAlways)
    }
}

#[derive(Debug, Clone, Default)]
#[must_use]
/// Construction options corresponding to `CBCentralManagerOptionShowPowerAlertKey` and related manager options.
pub struct CentralManagerOptions {
    /// Dispatch queue label passed to `CoreBluetooth` when constructing the manager.
    pub queue_label: Option<String>,
    /// Value forwarded to the `CoreBluetooth` show-power-alert option key.
    pub show_power_alert: Option<bool>,
    /// Identifier forwarded to the `CoreBluetooth` state-restoration option key.
    pub restore_identifier: Option<String>,
}

impl CentralManagerOptions {
    /// Creates empty `CBCentralManager` construction options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the dispatch queue label used when constructing `CBCentralManager`.
    pub fn with_queue_label(mut self, queue_label: impl Into<String>) -> Self {
        self.queue_label = Some(queue_label.into());
        self
    }

    /// Sets the `CoreBluetooth` show-power-alert option for `CBCentralManager`.
    pub fn with_show_power_alert(mut self, show_power_alert: bool) -> Self {
        self.show_power_alert = Some(show_power_alert);
        self
    }

    /// Sets the state-restoration identifier for `CBCentralManager`.
    pub fn with_restore_identifier(mut self, restore_identifier: impl Into<String>) -> Self {
        self.restore_identifier = Some(restore_identifier.into());
        self
    }
}

#[derive(Debug, Clone, Default)]
#[must_use]
/// Connection options corresponding to `connectPeripheral:options:`.
pub struct ConnectOptions {
    /// Value forwarded to `CBConnectPeripheralOptionNotifyOnConnectionKey`.
    pub notify_on_connection: Option<bool>,
    /// Value forwarded to `CBConnectPeripheralOptionNotifyOnDisconnectionKey`.
    pub notify_on_disconnection: Option<bool>,
    /// Value forwarded to `CBConnectPeripheralOptionNotifyOnNotificationKey`.
    pub notify_on_notification: Option<bool>,
    /// Value forwarded to `CBConnectPeripheralOptionStartDelayKey`.
    pub start_delay_seconds: Option<f64>,
    /// Value forwarded to `CBConnectPeripheralOptionEnableAutoReconnect`.
    pub enable_auto_reconnect: Option<bool>,
}

impl ConnectOptions {
    /// Creates empty connection options for `connectPeripheral:options:`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `CBConnectPeripheralOptionNotifyOnConnectionKey`.
    pub fn with_notify_on_connection(mut self, value: bool) -> Self {
        self.notify_on_connection = Some(value);
        self
    }

    /// Sets `CBConnectPeripheralOptionNotifyOnDisconnectionKey`.
    pub fn with_notify_on_disconnection(mut self, value: bool) -> Self {
        self.notify_on_disconnection = Some(value);
        self
    }

    /// Sets `CBConnectPeripheralOptionNotifyOnNotificationKey`.
    pub fn with_notify_on_notification(mut self, value: bool) -> Self {
        self.notify_on_notification = Some(value);
        self
    }

    /// Sets `CBConnectPeripheralOptionStartDelayKey`.
    pub fn with_start_delay_seconds(mut self, value: f64) -> Self {
        self.start_delay_seconds = Some(value);
        self
    }

    /// Sets `CBConnectPeripheralOptionEnableAutoReconnect`.
    pub fn with_enable_auto_reconnect(mut self, value: bool) -> Self {
        self.enable_auto_reconnect = Some(value);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
/// Scan options corresponding to `scanForPeripheralsWithServices:options:`.
pub struct ScanOptions {
    /// Whether duplicate discoveries should be reported while scanning.
    pub allow_duplicates: bool,
    /// Solicited service UUIDs forwarded to the `CoreBluetooth` scan options dictionary.
    pub solicited_service_uuids: Vec<BluetoothUuid>,
}

impl ScanOptions {
    /// Creates empty scan options for `scanForPeripheralsWithServices:options:`.
    pub fn new() -> Self {
        Self {
            allow_duplicates: false,
            solicited_service_uuids: Vec::new(),
        }
    }

    /// Sets `CBCentralManagerScanOptionAllowDuplicatesKey`.
    pub const fn with_allow_duplicates(mut self, allow_duplicates: bool) -> Self {
        self.allow_duplicates = allow_duplicates;
        self
    }

    /// Adds a UUID to `CBCentralManagerScanOptionSolicitedServiceUUIDsKey`.
    pub fn with_solicited_service_uuid(mut self, uuid: BluetoothUuid) -> Self {
        self.solicited_service_uuids.push(uuid);
        self
    }
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// State restored by `centralManager:willRestoreState:`.
pub struct CentralManagerRestoredState {
    /// Peripherals restored under `CBCentralManagerRestoredStatePeripheralsKey`.
    pub peripherals: Vec<Peripheral>,
    /// Service UUIDs restored under `CBCentralManagerRestoredStateScanServicesKey`.
    pub scan_service_uuids: Vec<BluetoothUuid>,
    /// Scan options restored under `CBCentralManagerRestoredStateScanOptionsKey`.
    pub scan_options: Option<ScanOptions>,
}

#[derive(Serialize)]
struct CentralManagerOptionsPayload {
    queue_label: Option<String>,
    show_power_alert: Option<bool>,
    restore_identifier: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ScanOptionsPayload {
    allow_duplicates: bool,
    #[serde(default)]
    solicited_service_uuids: Vec<String>,
}

#[derive(Serialize)]
struct ConnectOptionsPayload {
    notify_on_connection: Option<bool>,
    notify_on_disconnection: Option<bool>,
    notify_on_notification: Option<bool>,
    start_delay_seconds: Option<f64>,
    enable_auto_reconnect: Option<bool>,
}

#[derive(Deserialize)]
struct CentralManagerEventPayload {
    event: String,
    state: Option<i32>,
    authorization: Option<i32>,
    peripheral_handle: Option<u64>,
    peripheral_handles: Option<Vec<u64>>,
    rssi: Option<i32>,
    advertisement_data: Option<Value>,
    error: Option<BluetoothErrorInfo>,
    scan_service_uuids: Option<Vec<String>>,
    scan_options: Option<ScanOptionsPayload>,
    timestamp: Option<f64>,
    is_reconnecting: Option<bool>,
}

mod private {
    pub trait Sealed {}
}

/// Delegate callbacks corresponding to `CBCentralManagerDelegate`.
pub trait CentralManagerDelegate: Send + private::Sealed {
    /// Handles `centralManagerDidUpdateState:`.
    fn did_update_state(
        &mut self,
        state: CentralManagerState,
        authorization: ManagerAuthorization,
    ) {
        let _ = (state, authorization);
    }

    /// Handles `centralManager:willRestoreState:`.
    fn will_restore_state(&mut self, restored_state: CentralManagerRestoredState) {
        let _ = restored_state;
    }

    /// Handles `centralManager:didDiscoverPeripheral:advertisementData:RSSI:`.
    fn did_discover_peripheral(
        &mut self,
        peripheral: Peripheral,
        rssi: i32,
        advertisement_data: Value,
    ) {
        let _ = (peripheral, rssi, advertisement_data);
    }

    /// Handles `centralManager:didConnectPeripheral:`.
    fn did_connect_peripheral(&mut self, peripheral: Peripheral) {
        let _ = peripheral;
    }

    /// Handles `centralManager:didFailToConnectPeripheral:error:`.
    fn did_fail_to_connect_peripheral(
        &mut self,
        peripheral: Peripheral,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (peripheral, error);
    }

    /// Handles `centralManager:didDisconnectPeripheral:error:`.
    fn did_disconnect_peripheral(
        &mut self,
        peripheral: Peripheral,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (peripheral, error);
    }

    /// Handles `centralManager:didDisconnectPeripheral:timestamp:isReconnecting:error:`.
    fn did_disconnect_peripheral_details(
        &mut self,
        peripheral: Peripheral,
        timestamp: Option<f64>,
        is_reconnecting: bool,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (peripheral, timestamp, is_reconnecting, error);
    }
}

type StateHandler = Box<dyn FnMut(CentralManagerState, ManagerAuthorization) + Send + 'static>;
type RestoreHandler = Box<dyn FnMut(CentralManagerRestoredState) + Send + 'static>;
type DiscoveryHandler = Box<dyn FnMut(Peripheral, i32, Value) + Send + 'static>;
type ConnectionHandler = Box<dyn FnMut(Peripheral) + Send + 'static>;
type ErrorHandler = Box<dyn FnMut(Peripheral, Option<BluetoothErrorInfo>) + Send + 'static>;
type DisconnectDetailHandler =
    Box<dyn FnMut(Peripheral, Option<f64>, bool, Option<BluetoothErrorInfo>) + Send + 'static>;

#[allow(clippy::type_complexity)]
#[must_use]
/// Closure-based adapter for `CBCentralManagerDelegate`.
pub struct CentralManagerCallbacks {
    state: Option<StateHandler>,
    restore_state: Option<RestoreHandler>,
    discover: Option<DiscoveryHandler>,
    connect: Option<ConnectionHandler>,
    fail_to_connect: Option<ErrorHandler>,
    disconnect: Option<ErrorHandler>,
    disconnect_details: Option<DisconnectDetailHandler>,
}

impl CentralManagerCallbacks {
    /// Creates an empty closure-based adapter for `CBCentralManagerDelegate`.
    pub fn new() -> Self {
        Self {
            state: None,
            restore_state: None,
            discover: None,
            connect: None,
            fail_to_connect: None,
            disconnect: None,
            disconnect_details: None,
        }
    }

    /// Registers a closure for the corresponding callback from `CBCentralManagerDelegate`.
    pub fn on_state(
        mut self,
        callback: impl FnMut(CentralManagerState, ManagerAuthorization) + Send + 'static,
    ) -> Self {
        self.state = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBCentralManagerDelegate`.
    pub fn on_restore_state(
        mut self,
        callback: impl FnMut(CentralManagerRestoredState) + Send + 'static,
    ) -> Self {
        self.restore_state = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBCentralManagerDelegate`.
    pub fn on_discover(
        mut self,
        callback: impl FnMut(Peripheral, i32, Value) + Send + 'static,
    ) -> Self {
        self.discover = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBCentralManagerDelegate`.
    pub fn on_connect(mut self, callback: impl FnMut(Peripheral) + Send + 'static) -> Self {
        self.connect = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBCentralManagerDelegate`.
    pub fn on_fail_to_connect(
        mut self,
        callback: impl FnMut(Peripheral, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.fail_to_connect = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBCentralManagerDelegate`.
    pub fn on_disconnect(
        mut self,
        callback: impl FnMut(Peripheral, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.disconnect = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBCentralManagerDelegate`.
    pub fn on_disconnect_details(
        mut self,
        callback: impl FnMut(Peripheral, Option<f64>, bool, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.disconnect_details = Some(Box::new(callback));
        self
    }
}

impl Default for CentralManagerCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

impl private::Sealed for CentralManagerCallbacks {}
impl CentralManagerDelegate for CentralManagerCallbacks {
    fn did_update_state(
        &mut self,
        state: CentralManagerState,
        authorization: ManagerAuthorization,
    ) {
        if let Some(callback) = &mut self.state {
            callback(state, authorization);
        }
    }

    fn will_restore_state(&mut self, restored_state: CentralManagerRestoredState) {
        if let Some(callback) = &mut self.restore_state {
            callback(restored_state);
        }
    }

    fn did_discover_peripheral(
        &mut self,
        peripheral: Peripheral,
        rssi: i32,
        advertisement_data: Value,
    ) {
        if let Some(callback) = &mut self.discover {
            callback(peripheral, rssi, advertisement_data);
        }
    }

    fn did_connect_peripheral(&mut self, peripheral: Peripheral) {
        if let Some(callback) = &mut self.connect {
            callback(peripheral);
        }
    }

    fn did_fail_to_connect_peripheral(
        &mut self,
        peripheral: Peripheral,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.fail_to_connect {
            callback(peripheral, error);
        }
    }

    fn did_disconnect_peripheral(
        &mut self,
        peripheral: Peripheral,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.disconnect {
            callback(peripheral, error);
        }
    }

    fn did_disconnect_peripheral_details(
        &mut self,
        peripheral: Peripheral,
        timestamp: Option<f64>,
        is_reconnecting: bool,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.disconnect_details {
            callback(peripheral, timestamp, is_reconnecting, error);
        }
    }
}

struct CallbackState {
    delegate: Mutex<Box<dyn CentralManagerDelegate>>,
}

/// Wraps `CBCentralManager`.
pub struct CentralManager {
    raw: *mut c_void,
    callback_state: Option<Box<CallbackState>>,
}

unsafe extern "C" fn central_manager_event_trampoline(
    user_info: *mut c_void,
    payload_json: *const c_char,
) {
    if user_info.is_null() || payload_json.is_null() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        let state = unsafe { &*user_info.cast::<CallbackState>() };
        let payload_json = unsafe { core::ffi::CStr::from_ptr(payload_json) }
            .to_string_lossy()
            .into_owned();
        let Ok(payload): Result<CentralManagerEventPayload, _> =
            serde_json::from_str(&payload_json)
        else {
            return;
        };

        let mut delegate = match state.delegate.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        match payload.event.as_str() {
            "didUpdateState" => delegate.did_update_state(
                CentralManagerState::from_raw(payload.state.unwrap_or_default()),
                ManagerAuthorization::from_raw(payload.authorization.unwrap_or_default()),
            ),
            "willRestoreState" => {
                let peripherals = payload
                    .peripheral_handles
                    .unwrap_or_default()
                    .into_iter()
                    .map(Peripheral::from_retained_handle)
                    .collect();
                let scan_service_uuids = payload
                    .scan_service_uuids
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|uuid| BluetoothUuid::from_string(&uuid).ok())
                    .collect();
                let scan_options = payload.scan_options.map(|options| ScanOptions {
                    allow_duplicates: options.allow_duplicates,
                    solicited_service_uuids: options
                        .solicited_service_uuids
                        .into_iter()
                        .filter_map(|uuid| BluetoothUuid::from_string(&uuid).ok())
                        .collect(),
                });
                delegate.will_restore_state(CentralManagerRestoredState {
                    peripherals,
                    scan_service_uuids,
                    scan_options,
                });
            }
            "didDiscoverPeripheral" => {
                if let Some(handle) = payload.peripheral_handle {
                    delegate.did_discover_peripheral(
                        Peripheral::from_retained_handle(handle),
                        payload.rssi.unwrap_or_default(),
                        payload.advertisement_data.unwrap_or(Value::Null),
                    );
                }
            }
            "didConnectPeripheral" => {
                if let Some(handle) = payload.peripheral_handle {
                    delegate.did_connect_peripheral(Peripheral::from_retained_handle(handle));
                }
            }
            "didFailToConnectPeripheral" => {
                if let Some(handle) = payload.peripheral_handle {
                    delegate.did_fail_to_connect_peripheral(
                        Peripheral::from_retained_handle(handle),
                        payload.error,
                    );
                }
            }
            "didDisconnectPeripheral" => {
                if let Some(handle) = payload.peripheral_handle {
                    let peripheral = Peripheral::from_retained_handle(handle);
                    delegate.did_disconnect_peripheral(peripheral.clone(), payload.error.clone());
                    delegate.did_disconnect_peripheral_details(
                        peripheral,
                        payload.timestamp,
                        payload.is_reconnecting.unwrap_or(false),
                        payload.error,
                    );
                }
            }
            _ => {}
        }
    }));
}

impl CentralManager {
    /// Creates a new `CBCentralManager` wrapper using default options.
    pub fn new() -> Result<Self, CoreBluetoothError> {
        Self::with_options(CentralManagerOptions::default())
    }

    /// Creates a new `CBCentralManager` wrapper with explicit options.
    pub fn with_options(options: CentralManagerOptions) -> Result<Self, CoreBluetoothError> {
        Self::new_inner(options, None)
    }

    /// Creates a new `CBCentralManager` wrapper with a delegate implementing `CentralManagerDelegate`.
    pub fn with_delegate<D>(delegate: D) -> Result<Self, CoreBluetoothError>
    where
        D: CentralManagerDelegate + 'static,
    {
        Self::new_inner(CentralManagerOptions::default(), Some(Box::new(delegate)))
    }

    /// Creates a new `CBCentralManager` wrapper backed by `CentralManagerCallbacks`.
    pub fn with_callbacks(callbacks: CentralManagerCallbacks) -> Result<Self, CoreBluetoothError> {
        Self::with_delegate(callbacks)
    }

    /// Creates a new `CBCentralManager` wrapper on a named dispatch queue.
    pub fn with_queue_label(queue_label: &str) -> Result<Self, CoreBluetoothError> {
        Self::with_options(CentralManagerOptions::new().with_queue_label(queue_label))
    }

    /// Creates a new `CBCentralManager` wrapper on a named queue with a delegate.
    pub fn with_queue_label_and_delegate<D>(
        queue_label: &str,
        delegate: D,
    ) -> Result<Self, CoreBluetoothError>
    where
        D: CentralManagerDelegate + 'static,
    {
        Self::new_inner(
            CentralManagerOptions::new().with_queue_label(queue_label),
            Some(Box::new(delegate)),
        )
    }

    /// Returns the process-wide `CoreBluetooth` authorization state for `CBCentralManager`.
    pub fn current_authorization() -> ManagerAuthorization {
        ManagerAuthorization::from_raw(unsafe { ffi::cb_manager_global_authorization() })
    }

    fn new_inner(
        options: CentralManagerOptions,
        delegate: Option<Box<dyn CentralManagerDelegate>>,
    ) -> Result<Self, CoreBluetoothError> {
        let options_json = to_cstring(
            &serde_json::to_string(&CentralManagerOptionsPayload {
                queue_label: options.queue_label,
                show_power_alert: options.show_power_alert,
                restore_identifier: options.restore_identifier,
            })
            .map_err(|error| {
                CoreBluetoothError::FrameworkError(format!(
                    "failed to encode manager options: {error}"
                ))
            })?,
        )?;

        let mut raw = core::ptr::null_mut();
        let mut error = core::ptr::null_mut();
        let mut callback_state = delegate.map(|delegate| {
            Box::new(CallbackState {
                delegate: Mutex::new(delegate),
            })
        });
        let user_info = callback_state
            .as_deref_mut()
            .map_or(core::ptr::null_mut(), |state| {
                std::ptr::from_mut::<CallbackState>(state).cast::<c_void>()
            });
        let callback = if callback_state.is_some() {
            Some(central_manager_event_trampoline as ffi::JsonCallback)
        } else {
            None
        };

        let status = unsafe {
            ffi::cb_manager_new(
                options_json.as_ptr(),
                callback,
                user_info,
                &mut raw,
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(Self {
                raw,
                callback_state,
            })
        } else {
            Err(from_swift(status, error))
        }
    }

    #[cfg(feature = "async")]
    pub(crate) const fn as_raw(&self) -> *mut c_void {
        self.raw
    }

    /// Returns the current `CBCentralManagerState`.
    pub fn state(&self) -> CentralManagerState {
        CentralManagerState::from_raw(unsafe { ffi::cb_manager_state(self.raw) })
    }

    /// Returns the current `CBManagerAuthorization` reported by `CBCentralManager`.
    pub fn authorization(&self) -> ManagerAuthorization {
        ManagerAuthorization::from_raw(unsafe { ffi::cb_manager_authorization(self.raw) })
    }

    /// Returns whether `CBCentralManager.isScanning` is set.
    pub fn is_scanning(&self) -> bool {
        unsafe { ffi::cb_manager_is_scanning(self.raw) }
    }

    /// Invokes `scanForPeripheralsWithServices:options:`.
    pub fn scan_for_peripherals(
        &self,
        service_uuids: Option<&[&str]>,
        options: ScanOptions,
    ) -> Result<(), CoreBluetoothError> {
        let service_uuids = match service_uuids {
            Some(service_uuids) => Some(encode_string_slice(service_uuids)?),
            None => None,
        };
        let ScanOptions {
            allow_duplicates,
            solicited_service_uuids,
        } = options;
        let scan_options = encode_json(&ScanOptionsPayload {
            allow_duplicates,
            solicited_service_uuids: solicited_service_uuids
                .into_iter()
                .map(|uuid| uuid.uuid_string())
                .collect(),
        })?;
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_manager_scan_for_peripherals(
                self.raw,
                service_uuids
                    .as_ref()
                    .map_or(core::ptr::null(), |value| value.as_ptr()),
                scan_options.as_ptr(),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `stopScan`.
    pub fn stop_scan(&self) {
        unsafe { ffi::cb_manager_stop_scan(self.raw) };
    }

    /// Invokes `connectPeripheral:options:` with default options.
    pub fn connect(&self, peripheral: &Peripheral) -> Result<(), CoreBluetoothError> {
        self.connect_with_options(peripheral, &ConnectOptions::default())
    }

    /// Invokes `connectPeripheral:options:` with explicit options.
    pub fn connect_with_options(
        &self,
        peripheral: &Peripheral,
        options: &ConnectOptions,
    ) -> Result<(), CoreBluetoothError> {
        let options_json = encode_json(&ConnectOptionsPayload {
            notify_on_connection: options.notify_on_connection,
            notify_on_disconnection: options.notify_on_disconnection,
            notify_on_notification: options.notify_on_notification,
            start_delay_seconds: options.start_delay_seconds,
            enable_auto_reconnect: options.enable_auto_reconnect,
        })?;
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_manager_connect(self.raw, peripheral.raw, options_json.as_ptr(), &mut error)
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `cancelPeripheralConnection:`.
    pub fn cancel_peripheral_connection(&self, peripheral: &Peripheral) {
        unsafe { ffi::cb_manager_cancel_peripheral_connection(self.raw, peripheral.raw) };
    }

    /// Invokes `retrieveConnectedPeripheralsWithServices:`.
    pub fn retrieve_connected_peripherals(
        &self,
        service_uuids: &[&str],
    ) -> Result<Vec<Peripheral>, CoreBluetoothError> {
        let service_uuids = encode_string_slice(service_uuids)?;
        let mut array = core::ptr::null_mut();
        let mut count = 0;
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_manager_retrieve_connected_peripherals(
                self.raw,
                service_uuids.as_ptr(),
                &mut array,
                &mut count,
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(take_retained_pointer_array(array, count)
                .into_iter()
                .map(Peripheral::from_retained_raw)
                .collect())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `retrievePeripheralsWithIdentifiers:`.
    pub fn retrieve_peripherals_with_identifiers(
        &self,
        identifiers: &[&str],
    ) -> Result<Vec<Peripheral>, CoreBluetoothError> {
        let identifiers = encode_string_slice(identifiers)?;
        let mut array = core::ptr::null_mut();
        let mut count = 0;
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_manager_retrieve_peripherals_with_identifiers(
                self.raw,
                identifiers.as_ptr(),
                &mut array,
                &mut count,
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(take_retained_pointer_array(array, count)
                .into_iter()
                .map(Peripheral::from_retained_raw)
                .collect())
        } else {
            Err(from_swift(status, error))
        }
    }
}

impl Drop for CentralManager {
    fn drop(&mut self) {
        unsafe { ffi::cb_object_release(self.raw) };
        let _ = self.callback_state.take();
    }
}
