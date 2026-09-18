use super::{Core, Driver, Mode, StopReason, HEARTBEAT_TIMEOUT_MS};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering::*};

pub(super) const RECOVERY_MESSAGE: &str = "Keyboard capture interrupted. Release your switches, then press and release a switch to reconnect.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Resource {
    RawInput,
    Hook,
    Timer,
}
pub(super) trait NativeResources {
    fn acquire(&mut self, resource: Resource) -> bool;
    fn release(&mut self, resource: Resource) -> bool;
    fn replace_hook(&mut self) -> bool {
        false
    }
}
pub(super) struct Installation<'a, T: NativeResources> {
    api: &'a mut T,
    count: usize,
}
impl<'a, T: NativeResources> Installation<'a, T> {
    const ORDER: [Resource; 3] = [Resource::RawInput, Resource::Hook, Resource::Timer];
    pub fn start(api: &'a mut T) -> Option<Self> {
        let mut installation = Self { api, count: 0 };
        for resource in Self::ORDER {
            if !installation.api.acquire(resource) {
                return None;
            }
            installation.count += 1;
        }
        Some(installation)
    }
    pub fn stop(&mut self) -> bool {
        let mut success = true;
        while self.count > 0 {
            self.count -= 1;
            success &= self.api.release(Self::ORDER[self.count]);
        }
        success
    }
    pub fn replace_hook(&mut self) -> bool {
        self.api.replace_hook()
    }
}
impl<T: NativeResources> Drop for Installation<'_, T> {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(super) fn await_shutdown(
    mut poll: impl FnMut() -> anyhow::Result<bool>,
    mut wait: impl FnMut(),
    mut elapsed_ms: impl FnMut() -> u64,
) -> anyhow::Result<()> {
    while elapsed_ms() < 3000 {
        if poll()? {
            return Ok(());
        }
        wait();
    }
    anyhow::bail!("Keyboard capture is finishing. Try again.")
}

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
    pub recovering: AtomicBool,
    pub recovery_cancelled: AtomicBool,
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
            recovering: AtomicBool::new(false),
            recovery_cancelled: AtomicBool::new(false),
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
    pub fn drained(&self) -> bool {
        !self.enabled.load(Acquire) && !self.recovering() && !self.has_owned()
    }
    pub fn recovering(&self) -> bool {
        self.recovering.load(Acquire) && !self.recovery_cancelled.load(Acquire)
    }
    pub fn cancel(&self) {
        self.enabled.store(false, Release);
    }
    pub fn cancel_recovery(&self) {
        self.recovery_cancelled.store(true, Release);
        self.recovering.store(false, Release);
        self.cancel();
    }
    pub fn fail(&self, reason: StopReason) {
        let value = match reason {
            StopReason::Escape => 1,
            StopReason::QueueOverflow => 2,
            StopReason::HeartbeatTimeout => 3,
            StopReason::HoldEscape => 5,
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
                5 => StopReason::HoldEscape,
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
    observations: std::collections::VecDeque<Observation>,
    reinstall_at: Option<u64>,
    quarantine: bool,
    recovery_pressed: [Option<u64>; 256],
    recovered_gesture: bool,
    emergency: bool,
}

#[derive(Clone, Copy)]
struct Observation {
    code: u32,
    pressed: bool,
    raw: bool,
    ms: u64,
}
impl State {
    pub fn new(mode: Mode, active: [bool; 256], down: [bool; 256]) -> Self {
        Self {
            active,
            down,
            mode,
            learned: None,
            observations: std::collections::VecDeque::with_capacity(CAPACITY),
            reinstall_at: None,
            quarantine: false,
            recovery_pressed: [None; 256],
            recovered_gesture: false,
            emergency: false,
        }
    }
    fn watches(&self, shared: &Shared, code: u32) -> bool {
        code < 256
            && (self.active[code as usize] || code as usize == ESCAPE)
            && (shared.enabled.load(Acquire)
                || shared.owned[code as usize].load(Acquire)
                || shared.recovering())
    }
    fn observe(&mut self, shared: &Shared, code: u32, pressed: bool, raw: bool, ms: u64) {
        if !self.watches(shared, code) {
            return;
        }
        if let Some(index) = self.observations.iter().position(|edge| {
            edge.code == code
                && edge.pressed == pressed
                && edge.raw != raw
                && ms.saturating_sub(edge.ms) < HEARTBEAT_TIMEOUT_MS
        }) {
            self.observations.remove(index);
        } else if self.observations.len() == CAPACITY {
            self.interrupted(shared, ms);
        } else {
            self.observations.push_back(Observation {
                code,
                pressed,
                raw,
                ms,
            });
        }
    }
    pub fn raw(&mut self, shared: &Shared, code: u32, pressed: bool, generated: bool, ms: u64) {
        if !generated {
            self.observe(shared, code, pressed, true, ms);
        }
    }
    pub fn interrupted(&mut self, shared: &Shared, ms: u64) {
        self.observations.clear();
        self.quarantine = false;
        self.recovery_pressed.fill(None);
        self.recovered_gesture = false;
        self.emergency = false;
        self.reinstall_at = Some(ms);
        shared
            .recovering
            .store(!shared.recovery_cancelled.load(Acquire), Release);
        shared.fail(StopReason::CaptureLost);
    }
    pub fn maintenance(&mut self, shared: &Shared, ms: u64, escape_ms: u64) -> bool {
        if shared.recovery_cancelled.load(Acquire) {
            shared.recovering.store(false, Release);
            self.quarantine = false;
            self.recovery_pressed.fill(None);
        }
        let lost = self
            .observations
            .iter()
            .any(|edge| edge.raw && ms.saturating_sub(edge.ms) >= HEARTBEAT_TIMEOUT_MS);
        self.observations
            .retain(|edge| ms.saturating_sub(edge.ms) < HEARTBEAT_TIMEOUT_MS);
        if lost {
            self.interrupted(shared, ms);
        }
        if self.quarantine
            && self
                .recovery_pressed
                .iter()
                .flatten()
                .any(|at| ms.saturating_sub(*at) >= escape_ms)
        {
            self.emergency = true;
            self.recovery_pressed.fill(None);
            shared.fail(StopReason::HoldEscape);
        }
        self.reinstall_at.is_some_and(|at| ms >= at)
    }
    pub fn replaced(&mut self, success: bool, ms: u64) {
        if success {
            self.reinstall_at = None;
            self.quarantine = true;
            self.recovery_pressed.fill(None);
            self.observations.clear();
        } else {
            self.reinstall_at = Some(ms.saturating_add(2000));
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
        let consumed = self.physical_key(shared, code, pressed, ms);
        if !consumed {
            self.observe(shared, code, pressed, false, ms);
        }
        consumed
    }
    fn physical_key(&mut self, shared: &Shared, code: u32, pressed: bool, ms: u64) -> bool {
        if shared.recovering() && self.watches(shared, code) {
            let code = code as usize;
            let was_owned = shared.owned[code].swap(pressed, AcqRel);
            if self.quarantine {
                if code == ESCAPE && pressed {
                    self.emergency = true;
                    shared.fail(StopReason::Escape);
                } else if pressed && !was_owned {
                    self.recovery_pressed[code] = Some(ms);
                } else if !pressed {
                    self.recovered_gesture |= self.recovery_pressed[code].take().is_some();
                }
                if (self.recovered_gesture || self.emergency) && !shared.has_owned() {
                    shared.recovering.store(false, Release);
                    self.quarantine = false;
                }
            }
            self.down[code] = pressed;
            return true;
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
        self.consume(shared, code, pressed, ms)
    }
    fn consume(&mut self, shared: &Shared, code: usize, pressed: bool, ms: u64) -> bool {
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
    fn missing_hook_edges_cancel_actions_and_require_a_separate_recovery_gesture() {
        for missed_release in [false, true] {
            let (mut state, shared, mut core, names) = setup(Mode::Active);
            if missed_release {
                assert!(state.key(&shared, 32, true, false, 1));
            }
            state.raw(&shared, 32, !missed_release, false, 2);
            assert!(!state.maintenance(&shared, 1501, 4000));
            assert!(state.maintenance(&shared, 1502, 4000));
            shared.drain_into(&mut core, &names, 1502);
            assert_eq!(core.status.reason, Some(StopReason::CaptureLost));
            assert_eq!(core.events.len(), 1);
            assert!(shared.recovering.load(Acquire));
            assert!(!shared.drained());
            state.replaced(true, 1503);
            assert!(state.key(&shared, 32, false, false, 1504));
            assert!(shared.recovering.load(Acquire));
            assert!(state.key(&shared, 32, true, false, 1505));
            assert!(state.key(&shared, 32, true, false, 1506));
            assert!(state.key(&shared, 32, false, false, 1507));
            assert!(shared.drained());
            shared.drain_into(&mut core, &names, 1508);
            assert_eq!(core.events.len(), 1);
            assert!(core.physical.is_empty());
        }
    }
    #[test]
    fn expected_passes_match_raw_in_either_order_and_generated_input_is_ignored() {
        for raw_first in [true, false] {
            let (mut state, shared, _, _) = setup(Mode::Active);
            state.refresh_key(32, true);
            if raw_first {
                state.raw(&shared, 32, false, false, 1);
            }
            assert!(!state.key(&shared, 32, false, false, 2));
            if !raw_first {
                state.raw(&shared, 32, false, false, 3);
            }
            state.raw(&shared, 13, true, false, 4);
            state.raw(&shared, 32, true, true, 4);
            assert!(!state.key(&shared, 32, true, true, 4));
            assert!(!state.maintenance(&shared, 2000, 4000));
            assert!(!shared.recovering.load(Acquire));
            assert!(state.observations.is_empty());
        }
    }
    #[test]
    fn failed_replacement_retries_without_releasing_ownership_or_actions() {
        let (mut state, shared, mut core, names) = setup(Mode::Active);
        state.key(&shared, 32, true, false, 1);
        state.interrupted(&shared, 2);
        assert!(state.maintenance(&shared, 2, 4000));
        state.replaced(false, 2);
        assert!(!state.maintenance(&shared, 2001, 4000));
        assert!(state.maintenance(&shared, 2002, 4000));
        assert!(shared.owned[32].load(Acquire));
        assert!(state.key(&shared, 32, false, false, 2003));
        assert!(state.key(&shared, 32, true, false, 2004));
        assert!(state.key(&shared, 32, false, false, 2005));
        assert!(shared.recovering.load(Acquire));
        shared.drain_into(&mut core, &names, 2006);
        assert_eq!(core.events.len(), 1);
    }
    #[test]
    fn recovery_waits_for_every_uncertain_key_and_cannot_learn_or_run_a_hold() {
        let (mut state, shared, mut core, names) = setup(Mode::Learning);
        state.key(&shared, 32, true, false, 1);
        state.key(&shared, 13, true, false, 2);
        state.interrupted(&shared, 3);
        shared.drain_into(&mut core, &names, 4);
        state.replaced(true, 5);
        state.key(&shared, 32, false, false, 6);
        state.key(&shared, 32, true, false, 7);
        state.key(&shared, 32, false, false, 8);
        assert!(shared.recovering.load(Acquire));
        state.key(&shared, 13, false, false, 9);
        shared.drain_into(&mut core, &names, 10);
        assert!(shared.drained());
        assert_eq!(core.events.len(), 1);
        assert!(matches!(
            core.events[0],
            super::super::Event::Stopped { .. }
        ));
    }
    #[test]
    fn recovery_escape_and_emergency_hold_do_not_execute_a_switch() {
        for escape in [false, true] {
            let (mut state, shared, mut core, names) = setup(Mode::Active);
            state.interrupted(&shared, 1);
            shared.drain_into(&mut core, &names, 2);
            state.replaced(true, 3);
            let code = if escape { 27 } else { 32 };
            state.key(&shared, code, true, false, 4);
            if !escape {
                state.maintenance(&shared, 4004, 4000);
            }
            state.key(&shared, code, false, false, 4005);
            shared.drain_into(&mut core, &names, 4006);
            assert!(shared.drained());
            assert_eq!(core.events.len(), 1);
            assert_eq!(
                core.status.reason,
                Some(if escape {
                    StopReason::Escape
                } else {
                    StopReason::HoldEscape
                })
            );
        }
    }
    #[test]
    fn cancellation_wins_a_racing_recovery_publication() {
        let (mut state, shared, _, _) = setup(Mode::Active);
        state.interrupted(&shared, 1);
        shared.cancel_recovery();
        shared.recovering.store(true, Release);
        assert!(!shared.recovering());
        assert!(shared.drained());
        assert!(!state.key(&shared, 32, true, false, 2));
        state.maintenance(&shared, 3, 4000);
        assert!(!shared.recovering.load(Acquire));
    }
    #[test]
    fn explicit_cancellation_stops_recovery_and_only_drains_owned_presses() {
        for replaced in [false, true] {
            let (mut state, shared, mut core, names) = setup(Mode::Learning);
            state.key(&shared, 32, true, false, 1);
            state.interrupted(&shared, 2);
            state.replaced(replaced, 3);
            shared.cancel_recovery();
            assert!(!shared.recovering.load(Acquire));
            assert!(!state.key(&shared, 13, true, false, 4));
            assert!(!state.key(&shared, 13, false, false, 5));
            assert!(state.key(&shared, 32, true, false, 6));
            assert!(state.key(&shared, 32, false, false, 7));
            assert!(!state.key(&shared, 32, true, false, 8));
            shared.drain_into(&mut core, &names, 9);
            assert!(shared.drained());
            assert_eq!(core.events.len(), 1);
        }
    }
    #[test]
    fn diagnostic_ledger_is_bounded_and_idle_is_not_hook_health() {
        let (mut state, shared, _, _) = setup(Mode::Active);
        assert!(!state.maintenance(&shared, 100000, 4000));
        for i in 0..=CAPACITY {
            state.raw(&shared, 32, true, false, 100001 + i as u64);
        }
        assert!(state.observations.len() <= CAPACITY);
        assert!(shared.recovering.load(Acquire));
        state.replaced(true, 100500);
        state.raw(&shared, 32, false, false, 100501);
        assert!(state.maintenance(&shared, 102001, 4000));
        assert!(!state.quarantine);
    }
    #[test]
    fn immediate_restart_waits_for_shutdown_but_not_for_held_keys() {
        let polls = std::cell::Cell::new(0);
        let waits = std::cell::Cell::new(0);
        await_shutdown(
            || {
                polls.set(polls.get() + 1);
                Ok(polls.get() == 3)
            },
            || waits.set(waits.get() + 1),
            || 0,
        )
        .unwrap();
        assert_eq!(waits.get(), 2);
        assert!(await_shutdown(
            || anyhow::bail!("Release held keys"),
            || panic!("must report held keys immediately"),
            || 0
        )
        .is_err());
        let waits = std::cell::Cell::new(0);
        assert!(await_shutdown(
            || Ok(false),
            || waits.set(waits.get() + 1),
            || waits.get() * 16
        )
        .is_err());
        assert_eq!(waits.get(), 188);
    }
    #[test]
    fn startup_failure_rolls_back_and_shutdown_attempts_every_cleanup_once() {
        #[derive(Default)]
        struct Fake {
            fail_start: Option<Resource>,
            fail_stop: bool,
            acquired: Vec<Resource>,
            released: Vec<Resource>,
        }
        impl NativeResources for Fake {
            fn acquire(&mut self, resource: Resource) -> bool {
                if self.fail_start == Some(resource) {
                    return false;
                }
                self.acquired.push(resource);
                true
            }
            fn release(&mut self, resource: Resource) -> bool {
                self.released.push(resource);
                !self.fail_stop
            }
        }
        for failure in [
            Some(Resource::RawInput),
            Some(Resource::Hook),
            Some(Resource::Timer),
            None,
        ] {
            let mut fake = Fake {
                fail_start: failure,
                ..Default::default()
            };
            {
                let installation = Installation::start(&mut fake);
                assert_eq!(installation.is_none(), failure.is_some());
            }
            assert_eq!(
                fake.released,
                fake.acquired.iter().copied().rev().collect::<Vec<_>>()
            );
        }
        let mut fake = Fake {
            fail_stop: true,
            ..Default::default()
        };
        {
            let mut installation = Installation::start(&mut fake).unwrap();
            assert!(!installation.stop());
        }
        assert_eq!(
            fake.released,
            vec![Resource::Timer, Resource::Hook, Resource::RawInput]
        );
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
    fn cancellation_during_a_callback_must_be_confirmed_after_callback_completion() {
        let (mut state, shared, mut core, names) = setup(Mode::Active);
        state.down[32] = true;
        shared.cancel();
        core.stop(StopReason::Disabled);
        assert!(shared.drained());
        assert!(state.consume(&shared, 32, true, 1));
        assert!(!shared.drained());
        shared.drain_into(&mut core, &names, 2);
        assert_eq!(core.events.len(), 1);
        assert!(state.key(&shared, 32, true, false, 3));
        assert!(state.key(&shared, 32, false, false, 4));
        assert!(shared.drained());
        shared.drain_into(&mut core, &names, 5);
        assert!(core.physical.is_empty());
    }
    #[test]
    fn cancellation_reconciles_a_release_between_worker_dispatch_and_exit() {
        let (mut state, shared, mut core, names) = setup(Mode::Active);
        state.key(&shared, 32, true, false, 1);
        shared.cancel();
        core.stop(StopReason::Disabled);
        shared.drain_into(&mut core, &names, 2);
        assert!(core.physical.contains("Space"));
        assert!(state.key(&shared, 32, false, false, 3));
        assert!(!shared.has_owned());
        shared.drain_into(&mut core, &names, 4);
        assert!(core
            .begin_with_pressed_keys(Mode::Active, 5, Default::default())
            .is_ok());
    }
    #[test]
    fn unplug_without_release_requires_a_safe_complete_gesture_to_recover() {
        let (mut state, shared, mut core, names) = setup(Mode::Active);
        state.key(&shared, 32, true, false, 1);
        shared.fail(StopReason::CaptureLost);
        shared.drain_into(&mut core, &names, 2);
        assert!(shared.has_owned());
        assert!(state.key(&shared, 32, true, false, 3));
        assert!(state.key(&shared, 32, false, false, 4));
        shared.drain_into(&mut core, &names, 5);
        assert!(!shared.has_owned());
        assert!(core.physical.is_empty());
        assert_eq!(core.events.len(), 1);
        assert!(matches!(
            core.events[0],
            super::super::Event::Stopped {
                reason: StopReason::CaptureLost,
                ..
            }
        ));
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
