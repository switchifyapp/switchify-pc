#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! # API documentation
//!
//! Safe Rust bindings for Apple's
//! [CoreBluetooth](https://developer.apple.com/documentation/corebluetooth)
//! framework.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::new_without_default
)]

/// Advertisement-data builders and parsers corresponding to `CBAdvertisementData` keys.
pub mod advertisement;
#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub mod async_api;
/// ATT request and error wrappers corresponding to `CBATTRequest` and `CBATTError`.
pub mod att;
/// Compatibility re-exports for central-role `CoreBluetooth` types.
pub mod central;
/// `CBCentralManager` wrappers and related central-role types.
pub mod central_manager;
/// `CBCharacteristic` wrappers, properties, permissions, and write types.
pub mod characteristic;
/// `CBDescriptor` and `CBMutableDescriptor` wrappers.
pub mod descriptor;
/// Error types corresponding to `CBError` values and bridge failures.
pub mod error;
pub mod ffi;
/// `CBL2CAPChannel` wrappers and stream helpers.
pub mod l2cap_channel;
/// `CBMutableCharacteristic` wrappers for local GATT databases.
pub mod mutable_characteristic;
/// `CBMutableService` wrappers for local GATT databases.
pub mod mutable_service;
/// `CBPeripheral` wrappers and delegate helpers.
pub mod peripheral;
/// `CBPeripheralManager` wrappers and related peripheral-role types.
pub mod peripheral_manager;
mod private;
/// `CBService` wrappers.
pub mod service;
/// `CBUUID` wrappers and well-known descriptor UUID helpers.
pub mod uuid;

pub use advertisement::AdvertisementData;
pub use att::{AttError, AttRequest};
pub use central_manager::{
    CentralManager, CentralManagerCallbacks, CentralManagerDelegate, CentralManagerOptions,
    CentralManagerRestoredState, CentralManagerState, ConnectOptions, ManagerAuthorization,
    ScanOptions,
};
pub use characteristic::{
    AttributePermissions, Characteristic, CharacteristicProperties, CharacteristicWriteType,
};
pub use descriptor::{Descriptor, DescriptorValue, MutableDescriptor};
pub use error::{BluetoothErrorCode, BluetoothErrorInfo, CoreBluetoothError};
pub use l2cap_channel::{InputStreamHandle, L2capChannel, OutputStreamHandle, Peer, StreamStatus};
pub use mutable_characteristic::MutableCharacteristic;
pub use mutable_service::MutableService;
pub use peripheral::{
    Peripheral, PeripheralCallbacks, PeripheralDelegate, PeripheralState, Service,
};
pub use peripheral_manager::{
    Central, PeripheralManager, PeripheralManagerCallbacks, PeripheralManagerConnectionLatency,
    PeripheralManagerDelegate, PeripheralManagerOptions, PeripheralManagerRestoredState,
    PeripheralManagerState,
};
pub use uuid::BluetoothUuid;

/// Common imports.
pub mod prelude {
    pub use crate::advertisement::AdvertisementData;
    pub use crate::att::{AttError, AttRequest};
    pub use crate::central_manager::{
        CentralManager, CentralManagerCallbacks, CentralManagerDelegate, CentralManagerOptions,
        CentralManagerRestoredState, CentralManagerState, ConnectOptions, ManagerAuthorization,
        ScanOptions,
    };
    pub use crate::characteristic::{
        AttributePermissions, Characteristic, CharacteristicProperties, CharacteristicWriteType,
    };
    pub use crate::descriptor::{Descriptor, DescriptorValue, MutableDescriptor};
    pub use crate::error::{BluetoothErrorCode, BluetoothErrorInfo, CoreBluetoothError};
    pub use crate::l2cap_channel::{
        InputStreamHandle, L2capChannel, OutputStreamHandle, Peer, StreamStatus,
    };
    pub use crate::mutable_characteristic::MutableCharacteristic;
    pub use crate::mutable_service::MutableService;
    pub use crate::peripheral::{
        Peripheral, PeripheralCallbacks, PeripheralDelegate, PeripheralState, Service,
    };
    pub use crate::peripheral_manager::{
        Central, PeripheralManager, PeripheralManagerCallbacks, PeripheralManagerConnectionLatency,
        PeripheralManagerDelegate, PeripheralManagerOptions, PeripheralManagerRestoredState,
        PeripheralManagerState,
    };
    pub use crate::uuid::BluetoothUuid;
}
