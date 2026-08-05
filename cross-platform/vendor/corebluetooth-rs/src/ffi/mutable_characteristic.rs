#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

extern "C" {
    pub fn cb_mutable_characteristic_new(
        uuid: *mut c_void,
        properties: u64,
        bytes: *const u8,
        length: usize,
        permissions: u64,
        out_characteristic: *mut *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_mutable_characteristic_permissions(characteristic: *mut c_void) -> u64;
    pub fn cb_mutable_characteristic_set_permissions(characteristic: *mut c_void, permissions: u64);
    pub fn cb_mutable_characteristic_set_properties(characteristic: *mut c_void, properties: u64);
    pub fn cb_mutable_characteristic_set_value(
        characteristic: *mut c_void,
        bytes: *const u8,
        length: usize,
    );
    pub fn cb_mutable_characteristic_subscribed_centrals(
        characteristic: *mut c_void,
        out_array: *mut *mut c_void,
        out_count: *mut usize,
    );
    pub fn cb_mutable_characteristic_set_descriptors(
        characteristic: *mut c_void,
        descriptors: *const *mut c_void,
        count: usize,
        error_out: *mut *mut c_char,
    ) -> i32;
}
