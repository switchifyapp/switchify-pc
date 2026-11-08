use core::ffi::c_void;

use crate::error::{from_swift, take_owned_c_string, CoreBluetoothError};
use crate::ffi;
use crate::private::retained_handle_to_raw;
use crate::retained::cb_retained;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
/// Mirrors the Foundation stream status values used by `CBL2CAPChannel` streams.
pub enum StreamStatus {
    /// Corresponds to the matching `CoreBluetooth` stream handling case.
    NotOpen = 0,
    /// Corresponds to the matching `CoreBluetooth` stream handling case.
    Opening = 1,
    /// Corresponds to the matching `CoreBluetooth` stream handling case.
    Open = 2,
    /// Corresponds to the matching `CoreBluetooth` stream handling case.
    Reading = 3,
    /// Corresponds to the matching `CoreBluetooth` stream handling case.
    Writing = 4,
    /// Corresponds to the matching `CoreBluetooth` stream handling case.
    AtEnd = 5,
    /// Corresponds to the matching `CoreBluetooth` stream handling case.
    Closed = 6,
    /// Corresponds to the matching `CoreBluetooth` stream handling case.
    Error = 7,
}

impl StreamStatus {
    /// Converts a raw stream status integer into `StreamStatus`.
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            1 => Self::Opening,
            2 => Self::Open,
            3 => Self::Reading,
            4 => Self::Writing,
            5 => Self::AtEnd,
            6 => Self::Closed,
            7 => Self::Error,
            _ => Self::NotOpen,
        }
    }
}

/// Wraps `CBPeer`.
pub struct Peer {
    raw: *mut c_void,
}

impl Peer {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    /// Returns the identifier exposed by `CBPeer`.
    pub fn identifier(&self) -> String {
        take_owned_c_string(unsafe { ffi::cb_peer_identifier(self.raw) })
    }
}

cb_retained!(Peer);

/// Owns the `inputStream` exposed by `CBL2CAPChannel`.
pub struct InputStreamHandle {
    raw: *mut c_void,
}

impl InputStreamHandle {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    /// Returns the current stream status for `CBL2CAPChannel.inputStream`.
    pub fn status(&self) -> StreamStatus {
        StreamStatus::from_raw(unsafe { ffi::cb_stream_status(self.raw) })
    }

    /// Returns whether the input stream currently has bytes available.
    pub fn has_bytes_available(&self) -> bool {
        unsafe { ffi::cb_input_stream_has_bytes_available(self.raw) }
    }

    /// Opens `CBL2CAPChannel.inputStream`.
    pub fn open(&self) {
        unsafe { ffi::cb_stream_open(self.raw) };
    }

    /// Closes `CBL2CAPChannel.inputStream`.
    pub fn close(&self) {
        unsafe { ffi::cb_stream_close(self.raw) };
    }

    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, CoreBluetoothError> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut error = core::ptr::null_mut();
        let count = unsafe {
            ffi::cb_input_stream_read(self.raw, buffer.as_mut_ptr(), buffer.len(), &raw mut error)
        };
        usize::try_from(count).map_err(|_| from_swift(ffi::status::FRAMEWORK_ERROR, error))
    }
}

cb_retained!(InputStreamHandle);

/// Owns the `outputStream` exposed by `CBL2CAPChannel`.
pub struct OutputStreamHandle {
    raw: *mut c_void,
}

impl OutputStreamHandle {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    /// Returns the current stream status for `CBL2CAPChannel.outputStream`.
    pub fn status(&self) -> StreamStatus {
        StreamStatus::from_raw(unsafe { ffi::cb_stream_status(self.raw) })
    }

    /// Returns whether the output stream currently has buffer space available.
    pub fn has_space_available(&self) -> bool {
        unsafe { ffi::cb_output_stream_has_space_available(self.raw) }
    }

    /// Opens `CBL2CAPChannel.outputStream`.
    pub fn open(&self) {
        unsafe { ffi::cb_stream_open(self.raw) };
    }

    /// Closes `CBL2CAPChannel.outputStream`.
    pub fn close(&self) {
        unsafe { ffi::cb_stream_close(self.raw) };
    }

    pub fn write(&self, bytes: &[u8]) -> Result<usize, CoreBluetoothError> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let mut error = core::ptr::null_mut();
        let count = unsafe {
            ffi::cb_output_stream_write(self.raw, bytes.as_ptr(), bytes.len(), &raw mut error)
        };
        usize::try_from(count).map_err(|_| from_swift(ffi::status::FRAMEWORK_ERROR, error))
    }
}

cb_retained!(OutputStreamHandle);

/// Wraps `CBL2CAPChannel`.
pub struct L2capChannel {
    raw: *mut c_void,
}

impl L2capChannel {
    pub(crate) fn from_retained_raw(raw: *mut c_void) -> Self {
        Self { raw }
    }

    pub(crate) fn from_retained_handle(handle: u64) -> Self {
        Self::from_retained_raw(retained_handle_to_raw(handle))
    }

    /// Returns the peer exposed by `CBL2CAPChannel.peer`.
    pub fn peer(&self) -> Peer {
        Peer::from_retained_raw(unsafe { ffi::cb_l2cap_channel_peer(self.raw) })
    }

    /// Returns the PSM exposed by `CBL2CAPChannel.PSM`.
    pub fn psm(&self) -> u16 {
        unsafe { ffi::cb_l2cap_channel_psm(self.raw) }
    }

    /// Returns the `inputStream` exposed by `CBL2CAPChannel`.
    pub fn input_stream(&self) -> InputStreamHandle {
        InputStreamHandle::from_retained_raw(unsafe {
            ffi::cb_l2cap_channel_input_stream(self.raw)
        })
    }

    /// Returns the `outputStream` exposed by `CBL2CAPChannel`.
    pub fn output_stream(&self) -> OutputStreamHandle {
        OutputStreamHandle::from_retained_raw(unsafe {
            ffi::cb_l2cap_channel_output_stream(self.raw)
        })
    }
}

cb_retained!(L2capChannel);

#[cfg(test)]
mod tests {
    use apple_cf::cf::CFStreamPair;

    use super::{InputStreamHandle, OutputStreamHandle, StreamStatus};
    use crate::private::retain_raw;

    fn bound_pair() -> (InputStreamHandle, OutputStreamHandle) {
        let pair = CFStreamPair::new(64);
        (
            InputStreamHandle::from_retained_raw(retain_raw(pair.read.as_ptr())),
            OutputStreamHandle::from_retained_raw(retain_raw(pair.write.as_ptr())),
        )
    }

    #[test]
    fn channel_streams_move_bytes_through_the_bridge() {
        let (input, output) = bound_pair();
        input.open();
        output.open();
        assert_eq!(input.status(), StreamStatus::Open);
        assert!(output.has_space_available());

        assert_eq!(output.write(b"ping").expect("write"), 4);
        assert!(input.has_bytes_available());
        let mut buffer = [0_u8; 16];
        let read = input.read(&mut buffer).expect("read");
        assert_eq!(&buffer[..read], b"ping");

        assert_eq!(output.write(&[]).expect("empty write"), 0);
        assert_eq!(input.read(&mut []).expect("empty read"), 0);

        output.close();
        assert_eq!(input.read(&mut buffer).expect("read at end"), 0);
        input.close();
    }

    #[test]
    fn streams_that_are_not_open_report_errors() {
        let (input, output) = bound_pair();
        let mut buffer = [0_u8; 4];
        assert!(input.read(&mut buffer).is_err());
        assert!(output.write(b"data").is_err());
    }
}
