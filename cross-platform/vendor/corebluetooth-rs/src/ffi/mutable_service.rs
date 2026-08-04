#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

extern "C" {
    pub fn cb_mutable_service_new(
        uuid: *mut c_void,
        is_primary: bool,
        out_service: *mut *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_mutable_service_set_included_services(
        service: *mut c_void,
        services: *const *mut c_void,
        count: usize,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_mutable_service_set_characteristics(
        service: *mut c_void,
        characteristics: *const *mut c_void,
        count: usize,
        error_out: *mut *mut c_char,
    ) -> i32;
}
