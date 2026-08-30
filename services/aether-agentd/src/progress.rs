// Aether Agent Daemon - task progress visibility
//
// Emits discrete progress events so the shell (and any future UI)
// can render a "thinking / working / waiting for permission /
// done" indicator for the user's last request. The tracker keeps
// a bounded ring of transitions and exposes the current state.
//
// The discrete states are intentionally coarser than the runtime
// `SessionState` so the UI can map them to a small set of icons.
// Transitions are timestamped so the history is auditable.

use serde::{Deserialize, Serialize};

/// Maximum number of progress transitions kept in the ring.
pub const PROGRESS_RING_CAPACITY: usize = 64;

/// Discrete progress states surfaced to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressState {
    /// No user request is in flight.
    Idle,
    /// Reading the user's input and deciding what to do.
    Thinking,
    /// Building a structured plan of capabilities.
    Planning,
    /// Executing one or more capabilities.
    Working,
    /// A high-risk action is parked waiting for the user to grant
    /// or deny approval.
    WaitingForPermission,
    /// The last request finished successfully.
    Completed,
    /// The last request failed (validation, policy, executor).
    Failed,
    /// The last request is being retried after a transient error.
    Recovering,
}

impl ProgressState {
    /// Returns the state as a stable string for the IPC layer.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Thinking => "thinking",
            Self::Planning => "planning",
            Self::Working => "working",
            Self::WaitingForPermission => "waiting_for_permission",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Recovering => "recovering",
        }
    }
}

impl std::fmt::Display for ProgressState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single progress transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub state: ProgressState,
    /// Optional session id when the transition is tied to one.
    pub session_id: Option<String>,
    /// Optional human-readable note (e.g. the action name, the
    /// plan step, or the failure reason).
    pub message: String,
    pub timestamp_ms: u64,
}

impl ProgressEvent {
    pub fn new(
        state: ProgressState,
        session_id: Option<String>,
        message: &str,
        now_ms: u64,
    ) -> Self {
        Self { state, session_id, message: message.to_string(), timestamp_ms: now_ms }
    }

    /// Renders the event as a JSON object for the IPC layer.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "state": self.state.as_str(),
            "session_id": self.session_id,
            "message": self.message,
            "timestamp_ms": self.timestamp_ms,
        })
    }
}

/// Bounded, in-memory progress tracker. The daemon owns one of
/// these. The chat path pushes transitions as it works; the
/// `agent.progress.*` IPC commands read it back.
pub struct ProgressTracker {
    /// Newest-first ring of transitions.
    history: VecDeque<ProgressEvent>,
    /// Most-recent state. Defaults to `Idle`.
    current: ProgressState,
    /// Wall-clock source so the tracker stays testable.
    now_ms: fn() -> u64,
}

impl ProgressTracker {
    pub fn new(now_ms: fn() -> u64) -> Self {
        Self {
            history: VecDeque::with_capacity(PROGRESS_RING_CAPACITY),
            current: ProgressState::Idle,
            now_ms,
        }
    }

    /// Returns the most-recent state.
    pub fn current_state(&self) -> ProgressState {
        self.current
    }

    /// Records a transition. The previous state is kept in the
    /// history ring for `agent.progress.history`.
    pub fn transition(
        &mut self,
        state: ProgressState,
        session_id: Option<String>,
        message: &str,
    ) -> ProgressEvent {
        let event = ProgressEvent::new(state, session_id, message, (self.now_ms)());
        if self.history.len() >= PROGRESS_RING_CAPACITY {
            self.history.pop_back();
        }
        // Newest-first.
        self.history.push_front(event.clone());
        self.current = state;
        event
    }

    /// Returns the most-recent `n` transitions, newest first.
    pub fn history(&self, limit: usize) -> Vec<&ProgressEvent> {
        self.history.iter().take(limit).collect()
    }

    /// Resets the tracker to `Idle`. Used by the daemon on a clean
    /// start; tests also use it between cases.
    pub fn reset(&mut self) {
        self.history.clear();
        self.current = ProgressState::Idle;
    }
}

use std::collections::VecDeque;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn advance_clock() -> u64 {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    #[test]
    fn new_tracker_starts_idle() {
        let t = ProgressTracker::new(advance_clock);
        assert_eq!(t.current_state(), ProgressState::Idle);
        assert!(t.history(10).is_empty());
    }

    #[test]
    fn transition_updates_current_state() {
        let mut t = ProgressTracker::new(advance_clock);
        t.transition(ProgressState::Thinking, None, "user typed");
        assert_eq!(t.current_state(), ProgressState::Thinking);
        t.transition(ProgressState::Working, None, "executor");
        assert_eq!(t.current_state(), ProgressState::Working);
    }

    #[test]
    fn history_is_newest_first() {
        let mut t = ProgressTracker::new(advance_clock);
        t.transition(ProgressState::Thinking, None, "a");
        t.transition(ProgressState::Planning, None, "b");
        t.transition(ProgressState::Working, None, "c");
        let h = t.history(10);
        assert_eq!(h.len(), 3);
        assert_eq!(h[0].state, ProgressState::Working);
        assert_eq!(h[1].state, ProgressState::Planning);
        assert_eq!(h[2].state, ProgressState::Thinking);
    }

    #[test]
    fn history_respects_limit() {
        let mut t = ProgressTracker::new(advance_clock);
        for i in 0..10 {
            t.transition(ProgressState::Working, None, &format!("m{i}"));
        }
        assert_eq!(t.history(3).len(), 3);
    }

    #[test]
    fn ring_drops_oldest_at_capacity() {
        let mut t = ProgressTracker::new(advance_clock);
        for i in 0..(PROGRESS_RING_CAPACITY + 5) {
            t.transition(ProgressState::Working, None, &format!("m{i}"));
        }
        let h = t.history(usize::MAX);
        assert_eq!(h.len(), PROGRESS_RING_CAPACITY);
        // Newest is at the front.
        assert_eq!(h[0].message, format!("m{}", PROGRESS_RING_CAPACITY + 5 - 1));
    }

    #[test]
    fn reset_returns_to_idle() {
        let mut t = ProgressTracker::new(advance_clock);
        t.transition(ProgressState::Working, None, "x");
        t.reset();
        assert_eq!(t.current_state(), ProgressState::Idle);
        assert!(t.history(10).is_empty());
    }

    #[test]
    fn progress_state_as_str_is_stable() {
        // The shell and any future UI depend on these exact strings.
        assert_eq!(ProgressState::Idle.as_str(), "idle");
        assert_eq!(ProgressState::Thinking.as_str(), "thinking");
        assert_eq!(ProgressState::Planning.as_str(), "planning");
        assert_eq!(ProgressState::Working.as_str(), "working");
        assert_eq!(ProgressState::WaitingForPermission.as_str(), "waiting_for_permission");
        assert_eq!(ProgressState::Completed.as_str(), "completed");
        assert_eq!(ProgressState::Failed.as_str(), "failed");
        assert_eq!(ProgressState::Recovering.as_str(), "recovering");
    }

    #[test]
    fn progress_event_to_json_shape() {
        let e = ProgressEvent::new(ProgressState::Working, Some("s1".to_string()), "do thing", 42);
        let v = e.to_json();
        assert_eq!(v["state"], "working");
        assert_eq!(v["session_id"], "s1");
        assert_eq!(v["message"], "do thing");
        assert_eq!(v["timestamp_ms"], 42);
    }

    #[test]
    fn progress_state_serializes_as_string() {
        let s = serde_json::to_string(&ProgressState::WaitingForPermission).unwrap_or_default();
        assert_eq!(s, "\"waiting_for_permission\"");
    }
}
