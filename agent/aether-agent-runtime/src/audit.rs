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
}
