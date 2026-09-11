//! Explicitly opted-in X11 development runtime. No notification carries replies.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use std::time::{Duration, Instant};

use bluer::{
    adv::Advertisement,
    gatt::{
        local::{
            Application, Characteristic, CharacteristicRead, CharacteristicWrite,
            CharacteristicWriteMethod, ReqError, Service,
        },
        WriteOp,
    },
    Address, DeviceEvent, DeviceProperty,
};
use enigo::{Enigo, Mouse, Settings};
use futures::StreamExt;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use uuid::Uuid;

use crate::input::{DesktopInput, InputInjector};
use crate::linux_credential_worker::CredentialWorker;
use crate::linux_read_responses::{ReadError, ReadResponses, RESPONSE_TRANSPORT, RESPONSE_UUID};
use crate::protocol::{
    bluetooth_status_payload, pointer_profile_response, EngineEvent, PointerProfile,
};
use crate::state::{
    emit_state, now_ms, set_activity, AccessibilityState, ActivityKind, AppModel, AppState,
    BluetoothState, PairedDeviceView, SharedModel,
};
use crate::storage::AppStorage;

const SERVICE: Uuid = Uuid::from_u128(0x7a78f7e8_1d6d_4d92_9ef0_1f89d3db21f4);
const RX: Uuid = Uuid::from_u128(0x7a78f7e9_1d6d_4d92_9ef0_1f89d3db21f4);
const STATUS: Uuid = Uuid::from_u128(0x7a78f7eb_1d6d_4d92_9ef0_1f89d3db21f4);
const ERROR: &str = "Linux Bluetooth session stopped. Check Bluetooth and the X11 session, then restart Switchify PC.";
const ALLOWED: &[&str] = &[
    "mouse.move",
    "mouse.click",
    "mouse.doubleClick",
    "mouse.rightClick",
    "mouse.scroll",
    "mouse.dragStart",
    "mouse.dragEnd",
    "keyboard.key",
    "keyboard.modifierDown",
    "keyboard.modifierUp",
    "keyboard.shortcut",
    "keyboard.typeText",
    "keyboard.textStream.open",
    "keyboard.textStream.char",
    "keyboard.textStream.chunk",
    "keyboard.textStream.key",
    "keyboard.textStream.close",
    "media.control",
    "connection.disconnecting",
    "connection.ping",
    "pointer.profile",
];

#[derive(Default)]
struct Gate {
    peer: Option<Address>,
    generation: u64,
    responses: ReadResponses,
    touched: Option<Instant>,
}
impl Gate {
    fn close(&mut self) {
        self.responses.close();
        self.peer = None;
        self.generation = self.generation.wrapping_add(1);
        self.touched = None;
    }
    fn claim(&mut self, peer: Address) -> Result<u64, ReqError> {
        if self.peer.is_some_and(|owner| owner != peer) {
            return Err(ReqError::NotAuthorized);
        }
        self.responses
            .open(peer.0)
            .map_err(|_| ReqError::NotAuthorized)?;
        self.peer = Some(peer);
        self.touched = Some(Instant::now());
        Ok(self.generation)
    }
    fn enqueue(&mut self, generation: u64, message: &str) -> Result<(), ()> {
        if generation != self.generation {
            return Err(());
        }
        let owner = self.peer.ok_or(())?;
        let mailbox_generation = self.responses.open(owner.0).map_err(|_| ())?;
        self.responses
            .enqueue(mailbox_generation, message)
            .map_err(|_| ())
    }
}

enum Command {
    Frame {
        peer: Address,
        generation: u64,
        bytes: Vec<u8>,
        done: oneshot::Sender<Result<(), ReqError>>,
    },
    Approve {
        id: String,
        generation: u64,
        done: oneshot::Sender<Result<(), String>>,
    },
    Reject {
        id: String,
        generation: u64,
    },
    Forget {
        id: String,
        done: oneshot::Sender<Result<(), String>>,
    },
}
struct Handle {
    sender: mpsc::Sender<Command>,
    gate: Arc<Mutex<Gate>>,
    stop: Arc<AtomicBool>,
    finished: std::sync::mpsc::Receiver<()>,
}
static RUNTIME: OnceLock<Mutex<Option<Handle>>> = OnceLock::new();
fn runtime() -> &'static Mutex<Option<Handle>> {
    RUNTIME.get_or_init(|| Mutex::new(None))
}

pub fn requested() -> bool {
    std::env::var("SWITCHIFY_LINUX_EXPERIMENTAL").as_deref() == Ok("1")
}

pub fn install(app: AppHandle, shared: SharedModel) -> Result<(), String> {
    if std::env::var("XDG_SESSION_TYPE").as_deref() != Ok("x11")
        || std::env::var_os("DISPLAY").is_none()
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
    {
        return Err(
            "This experimental build requires a local X11 desktop session, not Wayland.".into(),
        );
    }
    let adapter = std::env::var("SWITCHIFY_LINUX_ADAPTER").unwrap_or_else(|_| "hci0".into());
    if !adapter.strip_prefix("hci").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.len() <= 4 && suffix.bytes().all(|b| b.is_ascii_digit())
    }) {
        return Err("Select a valid hciN Bluetooth adapter.".into());
    }
    let mut owner = runtime().lock().map_err(|_| ERROR)?;
    if owner.is_some() {
        return Err("Linux runtime has already started. Restart the application.".into());
    }
    let (sender, receiver) = mpsc::channel(64);
    let gate = Arc::new(Mutex::new(Gate::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let (finished_tx, finished) = std::sync::mpsc::channel();
    let thread_gate = gate.clone();
    let thread_stop = stop.clone();
    let thread_sender = sender.clone();
    std::thread::Builder::new()
        .name("linux-runtime".into())
        .spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| ())
                .and_then(|rt| {
                    rt.block_on(run(
                        &app,
                        &shared,
                        &adapter,
                        receiver,
                        thread_sender,
                        thread_gate.clone(),
                        thread_stop,
                    ))
                });
            thread_gate.lock().unwrap().close();
            {
                let mut data = shared.lock().unwrap();
                data.engine.reset_transport_session();
                data.state.pending_pairings.clear();
                data.state.connected_device_name = None;
                data.state.bluetooth = BluetoothState::Error;
                data.state.accessibility = AccessibilityState::Unavailable;
            }
            if result.is_err() {
                set_activity(&shared, ActivityKind::Error, ERROR);
            }
            emit_state(&app, &shared);
            let _ = finished_tx.send(());
        })
        .map_err(|_| ERROR)?;
    *owner = Some(Handle {
        sender,
        gate,
        stop,
        finished,
    });
    Ok(())
}

pub fn disconnect() {
    if let Ok(owner) = runtime().lock() {
        if let Some(handle) = owner.as_ref() {
            handle.gate.lock().unwrap().close();
        }
    }
}
pub fn shutdown() {
    if let Ok(owner) = runtime().lock() {
        if let Some(handle) = owner.as_ref() {
            handle.gate.lock().unwrap().close();
            handle.stop.store(true, Ordering::SeqCst);
            let _ = handle.finished.recv_timeout(Duration::from_secs(1));
        }
    }
}
pub async fn approve(app: &AppHandle, id: String) -> Result<AppState, String> {
    let (done, result) = oneshot::channel();
    {
        let owner = runtime().lock().map_err(|_| ERROR)?;
        let handle = owner.as_ref().ok_or(ERROR)?;
        let generation = handle.gate.lock().unwrap().generation;
        handle
            .sender
            .try_send(Command::Approve {
                id,
                generation,
                done,
            })
            .map_err(|_| ERROR)?;
    }
    timeout(Duration::from_secs(5), result)
        .await
        .map_err(|_| ERROR)?
        .map_err(|_| ERROR)??;
    Ok(app.state::<AppModel>().snapshot())
}
pub fn reject(id: &str) -> Result<(), String> {
    let owner = runtime().lock().map_err(|_| ERROR)?;
    let handle = owner.as_ref().ok_or(ERROR)?;
    let generation = handle.gate.lock().unwrap().generation;
    handle
        .sender
        .try_send(Command::Reject {
            id: id.into(),
            generation,
        })
        .map_err(|_| ERROR.into())
}

pub async fn forget(app: &AppHandle, id: String) -> Result<AppState, String> {
    if id.is_empty() || id.len() > 128 {
        return Err("Invalid device identifier.".into());
    }
    let (done, result) = oneshot::channel();
    {
        let owner = runtime().lock().map_err(|_| ERROR)?;
        let handle = owner.as_ref().ok_or(ERROR)?;
        handle.gate.lock().unwrap().close();
        handle
            .sender
            .try_send(Command::Forget { id, done })
            .map_err(|_| ERROR)?;
    }
    timeout(Duration::from_secs(5), result)
        .await
        .map_err(|_| ERROR)?
        .map_err(|_| ERROR)??;
    Ok(app.state::<AppModel>().snapshot())
}

fn cleanup<I: InputInjector>(input: &mut DesktopInput<I>, shared: &SharedModel) -> Result<(), ()> {
    let released = input.release_all().map_err(|_| ());
    let mut data = shared.lock().unwrap();
    data.engine.reset_transport_session();
    data.state.pending_pairings.clear();
    data.state.connected_device_name = None;
    data.state.bluetooth = BluetoothState::Advertising;
    released
}

async fn run(
    app: &AppHandle,
    shared: &SharedModel,
    adapter_name: &str,
    mut receiver: mpsc::Receiver<Command>,
    sender: mpsc::Sender<Command>,
    gate: Arc<Mutex<Gate>>,
    stop: Arc<AtomicBool>,
) -> Result<(), ()> {
    let session = timeout(Duration::from_secs(5), bluer::Session::new())
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    let adapter = session.adapter(adapter_name).map_err(|_| ())?;
    let mut adapter_events = timeout(Duration::from_secs(3), adapter.events())
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    if !timeout(Duration::from_secs(3), adapter.is_powered())
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?
    {
        return Err(());
    }
    let enigo = Enigo::new(&Settings::default()).map_err(|_| ())?;
    let (width, height) = enigo.main_display().map_err(|_| ())?;
    let mut input = DesktopInput::new(enigo);
    let mut credentials =
        CredentialWorker::start(Arc::new(AppStorage::new()), Duration::from_secs(3))
            .map_err(|_| ())?;
    let mut status: Value = serde_json::from_slice(
        &bluetooth_status_payload(
            "Switchify PC",
            &shared.lock().unwrap().state.desktop_id,
            "linux",
        )
        .map_err(|_| ())?,
    )
    .map_err(|_| ())?;
    status["responseTransport"] = json!(RESPONSE_TRANSPORT);
    let status = serde_json::to_vec(&status).map_err(|_| ())?;
    let read_gate = gate.clone();
    let rx_gate = gate.clone();
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
                            let sender = sender.clone();
                            let gate = rx_gate.clone();
                            Box::pin(async move {
                                if request.offset != 0 {
                                    return Err(ReqError::InvalidOffset);
                                }
                                if request.prepare_authorize || request.op_type == WriteOp::Reliable
                                {
                                    return Err(ReqError::NotSupported);
                                }
                                if bytes.is_empty() || bytes.len() > 512 {
                                    return Err(ReqError::InvalidValueLength);
                                }
                                let generation = gate
                                    .lock()
                                    .map_err(|_| ReqError::Failed)?
                                    .claim(request.device_address)?;
                                let (done, result) = oneshot::channel();
                                if sender
                                    .try_send(Command::Frame {
                                        peer: request.device_address,
                                        generation,
                                        bytes,
                                        done,
                                    })
                                    .is_err()
                                {
                                    gate.lock().map_err(|_| ReqError::Failed)?.close();
                                    return Err(ReqError::Failed);
                                }
                                match timeout(Duration::from_secs(4), result).await {
                                    Ok(Ok(result)) => result,
                                    _ => {
                                        gate.lock().map_err(|_| ReqError::Failed)?.close();
                                        Err(ReqError::Failed)
                                    }
                                }
                            })
                        })),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Characteristic {
                    uuid: Uuid::parse_str(RESPONSE_UUID).map_err(|_| ())?,
                    read: Some(CharacteristicRead {
                        read: true,
                        fun: Box::new(move |request| {
                            let result = read_gate.lock().map_err(|_| ReqError::Failed).and_then(
                                |mut gate| {
                                    let value = gate
                                        .responses
                                        .read(request.device_address.0, request.offset, request.mtu)
                                        .map_err(|error| match error {
                                            ReadError::Unauthorized => ReqError::NotAuthorized,
                                            ReadError::InvalidOffset => ReqError::InvalidOffset,
                                            _ => ReqError::Failed,
                                        })?;
                                    if gate.peer == Some(request.device_address) {
                                        gate.touched = Some(Instant::now());
                                    }
                                    Ok(value)
                                },
                            );
                            Box::pin(async move { result })
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                Characteristic {
                    uuid: STATUS,
                    read: Some(CharacteristicRead {
                        read: true,
                        fun: Box::new(move |request| {
                            let result = if request.mtu < 23 {
                                Err(ReqError::NotSupported)
                            } else {
                                status
                                    .get(usize::from(request.offset)..)
                                    .map(|tail| {
                                        tail[..tail.len().min(usize::from(request.mtu) - 1)]
                                            .to_vec()
                                    })
                                    .ok_or(ReqError::InvalidOffset)
                            };
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
    let service = timeout(
        Duration::from_secs(5),
        adapter.serve_gatt_application(application),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    let advertisement = timeout(
        Duration::from_secs(5),
        adapter.advertise(Advertisement {
            service_uuids: [SERVICE].into_iter().collect(),
            local_name: Some("Switchify PC Linux".into()),
            discoverable: Some(true),
            ..Default::default()
        }),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    {
        let mut data = shared.lock().unwrap();
        data.state.bluetooth = BluetoothState::Advertising;
        data.state.accessibility = AccessibilityState::Granted;
    }
    set_activity(
        shared,
        ActivityKind::Info,
        "Experimental X11 control is ready. Use a Remote with Linux read-response support.",
    );
    emit_state(app, shared);
    let profile = PointerProfile {
        display_id: "x11-primary".into(),
        scale_factor: 1.0,
        x: 0,
        y: 0,
        width: width as u32,
        height: height as u32,
        small_delta: 5,
        medium_delta: 20,
        large_delta: 60,
        display_navigation_supported: false,
        display_count: 1,
    };
    let mut active: Option<(Address, u64)> = None;
    let mut watch: Option<tokio::task::JoinHandle<()>> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(25));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_tick = now_ms();
    let result = async {
        loop {
            if stop.load(Ordering::SeqCst) { break; }
            tokio::select! {
                event = adapter_events.next() => {
                    match event {
                        None | Some(bluer::AdapterEvent::PropertyChanged(bluer::AdapterProperty::Powered(false))) => return Err(()),
                        Some(bluer::AdapterEvent::DeviceRemoved(peer)) => {
                            let mut gate = gate.lock().unwrap();
                            if gate.peer == Some(peer) { gate.close(); }
                        },
                        _ => (),
                    }
                },
                _ = tick.tick() => {
                    let now = now_ms();
                    let mut gate = gate.lock().unwrap();
                    if now.saturating_sub(last_tick) > 3000 || gate.touched.is_some_and(|last| last.elapsed() > Duration::from_secs(2)) { gate.close(); }
                    last_tick = now;
                    let generation = gate.generation;
                    let mut data = shared.lock().unwrap();
                    let expired: Vec<_> = data.engine.pending_pairings().into_iter().filter(|pending| now >= pending.expires_at).map(|pending| pending.request_id).collect();
                    let changed = !expired.is_empty();
                    for id in expired {
                        if let Some(response) = data.engine.expire_pairing(&id, now) {
                            if gate.enqueue(generation, &response).is_err() { gate.close(); }
                        }
                    }
                    data.state.pending_pairings = data.engine.pending_pairings();
                    drop(data); drop(gate);
                    if changed { emit_state(app, shared); }
                },
                command = receiver.recv() => {
                    let Some(command) = command else { break; };
                    match command {
                        Command::Frame { peer, generation, bytes, done } => {
                            if gate.lock().unwrap().generation != generation { let _ = done.send(Err(ReqError::NotAuthorized)); continue; }
                            if active != Some((peer, generation)) {
                                cleanup(&mut input, shared)?;
                                if let Some(watch) = watch.take() { watch.abort(); }
                                let device = adapter.device(peer).map_err(|_| ())?;
                                let mut events = timeout(Duration::from_secs(2), device.events()).await.map_err(|_| ())?.map_err(|_| ())?;
                                if !timeout(Duration::from_secs(2), device.is_connected()).await.map_err(|_| ())?.map_err(|_| ())? { return Err(()); }
                                let watch_gate = gate.clone();
                                watch = Some(tokio::spawn(async move {
                                    while let Some(event) = events.next().await {
                                        if matches!(event, DeviceEvent::PropertyChanged(DeviceProperty::Connected(false))) { break; }
                                    }
                                    let mut gate = watch_gate.lock().unwrap();
                                    if gate.generation == generation { gate.close(); }
                                }));
                                active = Some((peer, generation));
                            }
                            // The watcher can invalidate during asynchronous setup.
                            let mut locked_gate = gate.lock().unwrap();
                            if locked_gate.generation != generation { let _ = done.send(Err(ReqError::NotAuthorized)); continue; }
                            if now_ms().saturating_sub(last_tick) > 3000 { locked_gate.close(); let _ = done.send(Err(ReqError::Failed)); continue; }
                            let response = process_frame(&mut input, shared, &profile, &bytes, |connection| {
                                app.state::<AppModel>().record_authenticated_connection(&connection.device_id, connection.device_name.as_deref(), connection.connected_at, connection.received_order).unwrap_or(false)
                            });
                            let outcome = match response {
                                Ok(Some(response)) => locked_gate.enqueue(generation, &response),
                                Ok(None) => Ok(()),
                                Err(()) => Err(()),
                            };
                            if outcome.is_err() { locked_gate.close(); }
                            let _ = done.send(outcome.map_err(|_| ReqError::Failed));
                        },
                        Command::Approve { id, generation, done } => {
                            // No held input may remain while a native credential call is pending.
                            input.release_all().map_err(|_| ())?;
                            let outcome = if gate.lock().unwrap().generation != generation || done.is_closed() { Err(ERROR.into()) } else {
                                approve_inner(app, shared, &gate, &credentials, &id, || done.is_closed()).await
                            };
                            if outcome.is_err() { gate.lock().unwrap().close(); }
                            let _ = done.send(outcome);
                        },
                        Command::Reject { id, generation } => {
                            if gate.lock().unwrap().generation != generation { continue; }
                            let response = shared.lock().unwrap().engine.reject_pairing(&id);
                            if let Ok(response) = response {
                                let generation = gate.lock().unwrap().generation;
                                if gate.lock().unwrap().enqueue(generation, &response).is_err() { gate.lock().unwrap().close(); }
                            }
                        },
                        Command::Forget { id, done } => {
                            cleanup(&mut input, shared)?;
                            shared.lock().unwrap().engine.forget_device(&id);
                            let outcome = match credentials.delete(&id).await {
                                Ok(()) => {
                                    shared.lock().unwrap().state.paired_devices.retain(|device| device.device_id != id);
                                    app.state::<AppModel>().persist()
                                },
                                Err(_) => Err("Credential deletion failed. Unlock the desktop keyring and restart.".into()),
                            };
                            let _ = done.send(outcome);
                        },
                    }
                    let mut data = shared.lock().unwrap();
                    data.state.pending_pairings = data.engine.pending_pairings();
                    drop(data);
                    emit_state(app, shared);
                }
            }
            if active.is_some_and(|(_, generation)| generation != gate.lock().unwrap().generation) {
                credentials.invalidate();
                cleanup(&mut input, shared)?;
                if let Some(watch) = watch.take() { watch.abort(); }
                if let Some((peer, _)) = active.take() { let _ = timeout(Duration::from_secs(1), adapter.device(peer).map_err(|_| ())?.disconnect()).await; }
                emit_state(app, shared);
            }
        }
        Ok(())
    }.await;
    gate.lock().unwrap().close();
    if let Some(watch) = watch {
        watch.abort();
    }
    credentials.shutdown();
    let cleanup_result = cleanup(&mut input, shared);
    drop(advertisement);
    drop(service);
    result.and(cleanup_result)
}

async fn approve_inner(
    app: &AppHandle,
    shared: &SharedModel,
    gate: &Arc<Mutex<Gate>>,
    credentials: &CredentialWorker,
    id: &str,
    cancelled: impl Fn() -> bool,
) -> Result<(), String> {
    let generation = {
        let gate = gate.lock().unwrap();
        gate.peer.ok_or(ERROR)?;
        gate.generation
    };
    let (pending, approval) = {
        let mut data = shared.lock().unwrap();
        let pending = data
            .engine
            .pending_pairings()
            .into_iter()
            .find(|pending| pending.request_id == id)
            .ok_or(ERROR)?;
        let approval = data.engine.prepare_pairing(id, now_ms())?;
        // Replacement writes can fail after committing. Never keep an old token
        // active while persistence has an uncertain outcome.
        data.engine.forget_device(&pending.device_id);
        (pending, approval)
    };
    credentials
        .save(&approval.device_id, &approval.token)
        .await
        .map_err(|_| {
            "Credential storage failed. Unlock the desktop keyring and restart Switchify PC."
        })?;
    if cancelled() || gate.lock().unwrap().generation != generation {
        return Err(ERROR.into());
    }
    let previous = {
        let mut data = shared.lock().unwrap();
        let previous = data.state.paired_devices.clone();
        data.state
            .paired_devices
            .retain(|device| device.device_id != pending.device_id);
        data.state.paired_devices.push(PairedDeviceView {
            device_id: pending.device_id,
            device_name: pending.device_name,
            paired_at: now_ms(),
            last_seen_at: None,
        });
        previous
    };
    if app.state::<AppModel>().persist().is_err() {
        shared.lock().unwrap().state.paired_devices = previous;
        return Err("Pairing metadata could not be saved. Restart and pair again.".into());
    }
    let mut gate = gate.lock().unwrap();
    if cancelled() || gate.generation != generation {
        return Err(ERROR.into());
    }
    gate.enqueue(generation, &approval.response)
        .map_err(|_| ERROR)?;
    shared
        .lock()
        .unwrap()
        .engine
        .set_paired_token(approval.device_id, approval.token);
    drop(gate);
    emit_state(app, shared);
    Ok(())
}

fn process_frame<I: InputInjector>(
    input: &mut DesktopInput<I>,
    shared: &SharedModel,
    profile: &PointerProfile,
    bytes: &[u8],
    record_connection: impl FnOnce(&crate::protocol::AuthenticatedConnection) -> bool,
) -> Result<Option<String>, ()> {
    let event = shared
        .lock()
        .unwrap()
        .engine
        .receive_frame(bytes, now_ms())
        .map_err(|_| ())?;
    let settings = shared.lock().unwrap().state.settings.clone();
    input.set_pointer_scale_percent(settings.pointer_scale_percent);
    let response = match event {
        None => None,
        Some(EngineEvent::PendingPairing {
            replaced_response, ..
        }) => {
            let mut data = shared.lock().unwrap();
            data.state.pending_pairings = data.engine.pending_pairings();
            replaced_response
        }
        Some(EngineEvent::Response(response)) => {
            if serde_json::from_str::<Value>(&response)
                .ok()
                .is_some_and(|value| {
                    matches!(
                        value["error"]["code"].as_str(),
                        Some(
                            "invalid_auth"
                                | "unknown_device"
                                | "expired_timestamp"
                                | "duplicate_request"
                        )
                    )
                })
            {
                return Err(());
            }
            Some(response)
        }
        Some(EngineEvent::AuthenticatedConnection(connection)) => {
            let saved = record_connection(&connection);
            shared.lock().unwrap().state.bluetooth = BluetoothState::Connected;
            Some(
                shared
                    .lock()
                    .unwrap()
                    .engine
                    .complete_authenticated_connection(&connection, saved),
            )
        }
        Some(EngineEvent::PointerProfile(id)) => Some(linux_profile(&id, profile, &settings)),
        Some(EngineEvent::Text(command)) => {
            let result = input
                .type_text(&command.text)
                .map_err(|_| "X11 text input failed.");
            let response = shared
                .lock()
                .unwrap()
                .engine
                .complete_text_command(&command, result);
            if result.is_err() {
                input.release_all().map_err(|_| ())?;
            }
            response
        }
        Some(EngineEvent::MouseMove(command)) => {
            let result = input
                .move_pointer(command.dx.round() as i32, command.dy.round() as i32)
                .map_err(|_| "X11 pointer input failed.");
            let response = shared
                .lock()
                .unwrap()
                .engine
                .complete_mouse_move_command(&command, result);
            if result.is_err() {
                input.release_all().map_err(|_| ())?;
            }
            response
        }
        Some(EngineEvent::MouseClick(command)) => {
            let result = input
                .click_pointer(command.button, command.click_count)
                .map_err(|_| "X11 click failed.");
            let response = shared
                .lock()
                .unwrap()
                .engine
                .complete_mouse_click_command(&command, result);
            if result.is_err() {
                input.release_all().map_err(|_| ())?;
            }
            response
        }
        Some(EngineEvent::Desktop(command)) => {
            if !ALLOWED.contains(&command.command_type.as_str()) {
                Some(
                    shared
                        .lock()
                        .unwrap()
                        .engine
                        .complete_desktop_command_with_error(
                            &command,
                            Err((
                                "unsupported_command",
                                "This command is not enabled in the X11 development build.",
                            )),
                        )
                        .ok_or(())?,
                )
            } else {
                let result = input
                    .execute(
                        &command.device_id,
                        &command.command_type,
                        &command.payload,
                        &[],
                    )
                    .map(|_| ())
                    .map_err(|_| "X11 command failed.");
                let response = shared
                    .lock()
                    .unwrap()
                    .engine
                    .complete_desktop_command(&command, result);
                if result.is_err() {
                    input.release_all().map_err(|_| ())?;
                }
                if command.command_type == "connection.disconnecting" {
                    return Err(());
                }
                response
            }
        }
    };
    Ok(response)
}

fn linux_profile(
    id: &str,
    profile: &PointerProfile,
    settings: &crate::state::AppSettings,
) -> String {
    let mut value: Value = serde_json::from_str(&pointer_profile_response(id, profile, settings))
        .expect("internal profile JSON");
    let capabilities = &mut value["payload"]["capabilities"];
    capabilities["supportedCommands"] = json!(ALLOWED);
    capabilities["noAckCommands"] = json!(ALLOWED
        .iter()
        .filter(|command| !matches!(
            **command,
            "connection.disconnecting"
                | "connection.ping"
                | "pointer.profile"
                | "keyboard.textStream.open"
                | "keyboard.textStream.close"
        ))
        .collect::<Vec<_>>());
    for name in [
        "mouseRepeat",
        "keyRepeat",
        "pointerSpeed",
        "displayNavigation",
    ] {
        capabilities[name]["supported"] = json!(false);
        capabilities[name]["enabled"] = json!(false);
        capabilities[name]["setSupported"] = json!(false);
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{create_frames, MouseButton};
    use base64::Engine;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<Vec<&'static str>>>);
    impl InputInjector for Fake {
        fn inject_text(&mut self, _: &str) -> Result<(), String> {
            self.0.lock().unwrap().push("text");
            Ok(())
        }
        fn move_pointer(&mut self, _: i32, _: i32) -> Result<(), String> {
            self.0.lock().unwrap().push("move");
            Ok(())
        }
        fn move_pointer_absolute(&mut self, _: i32, _: i32) -> Result<(), String> {
            Ok(())
        }
        fn click_pointer(&mut self, _: MouseButton, _: u8) -> Result<(), String> {
            self.0.lock().unwrap().push("click");
            Ok(())
        }
        fn set_pointer_button(&mut self, _: MouseButton, down: bool) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .push(if down { "down" } else { "up" });
            Ok(())
        }
        fn scroll(&mut self, _: i32, _: i32) -> Result<(), String> {
            self.0.lock().unwrap().push("scroll");
            Ok(())
        }
        fn set_key(&mut self, _: &str, down: bool) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .push(if down { "key-down" } else { "key-up" });
            Ok(())
        }
        fn press_shortcut(&mut self, _: &[String]) -> Result<(), String> {
            self.0.lock().unwrap().push("shortcut");
            Ok(())
        }
        fn media(&mut self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn window(&mut self, _: &str) -> Result<(), String> {
            panic!("unsupported window action must not execute")
        }
    }
    fn profile() -> PointerProfile {
        PointerProfile {
            display_id: "test".into(),
            scale_factor: 1.0,
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            small_delta: 5,
            medium_delta: 20,
            large_delta: 60,
            display_navigation_supported: false,
            display_count: 1,
        }
    }
    fn model() -> AppModel {
        let model = AppModel::with_storage_for_test(AppStorage::at(
            std::env::temp_dir().join(format!("switchify-live-test-{}.json", Uuid::new_v4())),
        ));
        model
            .shared
            .lock()
            .unwrap()
            .engine
            .set_paired_token("fixture-device".into(), "fixture-token".into());
        model
    }
    fn signed(kind: &str, payload: Value) -> Value {
        let id = Uuid::new_v4().to_string();
        let now = now_ms();
        let canonical = format!("1\n{id}\nfixture-device\n{now}\n{kind}\n{payload}\nack");
        let mut mac = Hmac::<Sha256>::new_from_slice(b"fixture-token").unwrap();
        mac.update(canonical.as_bytes());
        let auth =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
        json!({ "version": 1, "id": id, "deviceId": "fixture-device", "timestamp": now, "type": kind, "payload": payload, "auth": auth })
    }
    fn deliver(
        input: &mut DesktopInput<Fake>,
        model: &AppModel,
        message: Value,
    ) -> Result<Option<String>, ()> {
        let mut response = None;
        for bytes in create_frames(&message.to_string()).unwrap() {
            response =
                process_frame(input, &model.shared, &profile(), &bytes, |_| true)?.or(response);
        }
        Ok(response)
    }

    #[test]
    fn authenticated_frames_drive_fake_input_and_cleanup_releases_drag() {
        let model = model();
        let fake = Fake::default();
        let mut input = DesktopInput::new(fake.clone());
        for (kind, payload) in [
            ("keyboard.typeText", json!({"text":"public fixture"})),
            ("mouse.move", json!({"dx":10,"dy":-5})),
            ("mouse.dragStart", json!({"button":"left"})),
            ("mouse.scroll", json!({"dx":0,"dy":1})),
        ] {
            let response: Value = serde_json::from_str(
                &deliver(&mut input, &model, signed(kind, payload))
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(response["ok"], true, "{kind}");
        }
        cleanup(&mut input, &model.shared).unwrap();
        assert_eq!(
            *fake.0.lock().unwrap(),
            ["text", "move", "down", "scroll", "up"]
        );
    }

    #[test]
    fn invalid_authentication_never_injects_and_unsupported_commands_are_honest() {
        let model = model();
        let fake = Fake::default();
        let mut input = DesktopInput::new(fake.clone());
        let mut command = signed("keyboard.typeText", json!({"text":"public fixture"}));
        command["auth"] = json!("invalid");
        assert!(deliver(&mut input, &model, command).is_err());
        let response: Value = serde_json::from_str(
            &deliver(
                &mut input,
                &model,
                signed("window.control", json!({"action":"minimize"})),
            )
            .unwrap()
            .unwrap(),
        )
        .unwrap();
        assert_eq!(response["error"]["code"], "unsupported_command");
        assert!(fake.0.lock().unwrap().is_empty());
    }

    #[test]
    fn profile_does_not_advertise_unimplemented_controls() {
        let response: Value = serde_json::from_str(&linux_profile(
            "fixture",
            &profile(),
            &crate::state::AppSettings::default(),
        ))
        .unwrap();
        let caps = &response["payload"]["capabilities"];
        assert_eq!(caps["supportedCommands"], json!(ALLOWED));
        for name in [
            "mouseRepeat",
            "keyRepeat",
            "pointerSpeed",
            "displayNavigation",
        ] {
            assert_eq!(caps[name]["supported"], false);
        }
        assert!(!ALLOWED.contains(&"window.control"));
        assert!(!ALLOWED.contains(&"switch.session.start"));
    }

    #[test]
    fn gate_prevents_peer_takeover_and_stale_publication() {
        let mut gate = Gate::default();
        let a = Address([1; 6]);
        let b = Address([2; 6]);
        let old = gate.claim(a).unwrap();
        assert_eq!(gate.claim(b), Err(ReqError::NotAuthorized));
        gate.close();
        let new = gate.claim(b).unwrap();
        assert_ne!(old, new);
        assert!(gate.enqueue(old, "public stale response").is_err());
        assert!(gate.responses.read(b.0, 0, 517).unwrap().is_empty());
    }
}
