#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

use super::core::JsonCallback;

extern "C" {
    pub fn cb_peripheral_manager_new(
        options_json: *const c_char,
        callback: Option<JsonCallback>,
        user_info: *mut c_void,
        out_manager: *mut *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_manager_state(manager: *mut c_void) -> i32;
    pub fn cb_peripheral_manager_authorization(manager: *mut c_void) -> i32;
    pub fn cb_peripheral_manager_global_authorization() -> i32;
    pub fn cb_peripheral_manager_is_advertising(manager: *mut c_void) -> bool;
    pub fn cb_peripheral_manager_start_advertising(
        manager: *mut c_void,
        advertisement_json: *const c_char,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_manager_stop_advertising(manager: *mut c_void);
    pub fn cb_peripheral_manager_set_desired_connection_latency(
        manager: *mut c_void,
        latency: i32,
        central: *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_manager_add_service(
        manager: *mut c_void,
        service: *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_manager_remove_service(manager: *mut c_void, service: *mut c_void);
    pub fn cb_peripheral_manager_remove_all_services(manager: *mut c_void);
    pub fn cb_peripheral_manager_respond_to_request(
        manager: *mut c_void,
        request: *mut c_void,
        result: i32,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_manager_update_value(
        manager: *mut c_void,
        bytes: *const u8,
        length: usize,
        characteristic: *mut c_void,
        centrals: *const *mut c_void,
        count: usize,
        out_sent: *mut bool,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_manager_publish_l2cap_channel(
        manager: *mut c_void,
        encryption_required: bool,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_manager_unpublish_l2cap_channel(
        manager: *mut c_void,
        psm: u16,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_central_identifier(central: *mut c_void) -> *mut c_char;
    pub fn cb_central_maximum_update_value_length(central: *mut c_void) -> usize;
}
