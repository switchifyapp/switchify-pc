use super::{Core, Event, Mode, Status};
use anyhow::{bail, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
};

const MAX_FRAME: usize = 128 * 1024;
pub(super) const VERSION: u32 = 1;
pub(super) const WORKER_FAILURE_MESSAGE: &str =
    "Keyboard capture worker stopped. Release your switches, then restart Switchify.";

pub(super) fn worker_failed(core: &mut Core) {
    core.worker_failed = true;
    core.native_lost = true;
    core.stop(super::StopReason::CaptureLost);
}

pub(super) fn ready(
    core: &mut Core,
    snapshot: &Snapshot,
    generation: u64,
    mode: Mode,
    now: u64,
) -> Result<()> {
    if core.worker_failed {
        bail!(WORKER_FAILURE_MESSAGE);
    }
    if core.status.mode != Mode::Off
        || core.status.generation.wrapping_add(1) != generation
        || snapshot.status.generation != generation
        || snapshot.status.mode != mode
    {
        bail!("Invalid capture worker startup acknowledgement.");
    }
    core.status = snapshot.status;
    core.native_lost = false;
    core.last_heartbeat = now;
    core.events.clear();
    apply(core, snapshot, generation, false)
}

pub(super) fn clock_age(parent_ms: u64, sent_ticks: u64, now_ticks: u64) -> Result<u64> {
    if parent_ms > sent_ticks || sent_ticks > now_ticks {
        bail!("Invalid capture worker clock origin.");
    }
    Ok(parent_ms + (now_ticks - sent_ticks))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Start {
    pub version: u32,
    pub generation: u64,
    pub mode: Mode,
    pub mappings: HashMap<String, String>,
    pub escape_ms: u64,
    pub parent_ms: u64,
    pub sent_ticks: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Poll {
    pub generation: u64,
    pub heartbeat_ms: u64,
    pub cancellation: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snapshot {
    pub status: Status,
    pub down: HashSet<String>,
    pub owned: Vec<String>,
    pub native_lost: bool,
    pub recovering: bool,
    pub finished: bool,
    pub events: Vec<Event>,
}

#[derive(Serialize, Deserialize)]
pub(super) enum Reply {
    Ready(Snapshot),
    Failed(String),
}

pub(super) fn read<T: DeserializeOwned>(input: &mut impl Read) -> Result<T> {
    let mut size = [0; 4];
    input.read_exact(&mut size)?;
    let size = u32::from_le_bytes(size) as usize;
    if size == 0 || size > MAX_FRAME {
        bail!("Invalid capture worker frame size.");
    }
    let mut bytes = vec![0; size];
    input.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub(super) fn write(output: &mut impl Write, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.is_empty() || bytes.len() > MAX_FRAME {
        bail!("Capture worker frame exceeds its limit.");
    }
    output.write_all(&(bytes.len() as u32).to_le_bytes())?;
    output.write_all(&bytes)?;
    output.flush()?;
    Ok(())
}

pub(super) fn apply(
    core: &mut Core,
    snapshot: &Snapshot,
    generation: u64,
    cancelled: bool,
) -> Result<()> {
    if snapshot.status.generation != generation
        || snapshot.events.len() > super::QUEUE_LIMIT
        || snapshot.owned.len() > 256
        || snapshot.down.len() > 256
    {
        bail!("Invalid capture worker state.");
    }
    if snapshot.events.iter().any(|event| match event {
        Event::Switch { generation: g, .. }
        | Event::Learned { generation: g, .. }
        | Event::Stopped { generation: g, .. } => *g != generation,
    }) {
        bail!("Invalid capture worker event generation.");
    }
    if core.status.generation != generation {
        return Ok(());
    }
    core.physical = snapshot.owned.iter().cloned().collect();
    core.down = snapshot.down.clone();
    if !cancelled && !core.worker_failed && core.status.mode != Mode::Off {
        core.status = snapshot.status;
        core.native_lost = snapshot.native_lost;
        if snapshot.status.mode == Mode::Off {
            core.events.clear();
        }
        for event in &snapshot.events {
            core.emit(event.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::switch_input::{Action, StopReason};

    fn snapshot() -> Snapshot {
        Snapshot {
            status: Status {
                generation: 9,
                mode: Mode::Active,
                reason: None,
            },
            down: HashSet::from(["Space".into()]),
            owned: vec!["Space".into()],
            native_lost: false,
            recovering: false,
            finished: false,
            events: vec![Event::Switch {
                generation: 9,
                switch_id: "one".into(),
                action: Action::Pressed,
                monotonic_ms: 60_000,
            }],
        }
    }
    #[test]
    fn worker_clock_preserves_long_parent_uptime_and_includes_launch_delay() {
        assert_eq!(
            clock_age(86_400_000, 900_000_000, 900_000_250).unwrap(),
            86_400_250
        );
        assert!(clock_age(20, 10, 30).is_err());
        assert!(clock_age(10, 30, 20).is_err());
        assert_eq!(clock_age(0, 0, u64::MAX).unwrap(), u64::MAX);
    }
    #[test]
    fn worker_death_latches_failure_even_before_ownership_was_reported() {
        for reported in [false, true] {
            let mut core = Core::default();
            core.begin(Mode::Active, 0);
            core.status.generation = 9;
            if reported {
                apply(&mut core, &snapshot(), 9, false).unwrap();
            }
            worker_failed(&mut core);
            assert!(core.worker_failed);
            assert_eq!(core.status.mode, Mode::Off);
            assert_eq!(core.physical.contains("Space"), reported);
            apply(&mut core, &snapshot(), 9, false).unwrap();
            assert_eq!(core.status.mode, Mode::Off);
            assert!(!core
                .events
                .iter()
                .any(|e| matches!(e, Event::Switch { .. })));
            core.stop(StopReason::Disabled);
            assert!(core.worker_failed);
        }
    }
    #[test]
    fn failure_during_shutdown_or_startup_cannot_rearm_capture() {
        let mut core = Core::default();
        core.status.generation = 8;
        assert!(!core.worker_failed);
        worker_failed(&mut core);
        assert!(ready(&mut core, &snapshot(), 9, Mode::Active, 60_000).is_err());
        assert_eq!(core.status.mode, Mode::Off);
        assert_eq!(core.status.generation, 8);
        assert!(core.worker_failed);
    }
    #[test]
    fn bounded_frames_reject_truncation_and_oversized_input_before_allocation() {
        assert!(read::<Poll>(&mut &u32::MAX.to_le_bytes()[..]).is_err());
        let mut bytes = Vec::new();
        write(&mut bytes, &snapshot()).unwrap();
        let restored: Snapshot = read(&mut bytes.as_slice()).unwrap();
        assert_eq!(restored.events, snapshot().events);
        bytes.pop();
        assert!(read::<Snapshot>(&mut bytes.as_slice()).is_err());
    }
    #[test]
    fn cancellation_preserves_owned_releases_without_reactivating_or_delivering_actions() {
        let mut core = Core::default();
        core.status.generation = 9;
        core.stop(StopReason::Disabled);
        apply(&mut core, &snapshot(), 9, true).unwrap();
        assert_eq!(core.status.mode, Mode::Off);
        assert!(core.physical.contains("Space"));
        assert!(core
            .events
            .iter()
            .all(|e| !matches!(e, Event::Switch { .. })));
        let mut released = snapshot();
        released.owned.clear();
        apply(&mut core, &released, 9, true).unwrap();
        assert!(core.physical.is_empty());
    }
    #[test]
    fn stale_or_mixed_generations_cannot_modify_the_current_session() {
        let mut core = Core::default();
        core.status.generation = 10;
        apply(&mut core, &snapshot(), 9, false).unwrap();
        assert!(core.physical.is_empty());
        let mut mixed = snapshot();
        mixed.events.push(Event::Stopped {
            generation: 8,
            reason: StopReason::Escape,
        });
        assert!(apply(&mut core, &mixed, 9, false).is_err());
    }
    #[test]
    fn stopped_snapshot_cancels_queued_actions_and_retains_parent_clock_values() {
        let mut core = Core::default();
        core.begin(Mode::Active, 59_000);
        core.status.generation = 9;
        apply(&mut core, &snapshot(), 9, false).unwrap();
        assert_eq!(core.events.front(), snapshot().events.first());
        let mut failed = snapshot();
        failed.status.mode = Mode::Off;
        failed.status.reason = Some(StopReason::CaptureLost);
        failed.events = vec![Event::Stopped {
            generation: 9,
            reason: StopReason::CaptureLost,
        }];
        apply(&mut core, &failed, 9, false).unwrap();
        assert_eq!(core.events.len(), 1);
        assert_eq!(core.events.front(), failed.events.first());
    }
}
