// Aether Update Core - planning layer + state machine for
// Aether self-updates.
//
// This crate is the **out-of-scope shell** for Phase 12.
// It defines the types a future `aether-update-agent`
// daemon will use, but contains no download / stage /
// apply logic. The point is to land the contract — the
// state machine, the plan shape, the version policy —
// before any I/O code is written, so the I/O code can be
// reviewed against a stable surface.
//
// Responsibilities:
//   * `UpdatePlan` — the declarative description of an
//     update: target, kind, target version, expected
//     source, monotonicity rule.
//   * `UpdateStage` — the typed set of states a live
//     update can be in.
//   * `UpdateStatus` — the in-memory state machine that
//     owns the current stage, attempt count, last error,
//     and a bounded history of transitions.
//   * `VersionPolicy` — accepts or rejects a target
//     version given the currently installed version, the
//     kind of update, and the operator's downgrade
//     permission.
//   * `RecoverySnapshot` — a typed record of "the
//     pre-update state" that rollback restores from.
//   * `plan_from_signed_update` — the integration point
//     with `aether-security::signed_update`: takes a
//     verified update and turns it into an `UpdatePlan`,
//     or rejects it on policy grounds.
//
// Out of scope (lives in the future update-agent daemon):
//   * Downloading the payload.
//   * Verifying cryptographic signatures (delegated to
//     `aether-security`).
//   * Staging the payload onto the filesystem.
//   * Atomically applying the update.
//   * Rolling back on failure.
//   * Rebooting the system.
//
// Threading model: every public type is `Send + Sync`
// when the inner types are too. There is no internal
// mutability or background work; the future daemon is
// responsible for driving the state machine.

pub mod plan;
pub mod recovery;
pub mod state;
pub mod version;

pub use plan::{plan_from_signed_update, UpdateAction, UpdatePlan, UpdatePlanError};
pub use recovery::{RecoverySnapshot, SnapshotComponent};
pub use state::{HistoryEntry, StageTransition, UpdateStage, UpdateStatus, MAX_HISTORY_ENTRIES};
pub use version::{VersionPolicy, VersionPolicyDecision, VersionRequirement};
