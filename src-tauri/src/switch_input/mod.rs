//! App-owned capture and generation-tagged switch edges.
//! State and macOS adapter adapted from usahp-core under LICENSE-MIT in this directory.
//! Native callbacks do bounded state work only; consumers drain typed events.
mod keys;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(any(target_os = "windows", test))]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod windows_hook;
#[cfg(target_os = "windows")]
mod windows_worker;
#[cfg(any(target_os = "windows", test))]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod windows_worker_protocol;
use anyhow::{bail, Result};
pub use keys::normalize as normalize_key;
pub fn prediction_key_code(name: &str) -> Option<u32> {
    #[cfg(target_os = "windows")]
    {
        windows::code_for_name(name)
    }
    #[cfg(target_os = "macos")]
    {
        macos::code_for_name(name).map(u32::from)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = name;
        None
    }
}
#[cfg(target_os = "windows")]
pub fn run_worker_from_args() -> bool {
    windows_worker::run_from_args()
}
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Instant,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Pressed,
    Released,
}
pub struct Mapping {
    pub id: String,
    pub code: String,
}

pub const HEARTBEAT_INTERVAL_MS: u64 = 500;
pub const HEARTBEAT_TIMEOUT_MS: u64 = 1500;
const QUEUE_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StopReason {
    Disabled,
    Escape,
    HoldEscape,
    HeartbeatTimeout,
    QueueOverflow,
    CaptureLost,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Event {
    Switch {
        generation: u64,
        switch_id: String,
        action: Action,
        monotonic_ms: u64,
    },
    Learned {
        generation: u64,
        code: String,
    },
    Stopped {
        generation: u64,
        reason: StopReason,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    Off,
    Learning,
    Active,
}
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Status {
    pub generation: u64,
    pub mode: Mode,
    pub reason: Option<StopReason>,
}
#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
struct Core {
    #[cfg(any(target_os = "windows", test))]
    worker_failed: bool,
    status: Status,
    native_lost: bool,
    mappings: HashMap<String, String>,
    physical: HashSet<String>,
    down: HashSet<String>,
    held: HashMap<String, u64>,
    learned: Option<String>,
    escape_ms: u64,
    last_heartbeat: u64,
    events: VecDeque<Event>,
}
impl Default for Core {
    fn default() -> Self {
        Self {
            #[cfg(any(target_os = "windows", test))]
            worker_failed: false,
            status: Status {
                generation: 0,
                mode: Mode::Off,
                reason: None,
            },
            native_lost: false,
            mappings: HashMap::new(),
            physical: HashSet::new(),
            down: HashSet::new(),
            held: HashMap::new(),
            learned: None,
            escape_ms: 4000,
            last_heartbeat: 0,
            events: VecDeque::new(),
        }
    }
}
#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
impl Core {
    fn stop(&mut self, reason: StopReason) {
        self.status.mode = Mode::Off;
        self.status.reason = Some(reason);
        self.held.clear();
        self.learned = None;
        // Discard pending edges atomically. Reset releases are never physical edges.
        self.events.clear();
        self.events.push_back(Event::Stopped {
            generation: self.status.generation,
            reason,
        });
    }
    fn begin(&mut self, mode: Mode, now: u64) {
        self.stop(StopReason::Disabled);
        self.events.clear();
        self.status.generation = self.status.generation.wrapping_add(1);
        self.status.mode = mode;
        self.status.reason = None;
        self.last_heartbeat = now;
    }
    fn reconcile_pressed_keys(&mut self, down: HashSet<String>) {
        self.physical.retain(|key| down.contains(key));
        self.down = down;
    }
    fn begin_with_pressed_keys(
        &mut self,
        mode: Mode,
        now: u64,
        down: HashSet<String>,
    ) -> Result<u64> {
        if self.status.mode != Mode::Off {
            bail!("Capture is already active.");
        }
        if self.native_lost {
            bail!("Native switch capture was lost during startup.");
        }
        self.reconcile_pressed_keys(down);
        let mut blocking = self.physical.iter().cloned().collect::<HashSet<_>>();
        blocking.extend(
            self.down
                .iter()
                .filter(|key| {
                    key.as_str() == "Escape"
                        || (mode == Mode::Active && self.mappings.contains_key(*key))
                })
                .cloned(),
        );
        if !blocking.is_empty() {
            let mut keys = blocking.into_iter().collect::<Vec<_>>();
            keys.sort();
            bail!(
                "Release held keys before starting capture: {}.",
                keys.join(", ")
            );
        }
        self.begin(mode, now);
        Ok(self.status.generation)
    }
    fn emit(&mut self, event: Event) {
        if self.events.len() >= QUEUE_LIMIT {
            self.stop(StopReason::QueueOverflow);
        } else {
            self.events.push_back(event);
        }
    }
    fn tick(&mut self, now: u64) {
        if self.status.mode == Mode::Off {
            return;
        }
        if now.saturating_sub(self.last_heartbeat) >= HEARTBEAT_TIMEOUT_MS {
            self.stop(StopReason::HeartbeatTimeout);
            return;
        }
        if self.status.mode == Mode::Active
            && self
                .held
                .values()
                .any(|start| now.saturating_sub(*start) >= self.escape_ms)
        {
            self.stop(StopReason::HoldEscape);
        }
    }
    fn key(&mut self, code: &str, pressed: bool, now: u64) -> bool {
        self.tick(now);
        let was_down = if pressed {
            !self.down.insert(code.into())
        } else {
            self.down.remove(code)
        };
        // Drain releases/repeats of consumed keys, even after cancellation.
        if self.status.mode == Mode::Off {
            return if pressed {
                self.physical.contains(code)
            } else {
                self.physical.remove(code)
            };
        }
        if code == "Escape" {
            if pressed {
                self.physical.insert(code.into());
                self.stop(StopReason::Escape);
            } else {
                self.physical.remove(code);
            }
            return true;
        }
        if self.status.mode == Mode::Learning {
            if pressed {
                if was_down {
                    return self.physical.contains(code);
                }
                self.physical.insert(code.into());
                if self.learned.is_none() {
                    self.learned = Some(code.into());
                }
            } else {
                if !self.physical.remove(code) {
                    return false;
                }
                if self.learned.as_deref() == Some(code) {
                    self.status.mode = Mode::Off;
                    self.learned = None;
                    self.emit(Event::Learned {
                        generation: self.status.generation,
                        code: code.into(),
                    });
                }
            }
            return true;
        }
        let Some(mapping_id) = self.mappings.get(code).cloned() else {
            return false;
        };
        if pressed {
            if was_down {
                return self.physical.contains(code);
            }
            if !self.physical.insert(code.into()) {
                return true;
            }
        } else if !self.physical.remove(code) {
            return false;
        }
        let action = if pressed {
            Action::Pressed
        } else {
            Action::Released
        };
        if pressed {
            self.held.insert(mapping_id.clone(), now);
        } else {
            self.held.remove(&mapping_id);
        }
        self.emit(Event::Switch {
            generation: self.status.generation,
            switch_id: mapping_id,
            action,
            monotonic_ms: now,
        });
        true
    }
}
#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
#[derive(Clone)]
struct Driver {
    core: Arc<Mutex<Core>>,
    started: Instant,
}
#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
impl Driver {
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    fn key(&self, code: &str, pressed: bool) -> bool {
        self.core
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .key(code, pressed, self.now())
    }
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    fn tick(&self) {
        self.core
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .tick(self.now());
    }
    fn lost(&self) {
        let mut core = self.core.lock().unwrap_or_else(|p| p.into_inner());
        core.native_lost = true;
        core.stop(StopReason::CaptureLost);
    }
    fn ready(&self, down: HashSet<String>) {
        let mut core = self.core.lock().unwrap_or_else(|p| p.into_inner());
        core.reconcile_pressed_keys(down);
        core.native_lost = false;
    }
}
/// One owner per process. Creation does not install hooks or capture input.
/// Call heartbeat at least every 500ms, independently of rendering.
/// Configure/learn only while off. Always stop and drain old gestures before re-enabling.
pub struct Capture {
    driver: Driver,
    #[cfg(target_os = "windows")]
    native: Option<windows_worker::Capture>,
    #[cfg(target_os = "macos")]
    native: Option<macos::Capture>,
}
impl Default for Capture {
    fn default() -> Self {
        Self::new()
    }
}
impl Capture {
    pub fn new() -> Self {
        Self {
            driver: Driver {
                core: Arc::new(Mutex::new(Core::default())),
                started: Instant::now(),
            },
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            native: None,
        }
    }
    pub fn status(&self) -> Status {
        self.driver
            .core
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .status
    }
    pub fn heartbeat(&self) {
        #[cfg(target_os = "windows")]
        if let Some(native) = &self.native {
            native.check_health();
        }
        let mut core = self.driver.core.lock().unwrap_or_else(|p| p.into_inner());
        core.last_heartbeat = self.driver.now();
    }
    pub fn drain(&self) -> Vec<Event> {
        let mut core = self.driver.core.lock().unwrap_or_else(|p| p.into_inner());
        core.tick(self.driver.now());
        core.events.drain(..).collect()
    }
    pub fn configure(&mut self, mappings: &[Mapping], escape_ms: u64) -> Result<()> {
        if self.status().mode != Mode::Off {
            bail!("Disable capture before changing mappings.");
        }
        if mappings.len() > 128 || escape_ms < 4000 {
            bail!("Invalid local mapping count or escape duration.");
        }
        let mut keys = HashMap::new();
        let mut ids = HashSet::new();
        for m in mappings {
            if m.id.trim().is_empty() || !ids.insert(m.id.clone()) {
                bail!("Invalid local keyboard mapping.");
            }
            let Some(code) = normalize_key(&m.code) else {
                bail!("Unsupported switch key: {}", m.code);
            };
            if code == "Escape" || !supported_key(&code) {
                bail!("Switch key is reserved or unavailable on this platform: {code}");
            }
            if keys.insert(code, m.id.clone()).is_some() {
                bail!("Each physical key can be mapped only once.");
            }
        }
        let mut core = self.driver.core.lock().unwrap_or_else(|p| p.into_inner());
        core.mappings = keys;
        core.escape_ms = escape_ms;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    fn ensure_native(&mut self) -> Result<()> {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        if self
            .driver
            .core
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .native_lost
        {
            self.native.take();
        }
        #[cfg(target_os = "macos")]
        {
            if self.native.is_none() {
                self.native = Some(macos::Capture::start(self.driver.clone())?);
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            bail!("Local keyboard capture is supported on Windows and macOS.");
        }
    }
    fn begin(&mut self, mode: Mode) -> Result<u64> {
        if self.status().mode != Mode::Off {
            bail!("Capture is already active.");
        }
        #[cfg(target_os = "windows")]
        {
            if self
                .driver
                .core
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .worker_failed
            {
                bail!("{}", windows_worker_protocol::WORKER_FAILURE_MESSAGE);
            }
            if self
                .native
                .as_ref()
                .is_some_and(windows_worker::Capture::recovering)
            {
                bail!("{}", windows_hook::RECOVERY_MESSAGE);
            }
            let mut held = self
                .driver
                .core
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .physical
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            if let Some(native) = &self.native {
                held.extend(native.held_keys());
            }
            if !held.is_empty() {
                held.sort();
                held.dedup();
                bail!(
                    "Release held keys before starting capture: {}.",
                    held.join(", ")
                );
            }
            if let Some(native) = &self.native {
                native.await_shutdown()?;
            }
            self.native.take();
            self.native = Some(windows_worker::Capture::start(self.driver.clone(), mode)?);
            Ok(self.status().generation)
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.ensure_native()?;
            let mut core = self.driver.core.lock().unwrap_or_else(|p| p.into_inner());
            #[cfg(target_os = "macos")]
            let down = macos::pressed_keys();
            #[cfg(not(target_os = "macos"))]
            let down = core.down.clone();
            core.begin_with_pressed_keys(mode, self.driver.now(), down)
        }
    }
    pub fn enable(&mut self) -> Result<u64> {
        self.begin(Mode::Active)
    }
    pub fn learn(&mut self) -> Result<u64> {
        self.begin(Mode::Learning)
    }
    pub fn monotonic_ms(&self) -> u64 {
        self.driver.now()
    }
    pub fn shutdown(&mut self) {
        self.stop();
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            self.native.take();
        }
        let mut core = self.driver.core.lock().unwrap_or_else(|p| p.into_inner());
        core.physical.clear();
        core.down.clear();
    }
    pub fn stop(&mut self) {
        #[cfg(target_os = "windows")]
        if let Some(native) = &self.native {
            native.cancel();
        }
        self.driver
            .core
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .stop(StopReason::Disabled);
    }
    pub fn stop_for_recovery(&mut self) {
        #[cfg(target_os = "windows")]
        if let Some(native) = &self.native {
            native.cancel_for_recovery();
        }
        self.driver
            .core
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .stop(StopReason::Disabled);
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            self.native.take();
        }
    }
}
pub fn supported_key(code: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        code != "F12" && windows::code_for_name(code).is_some()
    }
    #[cfg(target_os = "macos")]
    {
        macos::code_for_name(code).is_some()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = code;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn core() -> Core {
        let mut c = Core::default();
        let m = Mapping {
            id: "space".into(),
            code: "Space".into(),
        };
        c.mappings.insert("Space".into(), "space".into());
        c.mappings.insert(m.code, m.id);
        c.begin(Mode::Active, 0);
        c
    }
    #[test]
    fn repeat_and_unmatched_release_never_activate() {
        let mut c = core();
        assert!(!c.key("Space", false, 0));
        assert!(c.events.is_empty());
        c.key("Space", true, 1);
        c.key("Space", true, 2);
        c.key("Space", false, 3);
        assert_eq!(c.events.len(), 2);
        assert!(!c.key("A", true, 4));
    }
    #[test]
    fn cancellation_discards_edges_without_synthetic_releases() {
        let mut c = core();
        c.key("Space", true, 1);
        c.stop(StopReason::Disabled);
        assert!(matches!(c.events.front(), Some(Event::Stopped { .. })));
        assert_eq!(c.events.len(), 1);
        assert!(c.key("Space", false, 2));
        assert_eq!(c.events.len(), 1);
    }
    #[test]
    fn heartbeat_and_long_hold_fail_open() {
        let mut c = core();
        c.key("Space", true, 0);
        c.tick(1500);
        assert_eq!(c.status.reason, Some(StopReason::HeartbeatTimeout));
        let mut c = core();
        c.escape_ms = 6000;
        c.key("Space", true, 0);
        for n in 1..=12 {
            c.last_heartbeat = n * 500;
            c.tick(n * 500);
        }
        assert_eq!(c.status.reason, Some(StopReason::HoldEscape));
        assert!(!c.key("A", true, 6001));
    }
    #[test]
    fn learns_only_complete_press_and_escape_cancels() {
        let mut c = Core::default();
        c.begin(Mode::Learning, 0);
        c.key("Space", false, 1);
        assert!(c.events.is_empty());
        c.key("Space", true, 2);
        c.key("Space", true, 3);
        c.key("Space", false, 4);
        assert!(matches!(c.events.front(),Some(Event::Learned{code,..}) if code=="Space"));
        c.begin(Mode::Learning, 5);
        c.key("Escape", true, 6);
        assert_eq!(c.status.reason, Some(StopReason::Escape));
    }
    #[test]
    fn overflow_cancels_instead_of_delivering_partial_gesture() {
        let mut c = core();
        for n in 0..300 {
            c.key("Space", n % 2 == 0, n);
        }
        assert_eq!(c.status.reason, Some(StopReason::QueueOverflow));
        assert_eq!(c.events.len(), 1);
        assert!(matches!(c.events[0], Event::Stopped { .. }));
    }
    #[test]
    fn learning_drains_overlapping_keys() {
        let mut c = Core::default();
        c.begin(Mode::Learning, 0);
        for (code, down) in [("A", true), ("B", true), ("B", false), ("A", false)] {
            assert!(c.key(code, down, 1));
        }
        assert!(c.physical.is_empty());
        assert!(c.down.is_empty());
        assert!(matches!(c.events.front(),Some(Event::Learned{code,..}) if code=="A"));
    }
    #[test]
    fn prior_passed_press_retains_passed_release() {
        let mut c = core();
        c.stop(StopReason::Disabled);
        assert!(!c.key("Space", true, 1));
        c.begin(Mode::Active, 2);
        assert!(!c.key("Space", true, 3));
        assert!(!c.key("Space", false, 4));
        assert!(c.events.is_empty());
    }
    #[test]
    fn cleanup_does_not_erase_native_failure() {
        let driver = Driver {
            core: Arc::new(Mutex::new(Core::default())),
            started: Instant::now(),
        };
        driver.core.lock().unwrap().begin(Mode::Learning, 0);
        driver.key("Space", true);
        driver.lost();
        driver.core.lock().unwrap().stop(StopReason::Disabled);
        assert!(driver.core.lock().unwrap().native_lost);
        driver.ready(HashSet::new());
        assert!(!driver.core.lock().unwrap().native_lost);
        assert!(driver.core.lock().unwrap().down.is_empty());
        assert!(driver.core.lock().unwrap().physical.is_empty());
    }
    #[test]
    fn retry_after_missed_release_accepts_a_fresh_learning_gesture() {
        let mut c = Core::default();
        c.begin(Mode::Learning, 0);
        c.key("Space", true, 1);
        c.stop(StopReason::Disabled);
        let generation = c
            .begin_with_pressed_keys(Mode::Learning, 2, HashSet::new())
            .unwrap();
        assert!(c.events.is_empty());
        assert!(!c.key("Space", false, 3));
        assert!(c.events.is_empty());
        assert!(c.key("Space", true, 4));
        assert!(c.key("Space", false, 5));
        assert!(
            matches!(c.events.front(), Some(Event::Learned { generation: g, code }) if *g == generation && code == "Space")
        );
    }
    #[test]
    fn refreshed_state_still_blocks_held_keys_until_release() {
        for mode in [Mode::Learning, Mode::Active] {
            let mut c = Core::default();
            c.mappings.insert("Space".into(), "select".into());
            let down = HashSet::from(["Escape".to_string()]);
            assert!(c.begin_with_pressed_keys(mode, 0, down).is_err());
            assert_eq!(c.status.mode, Mode::Off);
            assert!(c.events.is_empty());
            assert!(c.begin_with_pressed_keys(mode, 1, HashSet::new()).is_ok());
            assert_eq!(c.status.mode, mode);
        }
    }
    #[test]
    fn unrelated_held_key_does_not_block_assigned_switch() {
        let mut c = Core::default();
        c.mappings.insert("Space".into(), "select".into());
        c.begin_with_pressed_keys(Mode::Active, 0, HashSet::from(["A".into()]))
            .unwrap();
        assert!(!c.key("A", true, 1));
        assert!(!c.key("A", false, 2));
        assert!(c.events.is_empty());
        assert!(c.key("Space", true, 3));
        assert!(c.key("Space", false, 4));
        assert_eq!(c.events.len(), 2);
        assert!(matches!(
            c.events.pop_front(),
            Some(Event::Switch {
                action: Action::Pressed,
                ..
            })
        ));
        assert!(matches!(
            c.events.pop_front(),
            Some(Event::Switch {
                action: Action::Released,
                ..
            })
        ));
    }
    #[test]
    fn assigned_held_keys_are_named_and_block_startup() {
        let mut c = Core::default();
        c.mappings.insert("Space".into(), "select".into());
        let error = c
            .begin_with_pressed_keys(
                Mode::Active,
                0,
                HashSet::from(["Space".into(), "Escape".into(), "A".into()]),
            )
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Release held keys before starting capture: Escape, Space."
        );
        assert_eq!(c.status.mode, Mode::Off);
        assert!(c.events.is_empty());
        c.begin_with_pressed_keys(Mode::Active, 1, HashSet::from(["A".into()]))
            .unwrap();
        assert!(!c.key("Space", false, 2));
        assert!(c.events.is_empty());
    }
    #[test]
    fn learning_ignores_preheld_keys_until_a_fresh_gesture() {
        for fresh in ["Space", "A"] {
            let mut c = Core::default();
            c.begin_with_pressed_keys(Mode::Learning, 0, HashSet::from(["A".into()]))
                .unwrap();
            assert!(!c.key("A", true, 1));
            if fresh == "A" {
                assert!(!c.key("A", false, 2));
            }
            assert!(c.events.is_empty());
            assert!(c.key(fresh, true, 3));
            assert!(c.key(fresh, false, 4));
            assert!(
                matches!(c.events.pop_front(), Some(Event::Learned { code, .. }) if code == fresh)
            );
            assert!(c.events.is_empty());
        }
    }
    #[test]
    fn consumed_keys_must_drain_before_either_mode_restarts() {
        for mode in [Mode::Active, Mode::Learning] {
            let mut c = Core::default();
            c.begin(Mode::Learning, 0);
            assert!(c.key("A", true, 1));
            c.stop(StopReason::Disabled);
            let error = c
                .begin_with_pressed_keys(mode, 2, HashSet::from(["A".into()]))
                .unwrap_err();
            assert!(error.to_string().ends_with("A."));
            assert!(c.key("A", true, 3));
            assert!(c.key("A", false, 4));
            c.begin_with_pressed_keys(mode, 5, HashSet::new()).unwrap();
            assert!(c.events.is_empty());
            assert!(!c.key("A", false, 6));
            assert!(c.events.is_empty());
        }
    }
    #[test]
    fn active_capture_and_native_loss_cannot_be_reset_by_key_refresh() {
        let mut c = core();
        c.key("Space", true, 1);
        assert!(c
            .begin_with_pressed_keys(Mode::Learning, 2, HashSet::new())
            .is_err());
        assert!(c.physical.contains("Space"));
        c.stop(StopReason::CaptureLost);
        c.native_lost = true;
        assert!(c
            .begin_with_pressed_keys(Mode::Learning, 3, HashSet::new())
            .is_err());
        assert_eq!(c.status.reason, Some(StopReason::CaptureLost));
    }
    #[test]
    fn generations_change_and_names_are_stable() {
        let mut c = core();
        let old = c.status.generation;
        c.stop(StopReason::Disabled);
        c.begin(Mode::Active, 1);
        assert_ne!(c.status.generation, old);
        assert_eq!(normalize_key("Return").as_deref(), Some("Enter"));
        assert!(normalize_key("F24").is_some());
        assert!(normalize_key("F25").is_none());
    }
}
