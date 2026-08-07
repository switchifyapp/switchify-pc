use core::ffi::c_char;
use core::fmt;

use libc::free;
use serde::Deserialize;

use crate::ffi;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
/// Structured `NSError` information reported by `CoreBluetooth` callbacks.
pub struct BluetoothErrorInfo {
    /// The `CoreBluetooth` `NSError` domain.
    pub domain: String,
    /// The integer code from the `CoreBluetooth` `NSError`.
    pub code: i32,
    /// The localized message from the `CoreBluetooth` `NSError`.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
/// Mirrors `CBError` codes.
pub enum BluetoothErrorCode {
    /// Corresponds to the matching `CBError` case.
    Unknown = 0,
    /// Corresponds to the matching `CBError` case.
    InvalidParameters = 1,
    /// Corresponds to the matching `CBError` case.
    InvalidHandle = 2,
    /// Corresponds to the matching `CBError` case.
    NotConnected = 3,
    /// Corresponds to the matching `CBError` case.
    OutOfSpace = 4,
    /// Corresponds to the matching `CBError` case.
    OperationCancelled = 5,
    /// Corresponds to the matching `CBError` case.
    ConnectionTimeout = 6,
    /// Corresponds to the matching `CBError` case.
    PeripheralDisconnected = 7,
    /// Corresponds to the matching `CBError` case.
    UuidNotAllowed = 8,
    /// Corresponds to the matching `CBError` case.
    AlreadyAdvertising = 9,
    /// Corresponds to the matching `CBError` case.
    ConnectionFailed = 10,
    /// Corresponds to the matching `CBError` case.
    ConnectionLimitReached = 11,
    /// Corresponds to the matching `CBError` case.
    UnknownDevice = 12,
    /// Corresponds to the matching `CBError` case.
    OperationNotSupported = 13,
    /// Corresponds to the matching `CBError` case.
    PeerRemovedPairingInformation = 14,
    /// Corresponds to the matching `CBError` case.
    EncryptionTimedOut = 15,
    /// Corresponds to the matching `CBError` case.
    TooManyLePairedDevices = 16,
}

impl BluetoothErrorCode {
    #[must_use]
    /// Converts a raw `CBError` code into `BluetoothErrorCode`.
    pub const fn from_raw(raw: i32) -> Self {
        match raw {
            1 => Self::InvalidParameters,
            2 => Self::InvalidHandle,
            3 => Self::NotConnected,
            4 => Self::OutOfSpace,
            5 => Self::OperationCancelled,
            6 => Self::ConnectionTimeout,
            7 => Self::PeripheralDisconnected,
            8 => Self::UuidNotAllowed,
            9 => Self::AlreadyAdvertising,
            10 => Self::ConnectionFailed,
            11 => Self::ConnectionLimitReached,
            12 => Self::UnknownDevice,
            13 => Self::OperationNotSupported,
            14 => Self::PeerRemovedPairingInformation,
            15 => Self::EncryptionTimedOut,
            16 => Self::TooManyLePairedDevices,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
/// Errors produced by the `CoreBluetooth` bridge or safe wrapper.
pub enum CoreBluetoothError {
    /// The Rust wrapper rejected an invalid argument before calling `CoreBluetooth`.
    InvalidArgument(String),
    /// `CoreBluetooth` reported a framework-level failure.
    FrameworkError(String),
    /// The bridge returned a non-standard error code.
    Unknown {
        /// The integer error code returned by `CoreBluetooth`.
        code: i32,
        /// The human-readable message returned by `CoreBluetooth`.
        message: String,
    },
}

impl CoreBluetoothError {
    #[must_use]
    /// Returns the integer status code associated with this bridge error.
    pub const fn code(&self) -> i32 {
        match self {
            Self::InvalidArgument(_) => ffi::status::INVALID_ARGUMENT,
            Self::FrameworkError(_) => ffi::status::FRAMEWORK_ERROR,
            Self::Unknown { code, .. } => *code,
        }
    }

    #[must_use]
    /// Returns the human-readable message associated with this bridge error.
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidArgument(message)
            | Self::FrameworkError(message)
            | Self::Unknown { message, .. } => message,
        }
    }
}

impl fmt::Display for CoreBluetoothError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (code {})", self.message(), self.code())
    }
}

impl std::error::Error for CoreBluetoothError {}

pub(crate) fn take_owned_c_string(ptr: *mut c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }

    let string = unsafe { core::ffi::CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { free(ptr.cast()) };
    string
}

pub(crate) fn from_swift(status: i32, error_str: *mut c_char) -> CoreBluetoothError {
    from_status_message(status, take_owned_c_string(error_str))
}

pub(crate) fn from_status_message(status: i32, message: String) -> CoreBluetoothError {
    match status {
        ffi::status::INVALID_ARGUMENT => CoreBluetoothError::InvalidArgument(message),
        ffi::status::FRAMEWORK_ERROR => CoreBluetoothError::FrameworkError(message),
        code => CoreBluetoothError::Unknown { code, message },
    }
}
