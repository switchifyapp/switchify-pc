//! The ranges timing settings accept, set in Settings with a − / + stepper.
//!
//! Each setting takes any multiple of its step between its minimum and its
//! maximum, in milliseconds. `src/settings/timing.ts` mirrors these ranges.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeRange {
    pub min: u32,
    pub max: u32,
    pub step: u32,
}

impl TimeRange {
    pub const fn new(min: u32, max: u32, step: u32) -> Self {
        Self { min, max, step }
    }

    pub fn contains(self, ms: u32) -> bool {
        (self.min..=self.max).contains(&ms) && ms.is_multiple_of(self.step)
    }
}

/// Click when I stop: the wait before clicking.
pub const DWELL_CLICK_DELAY: TimeRange = TimeRange::new(100, 10_000, 100);
/// Repeat held keys: the delay before the first repeat.
pub const KEY_REPEAT_INITIAL_DELAY: TimeRange = TimeRange::new(0, 2000, 50);
/// Repeat held keys: the time between repeats. Switchify Remote is told
/// 100 to 1000 ms, so the range stays inside it.
pub const KEY_REPEAT_INTERVAL: TimeRange = TimeRange::new(100, 1000, 50);
/// Repeat mouse movement and scrolling: the time between moves, and between
/// scrolls. Switchify Remote is told 100 to 2000 ms, so the range stays
/// inside it.
pub const MOUSE_REPEAT_INTERVAL: TimeRange = TimeRange::new(100, 2000, 50);
/// Repeat mouse movement: how long the pointer takes to reach full speed.
/// 0 turns acceleration off.
pub const MOUSE_REPEAT_ACCELERATION: TimeRange = TimeRange::new(0, 3000, 100);
/// Move in blocks: a set time between block hops. 0, the mouse panel's scan
/// speed, is allowed beside this range.
pub const MOUSE_BLOCK_INTERVAL: TimeRange = TimeRange::new(100, 10_000, 100);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_takes_multiples_of_its_step_between_its_ends() {
        let range = TimeRange::new(100, 1000, 50);
        for ms in [100, 150, 250, 950, 1000] {
            assert!(range.contains(ms), "{ms}");
        }
        for ms in [0, 50, 125, 1050, u32::MAX] {
            assert!(!range.contains(ms), "{ms}");
        }
    }

    #[test]
    fn every_earlier_choice_is_still_accepted() {
        for ms in [500, 1000, 1500, 2000, 3000, 4000, 5000, 6000, 7000, 8000] {
            assert!(DWELL_CLICK_DELAY.contains(ms), "dwell {ms}");
        }
        for ms in [0, 250, 500, 1000] {
            assert!(KEY_REPEAT_INITIAL_DELAY.contains(ms), "delay {ms}");
        }
        for ms in [100, 250, 500, 1000] {
            assert!(KEY_REPEAT_INTERVAL.contains(ms), "interval {ms}");
        }
        for ms in [100, 250, 500, 1000] {
            assert!(MOUSE_REPEAT_INTERVAL.contains(ms), "mouse interval {ms}");
        }
        for ms in [0, 500, 1000, 2000] {
            assert!(MOUSE_REPEAT_ACCELERATION.contains(ms), "acceleration {ms}");
        }
    }
}
