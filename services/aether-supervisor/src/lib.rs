// Aether Supervisor - child process supervision with restart policies.
//
// Keeps a managed child process alive according to its restart policy,
// applying exponential backoff between restarts and recording every
// transition. The spawn mechanism is abstracted so policy logic is
// testable on any host OS.

use aether_core::manifest::RestartPolicy;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// One supervision decision outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupervisionAction {
    /// (Re)launch the child now.
    Launch,
    /// Child is running; nothing to do.
    Wait,
    /// Give up: restart limit exhausted or policy forbids restart.
    GiveUp,
}

/// Backoff schedule state for one supervised unit.
#[derive(Debug, Clone)]
pub struct Backoff {
    pub base_ms: u64,
    pub max_ms: u64,
    attempt: u32,
    last_transition: Option<Instant>,
}

impl Backoff {
    pub fn new(base_ms: u64, max_ms: u64) -> Self {
        Self {
            base_ms: base_ms.max(1),
            max_ms: max_ms.max(base_ms.max(1)),
            attempt: 0,
            last_transition: None,
        }
    }

    /// Records a failure and returns the delay before the next launch.
    pub fn record_failure(&mut self) -> Duration {
        self.attempt = self.attempt.saturating_add(1);
        let shift = self.attempt.saturating_sub(1).min(16);
        let delay = self.base_ms.saturating_mul(1u64 << shift).min(self.max_ms);
        self.last_transition = Some(Instant::now());
        Duration::from_millis(delay)
    }

    /// Resets the backoff after a stable run of at least `stable` duration.
    pub fn reset_if_stable(&mut self, stable: Duration) -> bool {
        if let Some(t) = self.last_transition {
            if t.elapsed() >= stable {
                self.attempt = 0;
                return true;
            }
        }
        false
    }

    /// Current attempt counter.
    pub fn attempts(&self) -> u32 {
        self.attempt
    }
}

/// Decide what to do with a supervised child given observed facts.
pub fn decide(
    policy: RestartPolicy,
    running: bool,
    restarts: u32,
    restart_limit: u32,
    backoff_elapsed: Option<Duration>,
    required_backoff: Option<Duration>,
) -> SupervisionAction {
    if running {
        return SupervisionAction::Wait;
    }
    match policy {
        RestartPolicy::Never => SupervisionAction::GiveUp,
        RestartPolicy::OnFailure | RestartPolicy::Always => {
            if restarts >= restart_limit {
                return SupervisionAction::GiveUp;
            }
            match (backoff_elapsed, required_backoff) {
                (Some(elapsed), Some(required)) if elapsed < required => SupervisionAction::Wait,
                _ => SupervisionAction::Launch,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_while_running() {
        assert_eq!(
            decide(RestartPolicy::Always, true, 0, 3, None, None),
            SupervisionAction::Wait
        );
    }

    #[test]
    fn never_policy_gives_up_immediately() {
        assert_eq!(
            decide(RestartPolicy::Never, false, 0, 3, None, None),
            SupervisionAction::GiveUp
        );
    }

    #[test]
    fn limit_stops_restarts() {
        assert_eq!(
            decide(RestartPolicy::OnFailure, false, 5, 5, None, None),
            SupervisionAction::GiveUp
        );
    }

    #[test]
    fn backoff_defers_launch() {
        let required = Duration::from_millis(500);
        assert_eq!(
            decide(
                RestartPolicy::OnFailure,
                false,
                1,
                5,
                Some(Duration::from_millis(100)),
                Some(required)
            ),
            SupervisionAction::Wait
        );
        assert_eq!(
            decide(
                RestartPolicy::OnFailure,
                false,
                1,
                5,
                Some(Duration::from_millis(600)),
                Some(required)
            ),
            SupervisionAction::Launch
        );
    }

    #[test]
    fn exponential_backoff_capped_at_max() {
        let mut b = Backoff::new(10, 100);
        assert_eq!(b.record_failure(), Duration::from_millis(10));
        assert_eq!(b.record_failure(), Duration::from_millis(20));
        assert_eq!(b.record_failure(), Duration::from_millis(40));
        assert_eq!(b.record_failure(), Duration::from_millis(80));
        assert_eq!(b.record_failure(), Duration::from_millis(100));
        assert_eq!(b.record_failure(), Duration::from_millis(100));
    }

    #[test]
    fn reset_after_stability() {
        let mut b = Backoff::new(10, 100);
        b.record_failure();
        // Immediately after failure we are not stable for long durations.
        assert!(!b.reset_if_stable(Duration::from_secs(3600)));
        assert_eq!(b.attempts(), 1);
    }
}
