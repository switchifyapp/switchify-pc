use core::ffi::{c_char, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

use serde::Deserialize;

pub use crate::characteristic::{
    AttributePermissions, Characteristic, CharacteristicProperties, CharacteristicWriteType,
};
pub use crate::descriptor::{Descriptor, DescriptorValue, MutableDescriptor};
pub use crate::l2cap_channel::L2capChannel;
pub use crate::service::Service;

use crate::error::{from_swift, take_owned_c_string, BluetoothErrorInfo, CoreBluetoothError};
use crate::ffi;
use crate::private::{encode_string_slice, take_retained_pointer_array};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[repr(i32)]
/// Mirrors `CBPeripheralState`.
pub enum PeripheralState {
    /// Corresponds to the matching `CBPeripheralState` case.
    Disconnected = 0,
    /// Corresponds to the matching `CBPeripheralState` case.
    Connecting = 1,
    /// Corresponds to the matching `CBPeripheralState` case.
    Connected = 2,
    /// Corresponds to the matching `CBPeripheralState` case.
    Disconnecting = 3,
}

impl PeripheralState {
    /// Converts a raw `CBPeripheralState` value into `PeripheralState`.
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            1 => Self::Connecting,
            2 => Self::Connected,
            3 => Self::Disconnecting,
            _ => Self::Disconnected,
        }
    }
}

#[derive(Deserialize)]
struct PeripheralEventPayload {
    event: String,
    service_handles: Option<Vec<u64>>,
    invalidated_service_handles: Option<Vec<u64>>,
    service_handle: Option<u64>,
    characteristic_handles: Option<Vec<u64>>,
    characteristic_handle: Option<u64>,
    descriptor_handle: Option<u64>,
    channel_handle: Option<u64>,
    rssi: Option<i32>,
    error: Option<BluetoothErrorInfo>,
}

fn retained_handle_to_raw(handle: u64) -> *mut c_void {
    usize::try_from(handle).unwrap_or_else(|_| {
        unreachable!("retained handles must fit into usize on supported targets")
    }) as *mut c_void
}

mod private {
    pub trait Sealed {}
}

/// Delegate callbacks corresponding to `CBPeripheralDelegate`.
pub trait PeripheralDelegate: Send + private::Sealed {
    /// Handles `peripheralDidUpdateName:`.
    fn did_update_name(&mut self) {}

    /// Handles `peripheral:didModifyServices:`.
    fn did_modify_services(&mut self, invalidated_services: Vec<Service>) {
        let _ = invalidated_services;
    }

    /// Handles `peripheral:didDiscoverServices:`.
    fn did_discover_services(&mut self, services: Vec<Service>, error: Option<BluetoothErrorInfo>) {
        let _ = (services, error);
    }

    /// Handles `peripheral:didDiscoverIncludedServicesForService:error:`.
    fn did_discover_included_services_for_service(
        &mut self,
        service: Service,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (service, error);
    }

    /// Handles `peripheral:didDiscoverCharacteristicsForService:error:`.
    fn did_discover_characteristics_for_service(
        &mut self,
        service: Service,
        characteristics: Vec<Characteristic>,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (service, characteristics, error);
    }

    /// Handles `peripheral:didUpdateValueForCharacteristic:error:`.
    fn did_update_value_for_characteristic(
        &mut self,
        characteristic: Characteristic,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (characteristic, error);
    }

    /// Handles `peripheral:didWriteValueForCharacteristic:error:`.
    fn did_write_value_for_characteristic(
        &mut self,
        characteristic: Characteristic,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (characteristic, error);
    }

    /// Handles `peripheral:didUpdateNotificationStateForCharacteristic:error:`.
    fn did_update_notification_state_for_characteristic(
        &mut self,
        characteristic: Characteristic,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (characteristic, error);
    }

    /// Handles `peripheral:didDiscoverDescriptorsForCharacteristic:error:`.
    fn did_discover_descriptors_for_characteristic(
        &mut self,
        characteristic: Characteristic,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (characteristic, error);
    }

    /// Handles `peripheral:didUpdateValueForDescriptor:error:`.
    fn did_update_value_for_descriptor(
        &mut self,
        descriptor: Descriptor,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (descriptor, error);
    }

    /// Handles `peripheral:didWriteValueForDescriptor:error:`.
    fn did_write_value_for_descriptor(
        &mut self,
        descriptor: Descriptor,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (descriptor, error);
    }

    /// Handles `peripheralIsReadyToSendWriteWithoutResponse:`.
    fn is_ready_to_send_write_without_response(&mut self) {}

    /// Handles `peripheral:didReadRSSI:error:`.
    fn did_read_rssi(&mut self, rssi: i32, error: Option<BluetoothErrorInfo>) {
        let _ = (rssi, error);
    }

    /// Handles `peripheral:didOpenL2CAPChannel:error:`.
    fn did_open_l2cap_channel(
        &mut self,
        channel: Option<L2capChannel>,
        error: Option<BluetoothErrorInfo>,
    ) {
        let _ = (channel, error);
    }
}

type NameHandler = Box<dyn FnMut() + Send + 'static>;
type ServicesHandler = Box<dyn FnMut(Vec<Service>, Option<BluetoothErrorInfo>) + Send + 'static>;
type ModifyServicesHandler = Box<dyn FnMut(Vec<Service>) + Send + 'static>;
type IncludedServicesHandler = Box<dyn FnMut(Service, Option<BluetoothErrorInfo>) + Send + 'static>;
type CharacteristicsHandler =
    Box<dyn FnMut(Service, Vec<Characteristic>, Option<BluetoothErrorInfo>) + Send + 'static>;
type CharacteristicHandler =
    Box<dyn FnMut(Characteristic, Option<BluetoothErrorInfo>) + Send + 'static>;
type DescriptorHandler = Box<dyn FnMut(Descriptor, Option<BluetoothErrorInfo>) + Send + 'static>;
type DescriptorDiscoveryHandler =
    Box<dyn FnMut(Characteristic, Option<BluetoothErrorInfo>) + Send + 'static>;
type ReadyHandler = Box<dyn FnMut() + Send + 'static>;
type RssiHandler = Box<dyn FnMut(i32, Option<BluetoothErrorInfo>) + Send + 'static>;
type L2capHandler =
    Box<dyn FnMut(Option<L2capChannel>, Option<BluetoothErrorInfo>) + Send + 'static>;

#[allow(clippy::type_complexity)]
#[must_use]
/// Closure-based adapter for `CBPeripheralDelegate`.
pub struct PeripheralCallbacks {
    name: Option<NameHandler>,
    modify_services: Option<ModifyServicesHandler>,
    services: Option<ServicesHandler>,
    included_services: Option<IncludedServicesHandler>,
    characteristics: Option<CharacteristicsHandler>,
    update_value: Option<CharacteristicHandler>,
    write_value: Option<CharacteristicHandler>,
    update_notification: Option<CharacteristicHandler>,
    discover_descriptors: Option<DescriptorDiscoveryHandler>,
    descriptor_update: Option<DescriptorHandler>,
    descriptor_write: Option<DescriptorHandler>,
    ready: Option<ReadyHandler>,
    rssi: Option<RssiHandler>,
    l2cap: Option<L2capHandler>,
}

impl PeripheralCallbacks {
    /// Creates an empty closure-based adapter for `CBPeripheralDelegate`.
    pub fn new() -> Self {
        Self {
            name: None,
            modify_services: None,
            services: None,
            included_services: None,
            characteristics: None,
            update_value: None,
            write_value: None,
            update_notification: None,
            discover_descriptors: None,
            descriptor_update: None,
            descriptor_write: None,
            ready: None,
            rssi: None,
            l2cap: None,
        }
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_name_update(mut self, callback: impl FnMut() + Send + 'static) -> Self {
        self.name = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_modify_services(
        mut self,
        callback: impl FnMut(Vec<Service>) + Send + 'static,
    ) -> Self {
        self.modify_services = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_services(
        mut self,
        callback: impl FnMut(Vec<Service>, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.services = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_included_services(
        mut self,
        callback: impl FnMut(Service, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.included_services = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_characteristics(
        mut self,
        callback: impl FnMut(Service, Vec<Characteristic>, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.characteristics = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_value_update(
        mut self,
        callback: impl FnMut(Characteristic, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.update_value = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_write(
        mut self,
        callback: impl FnMut(Characteristic, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.write_value = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_notification_state(
        mut self,
        callback: impl FnMut(Characteristic, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.update_notification = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_discover_descriptors(
        mut self,
        callback: impl FnMut(Characteristic, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.discover_descriptors = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_descriptor_value_update(
        mut self,
        callback: impl FnMut(Descriptor, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.descriptor_update = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_descriptor_write(
        mut self,
        callback: impl FnMut(Descriptor, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.descriptor_write = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_ready_to_send(mut self, callback: impl FnMut() + Send + 'static) -> Self {
        self.ready = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_rssi(
        mut self,
        callback: impl FnMut(i32, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.rssi = Some(Box::new(callback));
        self
    }

    /// Registers a closure for the corresponding callback from `CBPeripheralDelegate`.
    pub fn on_l2cap_channel(
        mut self,
        callback: impl FnMut(Option<L2capChannel>, Option<BluetoothErrorInfo>) + Send + 'static,
    ) -> Self {
        self.l2cap = Some(Box::new(callback));
        self
    }
}

impl Default for PeripheralCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

impl private::Sealed for PeripheralCallbacks {}
impl PeripheralDelegate for PeripheralCallbacks {
    fn did_update_name(&mut self) {
        if let Some(callback) = &mut self.name {
            callback();
        }
    }

    fn did_modify_services(&mut self, invalidated_services: Vec<Service>) {
        if let Some(callback) = &mut self.modify_services {
            callback(invalidated_services);
        }
    }

    fn did_discover_services(&mut self, services: Vec<Service>, error: Option<BluetoothErrorInfo>) {
        if let Some(callback) = &mut self.services {
            callback(services, error);
        }
    }

    fn did_discover_included_services_for_service(
        &mut self,
        service: Service,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.included_services {
            callback(service, error);
        }
    }

    fn did_discover_characteristics_for_service(
        &mut self,
        service: Service,
        characteristics: Vec<Characteristic>,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.characteristics {
            callback(service, characteristics, error);
        }
    }

    fn did_update_value_for_characteristic(
        &mut self,
        characteristic: Characteristic,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.update_value {
            callback(characteristic, error);
        }
    }

    fn did_write_value_for_characteristic(
        &mut self,
        characteristic: Characteristic,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.write_value {
            callback(characteristic, error);
        }
    }

    fn did_update_notification_state_for_characteristic(
        &mut self,
        characteristic: Characteristic,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.update_notification {
            callback(characteristic, error);
        }
    }

    fn did_discover_descriptors_for_characteristic(
        &mut self,
        characteristic: Characteristic,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.discover_descriptors {
            callback(characteristic, error);
        }
    }

    fn did_update_value_for_descriptor(
        &mut self,
        descriptor: Descriptor,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.descriptor_update {
            callback(descriptor, error);
        }
    }

    fn did_write_value_for_descriptor(
        &mut self,
        descriptor: Descriptor,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.descriptor_write {
            callback(descriptor, error);
        }
    }

    fn is_ready_to_send_write_without_response(&mut self) {
        if let Some(callback) = &mut self.ready {
            callback();
        }
    }

    fn did_read_rssi(&mut self, rssi: i32, error: Option<BluetoothErrorInfo>) {
        if let Some(callback) = &mut self.rssi {
            callback(rssi, error);
        }
    }

    fn did_open_l2cap_channel(
        &mut self,
        channel: Option<L2capChannel>,
        error: Option<BluetoothErrorInfo>,
    ) {
        if let Some(callback) = &mut self.l2cap {
            callback(channel, error);
        }
    }
}

struct CallbackState {
    delegate: Mutex<Box<dyn PeripheralDelegate>>,
}

/// Wraps `CBPeripheral`.
pub struct Peripheral {
    pub(crate) raw: *mut c_void,
    callback_state: Option<Box<CallbackState>>,
}

#[allow(clippy::too_many_lines)]
unsafe extern "C" fn peripheral_event_trampoline(
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
        let Ok(payload): Result<PeripheralEventPayload, _> = serde_json::from_str(&payload_json)
        else {
            return;
        };

        let mut delegate = match state.delegate.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        match payload.event.as_str() {
            "didUpdateName" => delegate.did_update_name(),
            "didModifyServices" => delegate.did_modify_services(
                payload
                    .invalidated_service_handles
                    .unwrap_or_default()
                    .into_iter()
                    .map(Service::from_retained_handle)
                    .collect(),
            ),
            "didDiscoverServices" => delegate.did_discover_services(
                payload
                    .service_handles
                    .unwrap_or_default()
                    .into_iter()
                    .map(Service::from_retained_handle)
                    .collect(),
                payload.error,
            ),
            "didDiscoverIncludedServicesForService" => {
                if let Some(service_handle) = payload.service_handle {
                    delegate.did_discover_included_services_for_service(
                        Service::from_retained_handle(service_handle),
                        payload.error,
                    );
                }
            }
            "didDiscoverCharacteristicsForService" => {
                if let Some(service_handle) = payload.service_handle {
                    delegate.did_discover_characteristics_for_service(
                        Service::from_retained_handle(service_handle),
                        payload
                            .characteristic_handles
                            .unwrap_or_default()
                            .into_iter()
                            .map(Characteristic::from_retained_handle)
                            .collect(),
                        payload.error,
                    );
                }
            }
            "didUpdateValueForCharacteristic" => {
                if let Some(characteristic_handle) = payload.characteristic_handle {
                    delegate.did_update_value_for_characteristic(
                        Characteristic::from_retained_handle(characteristic_handle),
                        payload.error,
                    );
                }
            }
            "didWriteValueForCharacteristic" => {
                if let Some(characteristic_handle) = payload.characteristic_handle {
                    delegate.did_write_value_for_characteristic(
                        Characteristic::from_retained_handle(characteristic_handle),
                        payload.error,
                    );
                }
            }
            "didUpdateNotificationStateForCharacteristic" => {
                if let Some(characteristic_handle) = payload.characteristic_handle {
                    delegate.did_update_notification_state_for_characteristic(
                        Characteristic::from_retained_handle(characteristic_handle),
                        payload.error,
                    );
                }
            }
            "didDiscoverDescriptorsForCharacteristic" => {
                if let Some(characteristic_handle) = payload.characteristic_handle {
                    delegate.did_discover_descriptors_for_characteristic(
                        Characteristic::from_retained_handle(characteristic_handle),
                        payload.error,
                    );
                }
            }
            "didUpdateValueForDescriptor" => {
                if let Some(descriptor_handle) = payload.descriptor_handle {
                    delegate.did_update_value_for_descriptor(
                        Descriptor::from_retained_handle(descriptor_handle),
                        payload.error,
                    );
                }
            }
            "didWriteValueForDescriptor" => {
                if let Some(descriptor_handle) = payload.descriptor_handle {
                    delegate.did_write_value_for_descriptor(
                        Descriptor::from_retained_handle(descriptor_handle),
                        payload.error,
                    );
                }
            }
            "isReadyToSendWriteWithoutResponse" => {
                delegate.is_ready_to_send_write_without_response();
            }
            "didReadRSSI" => {
                delegate.did_read_rssi(payload.rssi.unwrap_or_default(), payload.error);
            }
            "didOpenL2CAPChannel" => {
                delegate.did_open_l2cap_channel(
                    payload
                        .channel_handle
                        .map(L2capChannel::from_retained_handle),
                    payload.error,
                );
            }
            _ => {}
        }
    }));
}

impl Peripheral {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self {
            raw,
            callback_state: None,
        }
    }

    pub(crate) fn from_retained_handle(handle: u64) -> Self {
        Self::from_retained_raw(retained_handle_to_raw(handle))
    }

    #[cfg(feature = "async")]
    pub(crate) const fn as_raw(&self) -> *mut c_void {
        self.raw
    }

    /// Registers a delegate implementing `PeripheralDelegate` for this `CBPeripheral`.
    pub fn set_delegate<D>(&mut self, delegate: D) -> Result<(), CoreBluetoothError>
    where
        D: PeripheralDelegate + 'static,
    {
        let callback_state = Box::new(CallbackState {
            delegate: Mutex::new(Box::new(delegate)),
        });
        let mut error = core::ptr::null_mut();
        let mut callback_state = Some(callback_state);
        let user_info = callback_state
            .as_deref_mut()
            .map_or(core::ptr::null_mut(), |state| {
                std::ptr::from_mut::<CallbackState>(state).cast::<c_void>()
            });
        let status = unsafe {
            ffi::cb_peripheral_set_delegate(
                self.raw,
                Some(peripheral_event_trampoline as ffi::JsonCallback),
                user_info,
                &mut error,
            )
        };
        if status == ffi::status::OK {
            self.callback_state = callback_state;
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Registers `PeripheralCallbacks` for this `CBPeripheral`.
    pub fn set_callbacks(
        &mut self,
        callbacks: PeripheralCallbacks,
    ) -> Result<(), CoreBluetoothError> {
        self.set_delegate(callbacks)
    }

    /// Clears the current `CBPeripheralDelegate` bridge.
    pub fn clear_delegate(&mut self) {
        unsafe { ffi::cb_peripheral_clear_delegate(self.raw) };
        self.callback_state = None;
    }

    /// Returns the name exposed by `CBPeripheral`.
    pub fn name(&self) -> String {
        let ptr = unsafe { ffi::cb_peripheral_name(self.raw) };
        take_owned_c_string(ptr)
    }

    /// Returns the identifier exposed by `CBPeripheral`.
    pub fn identifier(&self) -> String {
        let ptr = unsafe { ffi::cb_peripheral_identifier(self.raw) };
        take_owned_c_string(ptr)
    }

    /// Returns the current `CBPeripheralState`.
    pub fn state(&self) -> PeripheralState {
        PeripheralState::from_raw(unsafe { ffi::cb_peripheral_state(self.raw) })
    }

    /// Returns the services exposed by `CBPeripheral`.
    pub fn services(&self) -> Vec<Service> {
        let mut array = core::ptr::null_mut();
        let mut count = 0;
        unsafe { ffi::cb_peripheral_services(self.raw, &mut array, &mut count) };
        take_retained_pointer_array(array, count)
            .into_iter()
            .map(Service::from_retained_raw)
            .collect()
    }

    /// Returns `CBPeripheral.canSendWriteWithoutResponse`.
    pub fn can_send_write_without_response(&self) -> bool {
        unsafe { ffi::cb_peripheral_can_send_write_without_response(self.raw) }
    }

    /// Invokes `discoverServices:`.
    pub fn discover_services(
        &self,
        service_uuids: Option<&[&str]>,
    ) -> Result<(), CoreBluetoothError> {
        let service_uuids = match service_uuids {
            Some(service_uuids) => Some(encode_string_slice(service_uuids)?),
            None => None,
        };
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_peripheral_discover_services(
                self.raw,
                service_uuids
                    .as_ref()
                    .map_or(core::ptr::null(), |value| value.as_ptr()),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `discoverIncludedServices:forService:`.
    pub fn discover_included_services(
        &self,
        service: &Service,
        included_service_uuids: Option<&[&str]>,
    ) -> Result<(), CoreBluetoothError> {
        let included_service_uuids = match included_service_uuids {
            Some(service_uuids) => Some(encode_string_slice(service_uuids)?),
            None => None,
        };
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_peripheral_discover_included_services(
                self.raw,
                service.raw,
                included_service_uuids
                    .as_ref()
                    .map_or(core::ptr::null(), |value| value.as_ptr()),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `readRSSI`.
    pub fn read_rssi(&self) -> Result<(), CoreBluetoothError> {
        let mut error = core::ptr::null_mut();
        let status = unsafe { ffi::cb_peripheral_read_rssi(self.raw, &mut error) };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `discoverCharacteristics:forService:`.
    pub fn discover_characteristics(
        &self,
        service: &Service,
        characteristic_uuids: Option<&[&str]>,
    ) -> Result<(), CoreBluetoothError> {
        let characteristic_uuids = match characteristic_uuids {
            Some(characteristic_uuids) => Some(encode_string_slice(characteristic_uuids)?),
            None => None,
        };
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_peripheral_discover_characteristics(
                self.raw,
                service.raw,
                characteristic_uuids
                    .as_ref()
                    .map_or(core::ptr::null(), |value| value.as_ptr()),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `readValueForCharacteristic:`.
    pub fn read_value_for_characteristic(
        &self,
        characteristic: &Characteristic,
    ) -> Result<(), CoreBluetoothError> {
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_peripheral_read_value_for_characteristic(
                self.raw,
                characteristic.raw,
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `maximumWriteValueLengthForType:`.
    pub fn maximum_write_value_length(&self, write_type: CharacteristicWriteType) -> usize {
        unsafe { ffi::cb_peripheral_maximum_write_value_length(self.raw, write_type as i32) }
    }

    /// Invokes `writeValue:forCharacteristic:type:`.
    pub fn write_value_for_characteristic(
        &self,
        characteristic: &Characteristic,
        value: &[u8],
        write_type: CharacteristicWriteType,
    ) -> Result<(), CoreBluetoothError> {
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_peripheral_write_value_for_characteristic(
                self.raw,
                characteristic.raw,
                value.as_ptr(),
                value.len(),
                matches!(write_type, CharacteristicWriteType::WithResponse),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `setNotifyValue:forCharacteristic:`.
    pub fn set_notify_value(
        &self,
        characteristic: &Characteristic,
        enabled: bool,
    ) -> Result<(), CoreBluetoothError> {
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_peripheral_set_notify_value(self.raw, characteristic.raw, enabled, &mut error)
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `discoverDescriptorsForCharacteristic:`.
    pub fn discover_descriptors(
        &self,
        characteristic: &Characteristic,
    ) -> Result<(), CoreBluetoothError> {
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_peripheral_discover_descriptors(self.raw, characteristic.raw, &mut error)
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `readValueForDescriptor:`.
    pub fn read_value_for_descriptor(
        &self,
        descriptor: &Descriptor,
    ) -> Result<(), CoreBluetoothError> {
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_peripheral_read_value_for_descriptor(self.raw, descriptor.raw, &mut error)
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `writeValue:forDescriptor:`.
    pub fn write_value_for_descriptor(
        &self,
        descriptor: &Descriptor,
        value: &[u8],
    ) -> Result<(), CoreBluetoothError> {
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_peripheral_write_value_for_descriptor(
                self.raw,
                descriptor.raw,
                value.as_ptr(),
                value.len(),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Invokes `openL2CAPChannel:`.
    pub fn open_l2cap_channel(&self, psm: u16) -> Result<(), CoreBluetoothError> {
        let mut error = core::ptr::null_mut();
        let status = unsafe { ffi::cb_peripheral_open_l2cap_channel(self.raw, psm, &mut error) };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }
}

impl Clone for Peripheral {
    fn clone(&self) -> Self {
        Self::from_retained_raw(unsafe { ffi::cb_object_retain(self.raw) })
    }
}

impl Drop for Peripheral {
    fn drop(&mut self) {
        if self.callback_state.is_some() {
            unsafe { ffi::cb_peripheral_clear_delegate(self.raw) };
        }
        unsafe { ffi::cb_object_release(self.raw) };
    }
}
