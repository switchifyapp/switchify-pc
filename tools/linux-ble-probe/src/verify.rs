//! Opt-in cooperative test client. Never pairs, handles tokens or injects input.
use std::time::{Duration, Instant};

use bluer::{gatt::remote::Characteristic, Address, Device};
use futures::{pin_mut, StreamExt};
use tokio::time::timeout;

use crate::{
    ble_wire::{create_notification_frames, BluetoothFrame, FrameReassembler},
    probe::{PUBLIC_RECEIPT, PUBLIC_RESPONSE, RX, SERVICE, STATUS, TX},
};

type Result<T> = std::result::Result<T, &'static str>;

fn is_probe_status(bytes: &[u8]) -> bool {
    if bytes.len() > 1024 {
        return false;
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return false;
    };
    value["protocolVersion"] == 1
        && value["platform"] == "linux"
        && value["displayName"] == "Switchify PC transport probe"
        && value["desktopId"].as_str().is_some_and(|id| {
            id.strip_prefix("probe-")
                .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
        })
}

fn accept_public_frame(frames: &mut FrameReassembler, bytes: &[u8], now: i64) -> Result<bool> {
    if bytes.len() > 512 {
        return Err("Notification exceeds probe bounds.");
    }
    let frame: BluetoothFrame =
        serde_json::from_slice(bytes).map_err(|_| "Invalid notification frame.")?;
    match frames
        .accept(frame, now)
        .map_err(|_| "Invalid notification framing.")?
    {
        None => Ok(false),
        Some(message) if message == PUBLIC_RESPONSE => Ok(true),
        Some(_) => Err("Unexpected notification; no receipt sent."),
    }
}

async fn characteristics(
    device: &Device,
) -> Result<(Characteristic, Characteristic, Characteristic)> {
    for service in device
        .services()
        .await
        .map_err(|_| "Service discovery failed.")?
    {
        if service
            .uuid()
            .await
            .map_err(|_| "Cannot read service UUID.")?
            != SERVICE
        {
            continue;
        }
        let (mut rx, mut tx, mut status) = (None, None, None);
        for characteristic in service
            .characteristics()
            .await
            .map_err(|_| "Characteristic discovery failed.")?
        {
            match characteristic
                .uuid()
                .await
                .map_err(|_| "Cannot read characteristic UUID.")?
            {
                RX => rx = Some(characteristic),
                TX => tx = Some(characteristic),
                STATUS => status = Some(characteristic),
                _ => (),
            }
        }
        return match (rx, tx, status) {
            (Some(rx), Some(tx), Some(status)) => Ok((rx, tx, status)),
            _ => Err("Probe characteristics missing."),
        };
    }
    Err("Probe service missing.")
}

async fn exchange(device: &Device) -> Result<()> {
    device
        .connect()
        .await
        .map_err(|_| "Probe connection failed.")?;
    let (rx, tx, status) = characteristics(device).await?;
    if !is_probe_status(&status.read().await.map_err(|_| "Status read failed.")?) {
        return Err("Target is not a recognized test probe; no writes sent.");
    }
    let notifications = tx
        .notify()
        .await
        .map_err(|_| "Notification subscription failed.")?;
    pin_mut!(notifications);
    let mut frames = FrameReassembler::default();
    let started = Instant::now();
    while let Some(bytes) = notifications.next().await {
        if !accept_public_frame(&mut frames, &bytes, started.elapsed().as_millis() as i64)? {
            continue;
        }
        println!("Fixed public notification received and reassembled.");
        // BlueR reports its conservative payload limit, already adjusted for overhead.
        let limit = rx
            .mtu()
            .await
            .map_err(|_| "Cannot read payload limit.")?
            .min(512);
        for frame in create_notification_frames(PUBLIC_RECEIPT, limit)
            .map_err(|_| "MTU too small for receipt framing.")?
        {
            rx.write(&frame)
                .await
                .map_err(|_| "Receipt write failed.")?;
        }
        println!("Fixed public receipt written. Check server public_receipts counter; this is not peer-isolation proof.");
        return Ok(());
    }
    Err("Notification session ended before the public response.")
}

#[tokio::main(flavor = "current_thread")]
pub async fn run(adapter_name: &str, address: Address) -> Result<()> {
    let setup = async {
        let session = bluer::Session::new()
            .await
            .map_err(|_| "System D-Bus unavailable.")?;
        let adapter = session
            .adapter(adapter_name)
            .map_err(|_| "Adapter unavailable.")?;
        if !adapter
            .is_powered()
            .await
            .map_err(|_| "Cannot query adapter power.")?
        {
            return Err("Adapter must already be powered.");
        }
        let device = adapter
            .device(address)
            .map_err(|_| "Target unavailable; discover the probe first.")?;
        if device
            .is_connected()
            .await
            .map_err(|_| "Cannot query target connection.")?
        {
            return Err("Target is already connected; refusing to disturb an existing session.");
        }
        Ok((session, device))
    };
    let (_session, device) = timeout(Duration::from_secs(5), setup)
        .await
        .map_err(|_| "Verifier setup timed out.")??;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|_| "Cannot install termination handler.")?;
    let result = tokio::select! {
        result = timeout(Duration::from_secs(25), exchange(&device)) => result.unwrap_or(Err("Probe verification timed out.")),
        _ = tokio::signal::ctrl_c() => Err("Probe verification cancelled."),
        _ = terminate.recv() => Err("Probe verification cancelled."),
    };
    // Disconnect also cancels an outstanding Connect request. Never remove pairings.
    if !matches!(
        timeout(Duration::from_secs(5), device.disconnect()).await,
        Ok(Ok(()))
    ) {
        return Err("Verifier cleanup was not confirmed; check the selected test connection.");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_probe_status_is_accepted() {
        let status = crate::ble_wire::bluetooth_status_payload(
            "Switchify PC transport probe",
            &format!("probe-{}", uuid::Uuid::nil()),
            "linux",
        )
        .unwrap();
        assert!(is_probe_status(&status));
        for value in [b"not JSON".as_slice(), b"{}", b"null", &[0; 1025]] {
            assert!(!is_probe_status(value));
        }
        assert!(!is_probe_status(
            &crate::ble_wire::bluetooth_status_payload("Real PC", "saved-id", "linux").unwrap()
        ));
    }
    #[test]
    fn public_response_reassembles_and_other_content_is_never_accepted() {
        for value in [PUBLIC_RESPONSE, "private unexpected payload"] {
            let mut receiver = FrameReassembler::default();
            let frames = create_notification_frames(value, 180).unwrap();
            for (index, frame) in frames.iter().enumerate() {
                let result = accept_public_frame(&mut receiver, frame, index as i64);
                if index + 1 < frames.len() {
                    assert_eq!(result, Ok(false));
                } else if value == PUBLIC_RESPONSE {
                    assert_eq!(result, Ok(true));
                } else {
                    assert_eq!(result, Err("Unexpected notification; no receipt sent."));
                }
            }
        }
    }
    #[test]
    fn invalid_frames_and_small_mtu_fail_closed() {
        assert!(accept_public_frame(&mut FrameReassembler::default(), b"private", 0).is_err());
        assert!(accept_public_frame(&mut FrameReassembler::default(), &[0; 513], 0).is_err());
        assert!(create_notification_frames(PUBLIC_RECEIPT, 20).is_err());
    }
}
