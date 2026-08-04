#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

extern "C" {
    pub fn cb_service_uuid(service: *mut c_void) -> *mut c_char;
    pub fn cb_service_uuid_handle(service: *mut c_void) -> *mut c_void;
    pub fn cb_service_peripheral(service: *mut c_void) -> *mut c_void;
    pub fn cb_service_is_primary(service: *mut c_void) -> bool;
    pub fn cb_service_included_services(
        service: *mut c_void,
        out_array: *mut *mut c_void,
        out_count: *mut usize,
    );
    pub fn cb_service_characteristics(
        service: *mut c_void,
        out_array: *mut *mut c_void,
        out_count: *mut usize,
    );
}
