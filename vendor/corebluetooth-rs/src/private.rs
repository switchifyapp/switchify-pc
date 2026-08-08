use core::ffi::c_void;
use std::ffi::CString;

use serde::de::DeserializeOwned;

use crate::error::{take_owned_c_string, CoreBluetoothError};
use crate::ffi;

pub fn to_cstring(value: &str) -> Result<CString, CoreBluetoothError> {
    CString::new(value).map_err(|_| {
        CoreBluetoothError::InvalidArgument("strings must not contain interior NUL bytes".into())
    })
}

pub fn encode_json<T: serde::Serialize>(value: &T) -> Result<CString, CoreBluetoothError> {
    let json = serde_json::to_string(value).map_err(|error| {
        CoreBluetoothError::FrameworkError(format!("failed to encode JSON payload: {error}"))
    })?;
    to_cstring(&json)
}

pub fn encode_string_slice(values: &[&str]) -> Result<CString, CoreBluetoothError> {
    encode_json(&values)
}

pub fn decode_json<T: DeserializeOwned>(
    ptr: *mut core::ffi::c_char,
) -> Result<T, CoreBluetoothError> {
    let json = take_owned_c_string(ptr);
    serde_json::from_str(&json).map_err(|error| {
        CoreBluetoothError::FrameworkError(format!("failed to decode bridge JSON payload: {error}"))
    })
}

pub fn decode_optional_json<T: DeserializeOwned>(
    ptr: *mut core::ffi::c_char,
) -> Result<Option<T>, CoreBluetoothError> {
    if ptr.is_null() {
        return Ok(None);
    }

    decode_json(ptr).map(Some)
}

pub fn take_retained_pointer_array(array: *mut c_void, count: usize) -> Vec<*mut c_void> {
    if array.is_null() || count == 0 {
        return Vec::new();
    }

    let typed = array.cast::<*mut c_void>();
    let values = unsafe { std::slice::from_raw_parts(typed, count) }.to_vec();
    unsafe { ffi::cb_pointer_array_free(array, count) };
    values
}

pub fn retain_raw(raw: *mut c_void) -> *mut c_void {
    unsafe { ffi::cb_object_retain(raw) }
}

pub fn retained_handle_to_raw(handle: u64) -> *mut c_void {
    usize::try_from(handle).unwrap_or_else(|_| {
        unreachable!("retained handles must fit into usize on supported targets")
    }) as *mut c_void
}
