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
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{ready, Context, Poll};

use doom_fish_utils::callback_context::CallbackContext;
use doom_fish_utils::stream::{AsyncStreamSender, BoundedAsyncStream, NextItem};
use serde::Deserialize;

use crate::advertisement::AdvertisementData;
use crate::att::AttRequest;
use crate::central_manager::{CentralManager, CentralManagerState, ManagerAuthorization};
use crate::characteristic::Characteristic;
use crate::descriptor::Descriptor;
use crate::error::BluetoothErrorInfo;
use crate::ffi::{ContextRefCallback, JsonCallback};
use crate::l2cap_channel::L2capChannel;
use crate::peripheral::Peripheral;
use crate::peripheral_manager::{Central, PeripheralManager, PeripheralManagerState};
use crate::private::{retain_raw, retained_handle_to_raw};
use crate::service::Service;

type SinkContext = CallbackContext<AsyncStreamSender<EventEnvelope>>;

type SubscribeFn = unsafe extern "C" fn(
    *mut c_void,
    JsonCallback,
    *mut c_void,
    Option<ContextRefCallback>,
    Option<ContextRefCallback>,
) -> *mut c_void;
type UnsubscribeFn = unsafe extern "C" fn(*mut c_void, *mut c_void);

struct Subscription {
    owner: *mut c_void,
    sink: *mut c_void,
    context: SinkContext,
    unsubscribe: UnsubscribeFn,
}

impl Subscription {
    fn new(
        owner: *mut c_void,
        sender: AsyncStreamSender<EventEnvelope>,
        subscribe: SubscribeFn,
        unsubscribe: UnsubscribeFn,
    ) -> Self {
        let owner = retain_raw(owner);
        let context = SinkContext::new(sender);
        let sink = unsafe {
            subscribe(
                owner,
                stream_event_cb,
                context.as_ptr(),
                Some(SinkContext::RETAIN),
                Some(SinkContext::RELEASE),
            )
        };
        Self {
            owner,
            sink,
            context,
            unsubscribe,
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.context.deactivate();
        unsafe { (self.unsubscribe)(self.owner, self.sink) };
        unsafe { crate::ffi::cb_object_release(self.owner) };
    }
}

unsafe extern "C" fn stream_event_cb(ctx: *mut c_void, payload: *const c_char) {
    if payload.is_null() {
        return;
    }
    doom_fish_utils::panic_safe::catch_user_panic("stream_event_cb", || {
        let json = unsafe { core::ffi::CStr::from_ptr(payload) }
            .to_str()
            .unwrap_or_default();
        let Ok(envelope) = serde_json::from_str::<EventEnvelope>(json) else {
            return;
        };
        unsafe {
            SinkContext::with(ctx, "stream_event_cb", |sender| sender.push(envelope));
        }
    });
}

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

impl Drop for EventEnvelope {
    fn drop(&mut self) {
        let handles = [
            self.peripheral_handle,
            self.service_handle,
            self.characteristic_handle,
            self.descriptor_handle,
            self.channel_handle,
            self.central_handle,
            self.request_handle,
        ];
        let handle_lists = [
            self.service_handles.as_deref(),
            self.invalidated_service_handles.as_deref(),
            self.characteristic_handles.as_deref(),
            self.request_handles.as_deref(),
        ];
        for handle in handles
            .into_iter()
            .flatten()
            .chain(handle_lists.into_iter().flatten().flatten().copied())
        {
            unsafe { crate::ffi::cb_object_release(retained_handle_to_raw(handle)) };
        }
    }
}

type ConvertFn<E> = fn(&mut EventEnvelope) -> Option<E>;

pub struct NextEvent<'a, E> {
    envelopes: &'a BoundedAsyncStream<EventEnvelope>,
    pending: NextItem<'a, EventEnvelope>,
    convert: ConvertFn<E>,
    _polled_where_created: PhantomData<*const E>,
}

impl<E> fmt::Debug for NextEvent<'_, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NextEvent").finish_non_exhaustive()
    }
}

impl<E> Future for NextEvent<'_, E> {
    type Output = Option<E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        loop {
            let Some(mut envelope) = ready!(Pin::new(&mut this.pending).poll(cx)) else {
                return Poll::Ready(None);
            };
            if let Some(event) = (this.convert)(&mut envelope) {
                return Poll::Ready(Some(event));
            }
            this.pending = this.envelopes.next();
        }
    }
}

fn next_event<E>(
    envelopes: &BoundedAsyncStream<EventEnvelope>,
    convert: ConvertFn<E>,
) -> NextEvent<'_, E> {
    NextEvent {
        envelopes,
        pending: envelopes.next(),
        convert,
        _polled_where_created: PhantomData,
    }
}

fn try_next_event<E>(
    envelopes: &BoundedAsyncStream<EventEnvelope>,
    convert: ConvertFn<E>,
) -> Option<E> {
    loop {
        let mut envelope = envelopes.try_next()?;
        if let Some(event) = convert(&mut envelope) {
            return Some(event);
        }
    }
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

fn central_manager_event_from_envelope(env: &mut EventEnvelope) -> Option<CentralManagerEvent> {
    match env.event.as_str() {
        "didUpdateState" => Some(CentralManagerEvent::StateChanged {
            state: CentralManagerState::from_raw(env.state.unwrap_or_default()),
            authorization: ManagerAuthorization::from_raw(env.authorization.unwrap_or_default()),
        }),
        "didDiscoverPeripheral" => Some(CentralManagerEvent::PeripheralDiscovered {
            peripheral: Peripheral::from_retained_handle(env.peripheral_handle.take()?),
            rssi: env.rssi.unwrap_or_default(),
            advertisement_data: parse_advertisement(env.advertisement_data.take()),
        }),
        "didConnectPeripheral" => Some(CentralManagerEvent::PeripheralConnected {
            peripheral: Peripheral::from_retained_handle(env.peripheral_handle.take()?),
        }),
        "didFailToConnectPeripheral" => Some(CentralManagerEvent::PeripheralFailedToConnect {
            peripheral: Peripheral::from_retained_handle(env.peripheral_handle.take()?),
            error: env.error.take(),
        }),
        "didDisconnectPeripheral" => Some(CentralManagerEvent::PeripheralDisconnected {
            peripheral: Peripheral::from_retained_handle(env.peripheral_handle.take()?),
            error: env.error.take(),
        }),
        _ => None,
    }
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

/// Async event stream for a [`CentralManager`].
///
/// Subscribe with [`CentralManagerEventStream::subscribe`] and
/// await events with `.next().await`.
///
/// Dropping the stream automatically unsubscribes from the underlying
/// Apple delegate.
pub struct CentralManagerEventStream {
    inner: BoundedAsyncStream<EventEnvelope>,
    _handle: Subscription,
}

impl CentralManagerEventStream {
    /// Subscribe to events from `manager` with a ring buffer of `capacity` events.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    pub fn subscribe(manager: &CentralManager, capacity: usize) -> Self {
        let (stream, sender) = BoundedAsyncStream::new(capacity);
        Self {
            inner: stream,
            _handle: Subscription::new(
                manager.as_raw(),
                sender,
                crate::ffi::cb_central_manager_stream_subscribe,
                crate::ffi::cb_central_manager_stream_unsubscribe,
            ),
        }
    }

    /// Await the next event. Returns `None` when the stream is closed.
    pub fn next(&self) -> NextEvent<'_, CentralManagerEvent> {
        next_event(&self.inner, central_manager_event_from_envelope)
    }

    /// Non-blocking: returns the next buffered event, or `None` if the buffer is empty.
    pub fn try_next(&self) -> Option<CentralManagerEvent> {
        try_next_event(&self.inner, central_manager_event_from_envelope)
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

fn peripheral_event_from_envelope(env: &mut EventEnvelope) -> Option<PeripheralEvent> {
    match env.event.as_str() {
        "didUpdateName" => Some(PeripheralEvent::DidUpdateName),
        "didModifyServices" => Some(PeripheralEvent::DidModifyServices {
            invalidated_services: env
                .invalidated_service_handles
                .take()
                .unwrap_or_default()
                .into_iter()
                .map(Service::from_retained_handle)
                .collect(),
        }),
        "didDiscoverServices" => Some(PeripheralEvent::DidDiscoverServices {
            services: env
                .service_handles
                .take()
                .unwrap_or_default()
                .into_iter()
                .map(Service::from_retained_handle)
                .collect(),
            error: env.error.take(),
        }),
        "didDiscoverIncludedServicesForService" => {
            Some(PeripheralEvent::DidDiscoverIncludedServices {
                service: Service::from_retained_handle(env.service_handle.take()?),
                error: env.error.take(),
            })
        }
        "didDiscoverCharacteristicsForService" => {
            Some(PeripheralEvent::DidDiscoverCharacteristics {
                service: Service::from_retained_handle(env.service_handle.take()?),
                characteristics: env
                    .characteristic_handles
                    .take()
                    .unwrap_or_default()
                    .into_iter()
                    .map(Characteristic::from_retained_handle)
                    .collect(),
                error: env.error.take(),
            })
        }
        "didUpdateValueForCharacteristic" => Some(PeripheralEvent::DidUpdateCharacteristicValue {
            characteristic: Characteristic::from_retained_handle(env.characteristic_handle.take()?),
            error: env.error.take(),
        }),
        "didWriteValueForCharacteristic" => Some(PeripheralEvent::DidWriteCharacteristicValue {
            characteristic: Characteristic::from_retained_handle(env.characteristic_handle.take()?),
            error: env.error.take(),
        }),
        "didUpdateNotificationStateForCharacteristic" => {
            Some(PeripheralEvent::DidUpdateNotificationState {
                characteristic: Characteristic::from_retained_handle(
                    env.characteristic_handle.take()?,
                ),
                error: env.error.take(),
            })
        }
        "didDiscoverDescriptorsForCharacteristic" => {
            Some(PeripheralEvent::DidDiscoverDescriptors {
                characteristic: Characteristic::from_retained_handle(
                    env.characteristic_handle.take()?,
                ),
                error: env.error.take(),
            })
        }
        "didUpdateValueForDescriptor" => Some(PeripheralEvent::DidUpdateDescriptorValue {
            descriptor: Descriptor::from_retained_handle(env.descriptor_handle.take()?),
            error: env.error.take(),
        }),
        "didWriteValueForDescriptor" => Some(PeripheralEvent::DidWriteDescriptorValue {
            descriptor: Descriptor::from_retained_handle(env.descriptor_handle.take()?),
            error: env.error.take(),
        }),
        "isReadyToSendWriteWithoutResponse" => {
            Some(PeripheralEvent::IsReadyToSendWriteWithoutResponse)
        }
        "didReadRSSI" => Some(PeripheralEvent::DidReadRssi {
            rssi: env.rssi.unwrap_or_default(),
            error: env.error.take(),
        }),
        "didOpenL2CAPChannel" => Some(PeripheralEvent::DidOpenL2capChannel {
            channel: env
                .channel_handle
                .take()
                .map(L2capChannel::from_retained_handle),
            error: env.error.take(),
        }),
        _ => None,
    }
}

/// Async event stream for a [`Peripheral`].
///
/// Subscribe with [`PeripheralEventStream::subscribe`] and
/// await events with `.next().await`.
pub struct PeripheralEventStream {
    inner: BoundedAsyncStream<EventEnvelope>,
    _handle: Subscription,
}

impl PeripheralEventStream {
    /// Subscribe to delegate events from `peripheral`.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    pub fn subscribe(peripheral: &Peripheral, capacity: usize) -> Self {
        let (stream, sender) = BoundedAsyncStream::new(capacity);
        Self {
            inner: stream,
            _handle: Subscription::new(
                peripheral.as_raw(),
                sender,
                crate::ffi::cb_peripheral_stream_subscribe,
                crate::ffi::cb_peripheral_stream_unsubscribe,
            ),
        }
    }

    /// Await the next event. Returns `None` when the stream is closed.
    pub fn next(&self) -> NextEvent<'_, PeripheralEvent> {
        next_event(&self.inner, peripheral_event_from_envelope)
    }

    /// Non-blocking pop.
    pub fn try_next(&self) -> Option<PeripheralEvent> {
        try_next_event(&self.inner, peripheral_event_from_envelope)
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

fn peripheral_manager_event_from_envelope(
    env: &mut EventEnvelope,
) -> Option<PeripheralManagerEvent> {
    match env.event.as_str() {
        "didUpdateState" => Some(PeripheralManagerEvent::StateChanged {
            state: PeripheralManagerState::from_raw(env.state.unwrap_or_default()),
            authorization: ManagerAuthorization::from_raw(env.authorization.unwrap_or_default()),
        }),
        "didStartAdvertising" => Some(PeripheralManagerEvent::DidStartAdvertising {
            error: env.error.take(),
        }),
        "didAddService" => Some(PeripheralManagerEvent::DidAddService {
            service: Service::from_retained_handle(env.service_handle.take()?),
            error: env.error.take(),
        }),
        "didSubscribeToCharacteristic" => Some(PeripheralManagerEvent::DidSubscribeCentral {
            central: Central::from_retained_handle(env.central_handle.take()?),
            characteristic: Characteristic::from_retained_handle(env.characteristic_handle.take()?),
        }),
        "didUnsubscribeFromCharacteristic" => Some(PeripheralManagerEvent::DidUnsubscribeCentral {
            central: Central::from_retained_handle(env.central_handle.take()?),
            characteristic: Characteristic::from_retained_handle(env.characteristic_handle.take()?),
        }),
        "isReadyToUpdateSubscribers" => Some(PeripheralManagerEvent::IsReadyToUpdateSubscribers),
        "didReceiveReadRequest" => Some(PeripheralManagerEvent::DidReceiveReadRequest {
            request: AttRequest::from_retained_handle(env.request_handle.take()?),
        }),
        "didReceiveWriteRequests" => Some(PeripheralManagerEvent::DidReceiveWriteRequests {
            requests: env
                .request_handles
                .take()
                .unwrap_or_default()
                .into_iter()
                .map(AttRequest::from_retained_handle)
                .collect(),
        }),
        "didPublishL2CAPChannel" => Some(PeripheralManagerEvent::DidPublishL2capChannel {
            psm: env.psm.unwrap_or_default(),
            error: env.error.take(),
        }),
        "didUnpublishL2CAPChannel" => Some(PeripheralManagerEvent::DidUnpublishL2capChannel {
            psm: env.psm.unwrap_or_default(),
            error: env.error.take(),
        }),
        "didOpenL2CAPChannel" => Some(PeripheralManagerEvent::DidOpenL2capChannel {
            channel: env
                .channel_handle
                .take()
                .map(L2capChannel::from_retained_handle),
            error: env.error.take(),
        }),
        _ => None,
    }
}

/// Async event stream for a [`PeripheralManager`].
pub struct PeripheralManagerEventStream {
    inner: BoundedAsyncStream<EventEnvelope>,
    _handle: Subscription,
}

impl PeripheralManagerEventStream {
    /// Subscribe to events from `manager`.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    pub fn subscribe(manager: &PeripheralManager, capacity: usize) -> Self {
        let (stream, sender) = BoundedAsyncStream::new(capacity);
        Self {
            inner: stream,
            _handle: Subscription::new(
                manager.as_raw(),
                sender,
                crate::ffi::cb_peripheral_manager_stream_subscribe,
                crate::ffi::cb_peripheral_manager_stream_unsubscribe,
            ),
        }
    }

    /// Await the next event. Returns `None` when the stream is closed.
    pub fn next(&self) -> NextEvent<'_, PeripheralManagerEvent> {
        next_event(&self.inner, peripheral_manager_event_from_envelope)
    }

    /// Non-blocking pop.
    pub fn try_next(&self) -> Option<PeripheralManagerEvent> {
        try_next_event(&self.inner, peripheral_manager_event_from_envelope)
    }

    /// Returns the number of currently buffered events.
    pub fn buffered_count(&self) -> usize {
        self.inner.buffered_count()
    }
}

#[cfg(test)]
mod tests {
    use core::ffi::c_void;
    use std::time::{Duration, Instant};

    use doom_fish_utils::stream::BoundedAsyncStream;

    use super::{
        next_event, peripheral_manager_event_from_envelope, try_next_event, EventEnvelope,
        PeripheralManagerEvent, Subscription,
    };
    use crate::private::retain_raw;
    use crate::{BluetoothUuid, CentralManager, MutableService, PeripheralManager};

    fn live_tests_enabled() -> bool {
        let enabled = std::env::var("COREBLUETOOTH_LIVE_TESTS").as_deref() == Ok("1");
        if !enabled {
            eprintln!(
                "skip: set COREBLUETOOTH_LIVE_TESTS=1 to run tests that create Bluetooth managers"
            );
        }
        enabled
    }

    fn closes_soon<T>(stream: &BoundedAsyncStream<T>) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if stream.is_closed() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        stream.is_closed()
    }

    fn retain_count(raw: *mut c_void) -> i64 {
        unsafe { apple_cf::raw::CFGetRetainCount(raw.cast_const().cast()) }
    }

    fn envelope(json: &str) -> EventEnvelope {
        serde_json::from_str(json).expect("event envelope")
    }

    fn block_on<F: core::future::Future>(future: F) -> F::Output {
        struct Unpark(std::thread::Thread);

        impl std::task::Wake for Unpark {
            fn wake(self: std::sync::Arc<Self>) {
                self.0.unpark();
            }
        }

        let waker = std::sync::Arc::new(Unpark(std::thread::current())).into();
        let mut context = std::task::Context::from_waker(&waker);
        let mut future = core::pin::pin!(future);
        loop {
            if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }
            std::thread::park_timeout(Duration::from_millis(50));
        }
    }

    #[test]
    fn unsubscribing_releases_the_swift_sink_and_its_sender() {
        if !live_tests_enabled() {
            return;
        }
        let manager = CentralManager::new().expect("central manager");
        let (stream, sender) = BoundedAsyncStream::<EventEnvelope>::new(4);
        let subscription = Subscription::new(
            manager.as_raw(),
            sender,
            crate::ffi::cb_central_manager_stream_subscribe,
            crate::ffi::cb_central_manager_stream_unsubscribe,
        );
        assert!(!subscription.sink.is_null());
        assert!(!stream.is_closed());

        drop(subscription);
        assert!(closes_soon(&stream));
    }

    #[test]
    fn a_subscription_keeps_a_dropped_manager_alive_until_it_unsubscribes() {
        if !live_tests_enabled() {
            return;
        }
        let manager = PeripheralManager::new().expect("peripheral manager");
        let (stream, sender) = BoundedAsyncStream::<EventEnvelope>::new(4);
        let subscription = Subscription::new(
            manager.as_raw(),
            sender,
            crate::ffi::cb_peripheral_manager_stream_subscribe,
            crate::ffi::cb_peripheral_manager_stream_unsubscribe,
        );
        drop(manager);
        std::thread::sleep(Duration::from_millis(20));
        assert!(!stream.is_closed());

        drop(subscription);
        assert!(closes_soon(&stream));
    }

    #[test]
    fn envelopes_release_every_handle_that_no_event_adopted() {
        let uuid =
            BluetoothUuid::from_string("7a4f0a2e-4a0d-4c21-9e53-5d0b1f7a6c11").expect("uuid");
        let service = MutableService::new(&uuid, true).expect("service");
        let before = retain_count(service.raw);
        let handle = || retain_raw(service.raw) as usize;

        let mut unknown = envelope(&format!(
            r#"{{"event":"somethingNew","service_handle":{},"request_handles":[{},{}]}}"#,
            handle(),
            handle(),
            handle()
        ));
        assert_eq!(retain_count(service.raw), before + 3);
        assert!(peripheral_manager_event_from_envelope(&mut unknown).is_none());
        drop(unknown);
        assert_eq!(retain_count(service.raw), before);

        let mut added = envelope(&format!(
            r#"{{"event":"didAddService","service_handle":{},"characteristic_handles":[{}]}}"#,
            handle(),
            handle()
        ));
        let event = peripheral_manager_event_from_envelope(&mut added);
        assert!(matches!(
            event,
            Some(PeripheralManagerEvent::DidAddService { .. })
        ));
        drop(added);
        assert_eq!(retain_count(service.raw), before + 1);
        drop(event);
        assert_eq!(retain_count(service.raw), before);
    }

    #[test]
    fn streams_skip_envelopes_that_are_not_events_and_build_events_where_they_are_polled() {
        let uuid =
            BluetoothUuid::from_string("0c1b6f2d-8e52-4f7a-a1d3-6b2e9c4f5a70").expect("uuid");
        let service = MutableService::new(&uuid, true).expect("service");
        let before = retain_count(service.raw);
        let (stream, sender) = BoundedAsyncStream::<EventEnvelope>::new(8);

        let producer = std::thread::spawn({
            let first = retain_raw(service.raw) as usize;
            let second = retain_raw(service.raw) as usize;
            move || {
                sender.push(envelope(&format!(
                    r#"{{"event":"somethingNew","service_handle":{first}}}"#
                )));
                sender.push(envelope(&format!(
                    r#"{{"event":"didAddService","service_handle":{second}}}"#
                )));
                sender.push(envelope(r#"{"event":"isReadyToUpdateSubscribers"}"#));
            }
        });
        producer.join().expect("producer");

        let added = block_on(next_event(&stream, peripheral_manager_event_from_envelope));
        let Some(PeripheralManagerEvent::DidAddService {
            service: added,
            error,
        }) = added
        else {
            panic!("expected the service event after the skipped envelope");
        };
        assert!(error.is_none());
        assert_eq!(retain_count(service.raw), before + 1);
        drop(added);
        assert_eq!(retain_count(service.raw), before);

        assert!(matches!(
            try_next_event(&stream, peripheral_manager_event_from_envelope),
            Some(PeripheralManagerEvent::IsReadyToUpdateSubscribers)
        ));
        assert!(try_next_event(&stream, peripheral_manager_event_from_envelope).is_none());
        assert!(block_on(next_event(&stream, peripheral_manager_event_from_envelope)).is_none());
    }
}
