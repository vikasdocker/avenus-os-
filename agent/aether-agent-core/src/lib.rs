// Aether Agent Core - planning layer + state machine
// for the Aether autonomous agent.
//
// This crate is the **out-of-scope shell** for Phase 13.
// It defines the types a future `aether-agent-runtime`
// daemon will use, but contains no model integration,
// no I/O, and no executor. The point is to land the
// contract — the task graph, the proposal shape, the
// state machine — before any code that actually calls
// the model is written, so the model code can be
// reviewed against a stable surface.
//
// Responsibilities:
//   * `AgentTask` — a typed unit of work the agent
//     can schedule.
//   * `TaskGraph` — a DAG of `AgentTask`s with
//     dependency edges, with a ready-queue iterator
//     and cycle detection.
//   * `Proposal` — what the agent wants to do: an
//     action, a risk level, reasoning, and the
//     evidence (observation ids) that support the
//     proposal. Gated behind user consent for
//     high-risk proposals.
//   * `TaskStage` — the typed lifecycle of a task
//     (Proposed → Approved → Executing → Done /
//     Failed / Cancelled).
//   * `AgentStatus` — the in-memory state machine
//     for the agent's runtime: a queue of tasks, a
//     bounded history of completed tasks, and a
//     bounded observation log.
//   * `Observation` — a fact the agent has surfaced
//     about the system's state ("storage is 95%
//     full", "service aether-agentd has restarted 4
//     times in the last hour"). Tagged with a
//     severity so the future executor can decide
//     which observations warrant a proposal.
//   * `propose_from_observations` — the contract
//     for "the agent noticed X, so it proposes Y".
//
// Out of scope (lives in the future agent runtime):
//   * Calling the model / LLM.
//   * Actually executing a task (the future daemon
//     uses the existing `aether-system-core` IPC
//     surface for that).
//   * Persisting observations to disk.
//   * Cross-device coordination.
//
// Threading model: every public type is `Send + Sync`
// when the inner types are too. There is no internal
// mutability or background work; the future daemon is
// responsible for driving the state machine.

pub mod observation;
pub mod proposal;
pub mod state;
pub mod task;

pub use observation::{Observation, ObservationSeverity, OBSERVATION_LOG_LIMIT};
pub use proposal::{propose_from_observations, Proposal, ProposalError, ProposalId, ProposalRisk};
pub use state::{AgentStatus, HistoryEntry, TaskStage, MAX_TASK_HISTORY};
pub use task::{AgentTask, TaskDependencyError, TaskGraph, TaskId, TaskKind, TaskRisk};
