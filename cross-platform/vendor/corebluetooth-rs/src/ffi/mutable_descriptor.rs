#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

extern "C" {
    pub fn cb_mutable_descriptor_new(
        uuid: *mut c_void,
        value_json: *const c_char,
        out_descriptor: *mut *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
}
