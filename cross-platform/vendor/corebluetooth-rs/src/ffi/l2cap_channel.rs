#![allow(missing_docs)]

use core::ffi::{c_char, c_void};

extern "C" {
    pub fn cb_l2cap_channel_peer(channel: *mut c_void) -> *mut c_void;
    pub fn cb_l2cap_channel_psm(channel: *mut c_void) -> u16;
    pub fn cb_l2cap_channel_input_stream(channel: *mut c_void) -> *mut c_void;
    pub fn cb_l2cap_channel_output_stream(channel: *mut c_void) -> *mut c_void;
    pub fn cb_peer_identifier(peer: *mut c_void) -> *mut c_char;
    pub fn cb_stream_status(stream: *mut c_void) -> i32;
    pub fn cb_input_stream_has_bytes_available(stream: *mut c_void) -> bool;
    pub fn cb_output_stream_has_space_available(stream: *mut c_void) -> bool;
    pub fn cb_stream_open(stream: *mut c_void);
    pub fn cb_stream_close(stream: *mut c_void);
}
