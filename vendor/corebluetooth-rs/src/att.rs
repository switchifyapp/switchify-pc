use core::ffi::c_void;

use crate::characteristic::Characteristic;
use crate::error::{take_owned_c_string, CoreBluetoothError};
use crate::ffi;
use crate::peripheral_manager::Central;
use crate::private::{decode_optional_json, retain_raw, retained_handle_to_raw};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
/// Mirrors `CBATTError` result codes.
pub enum AttError {
    /// Corresponds to the matching `CBATTError` case.
    Success = 0x00,
    /// Corresponds to the matching `CBATTError` case.
    InvalidHandle = 0x01,
    /// Corresponds to the matching `CBATTError` case.
    ReadNotPermitted = 0x02,
    /// Corresponds to the matching `CBATTError` case.
    WriteNotPermitted = 0x03,
    /// Corresponds to the matching `CBATTError` case.
    InvalidPdu = 0x04,
    /// Corresponds to the matching `CBATTError` case.
    InsufficientAuthentication = 0x05,
    /// Corresponds to the matching `CBATTError` case.
    RequestNotSupported = 0x06,
    /// Corresponds to the matching `CBATTError` case.
    InvalidOffset = 0x07,
    /// Corresponds to the matching `CBATTError` case.
    InsufficientAuthorization = 0x08,
    /// Corresponds to the matching `CBATTError` case.
    PrepareQueueFull = 0x09,
    /// Corresponds to the matching `CBATTError` case.
    AttributeNotFound = 0x0A,
    /// Corresponds to the matching `CBATTError` case.
    AttributeNotLong = 0x0B,
    /// Corresponds to the matching `CBATTError` case.
    InsufficientEncryptionKeySize = 0x0C,
    /// Corresponds to the matching `CBATTError` case.
    InvalidAttributeValueLength = 0x0D,
    /// Corresponds to the matching `CBATTError` case.
    UnlikelyError = 0x0E,
    /// Corresponds to the matching `CBATTError` case.
    InsufficientEncryption = 0x0F,
    /// Corresponds to the matching `CBATTError` case.
    UnsupportedGroupType = 0x10,
    /// Corresponds to the matching `CBATTError` case.
    InsufficientResources = 0x11,
}

impl AttError {
    /// Converts a raw `CBATTError` code into `AttError`.
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            0x01 => Self::InvalidHandle,
            0x02 => Self::ReadNotPermitted,
            0x03 => Self::WriteNotPermitted,
            0x04 => Self::InvalidPdu,
            0x05 => Self::InsufficientAuthentication,
            0x06 => Self::RequestNotSupported,
            0x07 => Self::InvalidOffset,
            0x08 => Self::InsufficientAuthorization,
            0x09 => Self::PrepareQueueFull,
            0x0A => Self::AttributeNotFound,
            0x0B => Self::AttributeNotLong,
            0x0C => Self::InsufficientEncryptionKeySize,
            0x0D => Self::InvalidAttributeValueLength,
            0x0E => Self::UnlikelyError,
            0x0F => Self::InsufficientEncryption,
            0x10 => Self::UnsupportedGroupType,
            0x11 => Self::InsufficientResources,
            _ => Self::Success,
        }
    }
}

/// Wraps `CBATTRequest`.
pub struct AttRequest {
    pub(crate) raw: *mut c_void,
}

impl AttRequest {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    pub(crate) fn from_retained_handle(handle: u64) -> Self {
        Self::from_retained_raw(retained_handle_to_raw(handle))
    }

    /// Returns the `CBCentral` associated with this `CBATTRequest`.
    pub fn central(&self) -> Central {
        Central::from_retained_raw(unsafe { ffi::cb_att_request_central(self.raw) })
    }

    /// Returns the `CBCharacteristic` targeted by this `CBATTRequest`.
    pub fn characteristic(&self) -> Characteristic {
        Characteristic::from_retained_raw(unsafe { ffi::cb_att_request_characteristic(self.raw) })
    }

    /// Returns the byte offset requested by `CBATTRequest`.
    pub fn offset(&self) -> usize {
        unsafe { ffi::cb_att_request_offset(self.raw) }
    }

    /// Returns the value attached to this `CBATTRequest`.
    pub fn value(&self) -> Result<Option<Vec<u8>>, CoreBluetoothError> {
        let json = unsafe { ffi::cb_att_request_value_json(self.raw) };
        decode_optional_json(json)
    }

    /// Sets the value that `CoreBluetooth` should use when responding to this `CBATTRequest`.
    pub fn set_value(&mut self, value: Option<&[u8]>) {
        unsafe {
            ffi::cb_att_request_set_value(
                self.raw,
                value.map_or(core::ptr::null(), <[u8]>::as_ptr),
                value.map_or(0, <[u8]>::len),
            );
        };
    }

    /// Returns the `CoreBluetooth` `CBError` domain string.
    pub fn error_domain() -> String {
        take_owned_c_string(unsafe { ffi::cb_error_domain() })
    }

    /// Returns the `CoreBluetooth` `CBATTError` domain string.
    pub fn att_error_domain() -> String {
        take_owned_c_string(unsafe { ffi::cb_att_error_domain() })
    }
}

impl Clone for AttRequest {
    fn clone(&self) -> Self {
        Self::from_retained_raw(retain_raw(self.raw))
    }
}

impl Drop for AttRequest {
    fn drop(&mut self) {
        unsafe { ffi::cb_object_release(self.raw) };
    }
}
