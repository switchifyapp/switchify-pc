use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use bluer::{
    adv::Advertisement,
    gatt::{
        local::{
            characteristic_control, Application, Characteristic, CharacteristicControlEvent,
            CharacteristicNotify, CharacteristicNotifyMethod, CharacteristicRead,
            CharacteristicWrite, CharacteristicWriteMethod, ReqError, Service,
        },
        CharacteristicWriter, WriteOp,
    },
    Address,
};
use futures::StreamExt;
use tokio::time::{interval, timeout};
use uuid::Uuid;

use crate::ble_wire::{
    bluetooth_status_payload, create_notification_frames, BluetoothFrame, FrameReassembler,
};

const SERVICE: Uuid = Uuid::from_u128(0x7a78f7e8_1d6d_4d92_9ef0_1f89d3db21f4);
const RX: Uuid = Uuid::from_u128(0x7a78f7e9_1d6d_4d92_9ef0_1f89d3db21f4);
const TX: Uuid = Uuid::from_u128(0x7a78f7ea_1d6d_4d92_9ef0_1f89d3db21f4);
const STATUS: Uuid = Uuid::from_u128(0x7a78f7eb_1d6d_4d92_9ef0_1f89d3db21f4);
// Never derived from an incoming message. Safe even if BlueZ broadcasts it.
const PUBLIC_RESPONSE: &str = r#"{"version":1,"id":"linux-transport-probe","type":"error","ok":false,"error":"linux_transport_probe_only"}"#;
const MAX_WRITE_BYTES: usize = 512;
const RX_IDLE_MS: i64 = 10_000;

#[derive(Default)]
struct Receiver {
    peer: Option<Address>,
    notifications: bool,
    last_write_ms: Option<i64>,
    frames: FrameReassembler,
    accepted: u64,
    completed: u64,
    rejected: u64,
}

impl Receiver {
    fn write(
        &mut self,
        peer: Address,
        bytes: &[u8],
        offset: u16,
        prepared: bool,
        now: i64,
    ) -> Result<(), ReqError> {
        let result = self.accept(peer, bytes, offset, prepared, now);
        if result.is_err() {
            self.rejected = self.rejected.saturating_add(1);
        }
        result
    }

    fn accept(
        &mut self,
        peer: Address,
        bytes: &[u8],
        offset: u16,
        prepared: bool,
        now: i64,
    ) -> Result<(), ReqError> {
        self.expire_rx_owner(now);
        if offset != 0 {
            return Err(ReqError::InvalidOffset);
        }
        if prepared {
            return Err(ReqError::NotSupported);
        }
        if bytes.is_empty() || bytes.len() > MAX_WRITE_BYTES {
            return Err(ReqError::InvalidValueLength);
        }
        if self.peer.is_some_and(|owner| owner != peer) {
            return Err(ReqError::NotAuthorized);
        }
        let frame: BluetoothFrame = serde_json::from_slice(bytes).map_err(|_| ReqError::Failed)?;
        // Validation and ownership changes happen under the same receiver lock.
        let message = match self.frames.accept(frame, now) {
            Ok(message) => message,
            Err(_) => {
                if self.peer.is_none() {
                    self.frames = FrameReassembler::default();
                }
                return Err(ReqError::Failed);
            }
        };
        self.peer = Some(peer);
        self.last_write_ms = Some(now);
        self.accepted = self.accepted.saturating_add(1);
        if message.is_some() {
            self.completed = self.completed.saturating_add(1);
        }
        // Drop message contents: do not execute, echo, log or persist them.
        Ok(())
    }

    fn clear_session(&mut self) {
        self.peer = None;
        self.notifications = false;
        self.last_write_ms = None;
        self.frames = FrameReassembler::default();
    }

    fn expire_rx_owner(&mut self, now: i64) {
        if !self.notifications
            && self
                .last_write_ms
                .is_some_and(|last| now - last >= RX_IDLE_MS)
        {
            self.clear_session();
        }
    }

    fn begin_notifications(&mut self, peer: Address, now: i64) -> bool {
        self.expire_rx_owner(now);
        if self.notifications || self.peer.is_some_and(|owner| owner != peer) {
            return false;
        }
        self.clear_session();
        self.peer = Some(peer);
        self.notifications = true;
        true
    }

    fn replace_notifications(&mut self, peer: Address, now: i64, previous_closed: bool) -> bool {
        // Notify and writer closure may become ready in the same select iteration.
        // Do not require the close branch to have won before accepting a reconnect.
        if previous_closed {
            self.clear_session();
        }
        self.begin_notifications(peer, now)
    }
}

fn read_status(value: &[u8], offset: u16, mtu: u16) -> Result<Vec<u8>, ReqError> {
    let tail = value
        .get(usize::from(offset)..)
        .ok_or(ReqError::InvalidOffset)?;
    if mtu < 23 {
        return Err(ReqError::NotSupported);
    }
    Ok(tail[..tail.len().min(usize::from(mtu) - 1)].to_vec())
}

#[tokio::main(flavor = "current_thread")]
pub async fn run(adapter_name: &str) -> Result<(), &'static str> {
    let session = timeout(Duration::from_secs(10), bluer::Session::new())
        .await
        .map_err(|_| "System D-Bus connection timed out.")?
        .map_err(|_| {
            "Cannot connect to system D-Bus. Run as your normal desktop user with BlueZ installed."
        })?;
    let adapter = session
        .adapter(adapter_name)
        .map_err(|_| "Invalid adapter.")?;
    let powered = timeout(Duration::from_secs(10), adapter.is_powered())
        .await
        .map_err(|_| "BlueZ adapter check timed out.")?
        .map_err(|_| {
            "Cannot query adapter. Check bluetoothd, adapter presence and D-Bus permissions."
        })?;
    if !powered {
        return Err(
            "Adapter is powered off. Enable Bluetooth in desktop settings and check rfkill.",
        );
    }
    let start = Instant::now();
    let received = Arc::new(Mutex::new(Receiver::default()));
    let rx_received = received.clone();
    let status = bluetooth_status_payload(
        "Switchify PC transport probe",
        &format!("probe-{}", Uuid::new_v4()),
        "linux",
    )
    .map_err(|_| "Cannot construct public status.")?;
    let (mut control, control_handle) = characteristic_control();
    let application = Application {
        services: vec![Service {
            uuid: SERVICE,
            primary: true,
            characteristics: vec![
                Characteristic {
                    uuid: RX,
                    write: Some(CharacteristicWrite {
                        write: true,
                        write_without_response: true,
                        method: CharacteristicWriteMethod::Fun(Box::new(move |bytes, request| {
                            let received = rx_received.clone();
                            Box::pin(async move {
                                received.lock().map_err(|_| ReqError::Failed)?.write(
                                    request.device_address,
                                    &bytes,
                                    request.offset,
                                    request.prepare_authorize
                                        || request.op_type == WriteOp::Reliable,
                                    start.elapsed().as_millis() as i64,
                                )
                            })
                        })),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Characteristic {
                    uuid: TX,
                    notify: Some(CharacteristicNotify {
                        notify: true,
                        method: CharacteristicNotifyMethod::Io,
                        ..Default::default()
                    }),
                    control_handle,
                    ..Default::default()
                },
                Characteristic {
                    uuid: STATUS,
                    read: Some(CharacteristicRead {
                        read: true,
                        fun: Box::new(move |request| {
                            let result = read_status(&status, request.offset, request.mtu);
                            Box::pin(async move { result })
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let app_handle = timeout(Duration::from_secs(10), adapter.serve_gatt_application(application)).await
        .map_err(|_| "GATT registration timed out.")?
        .map_err(|_| "GATT registration failed. Check BlueZ GATT server support and ordinary-user D-Bus permissions.")?;
    let advertisement = Advertisement {
        service_uuids: [SERVICE].into_iter().collect(),
        local_name: Some("Switchify probe".into()),
        discoverable: Some(true),
        ..Default::default()
    };
    let adv_handle = timeout(Duration::from_secs(10), adapter.advertise(advertisement)).await
        .map_err(|_| "Advertising registration timed out.")?
        .map_err(|_| "Advertising failed. Check LE peripheral support, rfkill and available advertising instances.")?;
    println!(
        "Probe registered on {adapter_name}; no pairing, credentials or input. Stop with Ctrl-C."
    );
    let mut writer: Option<CharacteristicWriter> = None;
    let mut tick = interval(Duration::from_secs(2));
    let deadline = tokio::time::sleep(Duration::from_secs(300));
    tokio::pin!(deadline);
    let stop = tokio::signal::ctrl_c();
    tokio::pin!(stop);
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|_| "Cannot listen for termination.")?;
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            _ = terminate.recv() => break,
            result = &mut stop => { result.map_err(|_| "Cannot listen for Ctrl-C.")?; break; },
            _ = async {
                match &writer {
                    Some(channel) => { let _ = channel.closed().await; },
                    None => std::future::pending::<()>().await,
                }
            } => {
                writer = None;
                received.lock().map_err(|_| "Receiver lock failed.")?.clear_session();
                println!("Notification session ended; reassembly cleared.");
            },
            event = control.next() => {
                let Some(CharacteristicControlEvent::Notify(candidate)) = event else { break; };
                let mut receiver = received.lock().map_err(|_| "Receiver lock failed.")?;
                let previous_closed = writer.as_ref().is_some_and(|channel| channel.is_closed().unwrap_or(true));
                if !receiver.replace_notifications(candidate.device_address(), start.elapsed().as_millis() as i64, previous_closed) {
                    println!("Competing notification session refused (not a security qualification).");
                    drop(candidate);
                    continue;
                }
                println!("Notification channel opened; payload limit {} bytes.", candidate.mtu());
                writer = Some(candidate);
            },
            _ = tick.tick() => {
                let mut failed = false;
                if let Some(channel) = &writer {
                    match create_notification_frames(PUBLIC_RESPONSE, channel.mtu()) {
                        Ok(frames) => {
                            for frame in frames {
                                if !matches!(timeout(Duration::from_secs(2), channel.send(&frame)).await, Ok(Ok(()))) {
                                    println!("Notification channel closed or backpressured; session cleared.");
                                    failed = true;
                                    break;
                                }
                            }
                        },
                        Err(_) => { println!("MTU too small for protocol v1 framing; session cleared."); failed = true; }
                    }
                }
                let mut receiver = received.lock().map_err(|_| "Receiver lock failed.")?;
                if failed { writer = None; receiver.clear_session(); }
                receiver.expire_rx_owner(start.elapsed().as_millis() as i64);
                receiver.frames.clear_expired(start.elapsed().as_millis() as i64);
                println!("RX totals: accepted={} complete={} rejected={}", receiver.accepted, receiver.completed, receiver.rejected);
            }
        }
    }
    drop(writer);
    received
        .lock()
        .map_err(|_| "Receiver lock failed.")?
        .clear_session();
    // Drop only registrations owned by this process; never unpair remote devices.
    drop(adv_handle);
    drop(app_handle);
    tokio::time::sleep(Duration::from_millis(250)).await;
    println!("Probe stopped.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ble_wire::{create_frames, PARTIAL_TIMEOUT_MS};

    fn peer(last: u8) -> Address {
        Address::new([0, 0, 0, 0, 0, last])
    }

    #[test]
    fn replacement_notification_clears_closed_owner_before_admission() {
        let mut receiver = Receiver::default();
        assert!(receiver.begin_notifications(peer(1), 0));
        let frames = create_frames(&"x".repeat(200)).unwrap();
        receiver.write(peer(1), &frames[0], 0, false, 1).unwrap();
        // A live writer still excludes competitors.
        assert!(!receiver.replace_notifications(peer(2), 2, false));
        // Simulate Notify winning selection while the previous writer is closed.
        assert!(receiver.replace_notifications(peer(2), 2, true));
        receiver.write(peer(2), &frames[1], 0, false, 3).unwrap();
        assert_eq!(receiver.completed, 0);
        assert_eq!(receiver.peer, Some(peer(2)));
        // Same-peer reconnects also succeed without a second subscription.
        assert!(receiver.replace_notifications(peer(2), 4, true));
    }

    #[test]
    fn rejected_first_frame_does_not_reserve_rx_or_notifications() {
        let frame = create_frames("test").unwrap().remove(0);
        let mut invalid: BluetoothFrame = serde_json::from_slice(&frame).unwrap();
        invalid.version = 2;
        let mut receiver = Receiver::default();
        assert!(receiver
            .write(peer(1), &serde_json::to_vec(&invalid).unwrap(), 0, false, 0)
            .is_err());
        assert_eq!(receiver.peer, None);
        receiver.write(peer(2), &frame, 0, false, 1).unwrap();
        assert!(receiver.begin_notifications(peer(2), 1));
    }

    #[test]
    fn rx_only_ownership_expires_without_combining_stale_fragments() {
        let frames = create_frames(&"x".repeat(200)).unwrap();
        let mut receiver = Receiver::default();
        receiver.write(peer(1), &frames[0], 0, false, 0).unwrap();
        assert!(!receiver.begin_notifications(peer(2), RX_IDLE_MS - 1));
        receiver
            .write(peer(2), &frames[1], 0, false, RX_IDLE_MS)
            .unwrap();
        assert_eq!(receiver.completed, 0);
        assert_eq!(receiver.peer, Some(peer(2)));
        assert!(receiver.begin_notifications(peer(3), RX_IDLE_MS * 2));
    }

    #[test]
    fn active_notification_owner_is_not_expired_by_rx_inactivity() {
        let mut receiver = Receiver::default();
        assert!(receiver.begin_notifications(peer(1), 0));
        let frame = create_frames("test").unwrap().remove(0);
        receiver.write(peer(1), &frame, 0, false, 1).unwrap();
        receiver.expire_rx_owner(RX_IDLE_MS * 2);
        assert!(!receiver.begin_notifications(peer(2), RX_IDLE_MS * 2));
        assert_eq!(
            receiver.write(peer(2), &frame, 0, false, RX_IDLE_MS * 2),
            Err(ReqError::NotAuthorized)
        );
        receiver.clear_session();
        assert!(receiver.begin_notifications(peer(2), RX_IDLE_MS * 2));
    }

    #[test]
    fn fragmented_messages_are_counted_without_retaining_contents() {
        let mut receiver = Receiver::default();
        for frame in create_frames(&"private text".repeat(80)).unwrap() {
            receiver.write(peer(1), &frame, 0, false, 0).unwrap();
        }
        assert_eq!(receiver.completed, 1);
        assert!(receiver.accepted > 1);
    }

    #[test]
    fn competing_peer_cannot_complete_another_peers_message() {
        let frames = create_frames(&"x".repeat(200)).unwrap();
        let mut receiver = Receiver::default();
        receiver.write(peer(1), &frames[0], 0, false, 0).unwrap();
        assert_eq!(
            receiver.write(peer(2), &frames[1], 0, false, 1),
            Err(ReqError::NotAuthorized)
        );
        assert_eq!(receiver.completed, 0);
        receiver.clear_session();
        receiver.write(peer(2), &frames[1], 0, false, 2).unwrap();
        assert_eq!(receiver.completed, 0);
    }

    #[test]
    fn rejects_offsets_prepared_writes_and_oversized_values() {
        let mut receiver = Receiver::default();
        let frame = create_frames("hello").unwrap().remove(0);
        assert_eq!(
            receiver.write(peer(1), &frame, 1, false, 0),
            Err(ReqError::InvalidOffset)
        );
        assert_eq!(
            receiver.write(peer(1), &frame, 0, true, 0),
            Err(ReqError::NotSupported)
        );
        assert_eq!(
            receiver.write(peer(1), &[0; 513], 0, false, 0),
            Err(ReqError::InvalidValueLength)
        );
        assert!(receiver.write(peer(1), b"not json", 0, false, 0).is_err());
        assert_eq!(receiver.rejected, 4);
        assert_eq!(receiver.accepted, 0);
    }

    #[test]
    fn stale_partial_messages_are_not_completed() {
        let frames = create_frames(&"x".repeat(200)).unwrap();
        let mut receiver = Receiver::default();
        receiver.write(peer(1), &frames[0], 0, false, 0).unwrap();
        receiver
            .write(peer(1), &frames[1], 0, false, PARTIAL_TIMEOUT_MS)
            .unwrap();
        assert_eq!(receiver.completed, 0);
    }

    #[test]
    fn status_supports_offsets_and_att_read_limits() {
        let value = bluetooth_status_payload("probe", "probe-id", "linux").unwrap();
        let mut reconstructed = Vec::new();
        while reconstructed.len() < value.len() {
            reconstructed.extend(read_status(&value, reconstructed.len() as u16, 23).unwrap());
        }
        assert_eq!(reconstructed, value);
        assert_eq!(
            read_status(&value, value.len() as u16 + 1, 23),
            Err(ReqError::InvalidOffset)
        );
        assert!(read_status(&value, value.len() as u16, 23)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn notifications_use_shared_framing_and_only_public_data() {
        assert!(create_notification_frames(PUBLIC_RESPONSE, 20).is_err());
        for limit in [180, 244, 512] {
            let mut reassembler = FrameReassembler::default();
            let mut complete = None;
            for bytes in create_notification_frames(PUBLIC_RESPONSE, limit).unwrap() {
                assert!(bytes.len() <= limit);
                complete = reassembler
                    .accept(serde_json::from_slice(&bytes).unwrap(), 0)
                    .unwrap();
            }
            assert_eq!(complete.as_deref(), Some(PUBLIC_RESPONSE));
        }
    }
}
