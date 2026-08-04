#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

extern "C" {
    pub fn cb_descriptor_uuid(descriptor: *mut c_void) -> *mut c_char;
    pub fn cb_descriptor_uuid_handle(descriptor: *mut c_void) -> *mut c_void;
    pub fn cb_descriptor_characteristic(descriptor: *mut c_void) -> *mut c_void;
    pub fn cb_descriptor_value_json(descriptor: *mut c_void) -> *mut c_char;
}
