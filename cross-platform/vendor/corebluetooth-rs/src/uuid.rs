use core::ffi::c_void;
use core::fmt;
use std::hash::{Hash, Hasher};

use serde_json::Value;

use crate::error::{take_owned_c_string, CoreBluetoothError};
use crate::ffi;
use crate::private::{decode_json, retain_raw, to_cstring};

const UUID_CONSTANT_EXTENDED_PROPERTIES: i32 = 0;
const UUID_CONSTANT_USER_DESCRIPTION: i32 = 1;
const UUID_CONSTANT_CLIENT_CONFIGURATION: i32 = 2;
const UUID_CONSTANT_SERVER_CONFIGURATION: i32 = 3;
const UUID_CONSTANT_FORMAT: i32 = 4;
const UUID_CONSTANT_AGGREGATE_FORMAT: i32 = 5;
const UUID_CONSTANT_VALID_RANGE: i32 = 6;
const UUID_CONSTANT_OBSERVATION_SCHEDULE: i32 = 7;
const UUID_CONSTANT_L2CAP_PSM: i32 = 8;

#[must_use]
/// Wraps `CBUUID`.
pub struct BluetoothUuid {
    pub(crate) raw: *mut c_void,
}

impl BluetoothUuid {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    /// Creates a `CBUUID` from its string representation.
    pub fn from_string(value: &str) -> Result<Self, CoreBluetoothError> {
        let value = to_cstring(value)?;
        Ok(Self::from_retained_raw(unsafe {
            ffi::cb_uuid_new_from_string(value.as_ptr())
        }))
    }

    /// Creates a `CBUUID` from the canonical UUID string accepted by `UUIDWithNSUUID:`.
    pub fn from_uuid_string(value: &str) -> Result<Self, CoreBluetoothError> {
        Self::from_string(value)
    }

    /// Creates a `CBUUID` from raw byte data.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_retained_raw(unsafe { ffi::cb_uuid_new_from_bytes(bytes.as_ptr(), bytes.len()) })
    }

    /// Creates a `CBUUID` from raw byte data.
    pub fn from_data(bytes: &[u8]) -> Self {
        Self::from_bytes(bytes)
    }

    fn constant(kind: i32) -> Self {
        let value = take_owned_c_string(unsafe { ffi::cb_uuid_constant_string(kind) });
        Self::from_string(&value)
            .expect("CoreBluetooth UUID constants never contain interior NUL bytes")
    }

    /// Returns the well-known extended-properties descriptor `CBUUID`.
    pub fn characteristic_extended_properties() -> Self {
        Self::constant(UUID_CONSTANT_EXTENDED_PROPERTIES)
    }

    /// Returns the well-known user-description descriptor `CBUUID`.
    pub fn characteristic_user_description() -> Self {
        Self::constant(UUID_CONSTANT_USER_DESCRIPTION)
    }

    /// Returns the well-known client-characteristic-configuration descriptor `CBUUID`.
    pub fn client_characteristic_configuration() -> Self {
        Self::constant(UUID_CONSTANT_CLIENT_CONFIGURATION)
    }

    /// Returns the well-known server-characteristic-configuration descriptor `CBUUID`.
    pub fn server_characteristic_configuration() -> Self {
        Self::constant(UUID_CONSTANT_SERVER_CONFIGURATION)
    }

    /// Returns the well-known characteristic-format descriptor `CBUUID`.
    pub fn characteristic_format() -> Self {
        Self::constant(UUID_CONSTANT_FORMAT)
    }

    /// Returns the well-known characteristic-aggregate-format descriptor `CBUUID`.
    pub fn characteristic_aggregate_format() -> Self {
        Self::constant(UUID_CONSTANT_AGGREGATE_FORMAT)
    }

    /// Returns the well-known valid-range descriptor `CBUUID`.
    pub fn characteristic_valid_range() -> Self {
        Self::constant(UUID_CONSTANT_VALID_RANGE)
    }

    /// Returns the well-known observation-schedule descriptor `CBUUID`.
    pub fn characteristic_observation_schedule() -> Self {
        Self::constant(UUID_CONSTANT_OBSERVATION_SCHEDULE)
    }

    /// Returns the well-known L2CAP PSM characteristic `CBUUID`.
    pub fn l2cap_psm_characteristic() -> Self {
        Self::constant(UUID_CONSTANT_L2CAP_PSM)
    }

    /// Returns the canonical string form of this `CBUUID`.
    pub fn uuid_string(&self) -> String {
        let ptr = unsafe { ffi::cb_uuid_string(self.raw) };
        take_owned_c_string(ptr)
    }

    /// Returns the raw byte data exposed by `CBUUID.data`.
    pub fn data(&self) -> Result<Vec<u8>, CoreBluetoothError> {
        let json = unsafe { ffi::cb_uuid_data_json(self.raw) };
        let value: Value = decode_json(json)?;
        Ok(match value {
            Value::Array(items) => items
                .into_iter()
                .filter_map(|item| item.as_u64())
                .filter_map(|item| u8::try_from(item).ok())
                .collect(),
            _ => Vec::new(),
        })
    }
}

impl Clone for BluetoothUuid {
    fn clone(&self) -> Self {
        Self::from_retained_raw(retain_raw(self.raw))
    }
}

impl Drop for BluetoothUuid {
    fn drop(&mut self) {
        unsafe { ffi::cb_object_release(self.raw) };
    }
}

impl fmt::Debug for BluetoothUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BluetoothUuid")
            .field(&self.uuid_string())
            .finish()
    }
}

impl fmt::Display for BluetoothUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.uuid_string())
    }
}

impl PartialEq for BluetoothUuid {
    fn eq(&self, other: &Self) -> bool {
        self.uuid_string() == other.uuid_string()
    }
}

impl Eq for BluetoothUuid {}

impl Hash for BluetoothUuid {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uuid_string().hash(state);
    }
}
