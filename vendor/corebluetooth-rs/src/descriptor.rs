use core::ffi::c_void;

use serde::Serialize;
use serde_json::Value;

use crate::characteristic::Characteristic;
use crate::error::{from_swift, take_owned_c_string, CoreBluetoothError};
use crate::ffi;
use crate::private::{decode_optional_json, encode_json, retain_raw, retained_handle_to_raw};
use crate::uuid::BluetoothUuid;

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
/// Value payloads accepted by `CBMutableDescriptor`.
pub enum DescriptorValue {
    /// Stores an `NSString`-backed descriptor payload.
    String(String),
    /// Stores a raw byte payload for `CBMutableDescriptor`.
    Bytes(Vec<u8>),
    /// Stores a signed integer descriptor payload.
    Integer(i64),
    /// Stores an unsigned integer descriptor payload.
    Unsigned(u64),
    /// Stores a boolean descriptor payload.
    Boolean(bool),
    /// Stores an explicit `nil` descriptor payload.
    Null,
}

#[derive(Serialize)]
struct DescriptorValuePayload {
    kind: &'static str,
    string_value: Option<String>,
    bytes_value: Option<Vec<u8>>,
    signed_value: Option<i64>,
    unsigned_value: Option<u64>,
    bool_value: Option<bool>,
}

impl DescriptorValue {
    /// Creates a string-backed value for `CBMutableDescriptor`.
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Creates a byte-backed value for `CBMutableDescriptor`.
    pub fn bytes(value: impl Into<Vec<u8>>) -> Self {
        Self::Bytes(value.into())
    }

    /// Creates a signed-integer value for `CBMutableDescriptor`.
    pub fn integer(value: i64) -> Self {
        Self::Integer(value)
    }

    /// Creates an unsigned-integer value for `CBMutableDescriptor`.
    pub fn unsigned(value: u64) -> Self {
        Self::Unsigned(value)
    }

    /// Creates a boolean value for `CBMutableDescriptor`.
    pub fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    fn into_payload(self) -> DescriptorValuePayload {
        match self {
            Self::String(value) => DescriptorValuePayload {
                kind: "string",
                string_value: Some(value),
                bytes_value: None,
                signed_value: None,
                unsigned_value: None,
                bool_value: None,
            },
            Self::Bytes(value) => DescriptorValuePayload {
                kind: "bytes",
                string_value: None,
                bytes_value: Some(value),
                signed_value: None,
                unsigned_value: None,
                bool_value: None,
            },
            Self::Integer(value) => DescriptorValuePayload {
                kind: "integer",
                string_value: None,
                bytes_value: None,
                signed_value: Some(value),
                unsigned_value: None,
                bool_value: None,
            },
            Self::Unsigned(value) => DescriptorValuePayload {
                kind: "unsigned",
                string_value: None,
                bytes_value: None,
                signed_value: None,
                unsigned_value: Some(value),
                bool_value: None,
            },
            Self::Boolean(value) => DescriptorValuePayload {
                kind: "boolean",
                string_value: None,
                bytes_value: None,
                signed_value: None,
                unsigned_value: None,
                bool_value: Some(value),
            },
            Self::Null => DescriptorValuePayload {
                kind: "null",
                string_value: None,
                bytes_value: None,
                signed_value: None,
                unsigned_value: None,
                bool_value: None,
            },
        }
    }
}

/// Wraps `CBDescriptor`.
pub struct Descriptor {
    pub(crate) raw: *mut c_void,
}

impl Descriptor {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    pub(crate) fn from_retained_handle(handle: u64) -> Self {
        Self::from_retained_raw(retained_handle_to_raw(handle))
    }

    /// Returns the UUID string exposed by `CBDescriptor`.
    pub fn uuid(&self) -> String {
        let ptr = unsafe { ffi::cb_descriptor_uuid(self.raw) };
        take_owned_c_string(ptr)
    }

    /// Returns the `CBUUID` exposed by `CBDescriptor`.
    pub fn uuid_object(&self) -> BluetoothUuid {
        BluetoothUuid::from_retained_raw(unsafe { ffi::cb_descriptor_uuid_handle(self.raw) })
    }

    /// Returns the owning `CBCharacteristic`, if one is available.
    pub fn characteristic(&self) -> Option<Characteristic> {
        let raw = unsafe { ffi::cb_descriptor_characteristic(self.raw) };
        (!raw.is_null()).then(|| Characteristic::from_retained_raw(raw))
    }

    /// Returns the value exposed by `CBDescriptor`.
    pub fn value(&self) -> Result<Option<Value>, CoreBluetoothError> {
        let json = unsafe { ffi::cb_descriptor_value_json(self.raw) };
        decode_optional_json(json)
    }
}

impl Clone for Descriptor {
    fn clone(&self) -> Self {
        Self::from_retained_raw(retain_raw(self.raw))
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe { ffi::cb_object_release(self.raw) };
    }
}

/// Wraps `CBMutableDescriptor`.
pub struct MutableDescriptor {
    pub(crate) raw: *mut c_void,
}

impl MutableDescriptor {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    /// Creates a new `CBMutableDescriptor` wrapper.
    pub fn new(uuid: &BluetoothUuid, value: DescriptorValue) -> Result<Self, CoreBluetoothError> {
        let value = encode_json(&value.into_payload())?;
        let mut raw = core::ptr::null_mut();
        let mut error = core::ptr::null_mut();
        let status = unsafe {
            ffi::cb_mutable_descriptor_new(uuid.raw, value.as_ptr(), &mut raw, &mut error)
        };
        if status == ffi::status::OK {
            Ok(Self { raw })
        } else {
            Err(from_swift(status, error))
        }
    }

    /// Returns this mutable descriptor as an immutable `CBDescriptor` view.
    pub fn as_descriptor(&self) -> Descriptor {
        Descriptor::from_retained_raw(retain_raw(self.raw))
    }

    /// Returns the UUID string exposed by `CBMutableDescriptor`.
    pub fn uuid(&self) -> String {
        self.as_descriptor().uuid()
    }

    /// Returns the `CBUUID` exposed by `CBMutableDescriptor`.
    pub fn uuid_object(&self) -> BluetoothUuid {
        self.as_descriptor().uuid_object()
    }

    /// Returns the current value exposed by `CBMutableDescriptor`.
    pub fn value(&self) -> Result<Option<Value>, CoreBluetoothError> {
        self.as_descriptor().value()
    }
}

impl Clone for MutableDescriptor {
    fn clone(&self) -> Self {
        Self::from_retained_raw(retain_raw(self.raw))
    }
}

impl Drop for MutableDescriptor {
    fn drop(&mut self) {
        unsafe { ffi::cb_object_release(self.raw) };
    }
}
