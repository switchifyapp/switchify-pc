use core::ffi::c_void;

use crate::descriptor::Descriptor;
use crate::error::{take_owned_c_string, CoreBluetoothError};
use crate::ffi;
use crate::private::{
    decode_optional_json, retain_raw, retained_handle_to_raw, take_retained_pointer_array,
};
use crate::service::Service;
use crate::uuid::BluetoothUuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
/// Mirrors `CBCharacteristicWriteType`.
pub enum CharacteristicWriteType {
    /// Corresponds to `CBCharacteristicWriteWithResponse`.
    WithResponse = 0,
    /// Corresponds to `CBCharacteristicWriteWithoutResponse`.
    WithoutResponse = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Bitflags mirroring `CBCharacteristicProperties`.
pub struct CharacteristicProperties(u64);

impl CharacteristicProperties {
    /// Corresponds to the `CBCharacteristicPropertyBroadcast` bit.
    pub const BROADCAST: Self = Self(0x01);
    /// Corresponds to the `CBCharacteristicPropertyRead` bit.
    pub const READ: Self = Self(0x02);
    /// Corresponds to the `CBCharacteristicPropertyWriteWithoutResponse` bit.
    pub const WRITE_WITHOUT_RESPONSE: Self = Self(0x04);
    /// Corresponds to the `CBCharacteristicPropertyWrite` bit.
    pub const WRITE: Self = Self(0x08);
    /// Corresponds to the `CBCharacteristicPropertyNotify` bit.
    pub const NOTIFY: Self = Self(0x10);
    /// Corresponds to the `CBCharacteristicPropertyIndicate` bit.
    pub const INDICATE: Self = Self(0x20);
    /// Corresponds to the `CBCharacteristicPropertyAuthenticatedSignedWrites` bit.
    pub const AUTHENTICATED_SIGNED_WRITES: Self = Self(0x40);
    /// Corresponds to the `CBCharacteristicPropertyExtendedProperties` bit.
    pub const EXTENDED_PROPERTIES: Self = Self(0x80);
    /// Corresponds to the `CBCharacteristicPropertyNotifyEncryptionRequired` bit.
    pub const NOTIFY_ENCRYPTION_REQUIRED: Self = Self(0x100);
    /// Corresponds to the `CBCharacteristicPropertyIndicateEncryptionRequired` bit.
    pub const INDICATE_ENCRYPTION_REQUIRED: Self = Self(0x200);

    /// Builds `CharacteristicProperties` from the raw `CoreBluetooth` bit pattern.
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Returns the raw `CoreBluetooth` bit pattern.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Returns whether all bits in `other` are set.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Bitflags mirroring `CBAttributePermissions`.
pub struct AttributePermissions(u64);

impl AttributePermissions {
    /// Corresponds to the readable `CBAttributePermissions` bit.
    pub const READABLE: Self = Self(0x01);
    /// Corresponds to the writable `CBAttributePermissions` bit.
    pub const WRITEABLE: Self = Self(0x02);
    /// Corresponds to the read-encryption-required `CBAttributePermissions` bit.
    pub const READ_ENCRYPTION_REQUIRED: Self = Self(0x04);
    /// Corresponds to the write-encryption-required `CBAttributePermissions` bit.
    pub const WRITE_ENCRYPTION_REQUIRED: Self = Self(0x08);

    /// Builds `AttributePermissions` from the raw `CoreBluetooth` bit pattern.
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Returns the raw `CoreBluetooth` bit pattern.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Returns whether all bits in `other` are set.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Wraps `CBCharacteristic`.
pub struct Characteristic {
    pub(crate) raw: *mut c_void,
}

impl Characteristic {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    pub(crate) fn from_retained_handle(handle: u64) -> Self {
        Self::from_retained_raw(retained_handle_to_raw(handle))
    }

    /// Returns the UUID string exposed by `CBCharacteristic`.
    pub fn uuid(&self) -> String {
        let ptr = unsafe { ffi::cb_characteristic_uuid(self.raw) };
        take_owned_c_string(ptr)
    }

    /// Returns the `CBUUID` exposed by `CBCharacteristic`.
    pub fn uuid_object(&self) -> BluetoothUuid {
        BluetoothUuid::from_retained_raw(unsafe { ffi::cb_characteristic_uuid_handle(self.raw) })
    }

    /// Returns the owning `CBService`, if `CoreBluetooth` still exposes one.
    pub fn service(&self) -> Option<Service> {
        let raw = unsafe { ffi::cb_characteristic_service(self.raw) };
        (!raw.is_null()).then(|| Service::from_retained_raw(raw))
    }

    /// Returns the `CBCharacteristicProperties` exposed by `CBCharacteristic`.
    pub fn properties(&self) -> CharacteristicProperties {
        CharacteristicProperties::from_bits(unsafe { ffi::cb_characteristic_properties(self.raw) })
    }

    /// Returns the value exposed by `CBCharacteristic`.
    pub fn value(&self) -> Result<Option<Vec<u8>>, CoreBluetoothError> {
        let json = unsafe { ffi::cb_characteristic_value_json(self.raw) };
        decode_optional_json(json)
    }

    /// Returns whether `CBCharacteristic.isNotifying` is set.
    pub fn is_notifying(&self) -> bool {
        unsafe { ffi::cb_characteristic_is_notifying(self.raw) }
    }

    /// Returns the descriptors exposed by `CBCharacteristic`.
    pub fn descriptors(&self) -> Vec<Descriptor> {
        let mut array = core::ptr::null_mut();
        let mut count = 0;
        unsafe { ffi::cb_characteristic_descriptors(self.raw, &mut array, &mut count) };
        take_retained_pointer_array(array, count)
            .into_iter()
            .map(Descriptor::from_retained_raw)
            .collect()
    }
}

impl Clone for Characteristic {
    fn clone(&self) -> Self {
        Self::from_retained_raw(retain_raw(self.raw))
    }
}

impl Drop for Characteristic {
    fn drop(&mut self) {
        unsafe { ffi::cb_object_release(self.raw) };
    }
}
