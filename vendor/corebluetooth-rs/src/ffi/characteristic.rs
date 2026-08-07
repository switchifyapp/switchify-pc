#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

extern "C" {
    pub fn cb_characteristic_uuid(characteristic: *mut c_void) -> *mut c_char;
    pub fn cb_characteristic_uuid_handle(characteristic: *mut c_void) -> *mut c_void;
    pub fn cb_characteristic_service(characteristic: *mut c_void) -> *mut c_void;
    pub fn cb_characteristic_properties(characteristic: *mut c_void) -> u64;
    pub fn cb_characteristic_value_json(characteristic: *mut c_void) -> *mut c_char;
    pub fn cb_characteristic_is_notifying(characteristic: *mut c_void) -> bool;
    pub fn cb_characteristic_descriptors(
        characteristic: *mut c_void,
        out_array: *mut *mut c_void,
        out_count: *mut usize,
    );
}
