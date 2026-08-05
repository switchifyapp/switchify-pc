#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

extern "C" {
    pub fn cb_uuid_new_from_string(uuid: *const c_char) -> *mut c_void;
    pub fn cb_uuid_new_from_bytes(bytes: *const u8, length: usize) -> *mut c_void;
    pub fn cb_uuid_string(uuid: *mut c_void) -> *mut c_char;
    pub fn cb_uuid_data_json(uuid: *mut c_void) -> *mut c_char;
    pub fn cb_uuid_constant_string(kind: i32) -> *mut c_char;
}
