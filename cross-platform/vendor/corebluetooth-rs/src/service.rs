use core::ffi::c_void;

use crate::characteristic::Characteristic;
use crate::error::take_owned_c_string;
use crate::ffi;
use crate::peripheral::Peripheral;
use crate::private::{retain_raw, retained_handle_to_raw, take_retained_pointer_array};
use crate::uuid::BluetoothUuid;

/// Wraps `CBService`.
pub struct Service {
    pub(crate) raw: *mut c_void,
}

impl Service {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    pub(crate) fn from_retained_handle(handle: u64) -> Self {
        Self::from_retained_raw(retained_handle_to_raw(handle))
    }

    /// Returns the UUID string exposed by `CBService`.
    pub fn uuid(&self) -> String {
        let ptr = unsafe { ffi::cb_service_uuid(self.raw) };
        take_owned_c_string(ptr)
    }

    /// Returns the `CBUUID` exposed by `CBService`.
    pub fn uuid_object(&self) -> BluetoothUuid {
        BluetoothUuid::from_retained_raw(unsafe { ffi::cb_service_uuid_handle(self.raw) })
    }

    /// Returns the owning `CBPeripheral`, if `CoreBluetooth` still exposes one.
    pub fn peripheral(&self) -> Option<Peripheral> {
        let raw = unsafe { ffi::cb_service_peripheral(self.raw) };
        (!raw.is_null()).then(|| Peripheral::from_retained_raw(raw))
    }

    /// Returns whether `CBService.isPrimary` is set.
    pub fn is_primary(&self) -> bool {
        unsafe { ffi::cb_service_is_primary(self.raw) }
    }

    /// Returns the included services exposed by `CBService`.
    pub fn included_services(&self) -> Vec<Self> {
        let mut array = core::ptr::null_mut();
        let mut count = 0;
        unsafe { ffi::cb_service_included_services(self.raw, &mut array, &mut count) };
        take_retained_pointer_array(array, count)
            .into_iter()
            .map(Self::from_retained_raw)
            .collect()
    }

    /// Returns the characteristics exposed by `CBService`.
    pub fn characteristics(&self) -> Vec<Characteristic> {
        let mut array = core::ptr::null_mut();
        let mut count = 0;
        unsafe { ffi::cb_service_characteristics(self.raw, &mut array, &mut count) };
        take_retained_pointer_array(array, count)
            .into_iter()
            .map(Characteristic::from_retained_raw)
            .collect()
    }
}

impl Clone for Service {
    fn clone(&self) -> Self {
        Self::from_retained_raw(retain_raw(self.raw))
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        unsafe { ffi::cb_object_release(self.raw) };
    }
}
