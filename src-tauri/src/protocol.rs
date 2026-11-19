#[cfg(any(target_os = "macos", test))]
use std::collections::VecDeque;
use std::collections::{BTreeMap, HashMap, HashSet};

use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, KeyInit, Mac};
use rand::Rng as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::Sha256;
use uuid::Uuid;

use crate::mouse_repeat::RepeatKey;
use crate::state::AppSettings;

pub const PROTOCOL_VERSION: i64 = 1;
pub const FRAME_PAYLOAD_BYTES: usize = 160;
const MAX_ENCODED_CHUNK_BYTES: usize = 4 * FRAME_PAYLOAD_BYTES.div_ceil(3);
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024;
pub const MAX_PARTIAL_MESSAGES: usize = 8;
pub const MAX_PENDING_PAIRINGS: usize = 8;
pub const MAX_IDENTIFIER_BYTES: usize = 128;
pub const PARTIAL_TIMEOUT_MS: i64 = 10_000;
pub const PAIRING_TIMEOUT_MS: i64 = 2 * 60 * 1_000;
pub const PAIRING_NOT_PENDING: &str = "Pairing request is no longer pending.";
pub const PAIRING_EXPIRED: &str = "Pairing request has expired.";
pub const COMMAND_TIMESTAMP_TOLERANCE_MS: i64 = 2 * 60 * 1_000;
pub const MAX_TEXT_UTF16_UNITS: usize = 2_000;
pub const MAX_POINTER_DELTA: f64 = 500.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothFrame {
    pub version: i64,
    pub message_id: String,
    pub sequence: i64,
    pub is_final: bool,
    pub total_bytes: i64,
    pub payload_base64: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BluetoothStatus<'a> {
    protocol_version: i64,
    display_name: &'a str,
    desktop_id: &'a str,
    platform: &'a str,
}

pub fn bluetooth_status_payload(
    display_name: &str,
    desktop_id: &str,
    platform: &str,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&BluetoothStatus {
        protocol_version: PROTOCOL_VERSION,
        display_name,
        desktop_id,
        platform,
    })
    .map_err(|error| error.to_string())
}

#[derive(Debug)]
struct PartialMessage {
    total_bytes: usize,
    created_at: i64,
    chunks: BTreeMap<i64, Vec<u8>>,
    final_sequence: Option<i64>,
}

#[derive(Debug, Default)]
pub struct FrameReassembler {
    partials: HashMap<String, PartialMessage>,
}

impl FrameReassembler {
    pub fn accept(&mut self, frame: BluetoothFrame, now_ms: i64) -> Result<Option<String>, String> {
        self.clear_expired(now_ms);
        if frame.version != PROTOCOL_VERSION
            || frame.message_id.is_empty()
            || frame.message_id.len() > MAX_IDENTIFIER_BYTES
            || frame.payload_base64.len() > MAX_ENCODED_CHUNK_BYTES
            || frame.sequence < 0
            || frame.total_bytes < 0
            || frame.total_bytes as usize > MAX_MESSAGE_BYTES
        {
            return Err(if frame.total_bytes > MAX_MESSAGE_BYTES as i64 {
                "message_too_large".into()
            } else {
                "invalid_frame".into()
            });
        }

        let chunk = general_purpose::STANDARD
            .decode(&frame.payload_base64)
            .map_err(|_| "invalid_frame".to_string())?;
        let total_bytes = frame.total_bytes as usize;
        let sequence = frame.sequence as usize;
        let invalid_empty_message =
            total_bytes == 0 && (sequence != 0 || !frame.is_final || !chunk.is_empty());
        let invalid_nonempty_message = total_bytes > 0
            && (chunk.is_empty()
                || chunk.len() > FRAME_PAYLOAD_BYTES
                || chunk.len() > total_bytes
                || sequence >= total_bytes);
        if invalid_empty_message || invalid_nonempty_message {
            self.partials.remove(&frame.message_id);
            return Err("invalid_frame".into());
        }
        if !self.partials.contains_key(&frame.message_id)
            && self.partials.len() >= MAX_PARTIAL_MESSAGES
        {
            return Err("invalid_frame".into());
        }
        let partial = self
            .partials
            .entry(frame.message_id.clone())
            .or_insert_with(|| PartialMessage {
                total_bytes,
                created_at: now_ms,
                chunks: BTreeMap::new(),
                final_sequence: None,
            });

        if partial.total_bytes != total_bytes {
            self.partials.remove(&frame.message_id);
            return Err("invalid_frame".into());
        }

        partial.chunks.entry(frame.sequence).or_insert(chunk);
        if partial.chunks.values().map(Vec::len).sum::<usize>() > total_bytes {
            self.partials.remove(&frame.message_id);
            return Err("invalid_frame".into());
        }
        if frame.is_final {
            if partial
                .final_sequence
                .is_some_and(|sequence| sequence != frame.sequence)
            {
                self.partials.remove(&frame.message_id);
                return Err("invalid_frame".into());
            }
            partial.final_sequence = Some(frame.sequence);
        }

        let Some(final_sequence) = partial.final_sequence else {
            return Ok(None);
        };
        if partial
            .chunks
            .keys()
            .any(|sequence| *sequence > final_sequence)
        {
            self.partials.remove(&frame.message_id);
            return Err("invalid_frame".into());
        }

        let mut payload = Vec::with_capacity(total_bytes);
        for sequence in 0..=final_sequence {
            let Some(chunk) = partial.chunks.get(&sequence) else {
                return Ok(None);
            };
            payload.extend_from_slice(chunk);
            if payload.len() > total_bytes {
                self.partials.remove(&frame.message_id);
                return Err("invalid_frame".into());
            }
        }
        if payload.len() != total_bytes {
            return Ok(None);
        }

        self.partials.remove(&frame.message_id);
        Ok(Some(String::from_utf8_lossy(&payload).into_owned()))
    }

    pub fn clear_expired(&mut self, now_ms: i64) -> usize {
        let before = self.partials.len();
        self.partials
            .retain(|_, partial| now_ms - partial.created_at < PARTIAL_TIMEOUT_MS);
        before - self.partials.len()
    }
}

#[cfg(test)]
pub fn create_frames(message: &str) -> Result<Vec<Vec<u8>>, String> {
    create_frames_with_payload_bytes(message, FRAME_PAYLOAD_BYTES)
}

pub fn create_notification_frames(
    message: &str,
    maximum_encoded_bytes: usize,
) -> Result<Vec<Vec<u8>>, String> {
    for payload_bytes in (1..=FRAME_PAYLOAD_BYTES).rev() {
        let frames = create_frames_with_payload_bytes(message, payload_bytes)?;
        if frames
            .iter()
            .all(|frame| frame.len() <= maximum_encoded_bytes)
        {
            return Ok(frames);
        }
    }
    Err("Bluetooth notification size is too small for a protocol frame.".into())
}

fn create_frames_with_payload_bytes(
    message: &str,
    payload_bytes: usize,
) -> Result<Vec<Vec<u8>>, String> {
    let bytes = message.as_bytes();
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err("message_too_large".into());
    }
    let message_id = Uuid::new_v4().to_string();
    let chunks: Vec<&[u8]> = if bytes.is_empty() {
        vec![&[]]
    } else {
        bytes.chunks(payload_bytes).collect()
    };
    chunks
        .iter()
        .enumerate()
        .map(|(sequence, chunk)| {
            serde_json::to_vec(&BluetoothFrame {
                version: PROTOCOL_VERSION,
                message_id: message_id.clone(),
                sequence: sequence as i64,
                is_final: sequence + 1 == chunks.len(),
                total_bytes: bytes.len() as i64,
                payload_base64: general_purpose::STANDARD.encode(chunk),
            })
            .map_err(|error| error.to_string())
        })
        .collect()
}

/// Which centrals must receive a protocol message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationTarget {
    /// Every subscribed central.
    AllSubscribers,
    /// Only the central with this transport identifier. Used for pairing
    /// responses so a token never reaches another connected central.
    Central(String),
}

#[cfg(any(target_os = "macos", test))]
#[derive(Debug)]
struct OutboundFrame {
    bytes: Vec<u8>,
    target: NotificationTarget,
}

#[derive(Debug, Default)]
#[cfg(any(target_os = "macos", test))]
pub struct OutboundQueue {
    frames: VecDeque<OutboundFrame>,
}

#[cfg(any(target_os = "macos", test))]
impl OutboundQueue {
    #[cfg(test)]
    pub fn push_message(&mut self, message: &str, maximum_frames: usize) -> Result<(), String> {
        let frames = create_frames(message)?;
        self.push_frames(frames, maximum_frames, NotificationTarget::AllSubscribers)
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn push_notification_message(
        &mut self,
        message: &str,
        maximum_frames: usize,
        maximum_encoded_bytes: usize,
        target: NotificationTarget,
    ) -> Result<(), String> {
        let frames = create_notification_frames(message, maximum_encoded_bytes)?;
        self.push_frames(frames, maximum_frames, target)
    }

    fn push_frames(
        &mut self,
        frames: Vec<Vec<u8>>,
        maximum_frames: usize,
        target: NotificationTarget,
    ) -> Result<(), String> {
        if self.frames.len() + frames.len() > maximum_frames {
            return Err("Bluetooth notification queue is full.".into());
        }
        self.frames
            .extend(frames.into_iter().map(|bytes| OutboundFrame {
                bytes,
                target: target.clone(),
            }));
        Ok(())
    }

    /// Sends queued frames in order. `send` returns `Ok(false)` when the
    /// transport cannot take more right now; the frame is kept for the next
    /// flush.
    pub fn flush(
        &mut self,
        mut send: impl FnMut(&[u8], &NotificationTarget) -> Result<bool, String>,
    ) -> Result<(), String> {
        while let Some(frame) = self.frames.pop_front() {
            if !send(&frame.bytes, &frame.target)? {
                self.frames.push_front(frame);
                break;
            }
        }
        Ok(())
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.frames.len()
    }
}

/// Remembers which central sent each pending pairing request so the
/// pairing response can be delivered to that central alone and so the
/// request is cancelled when that central disconnects.
#[derive(Debug, Default)]
pub struct PairingCentralRegistry {
    by_request: HashMap<String, String>,
}

impl PairingCentralRegistry {
    pub fn associate(&mut self, request_id: String, central_id: String) {
        self.by_request.insert(request_id, central_id);
    }

    pub fn retain_pending(&mut self, pending_request_ids: &HashSet<String>) {
        self.by_request
            .retain(|request_id, _| pending_request_ids.contains(request_id));
    }

    pub fn central_for(&self, request_id: &str) -> Option<&str> {
        self.by_request.get(request_id).map(String::as_str)
    }

    pub fn remove(&mut self, request_id: &str) -> Option<String> {
        self.by_request.remove(request_id)
    }

    /// Removes and returns the request IDs associated with `central_id`.
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn take_for_central(&mut self, central_id: &str) -> Vec<String> {
        let request_ids = self
            .by_request
            .iter()
            .filter(|(_, associated_central)| associated_central.as_str() == central_id)
            .map(|(request_id, _)| request_id.clone())
            .collect::<Vec<_>>();
        self.by_request
            .retain(|_, associated_central| associated_central != central_id);
        request_ids
    }

    /// Removes and returns the request IDs whose central is not in
    /// `connected_central_ids`.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub fn take_disconnected(&mut self, connected_central_ids: &HashSet<String>) -> Vec<String> {
        let request_ids = self
            .by_request
            .iter()
            .filter(|(_, central_id)| !connected_central_ids.contains(*central_id))
            .map(|(request_id, _)| request_id.clone())
            .collect::<Vec<_>>();
        self.by_request
            .retain(|_, central_id| connected_central_ids.contains(central_id));
        request_ids
    }

    pub fn clear(&mut self) {
        self.by_request.clear();
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingPairingSummary {
    pub request_id: String,
    pub device_id: String,
    pub device_name: String,
    pub verification_code: String,
    pub expires_at: i64,
}

/// What a Supabase pairing intent must match for the PC to approve a request
/// without the manual prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingIntent {
    pub desktop_id: String,
    pub device_id: String,
    pub nonce: String,
}

#[derive(Debug, Clone)]
struct PendingPairing {
    request_id: String,
    device_id: String,
    device_name: String,
    verification_code: String,
    nonce: String,
    expires_at: i64,
}

impl PendingPairing {
    fn summary(&self) -> PendingPairingSummary {
        PendingPairingSummary {
            request_id: self.request_id.clone(),
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            verification_code: self.verification_code.clone(),
            expires_at: self.expires_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextCommand {
    pub id: String,
    pub text: String,
    pub response_mode: ResponseMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MouseMoveCommand {
    pub id: String,
    pub dx: f64,
    pub dy: f64,
    pub response_mode: ResponseMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseClickCommand {
    pub id: String,
    pub button: MouseButton,
    pub click_count: u8,
    pub response_mode: ResponseMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesktopCommand {
    pub id: String,
    pub device_id: String,
    pub command_type: String,
    pub payload: Value,
    pub response_mode: ResponseMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedConnection {
    pub id: String,
    pub device_id: String,
    pub device_name: Option<String>,
    pub connected_at: i64,
    pub received_order: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointerProfile {
    pub display_id: String,
    pub scale_factor: f64,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub small_delta: u32,
    pub medium_delta: u32,
    pub large_delta: u32,
    pub display_navigation_supported: bool,
    pub display_count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMode {
    Ack,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    PendingPairing {
        request: PendingPairingSummary,
        replaced_response: Option<String>,
    },
    Response(String),
    AuthenticatedConnection(AuthenticatedConnection),
    PointerProfile(String),
    MouseMove(MouseMoveCommand),
    MouseClick(MouseClickCommand),
    Text(TextCommand),
    Desktop(DesktopCommand),
}

#[derive(Debug)]
pub struct ProtocolEngine {
    desktop_id: String,
    reassembler: FrameReassembler,
    pending_pairings: HashMap<String, PendingPairing>,
    tokens: HashMap<String, String>,
    replay_cache: HashMap<String, i64>,
    connection_order: u64,
    access_deadline: Option<i64>,
}

impl ProtocolEngine {
    pub fn new(desktop_id: String) -> Self {
        Self {
            desktop_id,
            reassembler: FrameReassembler::default(),
            pending_pairings: HashMap::new(),
            tokens: HashMap::new(),
            replay_cache: HashMap::new(),
            connection_order: 0,
            access_deadline: None,
        }
    }

    pub fn set_access_deadline(&mut self, deadline: Option<i64>) {
        self.access_deadline = deadline;
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn desktop_id(&self) -> &str {
        &self.desktop_id
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn pending_pairings(&self) -> Vec<PendingPairingSummary> {
        let mut pending: Vec<_> = self
            .pending_pairings
            .values()
            .map(PendingPairing::summary)
            .collect();
        pending.sort_by(|left, right| {
            right
                .expires_at
                .cmp(&left.expires_at)
                .then_with(|| left.request_id.cmp(&right.request_id))
        });
        pending
    }

    /// The identifiers an account pairing intent must match for a pending
    /// request. Kept out of `PendingPairingSummary` so the nonce never reaches
    /// app state.
    pub fn pairing_intent(&self, request_id: &str, now_ms: i64) -> Option<PairingIntent> {
        self.pending_pairings
            .get(request_id)
            .filter(|pending| now_ms < pending.expires_at)
            .map(|pending| PairingIntent {
                desktop_id: self.desktop_id.clone(),
                device_id: pending.device_id.clone(),
                nonce: pending.nonce.clone(),
            })
    }

    pub fn cancel_pairing(&mut self, request_id: &str) -> bool {
        self.pending_pairings.remove(request_id).is_some()
    }

    pub fn cancel_all_pairings(&mut self) -> usize {
        let request_ids = self.pending_pairings.keys().cloned().collect::<Vec<_>>();
        request_ids
            .iter()
            .filter(|request_id| self.cancel_pairing(request_id))
            .count()
    }

    pub fn reset_transport_session(&mut self) -> usize {
        self.reassembler = FrameReassembler::default();
        self.cancel_all_pairings()
    }

    pub fn set_paired_token(&mut self, device_id: String, token: String) {
        self.tokens.insert(device_id, token);
    }

    pub fn token_for(&self, device_id: &str) -> Option<&str> {
        self.tokens.get(device_id).map(String::as_str)
    }

    pub fn forget_device(&mut self, device_id: &str) {
        self.tokens.remove(device_id);
        self.replay_cache
            .retain(|key, _| !key.starts_with(&format!("{device_id}:")));
    }

    pub fn receive_frame(
        &mut self,
        bytes: &[u8],
        now_ms: i64,
    ) -> Result<Option<EngineEvent>, String> {
        let frame: BluetoothFrame =
            serde_json::from_slice(bytes).map_err(|_| "invalid_frame".to_string())?;
        let Some(message) = self.reassembler.accept(frame, now_ms)? else {
            return Ok(None);
        };
        self.process_message(&message, now_ms).map(Some)
    }

    pub fn approve_pairing(&mut self, request_id: &str, now_ms: i64) -> Result<String, String> {
        let pending = self
            .pending_pairings
            .remove(request_id)
            .ok_or_else(|| PAIRING_NOT_PENDING.to_string())?;
        if now_ms >= pending.expires_at {
            return Err(PAIRING_EXPIRED.into());
        }

        let mut token_bytes = [0_u8; 32];
        rand::rng().fill_bytes(&mut token_bytes);
        let token = general_purpose::URL_SAFE_NO_PAD.encode(token_bytes);
        self.tokens.insert(pending.device_id.clone(), token.clone());
        Ok(json!({
            "version": PROTOCOL_VERSION,
            "id": pending.request_id,
            "type": "pairing.complete",
            "ok": true,
            "payload": {
                "desktopId": self.desktop_id,
                "deviceId": pending.device_id,
                "token": token
            },
            "error": Value::Null
        })
        .to_string())
    }

    pub fn reject_pairing(&mut self, request_id: &str) -> Result<String, String> {
        let pending = self
            .pending_pairings
            .remove(request_id)
            .ok_or_else(|| PAIRING_NOT_PENDING.to_string())?;
        Ok(error_response(
            Some(&pending.request_id),
            "invalid_auth",
            "pairing_rejected",
        ))
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn expire_pairing(&mut self, request_id: &str, now_ms: i64) -> Option<String> {
        let should_expire = self
            .pending_pairings
            .get(request_id)
            .is_some_and(|pending| now_ms >= pending.expires_at);
        if !should_expire {
            return None;
        }
        let pending = self.pending_pairings.remove(request_id)?;
        Some(error_response(
            Some(&pending.request_id),
            "invalid_auth",
            "pairing_request_expired",
        ))
    }

    pub fn complete_text_command(
        &self,
        command: &TextCommand,
        result: Result<(), &str>,
    ) -> Option<String> {
        complete_input_command(&command.id, command.response_mode, result)
    }

    pub fn complete_mouse_move_command(
        &self,
        command: &MouseMoveCommand,
        result: Result<(), &str>,
    ) -> Option<String> {
        complete_input_command(&command.id, command.response_mode, result)
    }

    pub fn complete_mouse_click_command(
        &self,
        command: &MouseClickCommand,
        result: Result<(), &str>,
    ) -> Option<String> {
        complete_input_command(&command.id, command.response_mode, result)
    }

    pub fn complete_desktop_command(
        &self,
        command: &DesktopCommand,
        result: Result<(), &str>,
    ) -> Option<String> {
        complete_input_command(&command.id, command.response_mode, result)
    }

    pub fn complete_desktop_command_with_error(
        &self,
        command: &DesktopCommand,
        result: Result<(), (&str, &str)>,
    ) -> Option<String> {
        match result {
            Ok(()) if command.response_mode == ResponseMode::None => None,
            Ok(()) => Some(ack_response(&command.id)),
            Err((code, message)) => Some(error_response(Some(&command.id), code, message)),
        }
    }

    pub fn complete_authenticated_connection(
        &self,
        connection: &AuthenticatedConnection,
        saved: bool,
    ) -> String {
        if saved || connection.device_name.is_none() {
            ack_response(&connection.id)
        } else {
            error_response(
                Some(&connection.id),
                "name_update_failed",
                "The Remote name could not be saved.",
            )
        }
    }

    pub(crate) fn process_message(
        &mut self,
        raw: &str,
        now_ms: i64,
    ) -> Result<EngineEvent, String> {
        let value: Value = serde_json::from_str(raw).map_err(|_| "invalid_json".to_string())?;
        let request_id = value
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| valid_identifier(id));
        if value.get("version").and_then(Value::as_i64) != Some(PROTOCOL_VERSION) {
            return Ok(EngineEvent::Response(error_response(
                request_id,
                "invalid_version",
                "Unsupported protocol version.",
            )));
        }
        let Some(command_type) = value.get("type").and_then(Value::as_str) else {
            return Ok(EngineEvent::Response(error_response(
                request_id,
                "invalid_type",
                "Command type is required.",
            )));
        };
        if command_type == "pairing.request" {
            return self
                .process_pairing_request(&value, now_ms)
                .or_else(|reason| {
                    Ok(EngineEvent::Response(error_response(
                        request_id,
                        &reason,
                        "Pairing request is invalid.",
                    )))
                });
        }

        let validated = match self.validate_authenticated_command(&value, now_ms) {
            Ok(command) => command,
            Err(reason) => {
                return Ok(EngineEvent::Response(error_response(
                    request_id,
                    &reason,
                    "Command authentication failed.",
                )))
            }
        };

        if self
            .access_deadline
            .is_some_and(|deadline| now_ms / 1000 >= deadline)
            && !allowed_when_locked(command_type)
        {
            return Ok(EngineEvent::Response(error_response(
                Some(&validated.id),
                "invalid_state",
                crate::licensing::LOCKED,
            )));
        }

        match command_type {
            "connection.ping" => {
                let device_name = match validated.payload.get("deviceName") {
                    None => None,
                    Some(value) => match value.as_str().and_then(normalize_remote_name) {
                        Some(device_name) => Some(device_name),
                        None => {
                            return Ok(EngineEvent::Response(error_response(
                                Some(&validated.id),
                                "invalid_payload",
                                "Remote name is invalid.",
                            )))
                        }
                    },
                };
                self.connection_order = self.connection_order.saturating_add(1);
                Ok(EngineEvent::AuthenticatedConnection(
                    AuthenticatedConnection {
                        id: validated.id,
                        device_id: validated.device_id,
                        device_name,
                        connected_at: now_ms,
                        received_order: self.connection_order,
                    },
                ))
            }
            "pointer.profile" => Ok(EngineEvent::PointerProfile(validated.id)),
            "mouse.move" => {
                let Some(dx) = bounded_number(&validated.payload, "dx", MAX_POINTER_DELTA) else {
                    return Ok(EngineEvent::Response(error_response(
                        Some(&validated.id),
                        "invalid_payload",
                        "Pointer movement payload is invalid.",
                    )));
                };
                let Some(dy) = bounded_number(&validated.payload, "dy", MAX_POINTER_DELTA) else {
                    return Ok(EngineEvent::Response(error_response(
                        Some(&validated.id),
                        "invalid_payload",
                        "Pointer movement payload is invalid.",
                    )));
                };
                Ok(EngineEvent::MouseMove(MouseMoveCommand {
                    id: validated.id,
                    dx,
                    dy,
                    response_mode: validated.response_mode,
                }))
            }
            "mouse.click" | "mouse.doubleClick" => {
                let Some(button) = mouse_button(&validated.payload) else {
                    return Ok(EngineEvent::Response(error_response(
                        Some(&validated.id),
                        "invalid_payload",
                        "Mouse button payload is invalid.",
                    )));
                };
                Ok(EngineEvent::MouseClick(MouseClickCommand {
                    id: validated.id,
                    button,
                    click_count: if command_type == "mouse.doubleClick" {
                        2
                    } else {
                        1
                    },
                    response_mode: validated.response_mode,
                }))
            }
            "mouse.rightClick" => {
                if validated
                    .payload
                    .as_object()
                    .is_some_and(|payload| !payload.is_empty())
                {
                    return Ok(EngineEvent::Response(error_response(
                        Some(&validated.id),
                        "invalid_payload",
                        "Right-click payload must be empty.",
                    )));
                }
                Ok(EngineEvent::MouseClick(MouseClickCommand {
                    id: validated.id,
                    button: MouseButton::Right,
                    click_count: 1,
                    response_mode: validated.response_mode,
                }))
            }
            "keyboard.typeText" => {
                let Some(text) = validated.payload.get("text").and_then(Value::as_str) else {
                    return Ok(EngineEvent::Response(error_response(
                        Some(&validated.id),
                        "invalid_payload",
                        "Text payload is invalid.",
                    )));
                };
                if !is_safe_typed_text(text) {
                    return Ok(EngineEvent::Response(error_response(
                        Some(&validated.id),
                        "invalid_payload",
                        "Text payload is invalid.",
                    )));
                }
                Ok(EngineEvent::Text(TextCommand {
                    id: validated.id,
                    text: text.to_owned(),
                    response_mode: validated.response_mode,
                }))
            }
            command if is_desktop_command(command) => {
                if !valid_desktop_payload(command, &validated.payload) {
                    return Ok(EngineEvent::Response(error_response(
                        Some(&validated.id),
                        "invalid_payload",
                        "Desktop command payload is invalid.",
                    )));
                }
                Ok(EngineEvent::Desktop(DesktopCommand {
                    id: validated.id,
                    device_id: validated.device_id,
                    command_type: command.to_owned(),
                    payload: validated.payload,
                    response_mode: validated.response_mode,
                }))
            }
            _ => Ok(EngineEvent::Response(error_response(
                Some(&validated.id),
                "unsupported_command",
                "Unsupported command.",
            ))),
        }
    }

    fn process_pairing_request(
        &mut self,
        value: &Value,
        now_ms: i64,
    ) -> Result<EngineEvent, String> {
        let id = required_string(value, "id")?;
        let payload = value
            .get("payload")
            .and_then(Value::as_object)
            .ok_or_else(|| "invalid_payload".to_string())?;
        let device_id = required_map_string(payload, "deviceId")?;
        let device_name = required_map_string(payload, "deviceName")?;
        let desktop_id = required_map_string(payload, "desktopId")?;
        let nonce = required_map_string(payload, "requestNonce")?;
        if !valid_identifier(&id)
            || !valid_identifier(&device_id)
            || !valid_identifier(&desktop_id)
            || !valid_identifier(&nonce)
        {
            return Err("invalid_payload".into());
        }
        if desktop_id != self.desktop_id {
            return Ok(EngineEvent::Response(error_response(
                Some(&id),
                "invalid_auth",
                "pairing_mismatch",
            )));
        }
        let device_name =
            normalize_remote_name(&device_name).ok_or_else(|| "invalid_payload".to_string())?;

        let pending = PendingPairing {
            request_id: id,
            verification_code: verification_code(&self.desktop_id, &device_id, &nonce),
            nonce,
            device_id,
            device_name,
            expires_at: now_ms + PAIRING_TIMEOUT_MS,
        };
        if self.pending_pairings.contains_key(&pending.request_id) {
            return Ok(EngineEvent::Response(error_response(
                Some(&pending.request_id),
                "duplicate_request",
                "Pairing request ID is already pending.",
            )));
        }

        let summary = pending.summary();
        let replaced_request_id = self
            .pending_pairings
            .values()
            .find(|existing| existing.device_id == pending.device_id)
            .map(|existing| existing.request_id.clone());
        if replaced_request_id.is_none() && self.pending_pairings.len() >= MAX_PENDING_PAIRINGS {
            return Err("invalid_payload".into());
        }
        let replaced_response = replaced_request_id.and_then(|request_id| {
            self.pending_pairings.remove(&request_id).map(|_| {
                error_response(Some(&request_id), "invalid_auth", "pairing_request_expired")
            })
        });
        self.pending_pairings
            .insert(pending.request_id.clone(), pending);
        Ok(EngineEvent::PendingPairing {
            request: summary,
            replaced_response,
        })
    }

    fn validate_authenticated_command(
        &mut self,
        value: &Value,
        now_ms: i64,
    ) -> Result<ValidatedCommand, String> {
        let id = required_string(value, "id").map_err(|_| "invalid_payload")?;
        let device_id = required_string(value, "deviceId").map_err(|_| "invalid_payload")?;
        if !valid_identifier(&id) || !valid_identifier(&device_id) {
            return Err("invalid_payload".into());
        }
        let timestamp = value
            .get("timestamp")
            .and_then(Value::as_i64)
            .ok_or_else(|| "invalid_payload".to_string())?;
        let payload = value
            .get("payload")
            .filter(|payload| payload.is_object())
            .cloned()
            .ok_or_else(|| "invalid_payload".to_string())?;
        let response_mode = match value.get("responseMode") {
            None => ResponseMode::Ack,
            Some(Value::String(mode)) if mode == "ack" => ResponseMode::Ack,
            Some(Value::String(mode)) if mode == "none" => ResponseMode::None,
            _ => return Err("invalid_payload".into()),
        };
        let command_type = required_string(value, "type").map_err(|_| "invalid_payload")?;
        if response_mode == ResponseMode::None && !accepts_no_ack(&command_type) {
            return Err("invalid_payload".into());
        }
        let token = self
            .tokens
            .get(&device_id)
            .ok_or_else(|| "unknown_device".to_string())?;
        if now_ms.abs_diff(timestamp) > COMMAND_TIMESTAMP_TOLERANCE_MS as u64 {
            return Err("expired_timestamp".into());
        }

        // Keep an entry through the last millisecond at which the timestamp
        // check would still accept the command.
        self.replay_cache
            .retain(|_, expires_at| *expires_at >= now_ms);
        let replay_key = format!("{device_id}:{id}");
        if self.replay_cache.contains_key(&replay_key) {
            return Err("duplicate_request".into());
        }
        let actual = required_string(value, "auth").map_err(|_| "invalid_auth")?;
        if !auth_matches(value, token, &actual) {
            return Err("invalid_auth".into());
        }
        // A command is accepted while `now` is within the tolerance of its own
        // timestamp, so the replay entry must outlive that window when the
        // timestamp is ahead of the receive time, not just the receive window.
        self.replay_cache.insert(
            replay_key,
            now_ms.max(timestamp) + COMMAND_TIMESTAMP_TOLERANCE_MS,
        );

        Ok(ValidatedCommand {
            id,
            device_id,
            payload,
            response_mode,
        })
    }
}

/// Which builds advertise a command in `supportedCommands`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Available {
    Always,
    /// Only when the desktop can move between displays.
    DisplayNavigation,
    /// Only on Windows builds.
    Windows,
}

/// Who handles a command once it is accepted.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Route {
    /// Passed to the runtime as an [`EngineEvent::Desktop`] command.
    Desktop,
    /// Parsed by the engine itself.
    Engine,
}

/// Whether a client may send a command with `responseMode: "none"`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reply {
    Required,
    Optional,
}

/// Whether a command is still accepted after the licence access deadline.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Licence {
    Required,
    NotRequired,
}

/// One remote command and how the protocol treats it. Each column has its own
/// type, so a row with its columns in the wrong order does not compile.
struct CommandSpec {
    name: &'static str,
    route: Route,
    reply: Reply,
    licence: Licence,
    available: Available,
}

const fn command(
    name: &'static str,
    route: Route,
    reply: Reply,
    licence: Licence,
    available: Available,
) -> CommandSpec {
    CommandSpec {
        name,
        route,
        reply,
        licence,
        available,
    }
}

// Short names for the table columns.
const DESKTOP: Route = Route::Desktop;
const ENGINE: Route = Route::Engine;
const ACK: Reply = Reply::Required;
const NO_ACK: Reply = Reply::Optional;
const NEEDS_LICENCE: Licence = Licence::Required;
const LOCKED_OK: Licence = Licence::NotRequired;

/// Every remote command the desktop understands, listed once. The advertised
/// `supportedCommands` and `noAckCommands` lists, the locked-licence
/// allowlist and desktop routing are all read from this table, so they cannot
/// drift apart. Order matters: each advertised list follows table order, with
/// gated commands advertised after the rest.
///
/// A new `DESKTOP` command also needs:
/// - an arm in `valid_desktop_payload`, which rejects commands it does not
///   list, and a sample payload in the `every_desktop_command_has_a_payload_rule`
///   test;
/// - handling in `DesktopInput::execute` in `input.rs`, unless
///   `desktop_dispatch::dispatch` handles it before input;
/// - an entry in `MobileTypingRoute::for_command` if it types text, or in
///   `desktop_dispatch::cancels_dwell` if it should end a dwell countdown.
#[rustfmt::skip]
const COMMANDS: &[CommandSpec] = &[
    //      name                         route    reply   licence        advertised
    command("mouse.move",                ENGINE,  NO_ACK, NEEDS_LICENCE, Available::Always),
    command("mouse.click",               ENGINE,  NO_ACK, NEEDS_LICENCE, Available::Always),
    command("mouse.doubleClick",         ENGINE,  NO_ACK, NEEDS_LICENCE, Available::Always),
    command("mouse.rightClick",          ENGINE,  NO_ACK, NEEDS_LICENCE, Available::Always),
    command("mouse.scroll",              DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("mouse.dragStart",           DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("mouse.dragEnd",             DESKTOP, NO_ACK, LOCKED_OK,     Available::Always),
    command("mouse.repeat.start",        DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("mouse.repeat.stop",         DESKTOP, NO_ACK, LOCKED_OK,     Available::Always),
    command("connection.ping",           ENGINE,  ACK,    LOCKED_OK,     Available::Always),
    command("pointer.profile",           ENGINE,  ACK,    LOCKED_OK,     Available::Always),
    command("pointer.speed.set",         DESKTOP, ACK,    NEEDS_LICENCE, Available::Always),
    command("pointer.display.move",      DESKTOP, ACK,    NEEDS_LICENCE, Available::DisplayNavigation),
    command("keyboard.key",              DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("keyboard.modifierDown",     DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("keyboard.modifierUp",       DESKTOP, NO_ACK, LOCKED_OK,     Available::Always),
    command("keyboard.shortcut",         DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("keyboard.typeText",         ENGINE,  NO_ACK, NEEDS_LICENCE, Available::Always),
    command("keyboard.textStream.open",  DESKTOP, ACK,    NEEDS_LICENCE, Available::Always),
    command("keyboard.textStream.char",  DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("keyboard.textStream.chunk", DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("keyboard.textStream.key",   DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("keyboard.textStream.close", DESKTOP, ACK,    NEEDS_LICENCE, Available::Always),
    command("media.control",             DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("window.control",            DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    // Accepted without a reply on every build, but only advertised on Windows.
    command("grid.switch.set",           DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Windows),
    command("grid.switch.sync",          DESKTOP, ACK,    NEEDS_LICENCE, Available::Windows),
    command("switch.profile.list",       DESKTOP, ACK,    LOCKED_OK,     Available::Always),
    command("switch.session.start",      DESKTOP, ACK,    NEEDS_LICENCE, Available::Always),
    command("switch.edge",               DESKTOP, NO_ACK, NEEDS_LICENCE, Available::Always),
    command("switch.sync",               DESKTOP, ACK,    NEEDS_LICENCE, Available::Always),
    command("switch.session.stop",       DESKTOP, ACK,    LOCKED_OK,     Available::Always),
    command("connection.disconnecting",  DESKTOP, ACK,    LOCKED_OK,     Available::Always),
];

fn spec(command: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|spec| spec.name == command)
}

fn allowed_when_locked(command: &str) -> bool {
    spec(command).is_some_and(|spec| spec.licence == Licence::NotRequired)
}

fn accepts_no_ack(command: &str) -> bool {
    spec(command).is_some_and(|spec| spec.reply == Reply::Optional)
}

/// Commands a client may send with `responseMode: "none"`, as advertised in
/// `noAckCommands`.
fn no_ack_commands() -> Vec<&'static str> {
    COMMANDS
        .iter()
        .filter(|spec| spec.reply == Reply::Optional)
        .map(|spec| spec.name)
        .collect()
}

fn is_desktop_command(command: &str) -> bool {
    spec(command).is_some_and(|spec| spec.route == Route::Desktop)
}

fn valid_desktop_payload(command: &str, payload: &Value) -> bool {
    let Some(object) = payload.as_object() else {
        return false;
    };
    let positive_sequence = || {
        object
            .get("sessionId")
            .and_then(Value::as_str)
            .is_some_and(|id| Uuid::parse_str(id).is_ok())
            && object
                .get("sequence")
                .and_then(Value::as_i64)
                .is_some_and(|sequence| sequence > 0)
    };
    match command {
        "switch.profile.list" => {
            object.is_empty()
                || (object.len() == 1
                    && object.get("includeScanning").is_some_and(Value::is_boolean))
        }
        "connection.disconnecting" | "mouse.repeat.stop" => object.is_empty(),
        "mouse.repeat.start" => valid_repeat_start(object),
        "switch.session.start" => {
            object.len() == 4
                && object
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .is_some_and(|id| Uuid::parse_str(id).is_ok())
                && object
                    .get("profileId")
                    .and_then(Value::as_str)
                    .is_some_and(|id| (1..=80).contains(&id.len()))
                && object
                    .get("profileVersion")
                    .and_then(Value::as_i64)
                    .is_some_and(|version| version > 0)
                && object
                    .get("switchCount")
                    .and_then(Value::as_i64)
                    .is_some_and(|count| (1..=8).contains(&count))
        }
        "switch.edge" => {
            object.len() == 4
                && positive_sequence()
                && object
                    .get("switchId")
                    .and_then(Value::as_i64)
                    .is_some_and(|id| (1..=8).contains(&id))
                && matches!(
                    object.get("state").and_then(Value::as_str),
                    Some("down" | "up")
                )
        }
        "switch.sync" => {
            object.len() == 3
                && positive_sequence()
                && sorted_switch_ids(object.get("pressedSwitchIds"))
        }
        "switch.session.stop" => object.len() == 2 && positive_sequence(),
        "grid.switch.set" => {
            (object.len() == 2 || object.len() == 4 && positive_sequence())
                && object
                    .get("switchId")
                    .and_then(Value::as_i64)
                    .is_some_and(|id| (1..=8).contains(&id))
                && matches!(
                    object.get("state").and_then(Value::as_str),
                    Some("down" | "up")
                )
        }
        "grid.switch.sync" => {
            object.len() == 3
                && positive_sequence()
                && unique_switch_ids(object.get("pressedSwitchIds"))
        }
        "pointer.speed.set" => {
            object.len() == 1
                && object
                    .get("scalePercent")
                    .and_then(Value::as_f64)
                    .is_some_and(|value| value.is_finite() && value > 0.0)
        }
        "pointer.display.move" => {
            object.len() == 1
                && matches!(
                    object.get("direction").and_then(Value::as_str),
                    Some("left" | "right" | "up" | "down")
                )
        }
        "keyboard.textStream.open" => valid_stream_id(object.get("streamId")),
        "keyboard.textStream.char" => {
            valid_stream_item(object)
                && object
                    .get("text")
                    .and_then(Value::as_str)
                    .is_some_and(|text| text.chars().count() == 1 && is_safe_typed_text(text))
        }
        "keyboard.textStream.chunk" => {
            valid_stream_item(object)
                && object
                    .get("text")
                    .and_then(Value::as_str)
                    .is_some_and(is_safe_typed_text)
        }
        "keyboard.textStream.key" => {
            valid_stream_item(object)
                && object
                    .get("key")
                    .and_then(Value::as_str)
                    .is_some_and(|key| !key.is_empty() && key.len() <= 20)
        }
        "keyboard.textStream.close" => {
            valid_stream_id(object.get("streamId"))
                && object
                    .get("expectedCount")
                    .and_then(Value::as_i64)
                    .is_some_and(|count| (0..=10_000).contains(&count))
        }
        // `DesktopInput::execute` checks these payloads and replies with an
        // error when one is wrong.
        "mouse.scroll"
        | "mouse.dragStart"
        | "mouse.dragEnd"
        | "keyboard.key"
        | "keyboard.modifierDown"
        | "keyboard.modifierUp"
        | "keyboard.shortcut"
        | "media.control"
        | "window.control" => true,
        _ => false,
    }
}

fn valid_repeat_start(object: &Map<String, Value>) -> bool {
    if object.len() != 1 {
        return false;
    }
    let Some(command) = object.get("command").and_then(Value::as_object) else {
        return false;
    };
    if command.len() != 2 {
        return false;
    }
    let Some(command_type) = command.get("type").and_then(Value::as_str) else {
        return false;
    };
    let Some(payload) = command.get("payload") else {
        return false;
    };
    let Some(payload_object) = payload.as_object() else {
        return false;
    };
    match command_type {
        "mouse.move" => {
            payload_object.len() == 2
                && bounded_number(payload, "dx", MAX_POINTER_DELTA).is_some()
                && bounded_number(payload, "dy", MAX_POINTER_DELTA).is_some()
        }
        "mouse.scroll" => {
            payload_object.len() == 2
                && bounded_number(payload, "dx", 50.0).is_some()
                && bounded_number(payload, "dy", 50.0).is_some()
        }
        // The repeatable-key allowlist is owned by `RepeatKey` so the validator
        // and the runtime can never disagree about what may repeat.
        "keyboard.key" => {
            payload_object.len() == 1
                && payload_object
                    .get("key")
                    .and_then(Value::as_str)
                    .and_then(RepeatKey::parse)
                    .is_some()
        }
        _ => false,
    }
}

fn valid_stream_id(value: Option<&Value>) -> bool {
    value.and_then(Value::as_str).is_some_and(|id| {
        (1..=80).contains(&id.len()) && id.chars().all(|character| !character.is_control())
    })
}

fn valid_stream_item(object: &Map<String, Value>) -> bool {
    valid_stream_id(object.get("streamId"))
        && object
            .get("seq")
            .and_then(Value::as_i64)
            .is_some_and(|sequence| (0..=10_000).contains(&sequence))
}

fn sorted_switch_ids(value: Option<&Value>) -> bool {
    let Some(values) = value
        .and_then(Value::as_array)
        .filter(|values| values.len() <= 8)
    else {
        return false;
    };
    let mut previous = 0;
    for value in values {
        let Some(id) = value.as_i64() else {
            return false;
        };
        if !(1..=8).contains(&id) || id <= previous {
            return false;
        }
        previous = id;
    }
    true
}

fn unique_switch_ids(value: Option<&Value>) -> bool {
    let Some(values) = value
        .and_then(Value::as_array)
        .filter(|values| values.len() <= 8)
    else {
        return false;
    };
    let mut ids = std::collections::HashSet::new();
    values.iter().all(|value| {
        value
            .as_i64()
            .is_some_and(|id| (1..=8).contains(&id) && ids.insert(id))
    })
}

struct ValidatedCommand {
    id: String,
    device_id: String,
    payload: Value,
    response_mode: ResponseMode,
}

fn required_string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "invalid_payload".to_string())
}

fn required_map_string(value: &Map<String, Value>, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "invalid_payload".to_string())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_IDENTIFIER_BYTES
}

fn normalize_remote_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty() && trimmed.chars().count() <= 40 && !trimmed.chars().any(char::is_control))
        .then(|| trimmed.to_owned())
}

fn bounded_number(value: &Value, key: &str, maximum_magnitude: f64) -> Option<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite() && number.abs() <= maximum_magnitude)
}

fn mouse_button(payload: &Value) -> Option<MouseButton> {
    match payload.get("button").and_then(Value::as_str) {
        Some("left") => Some(MouseButton::Left),
        Some("middle") => Some(MouseButton::Middle),
        Some("right") => Some(MouseButton::Right),
        _ => None,
    }
}

pub fn verification_code(desktop_id: &str, device_id: &str, nonce: &str) -> String {
    let canonical = format!("{desktop_id}\n{device_id}\n{nonce}");
    let mut hash = 2_166_136_261_u32;
    for unit in canonical.encode_utf16() {
        hash ^= u32::from(unit);
        hash = hash.wrapping_mul(16_777_619);
    }
    let value = (hash as i32).wrapping_abs() % 1_000_000;
    format!("{value:06}")
}

pub fn is_safe_typed_text(text: &str) -> bool {
    text.encode_utf16().count() <= MAX_TEXT_UTF16_UNITS
        && !text.chars().any(|character| {
            let code = character as u32;
            (code <= 0x1f && !matches!(character, '\t' | '\n' | '\r'))
                || (0x7f..=0x9f).contains(&code)
        })
}

fn auth_matches(command: &Value, token: &str, actual: &str) -> bool {
    let Ok(actual) = general_purpose::URL_SAFE_NO_PAD.decode(actual) else {
        return false;
    };
    [SlashEscaping::AndroidHtmlSafe, SlashEscaping::AllSlashes]
        .into_iter()
        .any(|mode| {
            let Ok(canonical) = canonical_command(command, mode) else {
                return false;
            };
            let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(token.as_bytes()) else {
                return false;
            };
            mac.update(canonical.as_bytes());
            mac.verify_slice(&actual).is_ok()
        })
}

#[derive(Clone, Copy)]
enum SlashEscaping {
    AndroidHtmlSafe,
    AllSlashes,
}

fn canonical_command(command: &Value, mode: SlashEscaping) -> Result<String, String> {
    let version = command
        .get("version")
        .and_then(Value::as_i64)
        .ok_or_else(|| "invalid_payload".to_string())?;
    let id = required_string(command, "id")?;
    let device_id = required_string(command, "deviceId")?;
    let timestamp = command
        .get("timestamp")
        .and_then(Value::as_i64)
        .ok_or_else(|| "invalid_payload".to_string())?;
    let command_type = required_string(command, "type")?;
    let payload = command
        .get("payload")
        .ok_or_else(|| "invalid_payload".to_string())?;
    let response_mode = command
        .get("responseMode")
        .and_then(Value::as_str)
        .unwrap_or("ack");
    Ok(format!(
        "{version}\n{id}\n{device_id}\n{timestamp}\n{command_type}\n{}\n{response_mode}",
        stable_stringify(payload, mode)?
    ))
}

fn stable_stringify(value: &Value, mode: SlashEscaping) -> Result<String, String> {
    match value {
        Value::Object(object) => {
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort();
            let fields = keys
                .into_iter()
                .map(|key| {
                    Ok(format!(
                        "{}:{}",
                        json_quote(key, mode)?,
                        stable_stringify(&object[key], mode)?
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(format!("{{{}}}", fields.join(",")))
        }
        Value::Array(array) => Ok(format!(
            "[{}]",
            array
                .iter()
                .map(|item| stable_stringify(item, mode))
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        )),
        Value::String(value) => json_quote(value, mode),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => Ok("null".into()),
    }
}

fn json_quote(value: &str, mode: SlashEscaping) -> Result<String, String> {
    let quoted = serde_json::to_string(value).map_err(|error| error.to_string())?;
    Ok(match mode {
        SlashEscaping::AndroidHtmlSafe => quoted.replace("</", "<\\/"),
        SlashEscaping::AllSlashes => quoted.replace('/', "\\/"),
    })
}

fn ack_response(id: &str) -> String {
    json!({
        "version": PROTOCOL_VERSION,
        "id": id,
        "type": "ack",
        "ok": true,
        "error": Value::Null
    })
    .to_string()
}

fn complete_input_command(
    id: &str,
    response_mode: ResponseMode,
    result: Result<(), &str>,
) -> Option<String> {
    match result {
        Ok(()) if response_mode == ResponseMode::None => None,
        Ok(()) => Some(ack_response(id)),
        Err(message) => Some(error_response(Some(id), "input_failed", message)),
    }
}

fn error_response(id: Option<&str>, code: &str, message: &str) -> String {
    json!({
        "version": PROTOCOL_VERSION,
        "id": id,
        "type": "error",
        "ok": false,
        "error": { "code": code, "message": message }
    })
    .to_string()
}

pub fn pointer_profile_response(
    id: &str,
    profile: &PointerProfile,
    settings: &AppSettings,
) -> String {
    json!({
        "version": PROTOCOL_VERSION,
        "id": id,
        "type": "pointer.profile",
        "ok": true,
        "payload": {
            "displayId": profile.display_id,
            "scaleFactor": profile.scale_factor,
            "bounds": {
                "x": profile.x,
                "y": profile.y,
                "width": profile.width,
                "height": profile.height
            },
            "maxDelta": MAX_POINTER_DELTA as u32,
            "recommendedDeltas": {
                "small": profile.small_delta,
                "medium": profile.medium_delta,
                "large": profile.large_delta
            },
            "capabilities": {
                "noAckMouseMove": true,
                "switchScanning": cfg!(any(target_os = "windows", target_os = "macos")),
                "noAckCommands": no_ack_commands(),
                "supportedCommands": supported_commands(profile.display_navigation_supported),
                "mouseRepeat": {
                    "supported": true,
                    "enabled": settings.mouse_repeat_enabled,
                    "intervalMs": settings.move_repeat_interval_ms,
                    "moveIntervalMs": settings.move_repeat_interval_ms,
                    "scrollIntervalMs": settings.scroll_repeat_interval_ms,
                    "minIntervalMs": 100,
                    "maxIntervalMs": 2000,
                    "accelerationDurationMs": settings.mouse_repeat_acceleration_duration_ms,
                    // The earlier fixed choices, kept for clients that read
                    // them. Each is still accepted, but Settings now sets any
                    // tenth of a second up to 3 s, so accelerationDurationMs
                    // need not be one of them.
                    "accelerationDurationOptionsMs": [0, 500, 1000, 2000],
                    "accelerationInitialScalePercent": 25
                },
                "keyRepeat": {
                    "supported": true,
                    "enabled": settings.key_repeat_enabled,
                    "intervalMs": settings.key_repeat_interval_ms,
                    "initialDelayMs": settings.key_repeat_initial_delay_ms,
                    "minIntervalMs": 100,
                    "maxIntervalMs": 1000,
                    "repeatableKeys": RepeatKey::ALL
                        .iter()
                        .map(|key| key.protocol_name())
                        .collect::<Vec<_>>()
                },
                "pointerSpeed": {
                    "supported": true,
                    "setSupported": true,
                    "scalePercent": settings.pointer_scale_percent,
                    "minScalePercent": 5,
                    "maxScalePercent": 1350,
                    "stepPercent": 5,
                    "baseMoveDelta": crate::mouse_repeat::DIRECTION_MOVE_DELTA,
                    "effectiveMoveDelta": (f64::from(crate::mouse_repeat::DIRECTION_MOVE_DELTA) * f64::from(settings.pointer_scale_percent) / 100.0).round() as u32
                },
                "displayNavigation": {
                    "supported": profile.display_navigation_supported,
                    "displayCount": profile.display_count
                }
            }
        },
        "error": Value::Null
    })
    .to_string()
}

fn supported_commands(display_navigation_supported: bool) -> Vec<&'static str> {
    let mut gates = vec![Available::Always];
    if display_navigation_supported {
        gates.push(Available::DisplayNavigation);
    }
    if cfg!(target_os = "windows") {
        gates.push(Available::Windows);
    }
    gates
        .into_iter()
        .flat_map(|gate| {
            COMMANDS
                .iter()
                .filter(move |spec| spec.available == gate)
                .map(|spec| spec.name)
        })
        .collect()
}

pub fn switch_profile_catalog_response(
    id: &str,
    profiles: &[crate::state::SwitchProfile],
) -> String {
    let summaries: Vec<Value> =
        profiles
            .iter()
            .map(|profile| {
                let bindings: Vec<Value> = profile
                .bindings
                .iter()
                .map(|binding| {
                    let behavior = if profile.provider == "grid3" {
                        "stateful"
                    } else {
                        match binding.binding_type.as_str() {
                        "key" | "mouseButton" => "stateful",
                        "none" => "unassigned",
                        _ => "pulse",
                        }
                    };
                    let label = if profile.provider == "grid3" {
                        format!("Grid switch {}", binding.switch_id)
                    } else {
                        match binding.binding_type.as_str() {
                        "none" => "Unassigned".into(),
                        "shortcut" => binding
                            .keys
                            .as_ref()
                            .map(|keys| keys.join(" + "))
                            .or_else(|| binding.value.clone())
                            .unwrap_or_else(|| "Shortcut".into()),
                        "mouseClick" => {
                            format!("{} click", binding.value.as_deref().unwrap_or("Left"))
                        }
                        "scroll" => format!("Scroll {}", binding.value.as_deref().unwrap_or("")),
                        "media" => binding.value.clone().unwrap_or_else(|| "Media".into()),
                        _ => binding.value.clone().unwrap_or_else(|| "Unassigned".into()),
                        }
                    };
                    json!({ "switchId": binding.switch_id, "label": label, "behavior": behavior })
                })
                .collect();
                json!({
                    "id": profile.id,
                    "version": profile.version,
                    "name": profile.name,
                    "kind": profile.provider,
                    "bindings": bindings
                })
            })
            .collect();
    json!({
        "version": PROTOCOL_VERSION,
        "id": id,
        "type": "switch.profile.list",
        "ok": true,
        "payload": { "catalogRevision": 1, "profiles": summaries },
        "error": Value::Null
    })
    .to_string()
}

#[cfg(test)]
mod tests;
