use core::ffi::c_void;

use crate::characteristic::Characteristic;
use crate::error::{from_swift, CoreBluetoothError};
use crate::ffi;
use crate::mutable_characteristic::MutableCharacteristic;
use crate::private::{retain_raw, retained_handle_to_raw};
use crate::service::Service;
use crate::uuid::BluetoothUuid;

/// Wraps `CBMutableService`.
pub struct MutableService {
    pub(crate) raw: *mut c_void,
}

impl MutableService {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    pub(crate) fn from_retained_handle(handle: u64) -> Self {
        Self::from_retained_raw(retained_handle_to_raw(handle))
    }

    /// Creates a new `CBMutableService` wrapper.
    pub fn new(uuid: &BluetoothUuid, is_primary: bool) -> Result<Self, CoreBluetoothError> {
        let mut raw = core::ptr::null_mut();
        let mut error = core::ptr::null_mut();
        let status =
            unsafe { ffi::cb_mutable_service_new(uuid.raw, is_primary, &mut raw, &mut error) };
        if status == ffi::status::OK {
            Ok(Self { raw })
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Returns this mutable service as an immutable `CBService` view.
    pub fn as_service(&self) -> Service {
        Service::from_retained_raw(retain_raw(self.raw))
    }

    /// Returns the UUID string exposed by `CBMutableService`.
    pub fn uuid(&self) -> String {
        self.as_service().uuid()
    }

    /// Returns the `CBUUID` exposed by `CBMutableService`.
    pub fn uuid_object(&self) -> BluetoothUuid {
        self.as_service().uuid_object()
    }

    /// Returns whether `CBMutableService.isPrimary` is set.
    pub fn is_primary(&self) -> bool {
        self.as_service().is_primary()
    }

    /// Returns the included services currently attached to this `CBMutableService`.
    pub fn included_services(&self) -> Vec<Service> {
        self.as_service().included_services()
    }

    /// Returns the characteristics currently attached to this `CBMutableService`.
    pub fn characteristics(&self) -> Vec<Characteristic> {
        self.as_service().characteristics()
    }

    /// Sets the included mutable services exposed by `CBMutableService.includedServices`.
    pub fn set_included_services(&mut self, services: &[&Self]) -> Result<(), CoreBluetoothError> {
        let services: Vec<*mut c_void> = services.iter().map(|service| service.raw).collect();
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_mutable_service_set_included_services(
                self.raw,
                services.as_ptr(),
                services.len(),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Sets immutable service views on `CBMutableService.includedServices`.
    pub fn set_included_service_views(
        &mut self,
        services: &[&Service],
    ) -> Result<(), CoreBluetoothError> {
        let services: Vec<*mut c_void> = services.iter().map(|service| service.raw).collect();
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_mutable_service_set_included_services(
                self.raw,
                services.as_ptr(),
                services.len(),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Sets the mutable characteristics exposed by `CBMutableService.characteristics`.
    pub fn set_characteristics(
        &mut self,
        characteristics: &[&MutableCharacteristic],
    ) -> Result<(), CoreBluetoothError> {
        let characteristics: Vec<*mut c_void> = characteristics
            .iter()
            .map(|characteristic| characteristic.raw)
            .collect();
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_mutable_service_set_characteristics(
                self.raw,
                characteristics.as_ptr(),
                characteristics.len(),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Sets immutable characteristic views on `CBMutableService.characteristics`.
    pub fn set_characteristic_views(
        &mut self,
        characteristics: &[&Characteristic],
    ) -> Result<(), CoreBluetoothError> {
        let characteristics: Vec<*mut c_void> = characteristics
            .iter()
            .map(|characteristic| characteristic.raw)
            .collect();
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_mutable_service_set_characteristics(
                self.raw,
                characteristics.as_ptr(),
                characteristics.len(),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }
}

impl Clone for MutableService {
    fn clone(&self) -> Self {
        Self::from_retained_raw(retain_raw(self.raw))
    }
}

impl Drop for MutableService {
    fn drop(&mut self) {
        unsafe { ffi::cb_object_release(self.raw) };
    }
}
