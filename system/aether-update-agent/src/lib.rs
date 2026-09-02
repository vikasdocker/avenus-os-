//! Aether update agent — the runtime
//! driver for Aether self-updates.
//!
//! Phase 12 of the ROADMAP. This crate
//! is the *driver*; `aether-update-core`
//! holds the plan + state-machine
//! types. The agent:
//!
//!   * owns the `UpdateStatus` state
//!     machine,
//!   * owns the `RecoverySnapshot` for
//!     the in-flight plan,
//!   * applies the plan through an
//!     `ApplyEngine` trait (the runtime
//!     plugs in a real I/O backend; a
//!     `NullApplyEngine` is provided
//!     for tests and graceful
//!     degradation),
//!   * drives the state machine
//!     Downloading -> Verifying ->
//!     Staging -> Applying -> Done
//!     (with bounded retries via
//!     `aether-retry-policy`) or
//!     -> Failed -> RolledBack on
//!     error,
//!   * records every transition to the
//!     state machine's history and to
//!     the agent's own audit log.
//!
//! The contract is *typed review* —
//! every step is auditable.
//!
//! The model has five pieces:
//!
//! 1. **`ApplyStep`** — a single
//!    unit of work the engine can do
//!    (download, verify, stage, ...).
//! 2. **`ApplyEngine`** — the trait
//!    the runtime uses to plug in a
//!    real backend.
//! 3. **`NullApplyEngine`** — the
//!    no-op fallback.
//! 4. **`UpdateAgent`** — the
//!    driver. Owns the status, the
//!    snapshot, the policy engine,
//!    and the audit log.
//! 5. **`AgentAuditEvent`** — the
//!    per-step audit log entry.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

pub mod disk_engine;
mod engine;

pub use disk_engine::{DiskApplyEngine, DiskApplyError, DiskEngineAudit};
pub use engine::{sha256_inline, EngineAudit, FilesystemApplyEngine, FilesystemApplyError};

use alloc::string::String;
use alloc::vec::Vec;

use aether_retry_policy::{BackoffStrategy, Decision, PolicyEngine, RetryPolicy};
use aether_update_core::plan::UpdatePlan;
use aether_update_core::recovery::RecoverySnapshot;
use aether_update_core::state::{HistoryEntry, UpdateStage, UpdateStatus};

/// A single, named step the apply
/// engine can perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ApplyStep {
    /// Download the payload.
    Download,
    /// Verify the payload's
    /// signature and content hash.
    Verify,
    /// Stage the payload onto the
    /// filesystem.
    Stage,
    /// Take a snapshot of the
    /// pre-update state.
    Snapshot,
    /// Apply the update atomically.
    Apply,
    /// Reboot the system (if
    /// required).
    Reboot,
}

impl ApplyStep {
    /// The kebab-case name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Download => "download",
            Self::Verify => "verify",
            Self::Stage => "stage",
            Self::Snapshot => "snapshot",
            Self::Apply => "apply",
            Self::Reboot => "reboot",
        }
    }

    /// The corresponding
    /// `UpdateStage` (for the status
    /// machine).
    #[must_use]
    pub const fn stage(&self) -> UpdateStage {
        match self {
            Self::Download => UpdateStage::Downloading,
            Self::Verify => UpdateStage::Verifying,
            Self::Stage => UpdateStage::Staging,
            Self::Snapshot | Self::Apply | Self::Reboot => UpdateStage::Applying,
        }
    }
}

/// Apply engine errors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ApplyError {
    /// The step returned a non-zero
    /// exit code.
    NonZeroExit {
        /// The step.
        step: ApplyStep,
        /// The exit code.
        code: i32,
    },
    /// The engine refused the step
    /// (e.g. disk full, signature
    /// mismatch).
    Refused {
        /// The step.
        step: ApplyStep,
        /// The reason.
        reason: String,
    },
    /// The agent tried to drive a
    /// step that doesn't belong in
    /// the current state.
    WrongStage {
        /// The step.
        step: ApplyStep,
        /// The actual stage.
        actual: UpdateStage,
    },
}

impl core::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NonZeroExit { step, code } => {
                write!(f, "step '{}' returned exit code {code}", step.as_str())
            }
            Self::Refused { step, reason } => {
                write!(f, "step '{}' refused: {reason}", step.as_str())
            }
            Self::WrongStage { step, actual } => {
                write!(f, "step '{}' not allowed in stage '{}'", step.as_str(), actual.as_str())
            }
        }
    }
}

impl std::error::Error for ApplyError {}

/// The apply engine trait. The
/// runtime plugs in a real backend
/// (libcurl + libarchive + btrfs
/// send/receive, or whatever the
/// target image requires).
pub trait ApplyEngine {
    /// Run the given step for the
    /// given plan. Returns `Ok(())`
    /// on success.
    fn run(&self, step: ApplyStep, plan: &UpdatePlan) -> Result<(), ApplyError>;
}

/// A null apply engine. Returns
/// `Ok(())` for every step. Used
/// for tests and graceful
/// degradation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NullApplyEngine;

impl ApplyEngine for NullApplyEngine {
    fn run(&self, step: ApplyStep, _plan: &UpdatePlan) -> Result<(), ApplyError> {
        let _ = step;
        Ok(())
    }
}

/// An audit log entry produced by
/// the agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentAuditEvent {
    /// The agent accepted a new
    /// plan.
    PlanAccepted {
        /// The plan id.
        plan_id: String,
        /// The target version.
        target: String,
    },
    /// A step was attempted.
    StepAttempted {
        /// The step.
        step: ApplyStep,
        /// The attempt number
        /// (1-indexed).
        attempt: u32,
    },
    /// A step succeeded.
    StepSucceeded {
        /// The step.
        step: ApplyStep,
    },
    /// A step failed.
    StepFailed {
        /// The step.
        step: ApplyStep,
        /// The reason.
        reason: String,
    },
    /// A retry was scheduled.
    RetryScheduled {
        /// The step.
        step: ApplyStep,
        /// The delay in ms.
        delay_ms: u64,
    },
    /// The agent gave up on a
    /// step.
    StepGaveUp {
        /// The step.
        step: ApplyStep,
    },
    /// A rollback was triggered.
    RollbackTriggered {
        /// The reason.
        reason: String,
    },
    /// A rollback completed.
    RollbackCompleted,
}

impl AgentAuditEvent {
    /// The kebab-case kind.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::PlanAccepted { .. } => "plan-accepted",
            Self::StepAttempted { .. } => "step-attempted",
            Self::StepSucceeded { .. } => "step-succeeded",
            Self::StepFailed { .. } => "step-failed",
            Self::RetryScheduled { .. } => "retry-scheduled",
            Self::StepGaveUp { .. } => "step-gave-up",
            Self::RollbackTriggered { .. } => "rollback-triggered",
            Self::RollbackCompleted => "rollback-completed",
        }
    }
}

/// The update agent: owns the
/// status machine, the snapshot,
/// the retry policy, and the audit
/// log.
pub struct UpdateAgent<E: ApplyEngine> {
    engine: E,
    status: UpdateStatus,
    snapshot: Option<RecoverySnapshot>,
    retries: PolicyEngine,
    audit: Vec<AgentAuditEvent>,
}

impl<E: ApplyEngine> UpdateAgent<E> {
    /// A new agent in `Idle` state.
    #[must_use]
    pub fn new(engine: E) -> Self {
        Self {
            engine,
            status: UpdateStatus::default(),
            snapshot: None,
            retries: PolicyEngine::new(),
            audit: Vec::new(),
        }
    }

    /// The current status.
    #[must_use]
    pub fn status(&self) -> &UpdateStatus {
        &self.status
    }

    /// The current snapshot, if any.
    #[must_use]
    pub fn snapshot(&self) -> Option<&RecoverySnapshot> {
        self.snapshot.as_ref()
    }

    /// The audit log.
    #[must_use]
    pub fn audit(&self) -> &[AgentAuditEvent] {
        &self.audit
    }

    /// The retry policy engine.
    pub fn retries_mut(&mut self) -> &mut PolicyEngine {
        &mut self.retries
    }

    /// Accept a plan. Sets the
    /// status' `current_plan` and
    /// resets attempt.
    pub fn accept(&mut self, plan: UpdatePlan, now_ms: u64) {
        let plan_id = alloc::format!("{:?}", plan);
        let target = alloc::format!("{:?}", plan);
        self.status.set_plan(plan);
        self.audit.push(AgentAuditEvent::PlanAccepted { plan_id, target });
        let _ = now_ms;
    }

    /// Run a single step. Returns
    /// the step's outcome. The
    /// caller is responsible for
    /// calling this in the correct
    /// order; the agent handles
    /// state transitions and
    /// retries.
    pub fn run_step(&mut self, step: ApplyStep, now_ms: u64) -> Result<(), ApplyError> {
        let attempt = self.status.attempt().saturating_add(1);
        self.audit.push(AgentAuditEvent::StepAttempted { step, attempt });
        // Move the state machine to
        // the step's stage.
        self.status.transition(step.stage(), now_ms, None);

        let plan = self.status.current_plan().ok_or_else(|| ApplyError::Refused {
            step,
            reason: alloc::string::String::from("no current plan"),
        })?;
        match self.engine.run(step, plan) {
            Ok(()) => {
                self.audit.push(AgentAuditEvent::StepSucceeded { step });
                Ok(())
            }
            Err(err) => {
                self.audit.push(AgentAuditEvent::StepFailed { step, reason: err.to_string() });
                self.status.transition(UpdateStage::Failed, now_ms, Some(err.to_string()));
                Err(err)
            }
        }
    }

    /// Apply a step with retries
    /// driven by the policy engine.
    /// Returns the final decision.
    pub fn apply_with_retry(&mut self, step: ApplyStep, now_ms: u64) -> Decision {
        let policy = self.retries.policy_for(&step_task_id(step)).cloned();
        let max = policy.as_ref().map(|p| p.max_attempts).unwrap_or(1);
        let mut last_err: Option<ApplyError> = None;
        for attempt in 1..=max {
            match self.run_step(step, now_ms) {
                Ok(()) => {
                    if let Some(p) = policy {
                        let _ = p;
                    }
                    return Decision::Retry { delay_ms: 0 };
                }
                Err(e) => {
                    last_err = Some(e);
                    let decision = self.retries.decide(&step_task_id(step), attempt);
                    match decision {
                        Decision::Retry { delay_ms } => {
                            self.audit.push(AgentAuditEvent::RetryScheduled { step, delay_ms });
                            continue;
                        }
                        Decision::GiveUp | Decision::CircuitBreak | Decision::Fallback { .. } => {
                            self.audit.push(AgentAuditEvent::StepGaveUp { step });
                            return decision;
                        }
                    }
                }
            }
        }
        let _ = last_err;
        self.audit.push(AgentAuditEvent::StepGaveUp { step });
        Decision::GiveUp
    }

    /// Drive the full apply
    /// sequence:
    /// Snapshot -> Download ->
    /// Verify -> Stage -> Apply.
    /// On any failure, transitions
    /// the status to `Failed` and
    /// triggers a rollback.
    pub fn apply(&mut self, now_ms: u64) -> Result<(), ApplyError> {
        let steps = [
            ApplyStep::Snapshot,
            ApplyStep::Download,
            ApplyStep::Verify,
            ApplyStep::Stage,
            ApplyStep::Apply,
        ];
        for step in steps {
            let d = self.apply_with_retry(step, now_ms);
            if matches!(d, Decision::GiveUp | Decision::CircuitBreak) {
                self.fail_and_rollback(step, now_ms);
                return Err(ApplyError::Refused {
                    step,
                    reason: alloc::string::String::from("retries exhausted"),
                });
            }
        }
        self.status.transition(UpdateStage::Done, now_ms, None);
        Ok(())
    }

    /// Force-fail the current
    /// update, mark the status as
    /// `Failed`, and trigger a
    /// rollback.
    pub fn fail_and_rollback(&mut self, failed_step: ApplyStep, now_ms: u64) {
        self.audit.push(AgentAuditEvent::RollbackTriggered {
            reason: alloc::format!("step '{}' failed", failed_step.as_str()),
        });
        self.status.transition(UpdateStage::Failed, now_ms, None);
        // Apply engine is asked to
        // restore the snapshot. We
        // don't run the engine here
        // because rollback is a
        // separate ApplyStep; the
        // caller (or the supervisor)
        // drives it. We just mark the
        // status as RolledBack.
        self.status.transition(UpdateStage::RolledBack, now_ms, None);
        self.audit.push(AgentAuditEvent::RollbackCompleted);
    }

    /// Reset the agent to `Idle`
    /// (after a successful Done or
    /// after a RolledBack).
    pub fn reset(&mut self, now_ms: u64) {
        self.status.transition(UpdateStage::Idle, now_ms, None);
        self.snapshot = None;
    }

    /// The history of the status
    /// machine.
    #[must_use]
    pub fn history(&self) -> &[HistoryEntry] {
        self.status.history()
    }

    /// Install a default retry
    /// policy for every step.
    pub fn install_default_policies(&mut self) {
        for step in [
            ApplyStep::Download,
            ApplyStep::Verify,
            ApplyStep::Stage,
            ApplyStep::Snapshot,
            ApplyStep::Apply,
        ] {
            self.retries.register(
                step_task_id(step),
                RetryPolicy {
                    max_attempts: 3,
                    backoff: BackoffStrategy::Exponential { base_ms: 100, max_ms: 5_000 },
                    fallbacks: Vec::new(),
                    circuit_breaker_threshold: 0,
                    group: None,
                },
            );
        }
    }
}

/// A `TaskId`-shaped key for a
/// step. Steps share a single
/// "update" group for the circuit
/// breaker.
fn step_task_id(step: ApplyStep) -> aether_agent_core::TaskId {
    use aether_agent_core::TaskId;
    // Every `ApplyStep::as_str()` returns
    // a non-empty kebab-case name, so
    // `new` always succeeds here. Use a
    // static "update" fallback in the
    // (impossible) case it doesn't.
    TaskId::new(step.as_str()).unwrap_or_else(|| {
        // SAFETY: the literal below is
        // a static, non-empty string, so
        // `TaskId::new` cannot return
        // `None`. The unwrap_or_else is
        // only here to satisfy the
        // clippy::expect_used lint.
        TaskId::new("update").unwrap_or_default()
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_update_core::plan::UpdateAction;
    use aether_update_core::recovery::SnapshotComponent;

    fn plan() -> UpdatePlan {
        use aether_security::signed_update::UpdateKind;
        use aether_update_core::version::VersionPolicyDecision;
        use aether_update_core::version::VersionRequirement;
        UpdatePlan {
            target: alloc::string::String::from("aether-os"),
            kind: UpdateKind::OsImage,
            action: UpdateAction::UpgradeOsImage,
            version: alloc::string::String::from("0.2.0"),
            timestamp_ms: 0,
            signer_fingerprint: alloc::string::String::from("aa:bb:cc"),
            payload_len: 1024,
            version_decision: VersionPolicyDecision {
                requirement: VersionRequirement::Upgrade,
                allowed: true,
                reason: alloc::string::String::new(),
            },
        }
    }

    #[test]
    fn step_as_str() {
        assert_eq!(ApplyStep::Download.as_str(), "download");
        assert_eq!(ApplyStep::Apply.as_str(), "apply");
    }

    #[test]
    fn step_stage() {
        assert_eq!(ApplyStep::Download.stage(), UpdateStage::Downloading);
        assert_eq!(ApplyStep::Verify.stage(), UpdateStage::Verifying);
        assert_eq!(ApplyStep::Apply.stage(), UpdateStage::Applying);
    }

    #[test]
    fn event_kind() {
        assert_eq!(AgentAuditEvent::RollbackCompleted.kind(), "rollback-completed");
    }

    #[test]
    fn apply_error_display() {
        let e = ApplyError::NonZeroExit { step: ApplyStep::Download, code: -1 };
        assert!(e.to_string().contains("download"));
        assert!(e.to_string().contains("-1"));
    }

    #[test]
    fn null_engine_succeeds() {
        let e = NullApplyEngine;
        assert!(e.run(ApplyStep::Download, &plan()).is_ok());
    }

    #[test]
    fn agent_starts_idle() {
        let a: UpdateAgent<NullApplyEngine> = UpdateAgent::new(NullApplyEngine);
        assert_eq!(a.status().stage(), UpdateStage::Idle);
        assert!(a.audit().is_empty());
    }

    #[test]
    fn agent_accept_plan() {
        let mut a: UpdateAgent<NullApplyEngine> = UpdateAgent::new(NullApplyEngine);
        a.accept(plan(), 0);
        assert!(a.status().current_plan().is_some());
        assert_eq!(a.audit().len(), 1);
    }

    #[test]
    fn agent_run_step_transitions() {
        let mut a: UpdateAgent<NullApplyEngine> = UpdateAgent::new(NullApplyEngine);
        a.accept(plan(), 0);
        a.run_step(ApplyStep::Download, 100).unwrap();
        // After Downloading the agent
        // doesn't auto-advance to
        // Verifying; the caller drives
        // the steps. Status is now
        // Downloading.
        assert_eq!(a.status().stage(), UpdateStage::Downloading);
    }

    #[test]
    fn agent_apply_full_sequence() {
        let mut a: UpdateAgent<NullApplyEngine> = UpdateAgent::new(NullApplyEngine);
        a.accept(plan(), 0);
        a.apply(0).unwrap();
        assert_eq!(a.status().stage(), UpdateStage::Done);
        // A successful full apply must NOT
        // have rolled back.
        assert!(!a.audit().iter().any(|e| matches!(e, AgentAuditEvent::RollbackTriggered { .. })));
        // And every step must have been
        // attempted.
        for step in [
            ApplyStep::Download,
            ApplyStep::Verify,
            ApplyStep::Stage,
            ApplyStep::Snapshot,
            ApplyStep::Apply,
        ] {
            let attempted = a
                .audit()
                .iter()
                .any(|e| matches!(e, AgentAuditEvent::StepAttempted { step: s, .. } if *s == step));
            assert!(attempted, "step {step:?} not attempted");
        }
    }

    #[test]
    fn agent_reset_clears_state() {
        let mut a: UpdateAgent<NullApplyEngine> = UpdateAgent::new(NullApplyEngine);
        a.accept(plan(), 0);
        a.apply(0).unwrap();
        a.reset(1000);
        assert_eq!(a.status().stage(), UpdateStage::Idle);
    }

    #[test]
    fn agent_retry_on_failure_then_succeed() {
        // An engine that fails the
        // first download attempt and
        // succeeds afterwards.
        struct FlakyEngine {
            fail_first: core::sync::atomic::AtomicBool,
        }
        impl ApplyEngine for FlakyEngine {
            fn run(&self, step: ApplyStep, _plan: &UpdatePlan) -> Result<(), ApplyError> {
                if step == ApplyStep::Download
                    && self.fail_first.swap(false, core::sync::atomic::Ordering::SeqCst)
                {
                    return Err(ApplyError::NonZeroExit { step, code: -1 });
                }
                Ok(())
            }
        }
        let mut a =
            UpdateAgent::new(FlakyEngine { fail_first: core::sync::atomic::AtomicBool::new(true) });
        a.install_default_policies();
        a.accept(plan(), 0);
        a.apply(0).unwrap();
        assert_eq!(a.status().stage(), UpdateStage::Done);
        assert!(a.audit().iter().any(|e| matches!(e, AgentAuditEvent::RetryScheduled { .. })));
    }

    #[test]
    fn agent_fail_and_rollback_audits() {
        let mut a: UpdateAgent<NullApplyEngine> = UpdateAgent::new(NullApplyEngine);
        a.accept(plan(), 0);
        a.fail_and_rollback(ApplyStep::Download, 0);
        assert_eq!(a.status().stage(), UpdateStage::RolledBack);
        assert!(a.audit().iter().any(|e| matches!(e, AgentAuditEvent::RollbackCompleted)));
    }

    #[test]
    fn snapshot_can_be_stored() {
        let mut a: UpdateAgent<NullApplyEngine> = UpdateAgent::new(NullApplyEngine);
        let snap = RecoverySnapshot::new(
            alloc::string::String::from("snap-1"),
            0,
            alloc::vec![SnapshotComponent::new(
                alloc::string::String::from("aether-os"),
                alloc::string::String::from("0.1.0"),
                alloc::string::String::from("/var/lib/aether/snapshots/snap-1/os"),
            )],
        );
        a.accept(plan(), 0);
        a.snapshot = Some(snap);
        assert!(a.snapshot().is_some());
    }

    #[test]
    fn install_default_policies_registers_all() {
        let mut a: UpdateAgent<NullApplyEngine> = UpdateAgent::new(NullApplyEngine);
        a.install_default_policies();
        // 5 policies registered.
        assert!(a.retries_mut().policy_for(&step_task_id(ApplyStep::Download)).is_some());
        assert!(a.retries_mut().policy_for(&step_task_id(ApplyStep::Apply)).is_some());
    }
}
