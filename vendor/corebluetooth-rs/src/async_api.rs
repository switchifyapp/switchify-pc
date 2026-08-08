//! Executor-agnostic async event streams for `CoreBluetooth` delegates.
//!
//! Each stream surface corresponds to one Apple delegate protocol and
//! delivers typed events via a [`doom_fish_utils::stream::BoundedAsyncStream`].
//!
//! # Example
//! ```no_run
//! use corebluetooth::async_api::CentralManagerEventStream;
//!
//! # async fn demo() {
//! let manager = corebluetooth::CentralManager::new().unwrap();
//! let stream = CentralManagerEventStream::subscribe(&manager, 16);
//! if let Some(event) = stream.next().await {
//!     println!("{event:?}");
//! }
//! # }
//! ```
#![cfg(feature = "async")]

use core::ffi::{c_char, c_void};
use core::fmt;

use doom_fish_utils::stream::{AsyncStreamSender, BoundedAsyncStream, NextItem};
use serde::Deserialize;

use crate::advertisement::AdvertisementData;
use crate::att::AttRequest;
use crate::central_manager::{CentralManager, CentralManagerState, ManagerAuthorization};
use crate::characteristic::Characteristic;
use crate::descriptor::Descriptor;
use crate::error::BluetoothErrorInfo;
use crate::l2cap_channel::L2capChannel;
use crate::peripheral::Peripheral;
use crate::peripheral_manager::{Central, PeripheralManager, PeripheralManagerState};
use crate::service::Service;

#[derive(Deserialize)]
struct EventEnvelope {
    event: String,
    state: Option<i32>,
    authorization: Option<i32>,
    peripheral_handle: Option<u64>,
    rssi: Option<i32>,
    advertisement_data: Option<serde_json::Value>,
    service_handles: Option<Vec<u64>>,
    invalidated_service_handles: Option<Vec<u64>>,
    service_handle: Option<u64>,
    characteristic_handles: Option<Vec<u64>>,
    characteristic_handle: Option<u64>,
    descriptor_handle: Option<u64>,
    channel_handle: Option<u64>,
    central_handle: Option<u64>,
    request_handle: Option<u64>,
    request_handles: Option<Vec<u64>>,
    psm: Option<u16>,
    error: Option<BluetoothErrorInfo>,
}

/// An event emitted by a [`CentralManagerEventStream`].
#[non_exhaustive]
pub enum CentralManagerEvent {
    /// The central manager's state changed.
    StateChanged {
        /// The state value reported by `CoreBluetooth`.
        state: CentralManagerState,
        /// The authorization value reported by `CoreBluetooth`.
        authorization: ManagerAuthorization,
    },
    /// A peripheral was discovered during a scan.
    PeripheralDiscovered {
        /// The `CBPeripheral` associated with the callback.
        peripheral: Peripheral,
        /// The RSSI value reported by `CoreBluetooth`.
        rssi: i32,
        /// Advertisement data carried by the `CoreBluetooth` callback.
        advertisement_data: AdvertisementData,
    },
    /// A peripheral was successfully connected.
    PeripheralConnected {
        /// The `CBPeripheral` associated with the callback.
        peripheral: Peripheral,
    },
    /// A connection attempt to a peripheral failed.
    PeripheralFailedToConnect {
        /// The `CBPeripheral` associated with the callback.
        peripheral: Peripheral,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// A peripheral was disconnected.
    PeripheralDisconnected {
        /// The `CBPeripheral` associated with the callback.
        peripheral: Peripheral,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
}

fn parse_advertisement(raw: Option<serde_json::Value>) -> AdvertisementData {
    raw.and_then(|value| AdvertisementData::from_json_value(value).ok())
        .unwrap_or_default()
}

fn central_manager_event_from_envelope(env: EventEnvelope) -> Option<CentralManagerEvent> {
    match env.event.as_str() {
        "didUpdateState" => Some(CentralManagerEvent::StateChanged {
            state: CentralManagerState::from_raw(env.state.unwrap_or_default()),
            authorization: ManagerAuthorization::from_raw(env.authorization.unwrap_or_default()),
        }),
        "didDiscoverPeripheral" => Some(CentralManagerEvent::PeripheralDiscovered {
            peripheral: Peripheral::from_retained_handle(env.peripheral_handle?),
            rssi: env.rssi.unwrap_or_default(),
            advertisement_data: parse_advertisement(env.advertisement_data),
        }),
        "didConnectPeripheral" => Some(CentralManagerEvent::PeripheralConnected {
            peripheral: Peripheral::from_retained_handle(env.peripheral_handle?),
        }),
        "didFailToConnectPeripheral" => Some(CentralManagerEvent::PeripheralFailedToConnect {
            peripheral: Peripheral::from_retained_handle(env.peripheral_handle?),
            error: env.error,
        }),
        "didDisconnectPeripheral" => Some(CentralManagerEvent::PeripheralDisconnected {
            peripheral: Peripheral::from_retained_handle(env.peripheral_handle?),
            error: env.error,
        }),
        _ => None,
    }
}

unsafe extern "C" fn central_manager_event_cb(ctx: *mut c_void, payload: *const c_char) {
    if ctx.is_null() || payload.is_null() {
        return;
    }
    doom_fish_utils::panic_safe::catch_user_panic("central_manager_event_cb", || {
        // SAFETY: ctx is a valid *mut AsyncStreamSender<CentralManagerEvent> that was
        // created via Box::into_raw in CentralManagerEventStream::subscribe and is kept
        // alive until Drop calls Box::from_raw after unsubscribing the Swift delegate.
        // The delegate is cleared before Box::from_raw, so this borrow is valid on the
        // CoreBluetooth dispatch queue.  Note: if the manager was created with a custom
        // queue that differs from the thread calling drop(), an in-flight dispatch may
        // race with Box::from_raw — that window is documented and accepted.
        let sender = unsafe { &*ctx.cast::<AsyncStreamSender<CentralManagerEvent>>() };
        // SAFETY: payload is a non-null, NUL-terminated C string owned by the Swift
        // bridge for the duration of this call.
        let json = unsafe { core::ffi::CStr::from_ptr(payload) }
            .to_str()
            .unwrap_or_default();
        if let Ok(env) = serde_json::from_str::<EventEnvelope>(json) {
            if let Some(event) = central_manager_event_from_envelope(env) {
                sender.push(event);
            }
        }
    });
}

struct OpaqueDebug(&'static str);

impl fmt::Debug for OpaqueDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl fmt::Debug for CentralManagerEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateChanged {
                state,
                authorization,
            } => f
                .debug_struct("StateChanged")
                .field("state", state)
                .field("authorization", authorization)
                .finish(),
            Self::PeripheralDiscovered {
                rssi,
                advertisement_data,
                ..
            } => f
                .debug_struct("PeripheralDiscovered")
                .field("peripheral", &OpaqueDebug("Peripheral(..)"))
                .field("rssi", rssi)
                .field("advertisement_data", advertisement_data)
                .finish(),
            Self::PeripheralConnected { .. } => f
                .debug_struct("PeripheralConnected")
                .field("peripheral", &OpaqueDebug("Peripheral(..)"))
                .finish(),
            Self::PeripheralFailedToConnect { error, .. } => f
                .debug_struct("PeripheralFailedToConnect")
                .field("peripheral", &OpaqueDebug("Peripheral(..)"))
                .field("error", error)
                .finish(),
            Self::PeripheralDisconnected { error, .. } => f
                .debug_struct("PeripheralDisconnected")
                .field("peripheral", &OpaqueDebug("Peripheral(..)"))
                .field("error", error)
                .finish(),
        }
    }
}

impl fmt::Debug for PeripheralEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DidUpdateName => f.write_str("DidUpdateName"),
            Self::DidModifyServices {
                invalidated_services,
            } => f
                .debug_struct("DidModifyServices")
                .field("invalidated_service_count", &invalidated_services.len())
                .finish(),
            Self::DidDiscoverServices { services, error } => f
                .debug_struct("DidDiscoverServices")
                .field("service_count", &services.len())
                .field("error", error)
                .finish(),
            Self::DidDiscoverIncludedServices { error, .. } => f
                .debug_struct("DidDiscoverIncludedServices")
                .field("service", &OpaqueDebug("Service(..)"))
                .field("error", error)
                .finish(),
            Self::DidDiscoverCharacteristics {
                characteristics,
                error,
                ..
            } => f
                .debug_struct("DidDiscoverCharacteristics")
                .field("service", &OpaqueDebug("Service(..)"))
                .field("characteristic_count", &characteristics.len())
                .field("error", error)
                .finish(),
            Self::DidUpdateCharacteristicValue { error, .. } => f
                .debug_struct("DidUpdateCharacteristicValue")
                .field("characteristic", &OpaqueDebug("Characteristic(..)"))
                .field("error", error)
                .finish(),
            Self::DidWriteCharacteristicValue { error, .. } => f
                .debug_struct("DidWriteCharacteristicValue")
                .field("characteristic", &OpaqueDebug("Characteristic(..)"))
                .field("error", error)
                .finish(),
            Self::DidUpdateNotificationState { error, .. } => f
                .debug_struct("DidUpdateNotificationState")
                .field("characteristic", &OpaqueDebug("Characteristic(..)"))
                .field("error", error)
                .finish(),
            Self::DidDiscoverDescriptors { error, .. } => f
                .debug_struct("DidDiscoverDescriptors")
                .field("characteristic", &OpaqueDebug("Characteristic(..)"))
                .field("error", error)
                .finish(),
            Self::DidUpdateDescriptorValue { error, .. } => f
                .debug_struct("DidUpdateDescriptorValue")
                .field("descriptor", &OpaqueDebug("Descriptor(..)"))
                .field("error", error)
                .finish(),
            Self::DidWriteDescriptorValue { error, .. } => f
                .debug_struct("DidWriteDescriptorValue")
                .field("descriptor", &OpaqueDebug("Descriptor(..)"))
                .field("error", error)
                .finish(),
            Self::IsReadyToSendWriteWithoutResponse => {
                f.write_str("IsReadyToSendWriteWithoutResponse")
            }
            Self::DidReadRssi { rssi, error } => f
                .debug_struct("DidReadRssi")
                .field("rssi", rssi)
                .field("error", error)
                .finish(),
            Self::DidOpenL2capChannel { channel, error } => f
                .debug_struct("DidOpenL2capChannel")
                .field("channel_open", &channel.is_some())
                .field("error", error)
                .finish(),
        }
    }
}

impl fmt::Debug for PeripheralManagerEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateChanged {
                state,
                authorization,
            } => f
                .debug_struct("StateChanged")
                .field("state", state)
                .field("authorization", authorization)
                .finish(),
            Self::DidStartAdvertising { error } => f
                .debug_struct("DidStartAdvertising")
                .field("error", error)
                .finish(),
            Self::DidAddService { error, .. } => f
                .debug_struct("DidAddService")
                .field("service", &OpaqueDebug("Service(..)"))
                .field("error", error)
                .finish(),
            Self::DidSubscribeCentral { .. } => f
                .debug_struct("DidSubscribeCentral")
                .field("central", &OpaqueDebug("Central(..)"))
                .field("characteristic", &OpaqueDebug("Characteristic(..)"))
                .finish(),
            Self::DidUnsubscribeCentral { .. } => f
                .debug_struct("DidUnsubscribeCentral")
                .field("central", &OpaqueDebug("Central(..)"))
                .field("characteristic", &OpaqueDebug("Characteristic(..)"))
                .finish(),
            Self::IsReadyToUpdateSubscribers => f.write_str("IsReadyToUpdateSubscribers"),
            Self::DidReceiveReadRequest { .. } => f
                .debug_struct("DidReceiveReadRequest")
                .field("request", &OpaqueDebug("AttRequest(..)"))
                .finish(),
            Self::DidReceiveWriteRequests { requests } => f
                .debug_struct("DidReceiveWriteRequests")
                .field("request_count", &requests.len())
                .finish(),
            Self::DidPublishL2capChannel { psm, error } => f
                .debug_struct("DidPublishL2capChannel")
                .field("psm", psm)
                .field("error", error)
                .finish(),
            Self::DidUnpublishL2capChannel { psm, error } => f
                .debug_struct("DidUnpublishL2capChannel")
                .field("psm", psm)
                .field("error", error)
                .finish(),
            Self::DidOpenL2capChannel { channel, error } => f
                .debug_struct("DidOpenL2capChannel")
                .field("channel_open", &channel.is_some())
                .field("error", error)
                .finish(),
        }
    }
}

struct SubscriptionHandleCm {
    manager_ptr: *mut c_void,
    swift_handle: *mut c_void,
    sender_ptr: *mut AsyncStreamSender<CentralManagerEvent>,
}

impl Drop for SubscriptionHandleCm {
    fn drop(&mut self) {
        if !self.swift_handle.is_null() {
            // SAFETY: swift_handle is the retained opaque reference returned by
            // cb_central_manager_stream_subscribe.  This is the unique place we
            // release it; after this call the Swift bridge is no longer the
            // CBCentralManager delegate and will not fire the event callback again
            // (on the same dispatch queue as the manager).
            unsafe {
                crate::ffi::cb_central_manager_stream_unsubscribe(
                    self.manager_ptr,
                    self.swift_handle,
                );
            }
        }
        if !self.sender_ptr.is_null() {
            // SAFETY: sender_ptr was created via Box::into_raw in subscribe and is
            // not freed anywhere else.  Unsubscribe was called above so the Swift
            // bridge no longer holds this pointer as its ctx.
            unsafe {
                let _ = Box::from_raw(self.sender_ptr);
            }
        }
    }
}

unsafe impl Send for SubscriptionHandleCm {}
unsafe impl Sync for SubscriptionHandleCm {}

/// Async event stream for a [`CentralManager`].
///
/// Subscribe with [`CentralManagerEventStream::subscribe`] and
/// await events with `.next().await`.
///
/// Dropping the stream automatically unsubscribes from the underlying
/// Apple delegate.
pub struct CentralManagerEventStream {
    inner: BoundedAsyncStream<CentralManagerEvent>,
    _handle: SubscriptionHandleCm,
}

impl CentralManagerEventStream {
    /// Subscribe to events from `manager` with a ring buffer of `capacity` events.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    pub fn subscribe(manager: &CentralManager, capacity: usize) -> Self {
        let (stream, sender) = BoundedAsyncStream::new(capacity);
        let sender_ptr = Box::into_raw(Box::new(sender));
        let manager_ptr = manager.as_raw();
        // SAFETY: manager_ptr is a valid CBCentralManager handle for the lifetime of
        // `manager`.  sender_ptr is a valid heap allocation; ownership is transferred
        // to the SubscriptionHandleCm which frees it in Drop after unsubscribing.
        let swift_handle = unsafe {
            crate::ffi::cb_central_manager_stream_subscribe(
                manager_ptr,
                central_manager_event_cb,
                sender_ptr.cast(),
            )
        };
        Self {
            inner: stream,
            _handle: SubscriptionHandleCm {
                manager_ptr,
                swift_handle,
                sender_ptr,
            },
        }
    }

    /// Await the next event. Returns `None` when the stream is closed.
    pub fn next(&self) -> NextItem<'_, CentralManagerEvent> {
        self.inner.next()
    }

    /// Non-blocking: returns the next buffered event, or `None` if the buffer is empty.
    pub fn try_next(&self) -> Option<CentralManagerEvent> {
        self.inner.try_next()
    }

    /// Returns the number of currently buffered events.
    pub fn buffered_count(&self) -> usize {
        self.inner.buffered_count()
    }
}

/// An event emitted by a [`PeripheralEventStream`].
#[non_exhaustive]
pub enum PeripheralEvent {
    /// Corresponds to `peripheralDidUpdateName:`.
    DidUpdateName,
    /// Corresponds to `peripheral:didModifyServices:`.
    DidModifyServices {
        /// The services invalidated by `CoreBluetooth`.
        invalidated_services: Vec<Service>,
    },
    /// Corresponds to `peripheral:didDiscoverServices:`.
    DidDiscoverServices {
        /// Services carried by the `CoreBluetooth` callback or restored state.
        services: Vec<Service>,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheral:didDiscoverIncludedServicesForService:error:`.
    DidDiscoverIncludedServices {
        /// The `CBService` associated with the callback.
        service: Service,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheral:didDiscoverCharacteristicsForService:error:`.
    DidDiscoverCharacteristics {
        /// The `CBService` associated with the callback.
        service: Service,
        /// The characteristics delivered by `CoreBluetooth`.
        characteristics: Vec<Characteristic>,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheral:didUpdateValueForCharacteristic:error:`.
    DidUpdateCharacteristicValue {
        /// The `CBCharacteristic` associated with the callback.
        characteristic: Characteristic,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheral:didWriteValueForCharacteristic:error:`.
    DidWriteCharacteristicValue {
        /// The `CBCharacteristic` associated with the callback.
        characteristic: Characteristic,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheral:didUpdateNotificationStateForCharacteristic:error:`.
    DidUpdateNotificationState {
        /// The `CBCharacteristic` associated with the callback.
        characteristic: Characteristic,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheral:didDiscoverDescriptorsForCharacteristic:error:`.
    DidDiscoverDescriptors {
        /// The `CBCharacteristic` associated with the callback.
        characteristic: Characteristic,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheral:didUpdateValueForDescriptor:error:`.
    DidUpdateDescriptorValue {
        /// The `CBDescriptor` associated with the callback.
        descriptor: Descriptor,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheral:didWriteValueForDescriptor:error:`.
    DidWriteDescriptorValue {
        /// The `CBDescriptor` associated with the callback.
        descriptor: Descriptor,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheralIsReadyToSendWriteWithoutResponse:`.
    IsReadyToSendWriteWithoutResponse,
    /// Corresponds to `peripheral:didReadRSSI:error:`.
    DidReadRssi {
        /// The RSSI value reported by `CoreBluetooth`.
        rssi: i32,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheral:didOpenL2CAPChannel:error:`.
    DidOpenL2capChannel {
        /// The `CBL2CAPChannel` associated with the callback, if one was opened.
        channel: Option<L2capChannel>,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
}

fn peripheral_event_from_envelope(env: EventEnvelope) -> Option<PeripheralEvent> {
    match env.event.as_str() {
        "didUpdateName" => Some(PeripheralEvent::DidUpdateName),
        "didModifyServices" => Some(PeripheralEvent::DidModifyServices {
            invalidated_services: env
                .invalidated_service_handles
                .unwrap_or_default()
                .into_iter()
                .map(Service::from_retained_handle)
                .collect(),
        }),
        "didDiscoverServices" => Some(PeripheralEvent::DidDiscoverServices {
            services: env
                .service_handles
                .unwrap_or_default()
                .into_iter()
                .map(Service::from_retained_handle)
                .collect(),
            error: env.error,
        }),
        "didDiscoverIncludedServicesForService" => {
            Some(PeripheralEvent::DidDiscoverIncludedServices {
                service: Service::from_retained_handle(env.service_handle?),
                error: env.error,
            })
        }
        "didDiscoverCharacteristicsForService" => {
            Some(PeripheralEvent::DidDiscoverCharacteristics {
                service: Service::from_retained_handle(env.service_handle?),
                characteristics: env
                    .characteristic_handles
                    .unwrap_or_default()
                    .into_iter()
                    .map(Characteristic::from_retained_handle)
                    .collect(),
                error: env.error,
            })
        }
        "didUpdateValueForCharacteristic" => Some(PeripheralEvent::DidUpdateCharacteristicValue {
            characteristic: Characteristic::from_retained_handle(env.characteristic_handle?),
            error: env.error,
        }),
        "didWriteValueForCharacteristic" => Some(PeripheralEvent::DidWriteCharacteristicValue {
            characteristic: Characteristic::from_retained_handle(env.characteristic_handle?),
            error: env.error,
        }),
        "didUpdateNotificationStateForCharacteristic" => {
            Some(PeripheralEvent::DidUpdateNotificationState {
                characteristic: Characteristic::from_retained_handle(env.characteristic_handle?),
                error: env.error,
            })
        }
        "didDiscoverDescriptorsForCharacteristic" => {
            Some(PeripheralEvent::DidDiscoverDescriptors {
                characteristic: Characteristic::from_retained_handle(env.characteristic_handle?),
                error: env.error,
            })
        }
        "didUpdateValueForDescriptor" => Some(PeripheralEvent::DidUpdateDescriptorValue {
            descriptor: Descriptor::from_retained_handle(env.descriptor_handle?),
            error: env.error,
        }),
        "didWriteValueForDescriptor" => Some(PeripheralEvent::DidWriteDescriptorValue {
            descriptor: Descriptor::from_retained_handle(env.descriptor_handle?),
            error: env.error,
        }),
        "isReadyToSendWriteWithoutResponse" => {
            Some(PeripheralEvent::IsReadyToSendWriteWithoutResponse)
        }
        "didReadRSSI" => Some(PeripheralEvent::DidReadRssi {
            rssi: env.rssi.unwrap_or_default(),
            error: env.error,
        }),
        "didOpenL2CAPChannel" => Some(PeripheralEvent::DidOpenL2capChannel {
            channel: env.channel_handle.map(L2capChannel::from_retained_handle),
            error: env.error,
        }),
        _ => None,
    }
}

unsafe extern "C" fn peripheral_event_cb(ctx: *mut c_void, payload: *const c_char) {
    if ctx.is_null() || payload.is_null() {
        return;
    }
    doom_fish_utils::panic_safe::catch_user_panic("peripheral_event_cb", || {
        // SAFETY: ctx is a valid *mut AsyncStreamSender<PeripheralEvent> created via
        // Box::into_raw in PeripheralEventStream::subscribe and kept alive until Drop
        // calls Box::from_raw after unsubscribing the Swift bridge.
        let sender = unsafe { &*ctx.cast::<AsyncStreamSender<PeripheralEvent>>() };
        // SAFETY: payload is a non-null, NUL-terminated C string for the duration of
        // this call.
        let json = unsafe { core::ffi::CStr::from_ptr(payload) }
            .to_str()
            .unwrap_or_default();
        if let Ok(env) = serde_json::from_str::<EventEnvelope>(json) {
            if let Some(event) = peripheral_event_from_envelope(env) {
                sender.push(event);
            }
        }
    });
}

struct SubscriptionHandlePe {
    peripheral_ptr: *mut c_void,
    swift_handle: *mut c_void,
    sender_ptr: *mut AsyncStreamSender<PeripheralEvent>,
}

impl Drop for SubscriptionHandlePe {
    fn drop(&mut self) {
        if !self.swift_handle.is_null() {
            // SAFETY: swift_handle is the retained opaque reference returned by
            // cb_peripheral_stream_subscribe, released here exactly once.
            unsafe {
                crate::ffi::cb_peripheral_stream_unsubscribe(
                    self.peripheral_ptr,
                    self.swift_handle,
                );
            }
        }
        if !self.sender_ptr.is_null() {
            // SAFETY: sender_ptr was created via Box::into_raw in subscribe and is
            // not freed anywhere else.  Unsubscribe was called above so the Swift
            // bridge no longer holds this pointer as its ctx.
            unsafe {
                let _ = Box::from_raw(self.sender_ptr);
            }
        }
    }
}

unsafe impl Send for SubscriptionHandlePe {}
unsafe impl Sync for SubscriptionHandlePe {}

/// Async event stream for a [`Peripheral`].
///
/// Subscribe with [`PeripheralEventStream::subscribe`] and
/// await events with `.next().await`.
pub struct PeripheralEventStream {
    inner: BoundedAsyncStream<PeripheralEvent>,
    _handle: SubscriptionHandlePe,
}

impl PeripheralEventStream {
    /// Subscribe to delegate events from `peripheral`.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    pub fn subscribe(peripheral: &Peripheral, capacity: usize) -> Self {
        let (stream, sender) = BoundedAsyncStream::new(capacity);
        let sender_ptr = Box::into_raw(Box::new(sender));
        let peripheral_ptr = peripheral.as_raw();
        // SAFETY: peripheral_ptr is a valid CBPeripheral handle for the lifetime of
        // `peripheral`.  sender_ptr is a valid heap allocation owned by the returned
        // SubscriptionHandlePe which frees it in Drop after unsubscribing.
        let swift_handle = unsafe {
            crate::ffi::cb_peripheral_stream_subscribe(
                peripheral_ptr,
                peripheral_event_cb,
                sender_ptr.cast(),
            )
        };
        Self {
            inner: stream,
            _handle: SubscriptionHandlePe {
                peripheral_ptr,
                swift_handle,
                sender_ptr,
            },
        }
    }

    /// Await the next event. Returns `None` when the stream is closed.
    pub fn next(&self) -> NextItem<'_, PeripheralEvent> {
        self.inner.next()
    }

    /// Non-blocking pop.
    pub fn try_next(&self) -> Option<PeripheralEvent> {
        self.inner.try_next()
    }

    /// Returns the number of currently buffered events.
    pub fn buffered_count(&self) -> usize {
        self.inner.buffered_count()
    }
}

/// An event emitted by a [`PeripheralManagerEventStream`].
#[non_exhaustive]
pub enum PeripheralManagerEvent {
    /// Corresponds to `peripheralManagerDidUpdateState:`.
    StateChanged {
        /// The state value reported by `CoreBluetooth`.
        state: PeripheralManagerState,
        /// The authorization value reported by `CoreBluetooth`.
        authorization: ManagerAuthorization,
    },
    /// Corresponds to `peripheralManagerDidStartAdvertising:error:`.
    DidStartAdvertising {
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheralManager:didAddService:error:`.
    DidAddService {
        /// The `CBService` associated with the callback.
        service: Service,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheralManager:central:didSubscribeToCharacteristic:`.
    DidSubscribeCentral {
        /// The `CBCentral` associated with the callback.
        central: Central,
        /// The `CBCharacteristic` associated with the callback.
        characteristic: Characteristic,
    },
    /// Corresponds to `peripheralManager:central:didUnsubscribeFromCharacteristic:`.
    DidUnsubscribeCentral {
        /// The `CBCentral` associated with the callback.
        central: Central,
        /// The `CBCharacteristic` associated with the callback.
        characteristic: Characteristic,
    },
    /// Corresponds to `peripheralManagerIsReadyToUpdateSubscribers:`.
    IsReadyToUpdateSubscribers,
    /// Corresponds to `peripheralManager:didReceiveReadRequest:`.
    DidReceiveReadRequest {
        /// The `CBATTRequest` associated with the callback.
        request: AttRequest,
    },
    /// Corresponds to `peripheralManager:didReceiveWriteRequests:`.
    DidReceiveWriteRequests {
        /// The `CBATTRequest` values carried by the callback.
        requests: Vec<AttRequest>,
    },
    /// Corresponds to `peripheralManager:didPublishL2CAPChannel:error:`.
    DidPublishL2capChannel {
        /// The L2CAP PSM reported by `CoreBluetooth`.
        psm: u16,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheralManager:didUnpublishL2CAPChannel:error:`.
    DidUnpublishL2capChannel {
        /// The L2CAP PSM reported by `CoreBluetooth`.
        psm: u16,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
    /// Corresponds to `peripheralManager:didOpenL2CAPChannel:error:`.
    DidOpenL2capChannel {
        /// The `CBL2CAPChannel` associated with the callback, if one was opened.
        channel: Option<L2capChannel>,
        /// The `CoreBluetooth` error metadata, if any.
        error: Option<BluetoothErrorInfo>,
    },
}

fn peripheral_manager_event_from_envelope(env: EventEnvelope) -> Option<PeripheralManagerEvent> {
    match env.event.as_str() {
        "didUpdateState" => Some(PeripheralManagerEvent::StateChanged {
            state: PeripheralManagerState::from_raw(env.state.unwrap_or_default()),
            authorization: ManagerAuthorization::from_raw(env.authorization.unwrap_or_default()),
        }),
        "didStartAdvertising" => {
            Some(PeripheralManagerEvent::DidStartAdvertising { error: env.error })
        }
        "didAddService" => Some(PeripheralManagerEvent::DidAddService {
            service: Service::from_retained_handle(env.service_handle?),
            error: env.error,
        }),
        "didSubscribeCentral" => Some(PeripheralManagerEvent::DidSubscribeCentral {
            central: Central::from_retained_handle(env.central_handle?),
            characteristic: Characteristic::from_retained_handle(env.characteristic_handle?),
        }),
        "didUnsubscribeCentral" => Some(PeripheralManagerEvent::DidUnsubscribeCentral {
            central: Central::from_retained_handle(env.central_handle?),
            characteristic: Characteristic::from_retained_handle(env.characteristic_handle?),
        }),
        "isReadyToUpdateSubscribers" => Some(PeripheralManagerEvent::IsReadyToUpdateSubscribers),
        "didReceiveReadRequest" => Some(PeripheralManagerEvent::DidReceiveReadRequest {
            request: AttRequest::from_retained_handle(env.request_handle?),
        }),
        "didReceiveWriteRequests" => Some(PeripheralManagerEvent::DidReceiveWriteRequests {
            requests: env
                .request_handles
                .unwrap_or_default()
                .into_iter()
                .map(AttRequest::from_retained_handle)
                .collect(),
        }),
        "didPublishL2CAPChannel" => Some(PeripheralManagerEvent::DidPublishL2capChannel {
            psm: env.psm.unwrap_or_default(),
            error: env.error,
        }),
        "didUnpublishL2CAPChannel" => Some(PeripheralManagerEvent::DidUnpublishL2capChannel {
            psm: env.psm.unwrap_or_default(),
            error: env.error,
        }),
        "didOpenL2CAPChannel" => Some(PeripheralManagerEvent::DidOpenL2capChannel {
            channel: env.channel_handle.map(L2capChannel::from_retained_handle),
            error: env.error,
        }),
        _ => None,
    }
}

unsafe extern "C" fn peripheral_manager_event_cb(ctx: *mut c_void, payload: *const c_char) {
    if ctx.is_null() || payload.is_null() {
        return;
    }
    doom_fish_utils::panic_safe::catch_user_panic("peripheral_manager_event_cb", || {
        // SAFETY: ctx is a valid *mut AsyncStreamSender<PeripheralManagerEvent> created
        // via Box::into_raw in PeripheralManagerEventStream::subscribe and kept alive
        // until Drop calls Box::from_raw after unsubscribing the Swift bridge.
        let sender = unsafe { &*ctx.cast::<AsyncStreamSender<PeripheralManagerEvent>>() };
        // SAFETY: payload is a non-null, NUL-terminated C string for the duration of
        // this call.
        let json = unsafe { core::ffi::CStr::from_ptr(payload) }
            .to_str()
            .unwrap_or_default();
        if let Ok(env) = serde_json::from_str::<EventEnvelope>(json) {
            if let Some(event) = peripheral_manager_event_from_envelope(env) {
                sender.push(event);
            }
        }
    });
}

struct SubscriptionHandlePm {
    manager_ptr: *mut c_void,
    swift_handle: *mut c_void,
    sender_ptr: *mut AsyncStreamSender<PeripheralManagerEvent>,
}

impl Drop for SubscriptionHandlePm {
    fn drop(&mut self) {
        if !self.swift_handle.is_null() {
            // SAFETY: swift_handle is the retained opaque reference returned by
            // cb_peripheral_manager_stream_subscribe, released here exactly once.
            unsafe {
                crate::ffi::cb_peripheral_manager_stream_unsubscribe(
                    self.manager_ptr,
                    self.swift_handle,
                );
            }
        }
        if !self.sender_ptr.is_null() {
            // SAFETY: sender_ptr was created via Box::into_raw in subscribe and is
            // not freed anywhere else.  Unsubscribe was called above so the Swift
            // bridge no longer holds this pointer as its ctx.
            unsafe {
                let _ = Box::from_raw(self.sender_ptr);
            }
        }
    }
}

unsafe impl Send for SubscriptionHandlePm {}
unsafe impl Sync for SubscriptionHandlePm {}

/// Async event stream for a [`PeripheralManager`].
pub struct PeripheralManagerEventStream {
    inner: BoundedAsyncStream<PeripheralManagerEvent>,
    _handle: SubscriptionHandlePm,
}

impl PeripheralManagerEventStream {
    /// Subscribe to events from `manager`.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    pub fn subscribe(manager: &PeripheralManager, capacity: usize) -> Self {
        let (stream, sender) = BoundedAsyncStream::new(capacity);
        let sender_ptr = Box::into_raw(Box::new(sender));
        let manager_ptr = manager.as_raw();
        // SAFETY: manager_ptr is a valid CBPeripheralManager handle for the lifetime of
        // `manager`.  sender_ptr is a valid heap allocation owned by the returned
        // SubscriptionHandlePm which frees it in Drop after unsubscribing.
        let swift_handle = unsafe {
            crate::ffi::cb_peripheral_manager_stream_subscribe(
                manager_ptr,
                peripheral_manager_event_cb,
                sender_ptr.cast(),
            )
        };
        Self {
            inner: stream,
            _handle: SubscriptionHandlePm {
                manager_ptr,
                swift_handle,
                sender_ptr,
            },
        }
    }

    /// Await the next event. Returns `None` when the stream is closed.
    pub fn next(&self) -> NextItem<'_, PeripheralManagerEvent> {
        self.inner.next()
    }

    /// Non-blocking pop.
    pub fn try_next(&self) -> Option<PeripheralManagerEvent> {
        self.inner.try_next()
    }

    /// Returns the number of currently buffered events.
    pub fn buffered_count(&self) -> usize {
        self.inner.buffered_count()
    }
}
