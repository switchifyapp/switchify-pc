use core::ffi::c_void;

use crate::characteristic::{AttributePermissions, Characteristic, CharacteristicProperties};
use crate::descriptor::{Descriptor, MutableDescriptor};
use crate::error::{from_swift, CoreBluetoothError};
use crate::ffi;
use crate::peripheral_manager::Central;
use crate::private::{retain_raw, take_retained_pointer_array};
use crate::uuid::BluetoothUuid;

/// Wraps `CBMutableCharacteristic`.
pub struct MutableCharacteristic {
    pub(crate) raw: *mut c_void,
}

impl MutableCharacteristic {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    /// Creates a new `CBMutableCharacteristic` wrapper.
    pub fn new(
        uuid: &BluetoothUuid,
        properties: CharacteristicProperties,
        value: Option<&[u8]>,
        permissions: AttributePermissions,
    ) -> Result<Self, CoreBluetoothError> {
        let mut raw = core::ptr::null_mut();
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_mutable_characteristic_new(
                uuid.raw,
                properties.bits(),
                value.map_or(core::ptr::null(), <[u8]>::as_ptr),
                value.map_or(0, <[u8]>::len),
                permissions.bits(),
                &mut raw,
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(Self { raw })
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Returns this mutable characteristic as an immutable `CBCharacteristic` view.
    pub fn as_characteristic(&self) -> Characteristic {
        Characteristic::from_retained_raw(retain_raw(self.raw))
    }

    /// Returns the UUID string exposed by `CBMutableCharacteristic`.
    pub fn uuid(&self) -> String {
        self.as_characteristic().uuid()
    }

    /// Returns the `CBUUID` exposed by `CBMutableCharacteristic`.
    pub fn uuid_object(&self) -> BluetoothUuid {
        self.as_characteristic().uuid_object()
    }

    /// Returns the `CBCharacteristicProperties` exposed by `CBMutableCharacteristic`.
    pub fn properties(&self) -> CharacteristicProperties {
        self.as_characteristic().properties()
    }

    /// Sets the `CBCharacteristicProperties` exposed by `CBMutableCharacteristic`.
    pub fn set_properties(&mut self, properties: CharacteristicProperties) {
        unsafe { ffi::cb_mutable_characteristic_set_properties(self.raw, properties.bits()) };
    }

    /// Returns the `CBAttributePermissions` exposed by `CBMutableCharacteristic`.
    pub fn permissions(&self) -> AttributePermissions {
        AttributePermissions::from_bits(unsafe {
            ffi::cb_mutable_characteristic_permissions(self.raw)
        })
    }

    /// Sets the `CBAttributePermissions` exposed by `CBMutableCharacteristic`.
    pub fn set_permissions(&mut self, permissions: AttributePermissions) {
        unsafe { ffi::cb_mutable_characteristic_set_permissions(self.raw, permissions.bits()) };
    }

    /// Returns the value exposed by `CBMutableCharacteristic`.
    pub fn value(&self) -> Result<Option<Vec<u8>>, CoreBluetoothError> {
        self.as_characteristic().value()
    }

    /// Sets the value exposed by `CBMutableCharacteristic`.
    pub fn set_value(&mut self, value: Option<&[u8]>) {
        unsafe {
            ffi::cb_mutable_characteristic_set_value(
                self.raw,
                value.map_or(core::ptr::null(), <[u8]>::as_ptr),
                value.map_or(0, <[u8]>::len),
            );
        };
    }

    /// Returns the descriptors attached to `CBMutableCharacteristic`.
    pub fn descriptors(&self) -> Vec<Descriptor> {
        self.as_characteristic().descriptors()
    }

    /// Sets mutable descriptor children on `CBMutableCharacteristic.descriptors`.
    pub fn set_descriptors(
        &mut self,
        descriptors: &[&MutableDescriptor],
    ) -> Result<(), CoreBluetoothError> {
        let descriptors: Vec<*mut c_void> = descriptors
            .iter()
            .map(|descriptor| descriptor.raw)
            .collect();
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_mutable_characteristic_set_descriptors(
                self.raw,
                descriptors.as_ptr(),
                descriptors.len(),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Sets immutable descriptor views on `CBMutableCharacteristic.descriptors`.
    pub fn set_descriptor_views(
        &mut self,
        descriptors: &[&Descriptor],
    ) -> Result<(), CoreBluetoothError> {
        let descriptors: Vec<*mut c_void> = descriptors
            .iter()
            .map(|descriptor| descriptor.raw)
            .collect();
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_mutable_characteristic_set_descriptors(
                self.raw,
                descriptors.as_ptr(),
                descriptors.len(),
                &mut error,
            )
        };
        if status == ffi::status::OK {
            Ok(())
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Returns the subscribed centrals exposed by `CBMutableCharacteristic.subscribedCentrals`.
    pub fn subscribed_centrals(&self) -> Vec<Central> {
        let mut array = core::ptr::null_mut();
        let mut count = 0;
        unsafe {
            ffi::cb_mutable_characteristic_subscribed_centrals(self.raw, &mut array, &mut count);
        }
        take_retained_pointer_array(array, count)
            .into_iter()
            .map(Central::from_retained_raw)
            .collect()
    }
}

impl Clone for MutableCharacteristic {
    fn clone(&self) -> Self {
        Self::from_retained_raw(retain_raw(self.raw))
    }
}

impl Drop for MutableCharacteristic {
    fn drop(&mut self) {
        unsafe { ffi::cb_object_release(self.raw) };
    }
}
