use std::time::{Duration, Instant};

pub const RECOVERY_DELAYS: [Duration; 6] = [
    Duration::ZERO,
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(15),
];

const DUPLICATE_RESUME_WINDOW: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Active,
    Suspended,
    Recovering,
    Terminal,
}

#[derive(Debug)]
pub struct RecoveryCoordinator {
    generation: u64,
    phase: Phase,
    last_resume: Option<Instant>,
}

impl Default for RecoveryCoordinator {
    fn default() -> Self {
        Self {
            generation: 0,
            phase: Phase::Terminal,
            last_resume: None,
        }
    }
}

impl RecoveryCoordinator {
    pub fn begin_initial(&mut self) -> u64 {
        self.begin_recovery()
    }

    pub fn suspend(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1).max(1);
        self.phase = Phase::Suspended;
        self.generation
    }

    pub fn resume(&mut self, now: Instant) -> Option<u64> {
        if self.phase == Phase::Recovering {
            return None;
        }
        if self.phase != Phase::Suspended
            && self
                .last_resume
                .is_some_and(|last| now.saturating_duration_since(last) < DUPLICATE_RESUME_WINDOW)
        {
            return None;
        }
        self.last_resume = Some(now);
        Some(self.begin_recovery())
    }

    pub fn recover_from_terminal(&mut self) -> Option<u64> {
        (self.phase == Phase::Terminal).then(|| self.begin_recovery())
    }

    pub fn interrupt_terminal(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1).max(1);
        self.phase = Phase::Terminal;
        self.generation
    }

    pub fn mark_active(&mut self, generation: u64) -> bool {
        if !self.is_current(generation) {
            return false;
        }
        self.phase = Phase::Active;
        true
    }

    pub fn mark_terminal(&mut self, generation: u64) -> bool {
        if !self.is_current(generation) {
            return false;
        }
        self.phase = Phase::Terminal;
        true
    }

    pub fn is_current(&self, generation: u64) -> bool {
        self.generation == generation && matches!(self.phase, Phase::Recovering | Phase::Active)
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn is_active(&self, generation: u64) -> bool {
        self.generation == generation && self.phase == Phase::Active
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn should_retry(&self, generation: u64) -> bool {
        self.generation == generation && self.phase == Phase::Recovering
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn current_generation(&self) -> u64 {
        self.generation
    }

    fn begin_recovery(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1).max(1);
        self.phase = Phase::Recovering;
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_resume_events_are_coalesced() {
        let now = Instant::now();
        let mut coordinator = RecoveryCoordinator::default();
        coordinator.suspend();
        let generation = coordinator.resume(now).unwrap();
        assert_eq!(coordinator.resume(now + Duration::from_millis(100)), None);
        assert!(coordinator.mark_active(generation));
        assert_eq!(coordinator.resume(now + Duration::from_secs(1)), None);

        coordinator.suspend();
        assert!(coordinator
            .resume(now + Duration::from_millis(1_100))
            .is_some());
    }

    #[test]
    fn suspend_cancels_recovery_and_stale_generations() {
        let mut coordinator = RecoveryCoordinator::default();
        let generation = coordinator.begin_initial();
        coordinator.suspend();
        assert!(!coordinator.is_current(generation));
        assert!(!coordinator.mark_active(generation));
    }

    #[test]
    fn terminal_state_can_recover_after_a_radio_change() {
        let mut coordinator = RecoveryCoordinator::default();
        let first = coordinator.begin_initial();
        assert!(coordinator.mark_terminal(first));
        assert!(!coordinator.is_current(first));
        let second = coordinator.recover_from_terminal().unwrap();
        assert_ne!(first, second);
        assert!(coordinator.is_current(second));
    }

    #[test]
    fn only_the_current_active_generation_is_active() {
        let mut coordinator = RecoveryCoordinator::default();
        let first = coordinator.begin_initial();
        assert!(!coordinator.is_active(first));
        assert!(coordinator.mark_active(first));
        assert!(coordinator.is_active(first));
        let second = coordinator.interrupt_terminal();
        assert!(!coordinator.is_active(first));
        assert!(!coordinator.is_active(second));
    }

    #[test]
    fn retry_schedule_is_bounded_to_thirty_seconds() {
        assert_eq!(RECOVERY_DELAYS[0], Duration::ZERO);
        assert_eq!(RECOVERY_DELAYS.last(), Some(&Duration::from_secs(15)));
        assert_eq!(
            RECOVERY_DELAYS.iter().sum::<Duration>(),
            Duration::from_secs(30)
        );
    }
}
