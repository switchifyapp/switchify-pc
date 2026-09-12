//! Reusable scanning behavior. No Tauri, display discovery, persistence or input injection.
use serde::{Deserialize, Serialize};

pub const TICK_MS: u64 = 33;
pub const MAX_ELAPSED_MS: u64 = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Select,
    Next,
    Back,
    Pause,
    Reverse,
    Stop,
    Cancel,
}
impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Next => "Next",
            Self::Back => "Previous",
            Self::Pause => "Pause / resume",
            Self::Reverse => "Reverse direction",
            Self::Stop => "Stop scanning",
            Self::Cancel => "Disable scanning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SwitchSettings {
    pub automatic: bool,
    pub select_key: String,
    pub next_key: String,
    pub back_key: String,
    pub pause_key: String,
}
impl Default for SwitchSettings {
    fn default() -> Self {
        Self {
            automatic: true,
            select_key: "Space".into(),
            next_key: "Enter".into(),
            back_key: "Backspace".into(),
            pause_key: "F8".into(),
        }
    }
}
impl SwitchSettings {
    pub fn keys(&self) -> [&str; 5] {
        [
            &self.select_key,
            &self.next_key,
            &self.back_key,
            &self.pause_key,
            "Escape",
        ]
    }
    pub fn validate(&self) -> Result<(), String> {
        let keys = self.keys();
        for (index, key) in keys.iter().enumerate() {
            let function = key
                .strip_prefix('F')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=24).contains(&n));
            if !function
                && ![
                    "Space",
                    "Enter",
                    "Backspace",
                    "Escape",
                    "ArrowUp",
                    "ArrowDown",
                    "ArrowLeft",
                    "ArrowRight",
                ]
                .contains(key)
            {
                return Err("Choose a supported switch key.".into());
            }
            if keys[..index].contains(key) {
                return Err(
                    "Each switch action must use a different key. Escape is reserved for cancel."
                        .into(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|n| n.is_finite())
            && self.width >= 2.0
            && self.height >= 2.0
    }
}
/// Native hosts draw filled strips; techniques can compose lines, outlines and highlights.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Frame {
    pub strips: Vec<Rect>,
}

pub trait Technique {
    type Selection;
    type Phase: Clone + Default + Serialize;
    fn start(&mut self);
    fn advance(&mut self, elapsed_ms: u64);
    fn handle(&mut self, action: Action) -> Option<Self::Selection>;
    fn reset(&mut self);
    fn frame(&self) -> Frame;
    fn phase(&self) -> Self::Phase;
}

pub struct Session<T: Technique> {
    pub technique: T,
    automatic: bool,
    active: bool,
    paused: bool,
}
impl<T: Technique> Session<T> {
    pub fn new(technique: T, automatic: bool) -> Self {
        Self {
            technique,
            automatic,
            active: false,
            paused: false,
        }
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn paused(&self) -> bool {
        self.paused
    }
    pub fn action(&mut self, action: Action) -> Option<T::Selection> {
        if matches!(action, Action::Cancel | Action::Stop) {
            self.reset();
            return None;
        }
        if !self.active {
            if action == Action::Select {
                self.active = true;
                self.technique.start();
            }
            return None;
        }
        if action == Action::Pause {
            self.paused = !self.paused;
            return None;
        }
        let selection = self.technique.handle(action);
        if selection.is_some() {
            self.reset();
        }
        selection
    }
    pub fn tick(&mut self, elapsed_ms: u64, select_held: bool) {
        if self.active && self.automatic && !self.paused && !select_held && elapsed_ms > 0 {
            self.technique.advance(elapsed_ms.min(MAX_ELAPSED_MS));
        }
    }
    pub fn reset(&mut self) {
        self.active = false;
        self.paused = false;
        self.technique.reset();
    }
    pub fn frame(&self) -> Frame {
        if self.active {
            self.technique.frame()
        } else {
            Frame::default()
        }
    }
}

#[derive(Default)]
pub struct Interval {
    elapsed_ms: u64,
}
impl Interval {
    pub fn elapsed(&mut self, delta_ms: u64, period_ms: u64) -> bool {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        if self.elapsed_ms < period_ms.max(1) {
            return false;
        }
        self.elapsed_ms %= period_ms.max(1);
        true
    }
    pub fn reset(&mut self) {
        self.elapsed_ms = 0;
    }
}

#[derive(Debug, Default)]
pub struct Cycle {
    index: usize,
}
impl Cycle {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn step(&mut self, count: usize, forward: bool) {
        self.index = if count == 0 {
            0
        } else if forward {
            (self.index % count + 1) % count
        } else {
            (self.index % count + count - 1) % count
        };
    }
    pub fn reset(&mut self) {
        self.index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // A second, non-pointer technique proves the session is not tied to point scan.
    #[derive(Default)]
    struct Items {
        cursor: Cycle,
        interval: Interval,
        elapsed: u64,
    }
    impl Technique for Items {
        type Selection = &'static str;
        type Phase = usize;
        fn start(&mut self) {
            self.reset();
        }
        fn advance(&mut self, ms: u64) {
            self.elapsed += ms;
            if self.interval.elapsed(ms, 100) {
                self.cursor.step(3, true);
            }
        }
        fn handle(&mut self, action: Action) -> Option<Self::Selection> {
            match action {
                Action::Select => return Some(["one", "two", "three"][self.cursor.index()]),
                Action::Next => self.cursor.step(3, true),
                Action::Back => self.cursor.step(3, false),
                _ => {}
            }
            None
        }
        fn reset(&mut self) {
            self.cursor.reset();
            self.interval.reset();
        }
        fn frame(&self) -> Frame {
            Frame {
                strips: vec![Rect {
                    x: self.cursor.index() as f64 * 10.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                }],
            }
        }
        fn phase(&self) -> usize {
            self.cursor.index()
        }
    }
    #[test]
    fn item_technique_uses_shared_timing_pause_and_selection() {
        let mut s = Session::new(Items::default(), true);
        s.action(Action::Select);
        s.tick(100, false);
        s.action(Action::Pause);
        s.tick(100, false);
        assert_eq!(s.technique.phase(), 1);
        s.action(Action::Pause);
        s.tick(100, true);
        assert_eq!(s.technique.phase(), 1);
        assert_eq!(s.action(Action::Select), Some("two"));
        assert!(!s.active());
        assert!(s.frame().strips.is_empty());
        s.action(Action::Select);
        assert_eq!(s.technique.phase(), 0);
    }
    #[test]
    fn manual_items_wrap_and_cancel() {
        let mut s = Session::new(Items::default(), false);
        s.action(Action::Select);
        s.tick(1000, false);
        assert_eq!(s.technique.phase(), 0);
        s.action(Action::Back);
        assert_eq!(s.technique.phase(), 2);
        s.action(Action::Next);
        assert_eq!(s.technique.phase(), 0);
        s.action(Action::Cancel);
        assert!(!s.active());
        assert!(s.frame().strips.is_empty());
    }
    #[test]
    fn delayed_and_zero_ticks_are_bounded() {
        let mut s = Session::new(Items::default(), true);
        s.action(Action::Select);
        s.tick(0, false);
        s.tick(10000, false);
        assert_eq!(s.technique.elapsed, MAX_ELAPSED_MS);
    }
    #[test]
    fn cyclic_traversal_handles_empty_and_interval_reset() {
        let mut cycle = Cycle::default();
        cycle.step(0, false);
        assert_eq!(cycle.index(), 0);
        let mut timer = Interval::default();
        assert!(!timer.elapsed(70, 100));
        timer.reset();
        assert!(!timer.elapsed(40, 100));
        assert!(timer.elapsed(60, 100));
    }
}
