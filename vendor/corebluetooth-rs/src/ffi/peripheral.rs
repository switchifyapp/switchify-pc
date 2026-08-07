#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

use super::core::JsonCallback;

extern "C" {
    pub fn cb_peripheral_set_delegate(
        peripheral: *mut c_void,
        callback: Option<JsonCallback>,
        user_info: *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_clear_delegate(peripheral: *mut c_void);
    pub fn cb_peripheral_name(peripheral: *mut c_void) -> *mut c_char;
    pub fn cb_peripheral_identifier(peripheral: *mut c_void) -> *mut c_char;
    pub fn cb_peripheral_state(peripheral: *mut c_void) -> i32;
    pub fn cb_peripheral_services(
        peripheral: *mut c_void,
        out_array: *mut *mut c_void,
        out_count: *mut usize,
    );
    pub fn cb_peripheral_can_send_write_without_response(peripheral: *mut c_void) -> bool;
    pub fn cb_peripheral_discover_services(
        peripheral: *mut c_void,
        service_uuids_json: *const c_char,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_discover_included_services(
        peripheral: *mut c_void,
        service: *mut c_void,
        included_service_uuids_json: *const c_char,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_read_rssi(peripheral: *mut c_void, error_out: *mut *mut c_char) -> i32;
    pub fn cb_peripheral_discover_characteristics(
        peripheral: *mut c_void,
        service: *mut c_void,
        characteristic_uuids_json: *const c_char,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_read_value_for_characteristic(
        peripheral: *mut c_void,
        characteristic: *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_maximum_write_value_length(
        peripheral: *mut c_void,
        write_type: i32,
    ) -> usize;
    pub fn cb_peripheral_write_value_for_characteristic(
        peripheral: *mut c_void,
        characteristic: *mut c_void,
        bytes: *const u8,
        length: usize,
        with_response: bool,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_set_notify_value(
        peripheral: *mut c_void,
        characteristic: *mut c_void,
        enabled: bool,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_discover_descriptors(
        peripheral: *mut c_void,
        characteristic: *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_read_value_for_descriptor(
        peripheral: *mut c_void,
        descriptor: *mut c_void,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_write_value_for_descriptor(
        peripheral: *mut c_void,
        descriptor: *mut c_void,
        bytes: *const u8,
        length: usize,
        error_out: *mut *mut c_char,
    ) -> i32;
    pub fn cb_peripheral_open_l2cap_channel(
        peripheral: *mut c_void,
        psm: u16,
        error_out: *mut *mut c_char,
    ) -> i32;
}
