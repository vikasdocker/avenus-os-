//! Aether retry policy — the typed model for
//! how the agent runtime retries failed
//! tasks.
//!
//! Phase 2.8 of the ROADMAP. The runtime is
//! currently fail-and-report; this crate
//! adds the bounded retry machinery the
//! runtime needs to actually recover.
//!
//! The contract is *typed review*: every
//! retry decision is the output of a pure
//! function that the agent / shell can
//! audit. There is no IO, no clock drift,
//! no global state — the caller supplies
//! the wall clock.
//!
//! The model has four pieces:
//!
//! 1. **`BackoffStrategy`** — how long to
//!    wait between retries. Constant,
//!    linear, exponential (with optional
//!    jitter cap), or none.
//! 2. **`RetryPolicy`** — the per-task
//!    policy: max attempts, backoff, list
//!    of fallback tasks to schedule on
//!    exhaustion, and a circuit-breaker
//!    threshold.
//! 3. **`FailureRecord`** — a record of one
//!    failure (task id, error, timestamp).
//!    The runtime builds a list of these
//!    per task.
//! 4. **`PolicyEngine`** — the state
//!    machine. The runtime feeds failures
//!    in; the engine returns a `Decision`
//!    (Retry / Fallback / CircuitBreak /
//!    GiveUp).

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use aether_agent_core::TaskId;

use alloc::string::String;
use alloc::vec::Vec;

/// How to compute the delay between retry
/// attempts. The runtime is responsible for
/// actually sleeping; the engine computes
/// the delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackoffStrategy {
    /// No delay between attempts. Use only
    /// for tasks that are cheap and
    /// idempotent (e.g. a notification).
    None,
    /// Constant delay in milliseconds.
    Constant(u64),
    /// Linear delay: `base * attempt`.
    Linear {
        /// The base delay in milliseconds.
        base_ms: u64,
    },
    /// Exponential delay:
    /// `base * 2^(attempt - 1)`. The
    /// result is capped at `max_ms`.
    Exponential {
        /// The base delay in milliseconds.
        base_ms: u64,
        /// The maximum delay in
        /// milliseconds.
        max_ms: u64,
    },
}

impl BackoffStrategy {
    /// The delay (in milliseconds) before
    /// the `attempt`-th retry. `attempt` is
    /// 1-indexed (1 = first retry, after
    /// the first failure).
    #[must_use]
    pub fn delay_ms(&self, attempt: u32) -> u64 {
        match self {
            Self::None => 0,
            Self::Constant(ms) => *ms,
            Self::Linear { base_ms } => base_ms.saturating_mul(u64::from(attempt)),
            Self::Exponential { base_ms, max_ms } => {
                let exp = 2_u64.saturating_pow(attempt.saturating_sub(1).min(20));
                base_ms.saturating_mul(exp).min(*max_ms)
            }
        }
    }
}

/// The decision the engine returns for a
/// failed task.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Decision {
    /// Retry the same task after the
    /// attached delay.
    Retry {
        /// The delay before the next attempt.
        delay_ms: u64,
    },
    /// Give up retrying and run a fallback
    /// task instead.
    Fallback {
        /// The fallback task id (the
        /// engine looks it up in the
        /// task graph).
        fallback_id: TaskId,
    },
    /// The circuit breaker tripped: stop
    /// retrying the whole group until the
    /// host resets the breaker.
    CircuitBreak,
    /// Give up. The task is reported as
    /// failed.
    GiveUp,
}

impl Decision {
    /// The kebab-case name (for audit logs).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Retry { .. } => "retry",
            Self::Fallback { .. } => "fallback",
            Self::CircuitBreak => "circuit-break",
            Self::GiveUp => "give-up",
        }
    }
}

/// A single failure record. The runtime
/// builds a list of these per task and
/// feeds it to the engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FailureRecord {
    /// The task that failed.
    pub task_id: TaskId,
    /// The error message.
    pub error: String,
    /// When the failure happened
    /// (milliseconds since epoch).
    pub timestamp_ms: u64,
}

impl FailureRecord {
    /// A new failure record.
    #[must_use]
    pub fn new(task_id: TaskId, error: impl Into<String>, timestamp_ms: u64) -> Self {
        Self { task_id, error: error.into(), timestamp_ms }
    }
}

/// The retry policy attached to a single
/// task.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RetryPolicy {
    /// The maximum number of attempts
    /// (including the first try). A value
    /// of 1 means "no retries".
    pub max_attempts: u32,
    /// The backoff strategy.
    pub backoff: BackoffStrategy,
    /// The fallback task ids. The engine
    /// picks the first one when the retries
    /// are exhausted.
    pub fallbacks: Vec<TaskId>,
    /// The circuit-breaker threshold: the
    /// total number of consecutive failures
    /// across *all* tasks under this
    /// policy's `group` that triggers a
    /// `CircuitBreak` decision. A value of
    /// 0 disables the breaker.
    pub circuit_breaker_threshold: u32,
    /// The optional group id. Failures in
    /// the same group share the circuit
    /// breaker counter. If `None`, the
    /// breaker is per-task.
    pub group: Option<String>,
}

impl RetryPolicy {
    /// A no-retry policy: one attempt, give
    /// up on failure.
    #[must_use]
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            backoff: BackoffStrategy::None,
            fallbacks: Vec::new(),
            circuit_breaker_threshold: 0,
            group: None,
        }
    }

    /// A simple bounded retry policy: N
    /// attempts, constant backoff, no
    /// fallbacks, no circuit breaker.
    #[must_use]
    pub fn bounded(max_attempts: u32, delay_ms: u64) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            backoff: BackoffStrategy::Constant(delay_ms),
            fallbacks: Vec::new(),
            circuit_breaker_threshold: 0,
            group: None,
        }
    }

    /// A retry policy with an exponential
    /// backoff and a circuit breaker.
    #[must_use]
    pub fn exponential(
        max_attempts: u32,
        base_ms: u64,
        max_ms: u64,
        group: impl Into<String>,
        circuit_breaker_threshold: u32,
    ) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            backoff: BackoffStrategy::Exponential { base_ms, max_ms },
            fallbacks: Vec::new(),
            circuit_breaker_threshold,
            group: Some(group.into()),
        }
    }

    /// Add a fallback task.
    #[must_use]
    pub fn with_fallback(mut self, fallback_id: TaskId) -> Self {
        self.fallbacks.push(fallback_id);
        self
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::bounded(3, 1000)
    }
}

/// The policy engine. Holds the per-task
/// retry policy and the running failure
/// counts (per group, for the circuit
/// breaker).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PolicyEngine {
    /// The per-task policies.
    pub policies: Vec<(TaskId, RetryPolicy)>,
    /// The running group-failure counts.
    /// Reset by `reset_breaker`.
    pub group_failures: Vec<(String, u32)>,
}

impl PolicyEngine {
    /// A new, empty engine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a policy for a task.
    pub fn register(&mut self, task_id: TaskId, policy: RetryPolicy) {
        self.policies.push((task_id, policy));
    }

    /// Look up a policy for a task.
    #[must_use]
    pub fn policy_for(&self, task_id: &TaskId) -> Option<&RetryPolicy> {
        self.policies
            .iter()
            .find(|(id, _)| id == task_id)
            .map(|(_, p)| p)
    }

    /// Reset the circuit-breaker counter
    /// for a group.
    pub fn reset_breaker(&mut self, group: &str) {
        if let Some(entry) = self.group_failures.iter_mut().find(|(g, _)| g == group) {
            entry.1 = 0;
        }
    }

    /// Reset every breaker.
    pub fn reset_all_breakers(&mut self) {
        for entry in &mut self.group_failures {
            entry.1 = 0;
        }
    }

    /// Decide what to do for a task that
    /// just failed. `attempt` is the
    /// attempt number that just failed
    /// (1-indexed). The engine returns the
    /// next action.
    #[must_use]
    pub fn decide(&mut self, task_id: &TaskId, attempt: u32) -> Decision {
        let policy = match self.policy_for(task_id) {
            Some(p) => p.clone(),
            None => return Decision::GiveUp,
        };

        // Check the group circuit breaker
        // first.
        if let Some(group) = &policy.group {
            if policy.circuit_breaker_threshold > 0 {
                let current = self
                    .group_failures
                    .iter()
                    .find(|(g, _)| g == group)
                    .map(|(_, c)| *c)
                    .unwrap_or(0);
                if current >= policy.circuit_breaker_threshold {
                    return Decision::CircuitBreak;
                }
            }
            // Bump the group counter.
            let pos = self.group_failures.iter().position(|(g, _)| g == group);
            match pos {
                Some(i) => self.group_failures[i].1 += 1,
                None => self.group_failures.push((group.clone(), 1)),
            }
        }

        // Have we exhausted attempts?
        if attempt >= policy.max_attempts {
            if let Some(fb) = policy.fallbacks.first() {
                return Decision::Fallback { fallback_id: fb.clone() };
            }
            return Decision::GiveUp;
        }

        // Otherwise, retry.
        let delay_ms = policy.backoff.delay_ms(attempt + 1);
        Decision::Retry { delay_ms }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn task(id: &str) -> TaskId {
        TaskId::new(id).unwrap()
    }

    #[test]
    fn backoff_none_is_zero() {
        assert_eq!(BackoffStrategy::None.delay_ms(1), 0);
        assert_eq!(BackoffStrategy::None.delay_ms(99), 0);
    }

    #[test]
    fn backoff_constant() {
        let b = BackoffStrategy::Constant(500);
        assert_eq!(b.delay_ms(1), 500);
        assert_eq!(b.delay_ms(5), 500);
    }

    #[test]
    fn backoff_linear() {
        let b = BackoffStrategy::Linear { base_ms: 100 };
        assert_eq!(b.delay_ms(1), 100);
        assert_eq!(b.delay_ms(3), 300);
        assert_eq!(b.delay_ms(10), 1000);
    }

    #[test]
    fn backoff_exponential() {
        let b = BackoffStrategy::Exponential { base_ms: 100, max_ms: 5000 };
        assert_eq!(b.delay_ms(1), 100);
        assert_eq!(b.delay_ms(2), 200);
        assert_eq!(b.delay_ms(3), 400);
        assert_eq!(b.delay_ms(4), 800);
        assert_eq!(b.delay_ms(5), 1600);
        assert_eq!(b.delay_ms(6), 3200);
        // Capped.
        assert_eq!(b.delay_ms(20), 5000);
    }

    #[test]
    fn no_retry_policy_has_one_attempt() {
        let p = RetryPolicy::no_retry();
        assert_eq!(p.max_attempts, 1);
    }

    #[test]
    fn bounded_policy_at_least_one_attempt() {
        let p = RetryPolicy::bounded(0, 100);
        assert_eq!(p.max_attempts, 1);
    }

    #[test]
    fn bounded_policy_default_const() {
        let p = RetryPolicy::bounded(3, 100);
        assert_eq!(p.max_attempts, 3);
        assert_eq!(p.backoff.delay_ms(1), 100);
    }

    #[test]
    fn exponential_registers_group() {
        let p = RetryPolicy::exponential(5, 100, 5000, "network", 10);
        assert_eq!(p.group.as_deref(), Some("network"));
        assert_eq!(p.circuit_breaker_threshold, 10);
    }

    #[test]
    fn with_fallback_appends() {
        let p = RetryPolicy::no_retry().with_fallback(task("fb"));
        assert_eq!(p.fallbacks.len(), 1);
        assert_eq!(p.fallbacks[0], task("fb"));
    }

    #[test]
    fn default_policy_is_bounded_three() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_attempts, 3);
        assert_eq!(p.backoff.delay_ms(1), 1000);
    }

    #[test]
    fn engine_starts_empty() {
        let e = PolicyEngine::new();
        assert!(e.policy_for(&task("x")).is_none());
    }

    #[test]
    fn engine_register_and_lookup() {
        let mut e = PolicyEngine::new();
        e.register(task("a"), RetryPolicy::no_retry());
        assert!(e.policy_for(&task("a")).is_some());
    }

    #[test]
    fn engine_unknown_task_gives_up() {
        let mut e = PolicyEngine::new();
        let d = e.decide(&task("nope"), 1);
        assert_eq!(d, Decision::GiveUp);
    }

    #[test]
    fn engine_decide_retry_within_attempts() {
        let mut e = PolicyEngine::new();
        e.register(task("a"), RetryPolicy::bounded(3, 500));
        let d = e.decide(&task("a"), 1);
        assert!(matches!(d, Decision::Retry { delay_ms: 500 }));
    }

    #[test]
    fn engine_decide_give_up_at_max() {
        let mut e = PolicyEngine::new();
        e.register(task("a"), RetryPolicy::bounded(3, 500));
        let d = e.decide(&task("a"), 3);
        assert_eq!(d, Decision::GiveUp);
    }

    #[test]
    fn engine_decide_fallback_when_exhausted() {
        let mut e = PolicyEngine::new();
        e.register(
            task("a"),
            RetryPolicy::no_retry().with_fallback(task("fb")),
        );
        let d = e.decide(&task("a"), 1);
        assert!(matches!(d, Decision::Fallback { fallback_id } if fallback_id == task("fb")));
    }

    #[test]
    fn engine_circuit_breaker_trips_at_threshold() {
        let mut e = PolicyEngine::new();
        e.register(
            task("a"),
            RetryPolicy::exponential(5, 100, 1000, "g", 3),
        );
        e.register(
            task("b"),
            RetryPolicy::exponential(5, 100, 1000, "g", 3),
        );
        // 3 failures in group -> breaker
        // trips.
        let _ = e.decide(&task("a"), 1);
        let _ = e.decide(&task("b"), 1);
        let _ = e.decide(&task("a"), 1);
        let d = e.decide(&task("b"), 1);
        assert_eq!(d, Decision::CircuitBreak);
    }

    #[test]
    fn engine_reset_breaker() {
        let mut e = PolicyEngine::new();
        e.register(
            task("a"),
            RetryPolicy::exponential(5, 100, 1000, "g", 2),
        );
        let _ = e.decide(&task("a"), 1);
        let _ = e.decide(&task("a"), 1);
        // Breaker is at threshold.
        let d = e.decide(&task("a"), 1);
        assert_eq!(d, Decision::CircuitBreak);
        // Reset and try again.
        e.reset_breaker("g");
        let d = e.decide(&task("a"), 1);
        assert!(matches!(d, Decision::Retry { .. }));
    }

    #[test]
    fn engine_per_task_breaker_isolated() {
        let mut e = PolicyEngine::new();
        e.register(task("a"), RetryPolicy::bounded(2, 100));
        e.register(task("b"), RetryPolicy::bounded(2, 100));
        // a fails twice, b is untouched.
        let _ = e.decide(&task("a"), 1);
        let _ = e.decide(&task("a"), 1);
        let d = e.decide(&task("b"), 1);
        assert!(matches!(d, Decision::Retry { .. }));
    }

    #[test]
    fn decision_as_str() {
        assert_eq!(Decision::Retry { delay_ms: 0 }.as_str(), "retry");
        assert_eq!(Decision::CircuitBreak.as_str(), "circuit-break");
        assert_eq!(Decision::GiveUp.as_str(), "give-up");
        assert_eq!(
            Decision::Fallback { fallback_id: task("x") }.as_str(),
            "fallback"
        );
    }

    #[test]
    fn failure_record_new() {
        let f = FailureRecord::new(task("a"), "boom", 1234);
        assert_eq!(f.task_id, task("a"));
        assert_eq!(f.error, "boom");
        assert_eq!(f.timestamp_ms, 1234);
    }

    #[test]
    fn reset_all_breakers() {
        let mut e = PolicyEngine::new();
        e.register(
            task("a"),
            RetryPolicy::exponential(5, 100, 1000, "g1", 1),
        );
        e.register(
            task("b"),
            RetryPolicy::exponential(5, 100, 1000, "g2", 1),
        );
        let _ = e.decide(&task("a"), 1);
        let _ = e.decide(&task("b"), 1);
        e.reset_all_breakers();
        // Both groups can retry again.
        let d1 = e.decide(&task("a"), 1);
        let d2 = e.decide(&task("b"), 1);
        assert!(matches!(d1, Decision::Retry { .. }));
        assert!(matches!(d2, Decision::Retry { .. }));
    }
}
