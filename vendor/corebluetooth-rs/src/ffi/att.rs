#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

extern "C" {
    pub fn cb_att_request_central(request: *mut c_void) -> *mut c_void;
    pub fn cb_att_request_characteristic(request: *mut c_void) -> *mut c_void;
    pub fn cb_att_request_offset(request: *mut c_void) -> usize;
    pub fn cb_att_request_value_json(request: *mut c_void) -> *mut c_char;
    pub fn cb_att_request_set_value(request: *mut c_void, bytes: *const u8, length: usize);
    pub fn cb_error_domain() -> *mut c_char;
    pub fn cb_att_error_domain() -> *mut c_char;
}
