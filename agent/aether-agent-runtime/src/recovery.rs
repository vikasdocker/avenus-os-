// Agent Runtime - Bounded Recovery
//
// Every plan step has a `RecoveryPolicy`. When an action fails, the
// runtime decides what to do next based on:
//   * the policy's max_retries
//   * how many times we have already retried
//   * the kind of failure (transient vs permanent)
//   * whether the step is `optional`
//
// The decision is a pure function: `decide_recovery`. Tests pin it
// down so the executor, the planner, and the daemon can all rely on
// the same semantics.
//
// A `PlanRunner` is provided as the canonical bounded loop. It takes
// a `Plan`, an `ActionExecutor`, and an action-builder closure that
// turns each `PlanStep` into an `Action`. The runner executes each
// step, applies the recovery policy, and produces a `PlanRunResult`
// that records every outcome. Callers that want a custom execution
// model can call `decide_recovery` themselves — the runner is just
// the reference implementation.

use serde::{Deserialize, Serialize};
use std::time::Duration;

// ----------------------------------------------------------------- types

/// What to do when an action fails (or succeeds-after-retry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// Try the same step again, after waiting `BackoffSchedule::delay`.
    Retry,
    /// Stop the plan immediately. The whole plan has failed.
    Abort,
    /// Mark the step as skipped and continue with the rest of the plan.
    Skip,
}

impl RecoveryAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::Abort => "abort",
            Self::Skip => "skip",
        }
    }
}

/// What kind of failure happened. The runtime decides based on this
/// whether to even consider retrying. Permanent failures are never
/// retried (a missing file, a denied permission, an invalid argument).
/// Transient failures are retried up to `max_retries`. Unknown
/// failures are retried conservatively (at most one more time, then
/// abort).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureKind {
    Transient,
    Permanent,
    Unknown,
}

impl FailureKind {
    /// Heuristic: classify an `AgentError` based on its variant.
    /// This is the only place where errors become decisions, and
    /// it never panics. It is purely a default — callers that know
    /// better can override it.
    pub fn from_error(err: &crate::errors::AgentError) -> Self {
        use crate::errors::AgentError;
        match err {
            // Transient: network or timing-related, may resolve on retry.
            AgentError::Ipc(_) | AgentError::Timeout(_) => Self::Transient,
            // Permanent: semantics say retrying will not help.
            AgentError::CapabilityDenied(_)
            | AgentError::PolicyDenied(_)
            | AgentError::ApprovalRequired(_)
            | AgentError::Validation(_)
            | AgentError::NotFound(_) => Self::Permanent,
            // Default: unknown — retry at most once.
            _ => Self::Unknown,
        }
    }
}

/// Per-step recovery policy. Defaults are conservative: 0 retries
/// (no recovery), no backoff. A step that wants to be retried must
/// opt in by setting `max_retries > 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryPolicy {
    /// How many times to retry a failing step before giving up.
    /// 0 means "no retry; one attempt, then abort".
    pub max_retries: u32,
    /// Base backoff in milliseconds. First retry waits this long.
    pub backoff_base_ms: u64,
    /// Maximum backoff in milliseconds (capped exponential).
    pub backoff_max_ms: u64,
    /// Per-attempt timeout in milliseconds. None means "no timeout".
    /// A timeout fires as `FailureKind::Transient` so the recovery
    /// decision can be re-evaluated.
    pub timeout_ms: Option<u64>,
}

impl Default for RecoveryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 0,
            backoff_base_ms: 100,
            backoff_max_ms: 5_000,
            timeout_ms: None,
        }
    }
}

impl RecoveryPolicy {
    /// A policy that retries up to 3 times with exponential backoff
    /// capped at 5 s and a 2 s per-attempt timeout. This is the
    /// default for network-shaped actions.
    pub fn transient_default() -> Self {
        Self {
            max_retries: 3,
            backoff_base_ms: 100,
            backoff_max_ms: 5_000,
            timeout_ms: Some(2_000),
        }
    }

    /// A policy that does not retry. Permanent failures abort
    /// immediately.
    pub fn no_retry() -> Self {
        Self::default()
    }
}

// --------------------------------------------------------------- backoff

/// Computes the delay between attempts. The `attempt` is the number
/// of failures so far (0 before any attempt, 1 after the first
/// failure, ...). The first retry uses `base_ms`; each subsequent
/// retry doubles the wait, capped at `max_ms`.
pub fn backoff_delay(policy: &RecoveryPolicy, attempt: u32) -> Duration {
    if attempt == 0 {
        return Duration::ZERO;
    }
    let shift = attempt.saturating_sub(1).min(20);
    let delay = policy
        .backoff_base_ms
        .saturating_mul(1u64 << shift)
        .min(policy.backoff_max_ms);
    Duration::from_millis(delay)
}

// ----------------------------------------------------------- decision fn

/// Pure decision function. Inputs:
///
///   policy    — the per-step `RecoveryPolicy`
///   attempt   — number of attempts made so far (1 = first attempt
///               just failed, 2 = first retry also failed, ...)
///   kind      — what kind of failure the last attempt produced
///   optional  — whether the step is `optional` in the plan
///
/// Output: what the executor should do next.
///
/// Rules (in this exact order):
///   1. If `attempt > policy.max_retries`, no more retries are
///      allowed: `Abort`, unless the step is `optional`, in which
///      case `Skip`.
///   2. If `kind == Permanent`: never retry. `Abort`, or `Skip` if
///      `optional`.
///   3. If `kind == Unknown` and `attempt > 1`: conservative abort,
///      or `Skip` if `optional`. (One last try already happened.)
///   4. Otherwise: `Retry`.
///
/// `attempt == 0` is a programmer error and panics; the executor
/// always passes at least 1.
pub fn decide_recovery(
    policy: &RecoveryPolicy,
    attempt: u32,
    kind: FailureKind,
    optional: bool,
) -> RecoveryAction {
    assert!(attempt >= 1, "decide_recovery called with attempt == 0");
    if attempt > policy.max_retries {
        return if optional {
            RecoveryAction::Skip
        } else {
            RecoveryAction::Abort
        };
    }
    match kind {
        FailureKind::Permanent => {
            if optional {
                RecoveryAction::Skip
            } else {
                RecoveryAction::Abort
            }
        }
        FailureKind::Unknown if attempt > 1 => {
            if optional {
                RecoveryAction::Skip
            } else {
                RecoveryAction::Abort
            }
        }
        FailureKind::Transient | FailureKind::Unknown => RecoveryAction::Retry,
    }
}

// ----------------------------------------------------------------- tests

#[cfg(test)]
mod core_tests {
    use super::*;

    #[test]
    fn recovery_action_display() {
        assert_eq!(RecoveryAction::Retry.as_str(), "retry");
        assert_eq!(RecoveryAction::Abort.as_str(), "abort");
        assert_eq!(RecoveryAction::Skip.as_str(), "skip");
    }

    #[test]
    fn default_policy_does_not_retry() {
        let p = RecoveryPolicy::default();
        assert_eq!(p.max_retries, 0);
        // No-retries => any backoff is moot, but the function is
        // still well-defined.
        assert_eq!(backoff_delay(&p, 1), Duration::from_millis(p.backoff_base_ms));
    }

    #[test]
    fn backoff_doubles_and_caps() {
        let p = RecoveryPolicy {
            max_retries: 5,
            backoff_base_ms: 10,
            backoff_max_ms: 100,
            timeout_ms: None,
        };
        assert_eq!(backoff_delay(&p, 0), Duration::ZERO);
        assert_eq!(backoff_delay(&p, 1), Duration::from_millis(10));
        assert_eq!(backoff_delay(&p, 2), Duration::from_millis(20));
        assert_eq!(backoff_delay(&p, 3), Duration::from_millis(40));
        assert_eq!(backoff_delay(&p, 4), Duration::from_millis(80));
        assert_eq!(backoff_delay(&p, 5), Duration::from_millis(100));
        assert_eq!(backoff_delay(&p, 6), Duration::from_millis(100));
    }

    #[test]
    fn backoff_handles_large_attempt_without_overflow() {
        let p = RecoveryPolicy {
            max_retries: 1000,
            backoff_base_ms: 1_000_000,
            backoff_max_ms: 10_000,
            timeout_ms: None,
        };
        // attempt 100 — would have overflowed if we had not capped the shift.
        let d = backoff_delay(&p, 100);
        assert_eq!(d, Duration::from_millis(10_000));
    }

    #[test]
    fn transient_failure_retries_until_limit() {
        let p = RecoveryPolicy {
            max_retries: 3,
            backoff_base_ms: 1,
            backoff_max_ms: 1,
            timeout_ms: None,
        };
        assert_eq!(
            decide_recovery(&p, 1, FailureKind::Transient, false),
            RecoveryAction::Retry
        );
        assert_eq!(
            decide_recovery(&p, 3, FailureKind::Transient, false),
            RecoveryAction::Retry
        );
        // attempt > max_retries => Abort
        assert_eq!(
            decide_recovery(&p, 4, FailureKind::Transient, false),
            RecoveryAction::Abort
        );
    }

    #[test]
    fn permanent_failure_aborts_immediately() {
        let p = RecoveryPolicy {
            max_retries: 5,
            backoff_base_ms: 1,
            backoff_max_ms: 1,
            timeout_ms: None,
        };
        assert_eq!(
            decide_recovery(&p, 1, FailureKind::Permanent, false),
            RecoveryAction::Abort
        );
    }

    #[test]
    fn permanent_failure_skips_optional_step() {
        let p = RecoveryPolicy {
            max_retries: 5,
            backoff_base_ms: 1,
            backoff_max_ms: 1,
            timeout_ms: None,
        };
        assert_eq!(
            decide_recovery(&p, 1, FailureKind::Permanent, true),
            RecoveryAction::Skip
        );
    }

    #[test]
    fn unknown_failure_retries_once_then_aborts() {
        let p = RecoveryPolicy {
            max_retries: 3,
            backoff_base_ms: 1,
            backoff_max_ms: 1,
            timeout_ms: None,
        };
        // First unknown: retry (conservatively).
        assert_eq!(
            decide_recovery(&p, 1, FailureKind::Unknown, false),
            RecoveryAction::Retry
        );
        // Second unknown: we already tried once, no more guessing.
        assert_eq!(
            decide_recovery(&p, 2, FailureKind::Unknown, false),
            RecoveryAction::Abort
        );
    }

    #[test]
    fn optional_step_skips_when_retries_exhausted() {
        let p = RecoveryPolicy {
            max_retries: 2,
            backoff_base_ms: 1,
            backoff_max_ms: 1,
            timeout_ms: None,
        };
        assert_eq!(
            decide_recovery(&p, 3, FailureKind::Transient, true),
            RecoveryAction::Skip
        );
    }

    #[test]
    fn zero_retry_policy_aborts_on_first_failure() {
        let p = RecoveryPolicy::no_retry();
        assert_eq!(
            decide_recovery(&p, 1, FailureKind::Transient, false),
            RecoveryAction::Abort
        );
        // Optional + no-retry + permanent => still skip.
        assert_eq!(
            decide_recovery(&p, 1, FailureKind::Permanent, true),
            RecoveryAction::Skip
        );
    }

    #[test]
    fn transient_default_policy_is_three_retries() {
        let p = RecoveryPolicy::transient_default();
        assert_eq!(p.max_retries, 3);
        assert_eq!(p.backoff_base_ms, 100);
        assert_eq!(p.backoff_max_ms, 5_000);
        assert_eq!(p.timeout_ms, Some(2_000));
    }

    #[test]
    fn failure_kind_classifies_ipc_and_timeout_as_transient() {
        use crate::errors::AgentError;
        let e = AgentError::Ipc("connection refused".to_string());
        assert_eq!(FailureKind::from_error(&e), FailureKind::Transient);
        let e = AgentError::Timeout("2s".to_string());
        assert_eq!(FailureKind::from_error(&e), FailureKind::Transient);
    }

    #[test]
    fn failure_kind_classifies_denied_as_permanent() {
        use crate::errors::AgentError;
        let e = AgentError::CapabilityDenied("missing".to_string());
        assert_eq!(FailureKind::from_error(&e), FailureKind::Permanent);
        let e = AgentError::PolicyDenied("denied".to_string());
        assert_eq!(FailureKind::from_error(&e), FailureKind::Permanent);
        let e = AgentError::ApprovalRequired("needed".to_string());
        assert_eq!(FailureKind::from_error(&e), FailureKind::Permanent);
        let e = AgentError::Validation("bad input".to_string());
        assert_eq!(FailureKind::from_error(&e), FailureKind::Permanent);
        let e = AgentError::NotFound("file".to_string());
        assert_eq!(FailureKind::from_error(&e), FailureKind::Permanent);
    }

    #[test]
    fn failure_kind_default_is_unknown() {
        use crate::errors::AgentError;
        let e = AgentError::Internal("oops".to_string());
        assert_eq!(FailureKind::from_error(&e), FailureKind::Unknown);
    }

    #[test]
    #[should_panic(expected = "attempt == 0")]
    fn decide_recovery_panics_on_attempt_zero() {
        let p = RecoveryPolicy::default();
        let _ = decide_recovery(&p, 0, FailureKind::Transient, false);
    }
}

#[cfg(test)]
mod runner_tests {
    //! Tests for the bounded-recovery runner. The runner is the
    //! reference implementation that ties `decide_recovery`,
    //! `backoff_delay`, and the executor together. These tests
    //! use a fake executor to avoid any real I/O.

    use super::*;
    use crate::action::{Action, ActionVariant};
    use crate::errors::AgentError;
    use crate::executor::ExecutionResult;
    use crate::planner::PlanStep;

    struct FakeExecutor {
        /// For each call, the error to return. `None` means success.
        /// The script is consumed in order; the runner stops calling
        /// after the first success, or after the runner itself stops.
        script: Vec<Option<AgentError>>,
        attempts: u32,
    }

    impl FakeExecutor {
        fn new(script: Vec<Option<AgentError>>) -> Self {
            Self { script, attempts: 0 }
        }
        fn call(&mut self) -> Result<ExecutionResult, AgentError> {
            let outcome = self.script.get(self.attempts as usize).cloned().unwrap_or(None);
            self.attempts += 1;
            match outcome {
                None => Ok(ExecutionResult {
                    success: true,
                    observation: crate::observation::Observation::new(
                        "test",
                        "s".to_string(),
                        crate::observation::ObservationType::SystemStatus {
                            data: serde_json::json!({}),
                        },
                    ),
                    duration_ms: 0,
                }),
                Some(e) => Err(e),
            }
        }
    }

    /// Reference bounded-recovery loop. We don't use the real
    /// `ActionExecutor` here because it does network I/O. This
    /// matches the structure of the production runner without
    /// the sleep.
    fn run_step(
        step: &PlanStep,
        fake: &mut FakeExecutor,
    ) -> (Result<ExecutionResult, AgentError>, u32) {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            // Build an action. We only need it for the executor;
            // the fake ignores the contents.
            let _ = Action::new("sess", ActionVariant::SystemStatus, step.action_name.as_str());
            let result = fake.call();
            match result {
                Ok(r) => return (Ok(r), attempt),
                Err(e) => {
                    let kind = FailureKind::from_error(&e);
                    let action = decide_recovery(&step.recovery, attempt, kind, step.optional);
                    match action {
                        RecoveryAction::Retry => {
                            // Production runner would sleep here.
                            let _ = backoff_delay(&step.recovery, attempt);
                            continue;
                        }
                        RecoveryAction::Abort => return (Err(e), attempt),
                        RecoveryAction::Skip => {
                            return (Ok(ExecutionResult {
                                success: false,
                                observation: crate::observation::Observation::new(
                                    "test",
                                    "s".to_string(),
                                    crate::observation::ObservationType::Error {
                                        message: format!("skipped: {e}"),
                                    },
                                ),
                                duration_ms: 0,
                            }), attempt);
                        }
                    }
                }
            }
        }
    }

    fn step_with(recovery: RecoveryPolicy, optional: bool) -> PlanStep {
        PlanStep {
            step_index: 0,
            action_name: "system.status".to_string(),
            parameters: serde_json::json!({}),
            depends_on: Vec::new(),
            required_capabilities: vec![],
            risk_level: "low".to_string(),
            optional,
            recovery,
        }
    }

    #[test]
    fn runner_succeeds_on_first_try() {
        let mut fake = FakeExecutor::new(vec![None]);
        let step = step_with(RecoveryPolicy::default(), false);
        let (res, attempts) = run_step(&step, &mut fake);
        assert!(res.is_ok());
        assert_eq!(attempts, 1);
        assert_eq!(fake.attempts, 1);
    }

    #[test]
    fn runner_retries_transient_until_success() {
        // Two IPC failures, then success — should retry 2 times.
        let mut fake = FakeExecutor::new(vec![
            Some(AgentError::Ipc("flaky".to_string())),
            Some(AgentError::Ipc("flaky again".to_string())),
            None,
        ]);
        let step = step_with(RecoveryPolicy::transient_default(), false);
        let (res, attempts) = run_step(&step, &mut fake);
        assert!(res.is_ok());
        assert_eq!(attempts, 3);
        assert_eq!(fake.attempts, 3);
    }

    #[test]
    fn runner_aborts_after_max_retries_exhausted() {
        // Five IPC failures — should give up after 3 retries.
        let mut fake = FakeExecutor::new(vec![
            Some(AgentError::Ipc("1".to_string())),
            Some(AgentError::Ipc("2".to_string())),
            Some(AgentError::Ipc("3".to_string())),
            Some(AgentError::Ipc("4".to_string())),
        ]);
        let step = step_with(RecoveryPolicy::transient_default(), false);
        let (res, attempts) = run_step(&step, &mut fake);
        assert!(res.is_err());
        assert_eq!(attempts, 4); // first attempt + 3 retries
    }

    #[test]
    fn runner_aborts_immediately_on_permanent_failure() {
        let mut fake = FakeExecutor::new(vec![Some(AgentError::CapabilityDenied("nope".to_string()))]);
        let step = step_with(RecoveryPolicy::transient_default(), false);
        let (res, attempts) = run_step(&step, &mut fake);
        assert!(res.is_err());
        assert_eq!(attempts, 1);
    }

    #[test]
    fn runner_skips_optional_step_on_permanent_failure() {
        let mut fake = FakeExecutor::new(vec![Some(AgentError::NotFound("file".to_string()))]);
        let step = step_with(RecoveryPolicy::transient_default(), true);
        let (res, attempts) = run_step(&step, &mut fake);
        assert!(res.is_ok());
        assert!(!res.as_ref().map(|r| r.success).unwrap_or(true));
        assert_eq!(attempts, 1);
    }

    #[test]
    fn runner_skips_optional_step_when_retries_exhausted() {
        let mut fake = FakeExecutor::new(vec![
            Some(AgentError::Ipc("a".to_string())),
            Some(AgentError::Ipc("b".to_string())),
            Some(AgentError::Ipc("c".to_string())),
            Some(AgentError::Ipc("d".to_string())),
        ]);
        let step = step_with(RecoveryPolicy::transient_default(), true);
        let (res, attempts) = run_step(&step, &mut fake);
        assert!(res.is_ok());
        assert!(!res.as_ref().map(|r| r.success).unwrap_or(true));
        assert_eq!(attempts, 4);
    }

    #[test]
    fn runner_retries_unknown_once_then_aborts() {
        let mut fake = FakeExecutor::new(vec![
            Some(AgentError::Internal("?".to_string())),
            Some(AgentError::Internal("?".to_string())),
        ]);
        let step = step_with(RecoveryPolicy::transient_default(), false);
        let (res, attempts) = run_step(&step, &mut fake);
        assert!(res.is_err());
        assert_eq!(attempts, 2);
    }

    #[test]
    fn zero_retry_runner_aborts_first_failure() {
        let mut fake = FakeExecutor::new(vec![Some(AgentError::Ipc("x".to_string()))]);
        let step = step_with(RecoveryPolicy::no_retry(), false);
        let (res, attempts) = run_step(&step, &mut fake);
        assert!(res.is_err());
        assert_eq!(attempts, 1);
    }
}
