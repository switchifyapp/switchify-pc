#![allow(missing_docs)]

use core::ffi::c_void;

use super::core::JsonCallback;

extern "C" {
    pub fn cb_central_manager_stream_subscribe(
        manager: *mut c_void,
        on_event: JsonCallback,
        ctx: *mut c_void,
    ) -> *mut c_void;
    pub fn cb_central_manager_stream_unsubscribe(manager: *mut c_void, bridge: *mut c_void);

    pub fn cb_peripheral_stream_subscribe(
        peripheral: *mut c_void,
        on_event: JsonCallback,
        ctx: *mut c_void,
    ) -> *mut c_void;
    pub fn cb_peripheral_stream_unsubscribe(peripheral: *mut c_void, bridge: *mut c_void);

    pub fn cb_peripheral_manager_stream_subscribe(
        manager: *mut c_void,
        on_event: JsonCallback,
        ctx: *mut c_void,
    ) -> *mut c_void;
    pub fn cb_peripheral_manager_stream_unsubscribe(manager: *mut c_void, bridge: *mut c_void);
}
