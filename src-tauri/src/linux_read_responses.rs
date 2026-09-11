//! Linux replies use ATT reads, never characteristic notifications. The runtime
//! must close this mailbox on disconnect before admitting another connection.
use std::collections::VecDeque;

use crate::protocol::create_notification_frames;

pub const RESPONSE_UUID: &str = "7a78f7ec-1d6d-4d92-9ef0-1f89d3db21f4";
pub const RESPONSE_TRANSPORT: &str = "read-v1";
const FRAME_BYTES: usize = 180;
const MAX_QUEUED_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    Unauthorized,
    InvalidOffset,
    InvalidMtu,
    StaleSession,
    Full,
    InvalidMessage,
}

// No Debug: queued responses can contain pairing credentials.
#[derive(Default)]
pub struct ReadResponses {
    owner: Option<[u8; 6]>,
    generation: u64,
    frames: VecDeque<Vec<u8>>,
    bytes: usize,
    snapshot: Vec<u8>,
}

impl ReadResponses {
    pub fn open(&mut self, peer: [u8; 6]) -> Result<u64, ReadError> {
        match self.owner {
            Some(owner) if owner != peer => Err(ReadError::Unauthorized),
            Some(_) => Ok(self.generation),
            None => {
                self.close();
                self.owner = Some(peer);
                Ok(self.generation)
            }
        }
    }

    pub fn close(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.owner = None;
        self.frames.clear();
        self.snapshot.clear();
        self.bytes = 0;
    }

    pub fn enqueue(&mut self, generation: u64, message: &str) -> Result<(), ReadError> {
        if self.owner.is_none() || self.generation != generation {
            return Err(ReadError::StaleSession);
        }
        let frames = create_notification_frames(message, FRAME_BYTES)
            .map_err(|_| ReadError::InvalidMessage)?;
        let bytes: usize = frames.iter().map(Vec::len).sum();
        if self.bytes + bytes > MAX_QUEUED_BYTES {
            return Err(ReadError::Full);
        }
        self.bytes += bytes;
        self.frames.extend(frames);
        Ok(())
    }

    pub fn read(&mut self, peer: [u8; 6], offset: u16, mtu: u16) -> Result<Vec<u8>, ReadError> {
        if mtu < 23 {
            return Err(ReadError::InvalidMtu);
        }
        if self.owner.is_some_and(|owner| owner != peer) {
            return Err(ReadError::Unauthorized);
        }
        if offset == 0 {
            self.snapshot = self.frames.pop_front().unwrap_or_default();
            self.bytes -= self.snapshot.len();
        }
        let tail = self
            .snapshot
            .get(usize::from(offset)..)
            .ok_or(ReadError::InvalidOffset)?;
        Ok(tail[..tail.len().min(usize::from(mtu) - 1)].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::FrameReassembler;

    const A: [u8; 6] = [1; 6];
    const B: [u8; 6] = [2; 6];

    #[test]
    fn competitor_cannot_read_or_consume_owner_replies() {
        let mut queue = ReadResponses::default();
        let generation = queue.open(A).unwrap();
        queue.enqueue(generation, "public test response").unwrap();
        assert_eq!(queue.open(B), Err(ReadError::Unauthorized));
        assert_eq!(queue.read(B, 0, 517), Err(ReadError::Unauthorized));
        assert_eq!(queue.read(B, 1, 517), Err(ReadError::Unauthorized));
        assert!(!queue.read(A, 0, 517).unwrap().is_empty());
        assert!(queue.read(A, 0, 517).unwrap().is_empty());
    }

    #[test]
    fn long_reads_preserve_snapshot_and_shared_framing() {
        let mut queue = ReadResponses::default();
        let generation = queue.open(A).unwrap();
        let message = "public fixture ".repeat(300);
        queue.enqueue(generation, &message).unwrap();
        let mut assembler = FrameReassembler::default();
        let mut completed = None;
        loop {
            let mut frame = queue.read(A, 0, 23).unwrap();
            if frame.is_empty() {
                break;
            }
            loop {
                let part = queue.read(A, frame.len() as u16, 23).unwrap();
                if part.is_empty() {
                    break;
                }
                frame.extend(part);
            }
            assert!(frame.len() <= FRAME_BYTES);
            completed = assembler
                .accept(serde_json::from_slice(&frame).unwrap(), 0)
                .unwrap()
                .or(completed);
        }
        assert_eq!(completed.as_deref(), Some(message.as_str()));
    }

    #[test]
    fn disconnect_discards_snapshots_queue_and_late_results() {
        let mut queue = ReadResponses::default();
        let old = queue.open(A).unwrap();
        queue.enqueue(old, "public old response").unwrap();
        queue.read(A, 0, 23).unwrap();
        queue.close();
        let new = queue.open(B).unwrap();
        assert_ne!(old, new);
        assert_eq!(queue.enqueue(old, "late"), Err(ReadError::StaleSession));
        assert!(queue.read(B, 0, 517).unwrap().is_empty());
        assert_eq!(queue.read(B, 1, 517), Err(ReadError::InvalidOffset));
        assert_eq!(queue.read(B, 0, 22), Err(ReadError::InvalidMtu));
    }

    #[test]
    fn queue_is_bounded_and_overflow_does_not_partially_enqueue() {
        let mut queue = ReadResponses::default();
        let generation = queue.open(A).unwrap();
        let message = "x".repeat(16 * 1024);
        while queue.enqueue(generation, &message).is_ok() {}
        let before = queue.bytes;
        assert!(before <= MAX_QUEUED_BYTES);
        assert_eq!(queue.enqueue(generation, &message), Err(ReadError::Full));
        assert_eq!(queue.bytes, before);
    }
}
