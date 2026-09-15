use super::{Core, Driver, Mode, StopReason, HEARTBEAT_TIMEOUT_MS};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering::*};

const CAPACITY: usize = 256;
const ESCAPE: usize = 27;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Edge {
    pub generation: u64,
    pub code: u8,
    pub pressed: bool,
    pub ms: u64,
}
struct Slot {
    data: AtomicU64,
    ms: AtomicU64,
}
pub(super) struct Shared {
    pub enabled: AtomicBool,
    pub shutdown: AtomicBool,
    pub owned: [AtomicBool; 256],
    pub worker_ms: AtomicU64,
    fault: AtomicU8,
    generation: u64,
    slots: [Slot; CAPACITY],
    read: AtomicUsize,
    write: AtomicUsize,
}
impl Shared {
    pub fn new(generation: u64, now: u64) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            shutdown: AtomicBool::new(false),
            owned: std::array::from_fn(|_| AtomicBool::new(false)),
            worker_ms: AtomicU64::new(now),
            fault: AtomicU8::new(0),
            generation,
            slots: std::array::from_fn(|_| Slot {
                data: AtomicU64::new(0),
                ms: AtomicU64::new(0),
            }),
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
        }
    }
    pub fn has_owned(&self) -> bool {
        self.owned.iter().any(|key| key.load(Acquire))
    }
    pub fn cancel(&self) {
        self.enabled.store(false, Release);
    }
    pub fn fail(&self, reason: StopReason) {
        let value = match reason {
            StopReason::Escape => 1,
            StopReason::QueueOverflow => 2,
            StopReason::HeartbeatTimeout => 3,
            _ => 4,
        };
        let _ = self.fault.compare_exchange(0, value, AcqRel, Acquire);
        self.cancel();
    }
    fn push(&self, code: u8, pressed: bool, ms: u64) {
        let write = self.write.load(Relaxed);
        if write.wrapping_sub(self.read.load(Acquire)) >= CAPACITY {
            self.fail(StopReason::QueueOverflow);
            return;
        }
        let slot = &self.slots[write % CAPACITY];
        slot.data
            .store(code as u64 | ((pressed as u64) << 8), Relaxed);
        slot.ms.store(ms, Relaxed);
        self.write.store(write.wrapping_add(1), Release);
    }
    fn pop(&self) -> Option<Edge> {
        let read = self.read.load(Relaxed);
        if read == self.write.load(Acquire) {
            return None;
        }
        let slot = &self.slots[read % CAPACITY];
        let data = slot.data.load(Relaxed);
        let edge = Edge {
            generation: self.generation,
            code: data as u8,
            pressed: data & 256 != 0,
            ms: slot.ms.load(Relaxed),
        };
        self.read.store(read.wrapping_add(1), Release);
        Some(edge)
    }
    fn drain_into(&self, core: &mut Core, names: &[Option<String>; 256], now: u64) {
        if core.status.generation != self.generation {
            self.cancel();
            while self.pop().is_some() {}
            return;
        }
        let fault = self.fault.swap(0, AcqRel);
        if fault != 0 && core.status.generation == self.generation {
            let reason = match fault {
                1 => StopReason::Escape,
                2 => StopReason::QueueOverflow,
                3 => StopReason::HeartbeatTimeout,
                _ => StopReason::CaptureLost,
            };
            if reason == StopReason::CaptureLost {
                core.native_lost = true;
            }
            core.stop(reason);
        }
        if core.status.mode == Mode::Off {
            self.cancel();
        }
        for _ in 0..CAPACITY {
            let Some(edge) = self.pop() else {
                break;
            };
            if self.enabled.load(Acquire)
                && edge.generation == core.status.generation
                && core.status.mode != Mode::Off
            {
                if let Some(name) = &names[edge.code as usize] {
                    core.key(name, edge.pressed, edge.ms);
                }
            }
        }
        core.tick(now);
        if core.status.mode == Mode::Off {
            self.cancel();
            core.physical.clear();
            for (code, name) in names.iter().enumerate() {
                if self.owned[code].load(Acquire) {
                    if let Some(name) = name {
                        core.physical.insert(name.clone());
                    }
                }
            }
        }
        self.worker_ms.store(now, Release);
    }
    pub fn dispatch(&self, driver: &Driver, names: &[Option<String>; 256]) {
        let mut core = driver.core.lock().unwrap_or_else(|p| p.into_inner());
        self.drain_into(&mut core, names, driver.now());
    }
}

pub(super) struct State {
    active: [bool; 256],
    down: [bool; 256],
    mode: Mode,
    learned: Option<u8>,
}
impl State {
    pub fn new(mode: Mode, active: [bool; 256], down: [bool; 256]) -> Self {
        Self {
            active,
            down,
            mode,
            learned: None,
        }
    }
    pub fn refresh_key(&mut self, code: usize, down: bool) {
        self.down[code] = down;
    }
    pub fn key(
        &mut self,
        shared: &Shared,
        code: u32,
        pressed: bool,
        generated: bool,
        ms: u64,
    ) -> bool {
        if generated || code >= 256 {
            return false;
        }
        if shared.enabled.load(Acquire)
            && ms.saturating_sub(shared.worker_ms.load(Acquire)) >= HEARTBEAT_TIMEOUT_MS
        {
            shared.fail(StopReason::HeartbeatTimeout);
        }
        let code = code as usize;
        let was_down = self.down[code];
        self.down[code] = pressed;
        let owned = shared.owned[code].load(Acquire);
        if !shared.enabled.load(Acquire) || self.mode == Mode::Off {
            if !pressed {
                shared.owned[code].store(false, Release);
            }
            return owned;
        }
        if !self.active[code] && code != ESCAPE {
            return false;
        }
        if pressed && was_down {
            return owned;
        }
        if !pressed && !owned {
            shared.push(code as u8, false, ms);
            return false;
        }
        shared.owned[code].store(pressed, Release);
        if code == ESCAPE {
            if pressed {
                shared.fail(StopReason::Escape);
            }
            return true;
        }
        shared.push(code as u8, pressed, ms);
        if self.mode == Mode::Learning {
            if pressed && self.learned.is_none() {
                self.learned = Some(code as u8);
            }
            if !pressed && self.learned == Some(code as u8) {
                self.mode = Mode::Off;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup(mode: Mode) -> (State, Shared, Core, [Option<String>; 256]) {
        let mut core = Core::default();
        core.mappings.insert("Space".into(), "select".into());
        core.begin(mode, 0);
        let mut names = std::array::from_fn(|_| None);
        names[32] = Some("Space".into());
        names[13] = Some("Enter".into());
        names[27] = Some("Escape".into());
        let mut active = [false; 256];
        active[32] = true;
        active[13] = mode == Mode::Learning;
        (
            State::new(mode, active, [false; 256]),
            Shared::new(core.status.generation, 0),
            core,
            names,
        )
    }
    #[test]
    fn consumes_edges_and_repeats_but_not_unrelated_or_generated_input() {
        let (mut state, shared, mut core, names) = setup(Mode::Active);
        assert!(!state.key(&shared, 16, true, false, 1));
        assert!(state.key(&shared, 32, true, false, 2));
        assert!(!state.key(&shared, 32, false, true, 3));
        assert!(state.key(&shared, 32, true, false, 4));
        assert!(state.key(&shared, 32, false, false, 5));
        assert!(!state.key(&shared, 13, true, false, 6));
        shared.drain_into(&mut core, &names, 7);
        assert_eq!(core.events.len(), 2);
        assert!(matches!(
            core.events[0],
            super::super::Event::Switch {
                action: super::super::Action::Pressed,
                monotonic_ms: 2,
                ..
            }
        ));
        assert!(matches!(
            core.events[1],
            super::super::Event::Switch {
                action: super::super::Action::Released,
                monotonic_ms: 5,
                ..
            }
        ));
    }
    #[test]
    fn cancellation_and_escape_discard_pending_actions_and_drain_consumed_keys() {
        for escape in [true, false] {
            let (mut state, shared, mut core, names) = setup(Mode::Active);
            assert!(state.key(&shared, 32, true, false, 1));
            if escape {
                assert!(state.key(&shared, 27, true, false, 2));
            } else {
                shared.cancel();
                core.stop(StopReason::Disabled);
            }
            shared.drain_into(&mut core, &names, 3);
            assert_eq!(core.events.len(), 1);
            assert!(state.key(&shared, 32, true, false, 4));
            assert!(state.key(&shared, 32, false, false, 5));
            if escape {
                assert!(state.key(&shared, 27, false, false, 6));
            }
            shared.drain_into(&mut core, &names, 7);
            assert!(!shared.has_owned());
            assert!(core.physical.is_empty());
            assert!(!state.key(&shared, 32, true, false, 8));
        }
    }
    #[test]
    fn learning_ignores_preheld_keys_and_finishes_only_on_matching_release() {
        let (mut state, shared, mut core, names) = setup(Mode::Learning);
        state.down[32] = true;
        core.down.insert("Space".into());
        assert!(!state.key(&shared, 32, true, false, 1));
        assert!(!state.key(&shared, 32, false, false, 2));
        assert!(state.key(&shared, 13, true, false, 3));
        assert!(state.key(&shared, 13, false, false, 4));
        assert!(!state.key(&shared, 32, true, false, 5));
        shared.drain_into(&mut core, &names, 6);
        assert!(matches!(&core.events[0],super::super::Event::Learned{code,..} if code=="Enter"));
    }
    #[test]
    fn overflow_stale_generations_and_worker_stalls_cannot_execute_actions() {
        let (mut state, shared, mut core, names) = setup(Mode::Active);
        for i in 0..=CAPACITY {
            shared.push(32, i % 2 == 0, i as u64);
        }
        shared.drain_into(&mut core, &names, 500);
        assert_eq!(core.status.reason, Some(StopReason::QueueOverflow));
        assert_eq!(core.events.len(), 1);
        let (_, shared, mut core, names) = setup(Mode::Active);
        shared.push(32, true, 1);
        core.begin(Mode::Active, 2);
        shared.drain_into(&mut core, &names, 3);
        assert!(core.events.is_empty());
        let (_, shared, mut core, names) = setup(Mode::Active);
        assert!(!state.key(&shared, 32, true, false, 2000));
        shared.drain_into(&mut core, &names, 2001);
        assert_eq!(core.status.reason, Some(StopReason::HeartbeatTimeout));
    }
    #[test]
    fn emergency_hold_stops_without_releasing_an_action_or_leaking_the_release() {
        let (mut state, shared, mut core, names) = setup(Mode::Active);
        assert!(state.key(&shared, 32, true, false, 1));
        shared.drain_into(&mut core, &names, 2);
        for now in (500..=4500).step_by(500) {
            core.last_heartbeat = now;
            shared.drain_into(&mut core, &names, now);
        }
        assert_eq!(core.status.reason, Some(StopReason::HoldEscape));
        assert!(matches!(
            core.events.front(),
            Some(super::super::Event::Stopped { .. })
        ));
        assert_eq!(core.events.len(), 1);
        assert!(state.key(&shared, 32, true, false, 4501));
        assert!(state.key(&shared, 32, false, false, 4502));
        shared.drain_into(&mut core, &names, 4503);
        assert!(core.physical.is_empty());
        assert_eq!(core.events.len(), 1);
    }
    #[test]
    fn preheld_learning_key_can_be_learned_after_its_passed_release() {
        let (mut state, shared, mut core, names) = setup(Mode::Learning);
        state.refresh_key(32, true);
        core.down.insert("Space".into());
        assert!(!state.key(&shared, 32, true, false, 1));
        assert!(!state.key(&shared, 32, false, false, 2));
        assert!(state.key(&shared, 32, true, false, 3));
        assert!(state.key(&shared, 32, false, false, 4));
        shared.drain_into(&mut core, &names, 5);
        assert!(
            matches!(&core.events[0], super::super::Event::Learned { code, .. } if code == "Space")
        );
    }
    #[test]
    fn bounded_queue_preserves_edges_across_threads_and_wraparound() {
        let shared = std::sync::Arc::new(Shared::new(17, 0));
        let producer = shared.clone();
        let thread = std::thread::spawn(move || {
            for ms in 0..10000 {
                while producer
                    .write
                    .load(Acquire)
                    .wrapping_sub(producer.read.load(Acquire))
                    >= CAPACITY
                {
                    std::thread::yield_now();
                }
                producer.push(32, ms % 2 == 0, ms);
            }
        });
        for ms in 0..10000 {
            let edge = loop {
                if let Some(edge) = shared.pop() {
                    break edge;
                }
                std::thread::yield_now();
            };
            assert_eq!(
                edge,
                Edge {
                    generation: 17,
                    code: 32,
                    pressed: ms % 2 == 0,
                    ms
                }
            );
        }
        thread.join().unwrap();
        assert!(shared.enabled.load(Acquire));
        assert!(shared.pop().is_none());
    }
    #[test]
    fn native_loss_cancels_and_owned_keys_remain_suppressed_until_release() {
        let (mut state, shared, mut core, names) = setup(Mode::Active);
        state.key(&shared, 32, true, false, 1);
        shared.fail(StopReason::CaptureLost);
        shared.drain_into(&mut core, &names, 2);
        assert!(core.native_lost);
        assert!(state.key(&shared, 32, false, false, 3));
        shared.drain_into(&mut core, &names, 4);
        assert!(!shared.has_owned());
    }
}
