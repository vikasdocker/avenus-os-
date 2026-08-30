// Update state machine.
//
// `UpdateStatus` is the in-memory record of "where is
// the current update, if any, and what happened the
// last time we tried one?" It is the single source of
// truth for the future `aether-update-agent` daemon and
// the IPC layer that surfaces it.
//
// The state machine is intentionally small: the future
// daemon drives transitions by calling
// `UpdateStatus::transition`. The state machine does
// not call out to I/O; the daemon is responsible for
// doing the work *between* transitions and for
// recording the result with `transition`.
//
// Invariants:
//   * `stage` is always one of `UpdateStage`.
//   * `history` is bounded by `MAX_HISTORY_ENTRIES`.
//     Older entries are dropped on the front.
//   * `attempt` is incremented every time a new plan
//     is started, and reset to 0 only on a successful
//     `Done` transition.
//   * `last_error` is set on any transition to
//     `Failed`, and cleared on the next
//     `transition(Idle)` or `transition(Downloading)`.

use serde::{Deserialize, Serialize};

use crate::plan::UpdatePlan;

/// The maximum number of history entries kept in
/// memory. The IPC layer truncates earlier entries
/// when this limit is reached; the operator can still
/// read the audit log for the long-term record.
pub const MAX_HISTORY_ENTRIES: usize = 64;

/// The current stage of an in-flight update, or the
/// resting state of an idle system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateStage {
    /// The system is idle. No update is in flight.
    Idle,
    /// The update payload is being downloaded.
    Downloading,
    /// The downloaded payload is being verified
    /// (signature, content hash).
    Verifying,
    /// The verified payload is being copied to the
    /// staging area.
    Staging,
    /// The staged payload is being applied. The OS
    /// may reboot during this stage; on the next
    /// boot the agent reads the staged state and
    /// resumes.
    Applying,
    /// The update applied successfully.
    Done,
    /// The update failed at some stage; see
    /// `last_error` for the reason. The system is
    /// still in the previous (pre-update) state.
    Failed,
    /// The update failed and the system has been
    /// rolled back to the snapshot.
    RolledBack,
}

impl UpdateStage {
    /// Returns the canonical kebab-case name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Downloading => "downloading",
            Self::Verifying => "verifying",
            Self::Staging => "staging",
            Self::Applying => "applying",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::RolledBack => "rolled-back",
        }
    }

    /// Returns `true` for the resting states
    /// (Done, Failed, RolledBack). The IPC layer
    /// uses this to decide whether to surface a
    /// "what next?" hint.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::RolledBack)
    }
}

impl std::fmt::Display for UpdateStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single transition in the state machine. The
/// history is append-only; each entry records the
/// previous and new stage plus a wall-clock
/// timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageTransition {
    pub from: UpdateStage,
    pub to: UpdateStage,
    pub timestamp_ms: u64,
    pub note: Option<String>,
}

/// A history entry, pairing a transition with the plan
/// (if any) that drove it. Stored in `UpdateStatus::history`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub transition: StageTransition,
    pub plan: Option<UpdatePlan>,
}

/// The full state of the update subsystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateStatus {
    stage: UpdateStage,
    /// The plan currently in flight, if any.
    current_plan: Option<UpdatePlan>,
    /// The number of attempts the current plan has
    /// been through. Reset to 0 on a successful
    /// `Done` transition.
    attempt: u32,
    /// The most recent error message, if any.
    last_error: Option<String>,
    /// The bounded history of transitions.
    history: Vec<HistoryEntry>,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            stage: UpdateStage::Idle,
            current_plan: None,
            attempt: 0,
            last_error: None,
            history: Vec::new(),
        }
    }
}

impl UpdateStatus {
    /// Creates a fresh, idle status.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current stage.
    #[must_use]
    pub fn stage(&self) -> UpdateStage {
        self.stage
    }

    /// Returns the current plan, if any.
    #[must_use]
    pub fn current_plan(&self) -> Option<&UpdatePlan> {
        self.current_plan.as_ref()
    }

    /// Returns the attempt counter.
    #[must_use]
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Returns the last error message.
    #[must_use]
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Returns the bounded transition history,
    /// oldest-first.
    #[must_use]
    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    /// Drives the state machine to a new stage. The
    /// transition is appended to the history; the
    /// last-error is cleared on `Idle` /
    /// `Downloading` transitions and set on
    /// `Failed` transitions.
    ///
    /// Invalid transitions (e.g. `Done` → `Downloading`
    /// without an intermediate `Idle`) are allowed by
    /// the state machine — the future update-agent
    /// daemon is the source of truth on what
    /// transitions are appropriate. This crate
    /// records the state without judging the
    /// caller's choices, so the audit log captures
    /// any anomalies.
    pub fn transition(
        &mut self,
        to: UpdateStage,
        timestamp_ms: u64,
        note: Option<String>,
    ) -> &StageTransition {
        let from = self.stage;
        self.stage = to;
        if matches!(to, UpdateStage::Failed) {
            if let Some(ref n) = note {
                self.last_error = Some(n.clone());
            }
        } else if matches!(to, UpdateStage::Idle | UpdateStage::Downloading) {
            self.last_error = None;
        }
        if matches!(to, UpdateStage::Downloading) {
            // A new plan is starting: increment the
            // attempt counter.
            self.attempt = self.attempt.saturating_add(1);
        }
        if matches!(to, UpdateStage::Done) {
            self.attempt = 0;
        }
        let transition = StageTransition { from, to, timestamp_ms, note };
        self.history.push(HistoryEntry { transition, plan: self.current_plan.clone() });
        if self.history.len() > MAX_HISTORY_ENTRIES {
            let drop = self.history.len() - MAX_HISTORY_ENTRIES;
            self.history.drain(0..drop);
        }
        // The last entry is the one we just pushed; the
        // caller can read it through `last_transition`.
        &self.history[self.history.len() - 1].transition
    }

    /// Returns the most recent transition, if any.
    #[must_use]
    pub fn last_transition(&self) -> Option<&StageTransition> {
        self.history.last().map(|e| &e.transition)
    }

    /// Attaches a plan to the current status. Called
    /// by the future daemon immediately before
    /// transitioning to `Downloading`.
    pub fn set_plan(&mut self, plan: UpdatePlan) {
        self.current_plan = Some(plan);
    }

    /// Clears the current plan. Called after a
    /// `Done` / `Failed` / `RolledBack` transition
    /// when the daemon is ready to accept a new
    /// plan.
    pub fn clear_plan(&mut self) {
        self.current_plan = None;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::plan::UpdateAction;
    use aether_security::signed_update::UpdateKind;

    fn sample_plan() -> UpdatePlan {
        UpdatePlan {
            target: "aether-os".to_string(),
            kind: UpdateKind::OsImage,
            action: UpdateAction::UpgradeOsImage,
            version: "1.2.0".to_string(),
            timestamp_ms: 1_700_000_000_000,
            signer_fingerprint: "deadbeef".repeat(4),
            payload_len: 1024,
            version_decision: crate::version::VersionPolicyDecision {
                requirement: crate::version::VersionRequirement::Upgrade,
                allowed: true,
                reason: String::new(),
            },
        }
    }

    #[test]
    fn new_status_is_idle() {
        let s = UpdateStatus::new();
        assert_eq!(s.stage(), UpdateStage::Idle);
        assert!(s.current_plan().is_none());
        assert_eq!(s.attempt(), 0);
        assert!(s.last_error().is_none());
        assert!(s.history().is_empty());
    }

    #[test]
    fn transition_records_history() {
        let mut s = UpdateStatus::new();
        s.transition(UpdateStage::Downloading, 100, None);
        s.transition(UpdateStage::Verifying, 200, None);
        s.transition(UpdateStage::Failed, 300, Some("network".to_string()));
        assert_eq!(s.stage(), UpdateStage::Failed);
        assert_eq!(s.history().len(), 3);
        assert_eq!(s.history()[0].transition.from, UpdateStage::Idle);
        assert_eq!(s.history()[0].transition.to, UpdateStage::Downloading);
        assert_eq!(s.history()[2].transition.to, UpdateStage::Failed);
        assert_eq!(s.last_error(), Some("network"));
    }

    #[test]
    fn attempt_counter_increments_on_downloading() {
        let mut s = UpdateStatus::new();
        s.transition(UpdateStage::Downloading, 100, None);
        s.transition(UpdateStage::Failed, 200, Some("e".to_string()));
        s.transition(UpdateStage::Downloading, 300, None);
        assert_eq!(s.attempt(), 2);
    }

    #[test]
    fn done_resets_attempt_counter() {
        let mut s = UpdateStatus::new();
        s.transition(UpdateStage::Downloading, 100, None);
        s.transition(UpdateStage::Done, 200, None);
        assert_eq!(s.attempt(), 0);
    }

    #[test]
    fn idle_clears_last_error() {
        let mut s = UpdateStatus::new();
        s.transition(UpdateStage::Failed, 100, Some("e".to_string()));
        s.transition(UpdateStage::Idle, 200, None);
        assert!(s.last_error().is_none());
    }

    #[test]
    fn downloading_clears_last_error() {
        let mut s = UpdateStatus::new();
        s.transition(UpdateStage::Failed, 100, Some("e".to_string()));
        s.transition(UpdateStage::Downloading, 200, None);
        assert!(s.last_error().is_none());
    }

    #[test]
    fn history_is_bounded() {
        let mut s = UpdateStatus::new();
        // Push more than MAX_HISTORY_ENTRIES.
        for i in 0..(MAX_HISTORY_ENTRIES + 10) {
            let stage = if i % 2 == 0 { UpdateStage::Downloading } else { UpdateStage::Failed };
            s.transition(stage, i as u64, None);
        }
        assert_eq!(s.history().len(), MAX_HISTORY_ENTRIES);
    }

    #[test]
    fn set_and_clear_plan() {
        let mut s = UpdateStatus::new();
        s.set_plan(sample_plan());
        assert!(s.current_plan().is_some());
        s.clear_plan();
        assert!(s.current_plan().is_none());
    }

    #[test]
    fn stage_is_terminal_for_done_failed_rolledback() {
        assert!(!UpdateStage::Idle.is_terminal());
        assert!(!UpdateStage::Downloading.is_terminal());
        assert!(UpdateStage::Done.is_terminal());
        assert!(UpdateStage::Failed.is_terminal());
        assert!(UpdateStage::RolledBack.is_terminal());
    }

    #[test]
    fn stage_as_str_is_stable() {
        assert_eq!(UpdateStage::Idle.as_str(), "idle");
        assert_eq!(UpdateStage::Downloading.as_str(), "downloading");
        assert_eq!(UpdateStage::Verifying.as_str(), "verifying");
        assert_eq!(UpdateStage::Staging.as_str(), "staging");
        assert_eq!(UpdateStage::Applying.as_str(), "applying");
        assert_eq!(UpdateStage::Done.as_str(), "done");
        assert_eq!(UpdateStage::Failed.as_str(), "failed");
        assert_eq!(UpdateStage::RolledBack.as_str(), "rolled-back");
    }

    #[test]
    fn last_transition_returns_most_recent() {
        let mut s = UpdateStatus::new();
        s.transition(UpdateStage::Downloading, 100, None);
        s.transition(UpdateStage::Verifying, 200, None);
        let t = s.last_transition().expect("present");
        assert_eq!(t.from, UpdateStage::Downloading);
        assert_eq!(t.to, UpdateStage::Verifying);
        assert_eq!(t.timestamp_ms, 200);
    }
}
