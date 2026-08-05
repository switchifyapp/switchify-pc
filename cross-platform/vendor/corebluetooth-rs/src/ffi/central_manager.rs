#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

use super::core::JsonCallback;

extern "C" {
    pub fn cb_manager_new(
        options_json: *const c_char,
        callback: Option<JsonCallback>,
        user_info: *mut c_void,
        out_manager: *mut *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_manager_state(manager: *mut c_void) -> i32;
    pub fn cb_manager_authorization(manager: *mut c_void) -> i32;
    pub fn cb_manager_global_authorization() -> i32;
    pub fn cb_manager_is_scanning(manager: *mut c_void) -> bool;
    pub fn cb_manager_scan_for_peripherals(
        manager: *mut c_void,
        service_uuids_json: *const c_char,
        scan_options_json: *const c_char,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_manager_stop_scan(manager: *mut c_void);
    pub fn cb_manager_connect(
        manager: *mut c_void,
        peripheral: *mut c_void,
        options_json: *const c_char,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_manager_cancel_peripheral_connection(manager: *mut c_void, peripheral: *mut c_void);
    pub fn cb_manager_retrieve_connected_peripherals(
        manager: *mut c_void,
        service_uuids_json: *const c_char,
        out_array: *mut *mut c_void,
        out_count: *mut usize,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_manager_retrieve_peripherals_with_identifiers(
        manager: *mut c_void,
        identifiers_json: *const c_char,
        out_array: *mut *mut c_void,
        out_count: *mut usize,
        error_out: *mut *mut c_char,
    ) -> i32;
}
