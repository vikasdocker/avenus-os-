// Agent state machine.
//
// `AgentStatus` is the in-memory record of "where is
// the agent, what tasks does it have in flight, and
// what happened the last time it tried to run one?"
// It is the single source of truth for the future
// `aether-agent-runtime` daemon and the IPC layer
// that surfaces it.
//
// The state machine is intentionally small: the
// future runtime drives transitions by calling
// `AgentStatus::transition`. The shell does not call
// out to I/O; the runtime is responsible for doing
// the work *between* transitions and for recording
// the result with `transition`.
//
// Invariants:
//   * `tasks` is the live task graph (Proposed /
//     Approved / Executing).
//   * `history` is bounded by `MAX_TASK_HISTORY`.
//     Older entries are dropped on the front.
//   * `observations` is bounded by
//     `OBSERVATION_LOG_LIMIT` (from
//     `observation.rs`).
//   * `proposals` is the set of pending proposals
//     (those that have been validated but not yet
//     acted on).

use serde::{Deserialize, Serialize};

use crate::observation::{Observation, OBSERVATION_LOG_LIMIT};
use crate::proposal::{Proposal, ProposalId};
use crate::task::{AgentTask, TaskGraph, TaskId};

/// The maximum number of completed tasks kept in
/// memory. The IPC layer truncates older entries
/// when this limit is reached.
pub const MAX_TASK_HISTORY: usize = 64;

/// The current stage of a task in the agent's
/// lifecycle. Mirrors the Aether `UpdateStage`
/// vocabulary for consistency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStage {
    /// The task is on the graph but has not been
    /// picked up by the executor yet.
    Proposed,
    /// The user has approved the task (or it was
    /// low-risk and did not need approval).
    Approved,
    /// The executor is running the task.
    Executing,
    /// The task completed successfully.
    Done,
    /// The task failed; see `last_error` on the
    /// history entry for the reason.
    Failed,
    /// The task was cancelled (by the user, or by
    /// the runtime because a dependency failed).
    Cancelled,
}

impl TaskStage {
    /// Returns the canonical kebab-case name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Approved => "approved",
            Self::Executing => "executing",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Returns `true` for the resting states.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

impl std::fmt::Display for TaskStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A completed or failed task, kept in the bounded
/// `history`. The `stage` is always one of the
/// terminal stages (`Done`, `Failed`, `Cancelled`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub task: AgentTask,
    pub stage: TaskStage,
    pub timestamp_ms: u64,
    pub note: Option<String>,
}

/// The full state of the agent subsystem.
#[derive(Debug, Default)]
pub struct AgentStatus {
    tasks: TaskGraph,
    /// The set of pending proposals. Indexed by id
    /// for fast lookup.
    proposals: std::collections::BTreeMap<ProposalId, Proposal>,
    /// The bounded history of completed tasks.
    history: Vec<HistoryEntry>,
    /// The bounded observation log.
    observations: Vec<Observation>,
}

impl AgentStatus {
    /// Creates a fresh, empty status.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // -- task graph accessors --

    /// Returns a read-only view of the live task
    /// graph.
    #[must_use]
    pub fn tasks(&self) -> &TaskGraph {
        &self.tasks
    }

    /// Returns the number of live tasks.
    #[must_use]
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    // -- proposal accessors --

    /// Records a proposal. Returns `true` if the
    /// proposal is new; `false` if an existing
    /// proposal with the same id is overwritten.
    pub fn add_proposal(&mut self, proposal: Proposal) -> bool {
        self.proposals.insert(proposal.id.clone(), proposal).is_none()
    }

    /// Removes a proposal by id. Returns the
    /// removed proposal, if any.
    pub fn remove_proposal(&mut self, id: &ProposalId) -> Option<Proposal> {
        self.proposals.remove(id)
    }

    /// Returns a proposal by id.
    #[must_use]
    pub fn proposal(&self, id: &ProposalId) -> Option<&Proposal> {
        self.proposals.get(id)
    }

    /// Returns all pending proposals, sorted by id.
    #[must_use]
    pub fn proposals(&self) -> Vec<&Proposal> {
        self.proposals.values().collect()
    }

    /// Returns the number of pending proposals.
    #[must_use]
    pub fn proposal_count(&self) -> usize {
        self.proposals.len()
    }

    // -- history accessors --

    /// Returns the bounded task history,
    /// oldest-first.
    #[must_use]
    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    /// Returns the number of history entries.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Appends a terminal history entry. The stage
    /// must be one of `Done` / `Failed` /
    /// `Cancelled`.
    pub fn record_history(&mut self, entry: HistoryEntry) {
        debug_assert!(entry.stage.is_terminal());
        self.history.push(entry);
        if self.history.len() > MAX_TASK_HISTORY {
            let drop = self.history.len() - MAX_TASK_HISTORY;
            self.history.drain(0..drop);
        }
    }

    // -- observation accessors --

    /// Records an observation. The oldest
    /// observation is dropped on overflow.
    pub fn add_observation(&mut self, observation: Observation) {
        self.observations.push(observation);
        if self.observations.len() > OBSERVATION_LOG_LIMIT {
            let drop = self.observations.len() - OBSERVATION_LOG_LIMIT;
            self.observations.drain(0..drop);
        }
    }

    /// Returns the bounded observation log,
    /// oldest-first.
    #[must_use]
    pub fn observations(&self) -> &[Observation] {
        &self.observations
    }

    /// Returns the number of observations.
    #[must_use]
    pub fn observation_count(&self) -> usize {
        self.observations.len()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::observation::ObservationSeverity;
    use crate::task::TaskKind;

    fn sample_task(id: &str) -> AgentTask {
        AgentTask::new(id, TaskKind::Notify, "title", "description").expect("valid task")
    }

    fn sample_observation(id: &str) -> Observation {
        Observation::new(id, "storage", "summary", ObservationSeverity::Info, 1).expect("valid")
    }

    fn sample_proposal(id: &str) -> Proposal {
        Proposal::new(id, TaskKind::Notify, "title", "description", "reasoning", crate::proposal::ProposalRisk::Low, 1)
            .expect("valid")
    }

    #[test]
    fn new_status_is_empty() {
        let s = AgentStatus::new();
        assert_eq!(s.task_count(), 0);
        assert_eq!(s.proposal_count(), 0);
        assert_eq!(s.history_len(), 0);
        assert_eq!(s.observation_count(), 0);
    }

    #[test]
    fn add_proposal_dedupes_on_id() {
        let mut s = AgentStatus::new();
        let p1 = sample_proposal("p1");
        let p2 = sample_proposal("p1");
        assert!(s.add_proposal(p1));
        assert!(!s.add_proposal(p2));
        assert_eq!(s.proposal_count(), 1);
    }

    #[test]
    fn remove_proposal_returns_removed() {
        let mut s = AgentStatus::new();
        let p = sample_proposal("p1");
        s.add_proposal(p);
        let id = ProposalId::new("p1").unwrap();
        let removed = s.remove_proposal(&id);
        assert!(removed.is_some());
        assert!(s.proposal(&id).is_none());
    }

    #[test]
    fn record_history_drops_overflow() {
        let mut s = AgentStatus::new();
        for i in 0..(MAX_TASK_HISTORY + 10) {
            let task = sample_task(&format!("t{i}"));
            s.record_history(HistoryEntry {
                task,
                stage: TaskStage::Done,
                timestamp_ms: i as u64,
                note: None,
            });
        }
        assert_eq!(s.history_len(), MAX_TASK_HISTORY);
    }

    #[test]
    fn add_observation_drops_overflow() {
        let mut s = AgentStatus::new();
        for i in 0..(OBSERVATION_LOG_LIMIT + 5) {
            s.add_observation(sample_observation(&format!("o{i}")));
        }
        assert_eq!(s.observation_count(), OBSERVATION_LOG_LIMIT);
    }

    #[test]
    fn stage_is_terminal_for_done_failed_cancelled() {
        assert!(!TaskStage::Proposed.is_terminal());
        assert!(!TaskStage::Approved.is_terminal());
        assert!(!TaskStage::Executing.is_terminal());
        assert!(TaskStage::Done.is_terminal());
        assert!(TaskStage::Failed.is_terminal());
        assert!(TaskStage::Cancelled.is_terminal());
    }

    #[test]
    fn stage_as_str_is_stable() {
        assert_eq!(TaskStage::Proposed.as_str(), "proposed");
        assert_eq!(TaskStage::Approved.as_str(), "approved");
        assert_eq!(TaskStage::Executing.as_str(), "executing");
        assert_eq!(TaskStage::Done.as_str(), "done");
        assert_eq!(TaskStage::Failed.as_str(), "failed");
        assert_eq!(TaskStage::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn tasks_returns_a_read_only_view() {
        let mut s = AgentStatus::new();
        let t = sample_task("a");
        s.tasks_mut_for_test().insert(t).expect("insert");
        assert_eq!(s.tasks().len(), 1);
    }
}

// Test-only accessor: the public API does not expose
// `&mut TaskGraph` because the future runtime is the
// only thing that should mutate it. Tests need to
// insert tasks to exercise the accessors.
impl AgentStatus {
    #[cfg(test)]
    pub(crate) fn tasks_mut_for_test(&mut self) -> &mut TaskGraph {
        &mut self.tasks
    }

    /// Removes a task from the live graph by id.
    /// Returns the removed task. Used by the
    /// future runtime when it finishes a task.
    /// Public so the IPC layer's `agent.cancel`
    /// command can use it.
    pub fn remove_task(&mut self, id: &TaskId) -> Option<AgentTask> {
        self.tasks.remove(id)
    }

    /// Inserts a task into the live graph.
    /// Returns the dependency error if the
    /// task is duplicate / cyclic / has
    /// unknown dependencies. The IPC layer's
    /// `agent.approve` command uses this; the
    /// future runtime calls it directly when
    /// it schedules a task.
    pub fn insert_task(
        &mut self,
        task: AgentTask,
    ) -> Result<(), crate::task::TaskDependencyError> {
        self.tasks.insert(task)
    }
}
