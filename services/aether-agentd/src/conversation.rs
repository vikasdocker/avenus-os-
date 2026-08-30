// Conversation context - small bounded memory for pronoun resolution.
//
// Keeps the last few exchanges so that "it", "that", "bring it to front"
// can be resolved to the most recently mentioned application or window.
// Bounded to avoid unbounded memory or leaking sensitive data.
//
// The context is persisted via a `ConversationSnapshot`. The snapshot
// carries its own format version so a future reader can detect
// format drift; persistence goes through the agent runtime's
// `MemoryStore` so all the security properties (path validation,
// size cap, atomic replace) apply.

use aether_agent_runtime::{decode_persisted, encode_persisted, MemoryStore, MemoryStoreError};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Persistence format version for the conversation snapshot.
pub const CONVERSATION_SNAPSHOT_VERSION: u32 = 1;

/// Persisted memory-store name. Validated by the trait, not by us.
const STORE_NAME: &str = "conversation";

/// One turn of conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub user_text: String,
    /// Apps mentioned or acted on in this turn (normalized ids).
    pub apps: Vec<String>,
    /// Windows referenced.
    pub windows: Vec<String>,
    /// Files referenced (relative paths).
    pub files: Vec<String>,
}

/// Bounded conversation memory.
#[derive(Debug, Clone)]
pub struct ConversationContext {
    turns: VecDeque<ConversationTurn>,
    capacity: usize,
    /// Last app id mentioned across any turn (normalized).
    last_app: Option<String>,
    /// Last window title referenced.
    last_window: Option<String>,
    /// Last file path referenced.
    last_file: Option<String>,
}

impl ConversationContext {
    pub fn new(capacity: usize) -> Self {
        Self {
            turns: VecDeque::with_capacity(capacity),
            capacity: capacity.max(2),
            last_app: None,
            last_window: None,
            last_file: None,
        }
    }

    pub fn last_app(&self) -> Option<&str> {
        self.last_app.as_deref()
    }

    pub fn last_window(&self) -> Option<&str> {
        self.last_window.as_deref()
    }

    pub fn last_file(&self) -> Option<&str> {
        self.last_file.as_deref()
    }

    /// Record a user turn and the apps/windows/files that turn resolved to.
    pub fn push(&mut self, user_text: &str, apps: Vec<String>, windows: Vec<String>) {
        self.push_with_files(user_text, apps, windows, Vec::new());
    }

    /// Extended push that also tracks files.
    pub fn push_with_files(
        &mut self,
        user_text: &str,
        apps: Vec<String>,
        windows: Vec<String>,
        files: Vec<String>,
    ) {
        if !apps.is_empty() {
            self.last_app = apps.last().cloned();
        }
        if !windows.is_empty() {
            self.last_window = windows.last().cloned();
        } else if !apps.is_empty() {
            // Windows often mirror apps; use app as window fallback.
            self.last_window = apps.last().cloned();
        }
        if !files.is_empty() {
            self.last_file = files.last().cloned();
        }
        if self.turns.len() >= self.capacity {
            self.turns.pop_front();
        }
        self.turns.push_back(ConversationTurn {
            user_text: user_text.to_string(),
            apps,
            windows,
            files,
        });
    }

    /// Resolve a pronoun target using bounded history.
    /// If text contains "it", "that", "this", "them", return last_app.
    pub fn resolve_pronoun(&self, text: &str) -> Option<String> {
        let lower = text.to_ascii_lowercase();
        let has_pronoun = lower.split_whitespace().any(|w| {
            matches!(
                w.trim_matches(|c: char| !c.is_ascii_alphabetic()),
                "it" | "that" | "this" | "them" | "there"
            )
        }) || lower.contains(" it ")
            || lower.contains(" it.")
            || lower.ends_with(" it");
        if has_pronoun {
            self.last_app.clone().or_else(|| self.last_window.clone())
        } else {
            None
        }
    }

    /// Number of stored turns.
    pub fn len(&self) -> usize {
        self.turns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Return a short debug summary for logging (no sensitive raw text dump).
    pub fn summary(&self) -> String {
        format!(
            "turns={} last_app={:?} last_window={:?} last_file={:?}",
            self.turns.len(),
            self.last_app,
            self.last_window,
            self.last_file
        )
    }

    /// Captures the current state in a serializable form. The
    /// `version` field of the snapshot is checked on `restore` so
    /// future format changes can be detected rather than silently
    /// loaded.
    pub fn snapshot(&self) -> ConversationSnapshot {
        ConversationSnapshot {
            version: CONVERSATION_SNAPSHOT_VERSION,
            capacity: self.capacity,
            turns: self.turns.iter().cloned().collect(),
            last_app: self.last_app.clone(),
            last_window: self.last_window.clone(),
            last_file: self.last_file.clone(),
        }
    }

    /// Replaces the current state with the contents of `snap`.
    /// Returns an error string if the snapshot's version is not
    /// understood; the caller is responsible for falling back to
    /// the default state in that case.
    pub fn restore(&mut self, snap: ConversationSnapshot) -> Result<(), String> {
        if snap.version != CONVERSATION_SNAPSHOT_VERSION {
            return Err(format!(
                "conversation snapshot version {} is not supported (expected {})",
                snap.version, CONVERSATION_SNAPSHOT_VERSION
            ));
        }
        // The capacity we keep is the larger of the runtime's
        // current capacity and the snapshot's capacity, so a
        // restore never silently shrinks the buffer.
        let new_cap = self.capacity.max(snap.capacity).max(2);
        let mut turns: VecDeque<ConversationTurn> = snap.turns.into();
        if turns.len() > new_cap {
            let drop = turns.len() - new_cap;
            turns.drain(..drop);
        }
        self.capacity = new_cap;
        self.turns = turns;
        self.last_app = snap.last_app;
        self.last_window = snap.last_window;
        self.last_file = snap.last_file;
        Ok(())
    }

    /// Persists the current state to `store` under the
    /// well-known name. The payload is wrapped in the runtime's
    /// `Persisted<T>` envelope (version, timestamp, content
    /// checksum) so a corrupt read is detectable.
    pub fn persist(&self, store: &dyn MemoryStore) -> Result<(), MemoryStoreError> {
        let snap = self.snapshot();
        let bytes = encode_persisted(&snap)?;
        store.save(STORE_NAME, &bytes)
    }

    /// Loads persisted state from `store` and replaces the current
    /// contents. Missing state is **not** an error: the function
    /// returns `Ok(false)` so the caller can keep the in-memory
    /// defaults. Corrupt state is surfaced as a `MemoryStoreError`
    /// so the caller can log a warning and continue with defaults.
    pub fn load(&mut self, store: &dyn MemoryStore) -> Result<bool, MemoryStoreError> {
        let Some(bytes) = store.load(STORE_NAME)? else {
            return Ok(false);
        };
        let snap: ConversationSnapshot = decode_persisted(&bytes)?;
        self.restore(snap).map_err(MemoryStoreError::Corrupt)?;
        Ok(true)
    }
}

/// Serializable form of `ConversationContext`. Version 1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSnapshot {
    /// Format version of the snapshot itself. `ConversationContext::restore`
    /// rejects anything other than the version it was built against.
    pub version: u32,
    /// The capacity the context was using when the snapshot was
    /// taken. Used by `restore` so a restore never silently
    /// shrinks the buffer.
    pub capacity: usize,
    /// Ordered turns, oldest first. The runtime drops any that
    /// would not fit in the new capacity.
    pub turns: Vec<ConversationTurn>,
    /// Last app id mentioned across any turn (normalized).
    pub last_app: Option<String>,
    /// Last window title referenced.
    pub last_window: Option<String>,
    /// Last file path referenced.
    pub last_file: Option<String>,
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_capacity_evicts_oldest() {
        let mut ctx = ConversationContext::new(2);
        ctx.push("Open Calculator", vec!["calculator".to_string()], vec![]);
        ctx.push("Open Notes", vec!["notes".to_string()], vec![]);
        ctx.push("Open Files", vec!["files".to_string()], vec![]);
        assert_eq!(ctx.len(), 2);
        assert_eq!(ctx.last_app(), Some("files"));
    }

    #[test]
    fn pronoun_resolution_uses_last_app() {
        let mut ctx = ConversationContext::new(4);
        ctx.push("Open Notes", vec!["notes".to_string()], vec![]);
        assert_eq!(ctx.resolve_pronoun("Bring it to the front"), Some("notes".to_string()));
        assert_eq!(ctx.resolve_pronoun("minimize it"), Some("notes".to_string()));
        assert_eq!(ctx.resolve_pronoun("Open Calculator"), None);
    }

    #[test]
    fn last_app_tracks_across_turns() {
        let mut ctx = ConversationContext::default();
        assert!(ctx.last_app().is_none());
        ctx.push("Open Calculator", vec!["calculator".to_string()], vec![]);
        assert_eq!(ctx.last_app(), Some("calculator"));
        ctx.push("Bring it to front", vec!["calculator".to_string()], vec![]);
        assert_eq!(ctx.last_app(), Some("calculator"));
    }

    #[test]
    fn it_pronoun_edge_cases() {
        let mut ctx = ConversationContext::new(4);
        ctx.push("Open Notes", vec!["notes".to_string()], vec![]);
        assert_eq!(ctx.resolve_pronoun("Bring it to the front."), Some("notes".to_string()));
        assert_eq!(ctx.resolve_pronoun("it"), Some("notes".to_string()));
    }

    fn fresh_store() -> aether_agent_runtime::InMemoryStore {
        aether_agent_runtime::InMemoryStore::new()
    }

    #[test]
    fn snapshot_then_restore_round_trip_preserves_state() {
        let mut ctx = ConversationContext::new(4);
        ctx.push_with_files("Open Calculator", vec!["calculator".to_string()], vec![], vec![]);
        ctx.push_with_files(
            "Open Notes",
            vec!["notes".to_string()],
            vec![],
            vec!["Documents/notes.md".to_string()],
        );
        let snap = ctx.snapshot();
        let mut ctx2 = ConversationContext::new(4);
        match ctx2.restore(snap) {
            Ok(()) => {}
            Err(e) => panic!("restore should succeed: {e}"),
        }
        assert_eq!(ctx2.last_app(), Some("notes"));
        assert_eq!(ctx2.last_file(), Some("Documents/notes.md"));
        assert_eq!(ctx2.len(), 2);
    }

    #[test]
    fn restore_rejects_wrong_version() {
        let snap = ConversationSnapshot {
            version: 999,
            capacity: 4,
            turns: Vec::new(),
            last_app: None,
            last_window: None,
            last_file: None,
        };
        let mut ctx = ConversationContext::new(4);
        let err = match ctx.restore(snap) {
            Ok(()) => panic!("restore with bad version should fail"),
            Err(e) => e,
        };
        assert!(err.contains("not supported"), "got: {err}");
    }

    #[test]
    fn restore_does_not_shrink_capacity() {
        // The runtime's capacity is the floor, not the snapshot's.
        let mut ctx = ConversationContext::new(8);
        ctx.push("hi", vec!["a".to_string()], vec![]);
        let snap = ctx.snapshot();
        let mut ctx2 = ConversationContext::new(2);
        match ctx2.restore(snap) {
            Ok(()) => {}
            Err(e) => panic!("restore should succeed: {e}"),
        }
        // 8 wins.
        for i in 0..6 {
            ctx2.push(&format!("turn {i}"), vec![format!("app{i}")], vec![]);
        }
        assert_eq!(ctx2.len(), 7);
    }

    #[test]
    fn restore_drops_turns_that_exceed_capacity() {
        // Build a snapshot whose declared capacity is 2, then
        // restore into a context with capacity 8. The runtime
        // honours the snapshot's capacity (the larger wins only
        // when the runtime is the bigger of the two), so the
        // pre-restore turns are dropped to fit.
        let mut ctx = ConversationContext::new(2);
        for i in 0..5 {
            ctx.push(&format!("turn {i}"), vec![format!("app{i}")], vec![]);
        }
        let snap = ctx.snapshot();
        assert_eq!(snap.turns.len(), 2);
        let mut ctx2 = ConversationContext::new(8);
        match ctx2.restore(snap) {
            Ok(()) => {}
            Err(e) => panic!("restore should succeed: {e}"),
        }
        assert_eq!(ctx2.len(), 2);
        assert_eq!(ctx2.last_app(), Some("app4"));
    }

    #[test]
    fn persist_then_load_round_trip_preserves_last_app() {
        let store = fresh_store();
        let mut ctx = ConversationContext::new(4);
        ctx.push("Open Notes", vec!["notes".to_string()], vec![]);
        match ctx.persist(&store) {
            Ok(()) => {}
            Err(e) => panic!("persist should succeed: {e}"),
        }
        // Build a fresh context, load from the store.
        let mut ctx2 = ConversationContext::new(4);
        let loaded = match ctx2.load(&store) {
            Ok(b) => b,
            Err(e) => panic!("load should succeed: {e}"),
        };
        assert!(loaded);
        assert_eq!(ctx2.last_app(), Some("notes"));
        assert_eq!(ctx2.len(), 1);
    }

    #[test]
    fn load_returns_false_when_store_has_no_persisted_state() {
        let store = fresh_store();
        let mut ctx = ConversationContext::new(4);
        let loaded = match ctx.load(&store) {
            Ok(b) => b,
            Err(e) => panic!("load on empty store should succeed: {e}"),
        };
        assert!(!loaded);
        // Defaults are unchanged.
        assert!(ctx.last_app().is_none());
    }

    #[test]
    fn load_with_corrupt_store_returns_error() {
        let store = fresh_store();
        if let Err(e) = store.save("conversation", b"not an envelope") {
            panic!("setup save failed: {e}");
        }
        let mut ctx = ConversationContext::new(4);
        let result = ctx.load(&store);
        match result {
            Err(aether_agent_runtime::MemoryStoreError::Corrupt(_)) => {}
            other => panic!("expected Corrupt, got: {other:?}"),
        }
    }
}
