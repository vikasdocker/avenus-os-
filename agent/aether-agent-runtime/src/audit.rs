// Agent Runtime - Audit integration
//
// Records agent session creation, intents, plans, action requests,
// capability decisions, policy decisions, approvals, execution results,
// failures, and cancellations. Never logs API keys, passwords, tokens,
// or sensitive file contents.

use serde::{Deserialize, Serialize};

/// An audit entry for the agent runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub session_id: String,
    pub event_type: AuditEventType,
    pub detail: String,
    pub success: bool,
    pub component: String,
}

/// Types of audit events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    SessionCreated,
    SessionStarted,
    SessionCompleted,
    SessionFailed,
    SessionCancelled,
    IntentCreated,
    PlanCreated,
    ActionRequested,
    ActionApproved,
    ActionDenied,
    ActionStarted,
    ActionCompleted,
    ActionFailed,
    CapabilityCheck,
    PolicyCheck,
    ApprovalRequested,
    ApprovalGranted,
    ApprovalDenied,
    LlmRequest,
    LlmResponse,
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::SessionCreated => "session.created",
            Self::SessionStarted => "session.started",
            Self::SessionCompleted => "session.completed",
            Self::SessionFailed => "session.failed",
            Self::SessionCancelled => "session.cancelled",
            Self::IntentCreated => "intent.created",
            Self::PlanCreated => "plan.created",
            Self::ActionRequested => "action.requested",
            Self::ActionApproved => "action.approved",
            Self::ActionDenied => "action.denied",
            Self::ActionStarted => "action.started",
            Self::ActionCompleted => "action.completed",
            Self::ActionFailed => "action.failed",
            Self::CapabilityCheck => "capability.check",
            Self::PolicyCheck => "policy.check",
            Self::ApprovalRequested => "approval.requested",
            Self::ApprovalGranted => "approval.granted",
            Self::ApprovalDenied => "approval.denied",
            Self::LlmRequest => "llm.request",
            Self::LlmResponse => "llm.response",
        };
        write!(f, "{s}")
    }
}

/// In-memory audit log.
pub struct AuditLog {
    entries: std::collections::VecDeque<AuditEntry>,
    capacity: usize,
}

impl AuditLog {
    pub fn new(capacity: usize) -> Self {
        Self { entries: std::collections::VecDeque::with_capacity(capacity), capacity }
    }

    /// Records an audit entry.
    pub fn record(
        &mut self,
        session_id: &str,
        event_type: AuditEventType,
        detail: &str,
        success: bool,
        component: &str,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }

        self.entries.push_back(AuditEntry {
            timestamp: now,
            session_id: session_id.to_string(),
            event_type,
            detail: sanitize_detail(detail),
            success,
            component: component.to_string(),
        });
    }

    /// Returns recent entries.
    pub fn recent(&self, count: usize) -> Vec<&AuditEntry> {
        self.entries.iter().rev().take(count).collect()
    }

    /// Returns all entries for a session.
    pub fn for_session(&self, session_id: &str) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.session_id == session_id).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the most recent `count` entries as owned values, in
    /// chronological order (oldest first, newest last). Used by
    /// `AgentRuntimeHost::snapshot_audit_recent` for persistence.
    pub fn snapshot_recent(&self, count: usize) -> Vec<AuditEntry> {
        let n = count.min(self.entries.len());
        self.entries.iter().rev().take(n).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    }

    /// Replaces the current ring with the supplied entries. The
    /// ring's `capacity` is unchanged. If the input is longer than
    /// `capacity`, only the newest `capacity` entries are kept.
    /// Returns the number of entries actually retained.
    pub fn restore_recent(&mut self, mut entries: Vec<AuditEntry>) -> usize {
        if entries.len() > self.capacity {
            let drop = entries.len() - self.capacity;
            entries.drain(..drop);
        }
        let kept = entries.len();
        self.entries.clear();
        // Restore in chronological order so `recent(n)` returns
        // newest-first as it does on the recording path.
        for entry in entries {
            self.entries.push_back(entry);
        }
        kept
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new(1024)
    }
}

/// Sanitizes audit detail to remove sensitive content.
fn sanitize_detail(detail: &str) -> String {
    // Truncate and redact potential passwords/tokens
    let truncated =
        if detail.len() > 500 { format!("{}...", &detail[..497]) } else { detail.to_string() };

    // Redact values after sensitive keys
    redact_sensitive(&truncated)
}

/// Redacts values following sensitive key patterns.
fn redact_sensitive(input: &str) -> String {
    let sensitive_keys = ["password", "token", "api_key", "secret", "key", "credential"];
    let mut result = input.to_string();
    for key in &sensitive_keys {
        let pattern = format!("{key}=");
        let mut search_from = 0;
        while let Some(pos) = result[search_from..].find(&pattern) {
            let abs_pos = search_from + pos;
            let value_start = abs_pos + pattern.len();
            let value_end = result[value_start..]
                .find(|c: char| [' ', ',', ')', '"'].contains(&c))
                .map(|i| value_start + i)
                .unwrap_or(result.len());
            result.replace_range(abs_pos..value_end, &format!("{key}=[REDACTED]"));
            // Advance past the replacement to avoid infinite loop
            search_from = abs_pos + key.len() + "[REDACTED]".len();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_log_records_entries() {
        let mut log = AuditLog::new(100);
        log.record("s1", AuditEventType::SessionCreated, "new session", true, "runtime");
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn audit_log_bounded() {
        let mut log = AuditLog::new(3);
        for i in 0..5 {
            log.record(
                "s1",
                AuditEventType::ActionRequested,
                &format!("action {i}"),
                true,
                "runtime",
            );
        }
        assert_eq!(log.len(), 3);
        // Oldest entries evicted
        let entries = log.recent(10);
        assert_eq!(entries[0].detail, "action 4");
    }

    #[test]
    fn audit_log_for_session() {
        let mut log = AuditLog::new(100);
        log.record("s1", AuditEventType::SessionCreated, "s1 created", true, "r");
        log.record("s2", AuditEventType::SessionCreated, "s2 created", true, "r");
        log.record("s1", AuditEventType::ActionRequested, "action", true, "r");
        assert_eq!(log.for_session("s1").len(), 2);
        assert_eq!(log.for_session("s2").len(), 1);
    }

    #[test]
    fn sanitize_removes_passwords() {
        let s = sanitize_detail("user=admin password=secret123");
        assert!(s.contains("password=[REDACTED]"));
        assert!(!s.contains("secret123"));
    }

    #[test]
    fn sanitize_truncates_long() {
        let long = "x".repeat(1000);
        let s = sanitize_detail(&long);
        assert!(s.len() < 600);
    }

    #[test]
    fn snapshot_recent_returns_last_n_in_chronological_order() {
        let mut log = AuditLog::new(10);
        for i in 0..5 {
            log.record("s1", AuditEventType::ActionRequested, &format!("a{i}"), true, "r");
        }
        let snap = log.snapshot_recent(3);
        assert_eq!(snap.len(), 3);
        // The snapshot is oldest-first, so the most recent three
        // recorded events are `a2`, `a3`, `a4` in that order.
        assert_eq!(snap[0].detail, "a2");
        assert_eq!(snap[1].detail, "a3");
        assert_eq!(snap[2].detail, "a4");
    }

    #[test]
    fn snapshot_recent_zero_returns_empty() {
        let log = AuditLog::new(10);
        assert!(log.snapshot_recent(0).is_empty());
        assert!(log.snapshot_recent(100).is_empty());
    }

    #[test]
    fn restore_recent_pushes_back_evicting_oldest() {
        let mut log = AuditLog::new(3);
        log.record("s", AuditEventType::ActionRequested, "first", true, "r");
        log.record("s", AuditEventType::ActionRequested, "second", true, "r");
        let snap = log.snapshot_recent(2);
        // New log, then restore.
        let mut log2 = AuditLog::new(3);
        let kept = log2.restore_recent(snap);
        assert_eq!(kept, 2);
        let recent = log2.recent(10);
        // `recent` returns newest-first.
        assert_eq!(recent[0].detail, "second");
        assert_eq!(recent[1].detail, "first");
    }

    #[test]
    fn restore_recent_drops_excess_to_fit_capacity() {
        let mut log = AuditLog::new(2);
        let entries: Vec<AuditEntry> = (0..5)
            .map(|i| AuditEntry {
                timestamp: i as u64,
                session_id: "s".to_string(),
                event_type: AuditEventType::ActionRequested,
                detail: format!("e{i}"),
                success: true,
                component: "r".to_string(),
            })
            .collect();
        let kept = log.restore_recent(entries);
        assert_eq!(kept, 2);
        let recent = log.recent(10);
        // The two newest survive; the older three are dropped.
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].detail, "e4");
        assert_eq!(recent[1].detail, "e3");
    }

    #[test]
    fn snapshot_then_restore_preserves_full_ring() {
        let mut log = AuditLog::new(5);
        for i in 0..5 {
            log.record("s", AuditEventType::ActionRequested, &format!("a{i}"), true, "r");
        }
        let snap = log.snapshot_recent(5);
        let mut log2 = AuditLog::new(5);
        log2.restore_recent(snap);
        // The ring now has the same chronological order.
        for i in 0..5 {
            let recent = log2.recent(5);
            // `recent` returns newest-first, so index 0 is the
            // most recent (a4), index 4 is the oldest (a0).
            let expected = format!("a{}", 4 - i);
            assert_eq!(recent[i].detail, expected);
        }
    }
}
