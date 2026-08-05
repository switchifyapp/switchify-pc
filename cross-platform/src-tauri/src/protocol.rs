#[cfg(any(target_os = "macos", test))]
use std::collections::VecDeque;
use std::collections::{BTreeMap, HashMap};

use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::Sha256;
use uuid::Uuid;

use crate::state::AppSettings;

pub const PROTOCOL_VERSION: i64 = 1;
pub const FRAME_PAYLOAD_BYTES: usize = 160;
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024;
pub const PARTIAL_TIMEOUT_MS: i64 = 10_000;
pub const PAIRING_TIMEOUT_MS: i64 = 2 * 60 * 1_000;
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

#[derive(Debug, Default)]
#[cfg(any(target_os = "macos", test))]
pub struct OutboundQueue {
    frames: VecDeque<Vec<u8>>,
}

#[cfg(any(target_os = "macos", test))]
impl OutboundQueue {
    #[cfg(test)]
    pub fn push_message(&mut self, message: &str, maximum_frames: usize) -> Result<(), String> {
        let frames = create_frames(message)?;
        self.push_frames(frames, maximum_frames)
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn push_notification_message(
        &mut self,
        message: &str,
        maximum_frames: usize,
        maximum_encoded_bytes: usize,
    ) -> Result<(), String> {
        let frames = create_notification_frames(message, maximum_encoded_bytes)?;
        self.push_frames(frames, maximum_frames)
    }

    fn push_frames(&mut self, frames: Vec<Vec<u8>>, maximum_frames: usize) -> Result<(), String> {
        if self.frames.len() + frames.len() > maximum_frames {
            return Err("Bluetooth notification queue is full.".into());
        }
        self.frames.extend(frames);
        Ok(())
    }

    pub fn flush(
        &mut self,
        mut send: impl FnMut(&[u8]) -> Result<bool, String>,
    ) -> Result<(), String> {
        while let Some(frame) = self.frames.pop_front() {
            if !send(&frame)? {
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingPairingSummary {
    pub request_id: String,
    pub device_id: String,
    pub device_name: String,
    pub verification_code: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
struct PendingPairing {
    request_id: String,
    device_id: String,
    device_name: String,
    verification_code: String,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMode {
    Ack,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    PendingPairing(PendingPairingSummary),
    Response(String),
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
    pending_pairing: Option<PendingPairing>,
    tokens: HashMap<String, String>,
    replay_cache: HashMap<String, i64>,
}

impl ProtocolEngine {
    pub fn new(desktop_id: String) -> Self {
        Self {
            desktop_id,
            reassembler: FrameReassembler::default(),
            pending_pairing: None,
            tokens: HashMap::new(),
            replay_cache: HashMap::new(),
        }
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn desktop_id(&self) -> &str {
        &self.desktop_id
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn pending_pairing(&self) -> Option<PendingPairingSummary> {
        self.pending_pairing.as_ref().map(PendingPairing::summary)
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
        if !matches!(
            self.pending_pairing.as_ref(),
            Some(pending) if pending.request_id == request_id
        ) {
            return Err("Pairing request is no longer pending.".into());
        }
        let pending = self
            .pending_pairing
            .take()
            .ok_or_else(|| "Pairing request is no longer pending.".to_string())?;
        if now_ms >= pending.expires_at {
            return Err("Pairing request has expired.".into());
        }

        let mut token_bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut token_bytes);
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
        if !matches!(
            self.pending_pairing.as_ref(),
            Some(pending) if pending.request_id == request_id
        ) {
            return Err("Pairing request is no longer pending.".into());
        }
        let pending = self
            .pending_pairing
            .take()
            .ok_or_else(|| "Pairing request is no longer pending.".to_string())?;
        Ok(error_response(
            Some(&pending.request_id),
            "invalid_auth",
            "pairing_rejected",
        ))
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn expire_pairing(&mut self, request_id: &str, now_ms: i64) -> Option<String> {
        let should_expire = self.pending_pairing.as_ref().is_some_and(|pending| {
            pending.request_id == request_id && now_ms >= pending.expires_at
        });
        if !should_expire {
            return None;
        }
        let pending = self.pending_pairing.take()?;
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

    fn process_message(&mut self, raw: &str, now_ms: i64) -> Result<EngineEvent, String> {
        let value: Value = serde_json::from_str(raw).map_err(|_| "invalid_json".to_string())?;
        let request_id = value.get("id").and_then(Value::as_str);
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

        match command_type {
            "connection.ping" => Ok(EngineEvent::Response(ack_response(&validated.id))),
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
        if desktop_id != self.desktop_id {
            return Ok(EngineEvent::Response(error_response(
                Some(&id),
                "invalid_auth",
                "pairing_mismatch",
            )));
        }

        let pending = PendingPairing {
            request_id: id,
            verification_code: verification_code(&self.desktop_id, &device_id, &nonce),
            device_id,
            device_name,
            expires_at: now_ms + PAIRING_TIMEOUT_MS,
        };
        let summary = pending.summary();
        self.pending_pairing = Some(pending);
        Ok(EngineEvent::PendingPairing(summary))
    }

    fn validate_authenticated_command(
        &mut self,
        value: &Value,
        now_ms: i64,
    ) -> Result<ValidatedCommand, String> {
        let id = required_string(value, "id").map_err(|_| "invalid_payload")?;
        let device_id = required_string(value, "deviceId").map_err(|_| "invalid_payload")?;
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
        if response_mode == ResponseMode::None
            && !matches!(
                command_type.as_str(),
                "mouse.move"
                    | "mouse.click"
                    | "mouse.doubleClick"
                    | "mouse.rightClick"
                    | "mouse.scroll"
                    | "mouse.dragStart"
                    | "mouse.dragEnd"
                    | "keyboard.key"
                    | "keyboard.modifierDown"
                    | "keyboard.modifierUp"
                    | "keyboard.shortcut"
                    | "keyboard.typeText"
                    | "keyboard.textStream.char"
                    | "keyboard.textStream.chunk"
                    | "keyboard.textStream.key"
                    | "media.control"
                    | "window.control"
                    | "grid.switch.set"
                    | "switch.edge"
            )
        {
            return Err("invalid_payload".into());
        }
        let token = self
            .tokens
            .get(&device_id)
            .ok_or_else(|| "unknown_device".to_string())?;
        if now_ms.abs_diff(timestamp) > COMMAND_TIMESTAMP_TOLERANCE_MS as u64 {
            return Err("expired_timestamp".into());
        }

        self.replay_cache
            .retain(|_, expires_at| *expires_at > now_ms);
        let replay_key = format!("{device_id}:{id}");
        if self.replay_cache.contains_key(&replay_key) {
            return Err("duplicate_request".into());
        }
        let actual = required_string(value, "auth").map_err(|_| "invalid_auth")?;
        if !auth_matches(value, token, &actual) {
            return Err("invalid_auth".into());
        }
        self.replay_cache
            .insert(replay_key, now_ms + COMMAND_TIMESTAMP_TOLERANCE_MS);

        Ok(ValidatedCommand {
            id,
            device_id,
            payload,
            response_mode,
        })
    }
}

fn is_desktop_command(command: &str) -> bool {
    matches!(
        command,
        "mouse.scroll"
            | "mouse.repeat.start"
            | "mouse.repeat.stop"
            | "mouse.dragStart"
            | "mouse.dragEnd"
            | "keyboard.key"
            | "keyboard.modifierDown"
            | "keyboard.modifierUp"
            | "keyboard.shortcut"
            | "keyboard.textStream.open"
            | "keyboard.textStream.char"
            | "keyboard.textStream.chunk"
            | "keyboard.textStream.key"
            | "keyboard.textStream.close"
            | "media.control"
            | "window.control"
            | "grid.switch.set"
            | "grid.switch.sync"
            | "switch.profile.list"
            | "switch.session.start"
            | "switch.edge"
            | "switch.sync"
            | "switch.session.stop"
            | "pointer.display.move"
            | "pointer.speed.set"
            | "connection.disconnecting"
    )
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
        "switch.profile.list" | "connection.disconnecting" | "mouse.repeat.stop" => {
            object.is_empty()
        }
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
        _ => true,
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
    payload_object.len() == 2
        && match command_type {
            "mouse.move" => {
                bounded_number(payload, "dx", MAX_POINTER_DELTA).is_some()
                    && bounded_number(payload, "dy", MAX_POINTER_DELTA).is_some()
            }
            "mouse.scroll" => {
                bounded_number(payload, "dx", 50.0).is_some()
                    && bounded_number(payload, "dy", 50.0).is_some()
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
                "noAckCommands": [
                    "mouse.move",
                    "mouse.click",
                    "mouse.doubleClick",
                    "mouse.rightClick",
                    "mouse.scroll",
                    "mouse.dragStart",
                    "mouse.dragEnd",
                    "mouse.repeat.start",
                    "mouse.repeat.stop",
                    "keyboard.key",
                    "keyboard.modifierDown",
                    "keyboard.modifierUp",
                    "keyboard.shortcut",
                    "keyboard.typeText",
                    "keyboard.textStream.char",
                    "keyboard.textStream.chunk",
                    "keyboard.textStream.key",
                    "media.control",
                    "window.control",
                    "switch.edge"
                ],
                "supportedCommands": supported_commands(),
                "mouseRepeat": {
                    "supported": true,
                    "enabled": settings.mouse_repeat_enabled,
                    "intervalMs": settings.move_repeat_interval_ms,
                    "moveIntervalMs": settings.move_repeat_interval_ms,
                    "scrollIntervalMs": settings.scroll_repeat_interval_ms,
                    "minIntervalMs": 100,
                    "maxIntervalMs": 2000,
                    "accelerationDurationMs": settings.mouse_repeat_acceleration_duration_ms,
                    "accelerationDurationOptionsMs": [0, 500, 1000, 2000],
                    "accelerationInitialScalePercent": 25
                },
                "pointerSpeed": {
                    "supported": true,
                    "setSupported": true,
                    "scalePercent": settings.pointer_scale_percent,
                    "minScalePercent": 5,
                    "maxScalePercent": 225,
                    "stepPercent": 5,
                    "baseMoveDelta": 128,
                    "effectiveMoveDelta": (128.0 * f64::from(settings.pointer_scale_percent) / 100.0).round() as u32
                },
                "displayNavigation": {
                    "supported": false,
                    "displayCount": 1
                }
            }
        },
        "error": Value::Null
    })
    .to_string()
}

fn supported_commands() -> Vec<&'static str> {
    let commands = vec![
        "mouse.move",
        "mouse.click",
        "mouse.doubleClick",
        "mouse.rightClick",
        "mouse.scroll",
        "mouse.dragStart",
        "mouse.dragEnd",
        "mouse.repeat.start",
        "mouse.repeat.stop",
        "connection.ping",
        "pointer.profile",
        "pointer.speed.set",
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
        "window.control",
        "switch.profile.list",
        "switch.session.start",
        "switch.edge",
        "switch.sync",
        "switch.session.stop",
        "connection.disconnecting",
    ];
    #[cfg(target_os = "windows")]
    {
        let mut commands = commands;
        commands.extend(["grid.switch.set", "grid.switch.sync"]);
        commands
    }
    #[cfg(not(target_os = "windows"))]
    commands
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
mod tests {
    use super::*;

    const NOW: i64 = 1_724_000_000_000;
    const TOKEN: &str = "shared-token";

    #[test]
    fn frames_round_trip_out_of_order_and_ignore_duplicate_chunks() {
        let message = "x".repeat(500);
        let frames = create_frames(&message).unwrap();
        let mut decoded: Vec<BluetoothFrame> = frames
            .iter()
            .map(|frame| serde_json::from_slice(frame).unwrap())
            .collect();
        let duplicate = decoded[1].clone();
        decoded.swap(0, 2);
        decoded.insert(2, duplicate);
        let mut reassembler = FrameReassembler::default();
        let mut result = None;
        for frame in decoded {
            result = reassembler.accept(frame, NOW).unwrap().or(result);
        }
        assert_eq!(result.as_deref(), Some(message.as_str()));
    }

    #[test]
    fn rejects_oversized_and_expires_partial_messages() {
        let mut reassembler = FrameReassembler::default();
        let oversized = BluetoothFrame {
            version: 1,
            message_id: "large".into(),
            sequence: 0,
            is_final: true,
            total_bytes: MAX_MESSAGE_BYTES as i64 + 1,
            payload_base64: String::new(),
        };
        assert_eq!(
            reassembler.accept(oversized, NOW),
            Err("message_too_large".into())
        );

        let partial = BluetoothFrame {
            version: 1,
            message_id: "partial".into(),
            sequence: 0,
            is_final: false,
            total_bytes: 2,
            payload_base64: general_purpose::STANDARD.encode(b"a"),
        };
        assert_eq!(reassembler.accept(partial, NOW).unwrap(), None);
        assert_eq!(reassembler.clear_expired(NOW + PARTIAL_TIMEOUT_MS), 1);
    }

    #[test]
    fn matches_pairing_code_fixture() {
        assert_eq!(
            verification_code("desktop-1", "android-1", "nonce-1"),
            "610717"
        );
    }

    #[test]
    fn matches_android_hmac_and_slash_escaping_fixtures() {
        for (id, text, proof) in [
            (
                "apostrophe-1",
                "'",
                "W0OLnbhllDOCd0Gf_00WLpHRvfidYjHeY69nbcmTFYA",
            ),
            (
                "slash-1",
                "</",
                "70F2Z7SU6ur1gSYzR3Q9t1y5D02z_OuXfst15lQ_3yg",
            ),
            (
                "plain-slash-1",
                "/",
                "ckTEIQrJqsQSd7zoFkIaMzs0wuGvsDeaZwGvFnnU50E",
            ),
        ] {
            let command = json!({
                "version": 1,
                "id": id,
                "deviceId": "android-1",
                "timestamp": NOW,
                "type": "keyboard.typeText",
                "payload": { "text": text },
                "auth": proof
            });
            assert!(auth_matches(&command, TOKEN, proof), "fixture {id}");
        }
    }

    #[test]
    fn validates_utf16_length_and_controls() {
        assert!(is_safe_typed_text("hello\nworld\t"));
        assert!(is_safe_typed_text(&"😀".repeat(1_000)));
        assert!(!is_safe_typed_text(&"😀".repeat(1_001)));
        assert!(!is_safe_typed_text("hello\0world"));
        assert!(!is_safe_typed_text("bad\u{0085}"));
    }

    #[test]
    fn pairing_approval_is_memory_only_and_expires() {
        let mut engine = ProtocolEngine::new("desktop-1".into());
        let request = json!({
            "version": 1,
            "id": "pair-1",
            "type": "pairing.request",
            "payload": {
                "deviceId": "android-1",
                "deviceName": "Pixel",
                "desktopId": "desktop-1",
                "requestNonce": "nonce-1"
            }
        });
        let event = engine.process_message(&request.to_string(), NOW).unwrap();
        assert!(matches!(event, EngineEvent::PendingPairing(_)));
        let response = engine.approve_pairing("pair-1", NOW + 1).unwrap();
        let response: Value = serde_json::from_str(&response).unwrap();
        assert_eq!(response["type"], "pairing.complete");
        assert_eq!(response["payload"]["token"].as_str().unwrap().len(), 43);

        let mut expired = ProtocolEngine::new("desktop-1".into());
        expired.process_message(&request.to_string(), NOW).unwrap();
        assert!(expired
            .approve_pairing("pair-1", NOW + PAIRING_TIMEOUT_MS)
            .is_err());
    }

    #[test]
    fn rejects_expired_and_replayed_authenticated_commands() {
        let mut engine = ProtocolEngine::new("desktop-1".into());
        engine.tokens.insert("android-1".into(), TOKEN.into());
        let mut command = json!({
            "version": 1,
            "id": "text-1",
            "deviceId": "android-1",
            "timestamp": NOW,
            "type": "keyboard.typeText",
            "payload": { "text": "Hello" },
            "auth": ""
        });
        sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
        assert!(matches!(
            engine.process_message(&command.to_string(), NOW).unwrap(),
            EngineEvent::Text(_)
        ));
        let duplicate = engine.process_message(&command.to_string(), NOW).unwrap();
        assert!(
            matches!(duplicate, EngineEvent::Response(response) if response.contains("duplicate_request"))
        );

        command["id"] = Value::String("expired".into());
        command["timestamp"] = Value::from(NOW - COMMAND_TIMESTAMP_TOLERANCE_MS - 1);
        sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
        let expired = engine.process_message(&command.to_string(), NOW).unwrap();
        assert!(
            matches!(expired, EngineEvent::Response(response) if response.contains("expired_timestamp"))
        );
    }

    #[test]
    fn no_ack_text_suppresses_only_the_success_response() {
        let engine = ProtocolEngine::new("desktop-1".into());
        let command = TextCommand {
            id: "text-1".into(),
            text: "Hello".into(),
            response_mode: ResponseMode::None,
        };
        assert_eq!(engine.complete_text_command(&command, Ok(())), None);
        assert!(engine
            .complete_text_command(&command, Err("accessibility_required"))
            .unwrap()
            .contains("input_failed"));
    }

    #[test]
    fn authenticated_mouse_move_supports_ack_and_no_ack_modes() {
        for (id, response_mode, expected_mode) in [
            ("move-ack", None, ResponseMode::Ack),
            ("move-none", Some("none"), ResponseMode::None),
        ] {
            let mut engine = ProtocolEngine::new("desktop-1".into());
            engine.tokens.insert("android-1".into(), TOKEN.into());
            let mut command = json!({
                "version": 1,
                "id": id,
                "deviceId": "android-1",
                "timestamp": NOW,
                "type": "mouse.move",
                "payload": { "dx": 12, "dy": -6 },
                "auth": ""
            });
            if let Some(response_mode) = response_mode {
                command["responseMode"] = Value::String(response_mode.into());
            }
            sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
            let event = engine.process_message(&command.to_string(), NOW).unwrap();
            let EngineEvent::MouseMove(command) = event else {
                panic!("expected a mouse move command");
            };
            assert_eq!((command.dx, command.dy), (12.0, -6.0));
            assert_eq!(command.response_mode, expected_mode);
            let response = engine.complete_mouse_move_command(&command, Ok(()));
            assert_eq!(response.is_none(), expected_mode == ResponseMode::None);
        }
    }

    #[test]
    fn mouse_move_validates_boundaries_and_reports_no_ack_failures() {
        let mut engine = ProtocolEngine::new("desktop-1".into());
        engine.tokens.insert("android-1".into(), TOKEN.into());
        let mut boundary = json!({
            "version": 1,
            "id": "move-boundary",
            "deviceId": "android-1",
            "timestamp": NOW,
            "type": "mouse.move",
            "payload": { "dx": -500, "dy": 500 },
            "responseMode": "none",
            "auth": ""
        });
        sign(&mut boundary, TOKEN, SlashEscaping::AndroidHtmlSafe);
        let EngineEvent::MouseMove(command) =
            engine.process_message(&boundary.to_string(), NOW).unwrap()
        else {
            panic!("expected a boundary mouse move command");
        };
        assert!(engine
            .complete_mouse_move_command(&command, Err("accessibility_required"))
            .unwrap()
            .contains("input_failed"));

        for (id, payload) in [
            ("move-too-large", json!({ "dx": 501, "dy": 0 })),
            ("move-malformed", json!({ "dx": "right", "dy": 0 })),
        ] {
            let mut engine = ProtocolEngine::new("desktop-1".into());
            engine.tokens.insert("android-1".into(), TOKEN.into());
            let mut invalid = json!({
                "version": 1,
                "id": id,
                "deviceId": "android-1",
                "timestamp": NOW,
                "type": "mouse.move",
                "payload": payload,
                "auth": ""
            });
            sign(&mut invalid, TOKEN, SlashEscaping::AndroidHtmlSafe);
            assert!(matches!(
                engine.process_message(&invalid.to_string(), NOW).unwrap(),
                EngineEvent::Response(response) if response.contains("invalid_payload")
            ));
        }
    }

    #[test]
    fn authenticated_mouse_clicks_support_buttons_counts_and_no_ack() {
        for (id, command_type, payload, expected_button, expected_count) in [
            (
                "click-left",
                "mouse.click",
                json!({ "button": "left" }),
                MouseButton::Left,
                1,
            ),
            (
                "double-middle",
                "mouse.doubleClick",
                json!({ "button": "middle" }),
                MouseButton::Middle,
                2,
            ),
            (
                "right-click",
                "mouse.rightClick",
                json!({}),
                MouseButton::Right,
                1,
            ),
        ] {
            let mut engine = ProtocolEngine::new("desktop-1".into());
            engine.tokens.insert("android-1".into(), TOKEN.into());
            let mut command = json!({
                "version": 1,
                "id": id,
                "deviceId": "android-1",
                "timestamp": NOW,
                "type": command_type,
                "payload": payload,
                "responseMode": "none",
                "auth": ""
            });
            sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
            let EngineEvent::MouseClick(command) =
                engine.process_message(&command.to_string(), NOW).unwrap()
            else {
                panic!("expected a mouse click command");
            };
            assert_eq!(command.button, expected_button);
            assert_eq!(command.click_count, expected_count);
            assert_eq!(engine.complete_mouse_click_command(&command, Ok(())), None);
            assert!(engine
                .complete_mouse_click_command(&command, Err("accessibility_required"))
                .unwrap()
                .contains("input_failed"));
        }
    }

    #[test]
    fn mouse_clicks_reject_invalid_button_payloads() {
        for (id, command_type, payload) in [
            (
                "click-invalid",
                "mouse.click",
                json!({ "button": "primary" }),
            ),
            (
                "right-invalid",
                "mouse.rightClick",
                json!({ "button": "right" }),
            ),
        ] {
            let mut engine = ProtocolEngine::new("desktop-1".into());
            engine.tokens.insert("android-1".into(), TOKEN.into());
            let mut command = json!({
                "version": 1,
                "id": id,
                "deviceId": "android-1",
                "timestamp": NOW,
                "type": command_type,
                "payload": payload,
                "auth": ""
            });
            sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
            assert!(matches!(
                engine.process_message(&command.to_string(), NOW).unwrap(),
                EngineEvent::Response(response) if response.contains("invalid_payload")
            ));
        }
    }

    #[test]
    fn mouse_repeat_accepts_only_exact_bounded_nested_pointer_commands() {
        assert!(valid_desktop_payload(
            "mouse.repeat.start",
            &json!({"command":{"type":"mouse.move","payload":{"dx":500,"dy":-500}}})
        ));
        assert!(valid_desktop_payload(
            "mouse.repeat.start",
            &json!({"command":{"type":"mouse.scroll","payload":{"dx":50,"dy":-50}}})
        ));
        for payload in [
            json!({"command":{"type":"mouse.click","payload":{}}}),
            json!({"command":{"type":"mouse.move","payload":{"dx":501,"dy":0}}}),
            json!({"command":{"type":"mouse.scroll","payload":{"dx":0,"dy":51}}}),
            json!({"command":{"type":"mouse.move","payload":{"dx":1,"dy":1,"extra":true}}}),
        ] {
            assert!(!valid_desktop_payload("mouse.repeat.start", &payload));
        }
        assert!(valid_desktop_payload("mouse.repeat.stop", &json!({})));
        assert!(!valid_desktop_payload(
            "mouse.repeat.stop",
            &json!({"deviceId":"extra"})
        ));
    }

    #[test]
    fn pointer_profile_advertises_the_full_preview_command_set() {
        let profile = PointerProfile {
            display_id: "display:0:0:1920:1080:1".into(),
            scale_factor: 1.0,
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            small_delta: 49,
            medium_delta: 130,
            large_delta: 281,
        };
        let response: Value = serde_json::from_str(&pointer_profile_response(
            "profile-1",
            &profile,
            &AppSettings::default(),
        ))
        .unwrap();
        assert_eq!(response["payload"]["maxDelta"], 500);
        assert_eq!(response["payload"]["recommendedDeltas"]["medium"], 130);
        assert_eq!(response["payload"]["capabilities"]["noAckMouseMove"], true);
        assert_eq!(
            response["payload"]["capabilities"]["mouseRepeat"]["supported"],
            true
        );
        assert_eq!(
            response["payload"]["capabilities"]["mouseRepeat"]["accelerationInitialScalePercent"],
            25
        );
        let commands = response["payload"]["capabilities"]["supportedCommands"]
            .as_array()
            .unwrap();
        for command in [
            "mouse.move",
            "mouse.scroll",
            "keyboard.shortcut",
            "keyboard.typeText",
            "media.control",
            "window.control",
            "switch.session.start",
        ] {
            assert!(commands.contains(&json!(command)), "missing {command}");
        }
    }

    #[test]
    fn notification_frames_fit_the_negotiated_limit_and_round_trip() {
        let message = json!({
            "version": 1,
            "id": "android-00000000-0000-0000-0000-000000000000",
            "type": "ack",
            "ok": true,
            "error": Value::Null,
            "padding": "x".repeat(400)
        })
        .to_string();
        let frames = create_notification_frames(&message, 182).unwrap();
        assert!(frames.len() > 1);
        assert!(frames.iter().all(|frame| frame.len() <= 182));

        let mut reassembler = FrameReassembler::default();
        let mut result = None;
        for frame in frames {
            let frame = serde_json::from_slice(&frame).unwrap();
            result = reassembler.accept(frame, NOW).unwrap().or(result);
        }
        assert_eq!(result.as_deref(), Some(message.as_str()));
        assert!(create_notification_frames(&message, 64).is_err());
    }

    #[test]
    fn outbound_queue_preserves_backpressured_frame() {
        let mut queue = OutboundQueue::default();
        queue.push_message(&"x".repeat(400), 10).unwrap();
        let initial = queue.len();
        let mut attempts = 0;
        queue
            .flush(|_| {
                attempts += 1;
                Ok(false)
            })
            .unwrap();
        assert_eq!(attempts, 1);
        assert_eq!(queue.len(), initial);
        queue.flush(|_| Ok(true)).unwrap();
        assert_eq!(queue.len(), 0);
    }

    fn sign(command: &mut Value, token: &str, mode: SlashEscaping) {
        let canonical = canonical_command(command, mode).unwrap();
        let mut mac = Hmac::<Sha256>::new_from_slice(token.as_bytes()).unwrap();
        mac.update(canonical.as_bytes());
        command["auth"] =
            Value::String(general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()));
    }
}
