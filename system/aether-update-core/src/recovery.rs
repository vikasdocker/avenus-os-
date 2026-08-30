// Recovery snapshot: a typed record of "the pre-update
// state" that rollback restores from.
//
// A snapshot is constructed immediately before an
// `Applying` transition. The future `aether-update-agent`
// daemon is responsible for writing the actual data
// (the old OS image, the old service bundle, the
// previous model file, etc.) to durable storage; this
// type only describes *what* was snapshotted, not
// *where the bytes live*.
//
// A snapshot is consumed by `RecoverySnapshot::restore`
// (a no-op in the shell — the real restore logic lives
// in the future daemon). The shell keeps the type
// because the IPC layer reports snapshots back to the
// operator and the audit log records their IDs.

use serde::{Deserialize, Serialize};

/// A single component included in a recovery snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotComponent {
    /// The component id (matches `UpdatePlan::target`).
    pub target: String,
    /// The version of the component before the
    /// update was applied.
    pub from_version: String,
    /// Where the component's pre-update bytes are
    /// stored. The shell accepts any non-empty
    /// string; the future daemon will document the
    /// conventions (e.g. `/var/lib/aether/snapshots/<id>/`).
    pub stash_path: String,
    /// A short free-form note ("service bundle
    /// replaced via staging-area" / "OS image copied
    /// from active partition").
    pub note: Option<String>,
}

impl SnapshotComponent {
    /// Creates a new component record.
    #[must_use]
    pub fn new(
        target: impl Into<String>,
        from_version: impl Into<String>,
        stash_path: impl Into<String>,
    ) -> Self {
        Self {
            target: target.into(),
            from_version: from_version.into(),
            stash_path: stash_path.into(),
            note: None,
        }
    }

    /// Attaches a free-form note.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// A complete recovery snapshot. Contains the list of
/// components that were snapshotted, when the snapshot
/// was taken, and a short description of the update
/// that triggered it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySnapshot {
    /// A unique identifier for this snapshot. The
    /// future daemon generates a UUIDv7 or a
    /// timestamp-derived id; the shell accepts any
    /// non-empty string.
    pub snapshot_id: String,
    /// The wall-clock timestamp at which the
    /// snapshot was taken.
    pub taken_at_ms: u64,
    /// The components included in the snapshot.
    pub components: Vec<SnapshotComponent>,
}

impl RecoverySnapshot {
    /// Creates a new snapshot. `snapshot_id` must be
    /// non-empty; `components` must be non-empty (an
    /// empty snapshot is meaningless).
    #[must_use]
    pub fn new(
        snapshot_id: impl Into<String>,
        taken_at_ms: u64,
        components: Vec<SnapshotComponent>,
    ) -> Self {
        Self {
            snapshot_id: snapshot_id.into(),
            taken_at_ms,
            components,
        }
    }

    /// Returns `true` if every component has a
    /// non-empty `stash_path`. Used by the future
    /// daemon to decide whether the snapshot is
    /// restorable.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.snapshot_id.is_empty()
            && !self.components.is_empty()
            && self.components.iter().all(|c| !c.stash_path.is_empty())
    }

    /// Returns the version a given target is at in
    /// this snapshot, if any.
    #[must_use]
    pub fn version_of(&self, target: &str) -> Option<&str> {
        self.components
            .iter()
            .find(|c| c.target == target)
            .map(|c| c.from_version.as_str())
    }

    /// Returns the number of components in the
    /// snapshot.
    #[must_use]
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Returns the targets in the snapshot.
    #[must_use]
    pub fn targets(&self) -> Vec<&str> {
        self.components.iter().map(|c| c.target.as_str()).collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn sample_snapshot() -> RecoverySnapshot {
        RecoverySnapshot::new(
            "snap-1",
            1_700_000_000_000,
            vec![
                SnapshotComponent::new(
                    "aether-os",
                    "1.1.0",
                    "/var/lib/aether/snapshots/snap-1/os",
                )
                .with_note("active partition copied to B"),
                SnapshotComponent::new(
                    "aether-agentd",
                    "0.5.0",
                    "/var/lib/aether/snapshots/snap-1/agentd",
                ),
            ],
        )
    }

    #[test]
    fn complete_snapshot_is_complete() {
        let s = sample_snapshot();
        assert!(s.is_complete());
    }

    #[test]
    fn empty_components_is_incomplete() {
        let s = RecoverySnapshot::new("snap-1", 0, vec![]);
        assert!(!s.is_complete());
    }

    #[test]
    fn empty_snapshot_id_is_incomplete() {
        let s = RecoverySnapshot::new("", 0, vec![SnapshotComponent::new("a", "1", "/p")]);
        assert!(!s.is_complete());
    }

    #[test]
    fn empty_stash_path_is_incomplete() {
        let s = RecoverySnapshot::new(
            "snap-1",
            0,
            vec![SnapshotComponent::new("a", "1", "")],
        );
        assert!(!s.is_complete());
    }

    #[test]
    fn version_of_returns_correct_version() {
        let s = sample_snapshot();
        assert_eq!(s.version_of("aether-os"), Some("1.1.0"));
        assert_eq!(s.version_of("aether-agentd"), Some("0.5.0"));
        assert_eq!(s.version_of("nope"), None);
    }

    #[test]
    fn targets_returns_all_component_targets() {
        let s = sample_snapshot();
        let mut t = s.targets();
        t.sort();
        assert_eq!(t, vec!["aether-agentd", "aether-os"]);
    }

    #[test]
    fn component_count_matches_components() {
        let s = sample_snapshot();
        assert_eq!(s.component_count(), 2);
    }

    #[test]
    fn snapshot_with_note_is_serialised() {
        let s = sample_snapshot();
        let j = serde_json::to_string(&s).expect("serialises");
        assert!(j.contains("\"note\":\"active partition copied to B\""));
    }
}
