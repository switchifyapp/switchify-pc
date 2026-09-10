use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
/// Transport-only protocol v1 framing shared with the Linux hardware probe.
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

pub const PROTOCOL_VERSION: i64 = 1;
pub const FRAME_PAYLOAD_BYTES: usize = 160;
const MAX_ENCODED_CHUNK_BYTES: usize = 4 * FRAME_PAYLOAD_BYTES.div_ceil(3);
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024;
pub const MAX_PARTIAL_MESSAGES: usize = 8;
pub const MAX_IDENTIFIER_BYTES: usize = 128;
pub const PARTIAL_TIMEOUT_MS: i64 = 10_000;

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
