#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

extern "C" {
    pub fn cb_object_release(ptr: *mut c_void);
    pub fn cb_object_retain(ptr: *mut c_void) -> *mut c_void;
    pub fn cb_pointer_array_free(array: *mut c_void, count: usize);
}

pub mod status {
    pub const OK: i32 = 0;
    pub const INVALID_ARGUMENT: i32 = -1;
    pub const FRAMEWORK_ERROR: i32 = -2;
    pub const UNKNOWN: i32 = -99;
}

pub type JsonCallback = unsafe extern "C" fn(user_info: *mut c_void, payload_json: *const c_char);
