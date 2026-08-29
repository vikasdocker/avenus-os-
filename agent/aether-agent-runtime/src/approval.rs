// Agent Runtime - Approval abstraction
//
// High-risk actions must enter WAITING_APPROVAL before execution.
// The API exists for future UI/voice integration.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique approval request identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApprovalRequestId(Uuid);

impl ApprovalRequestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ApprovalRequestId {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

/// An approval request for a high-risk action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: ApprovalRequestId,
    pub session_id: String,
    pub action_id: String,
    pub action_name: String,
    pub risk_level: String,
    pub reason: String,
    pub status: ApprovalStatus,
    pub created_at: u64,
    pub resolved_at: Option<u64>,
}

impl ApprovalRequest {
    pub fn new(
        session_id: &str,
        action_id: &str,
        action_name: &str,
        risk_level: &str,
        reason: &str,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            id: ApprovalRequestId::new(),
            session_id: session_id.to_string(),
            action_id: action_id.to_string(),
            action_name: action_name.to_string(),
            risk_level: risk_level.to_string(),
            reason: reason.to_string(),
            status: ApprovalStatus::Pending,
            created_at: now,
            resolved_at: None,
        }
    }

    /// Approves the request.
    pub fn approve(&mut self) {
        self.status = ApprovalStatus::Approved;
        self.resolved_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );
    }

    /// Denies the request.
    pub fn deny(&mut self) {
        self.status = ApprovalStatus::Denied;
        self.resolved_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        );
    }

    pub fn is_pending(&self) -> bool {
        self.status == ApprovalStatus::Pending
    }

    pub fn is_approved(&self) -> bool {
        self.status == ApprovalStatus::Approved
    }
}

/// An approval decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    pub request_id: ApprovalRequestId,
    pub decision: ApprovalStatus,
    pub reason: String,
}

impl ApprovalDecision {
    pub fn approve(request_id: ApprovalRequestId, reason: &str) -> Self {
        Self { request_id, decision: ApprovalStatus::Approved, reason: reason.to_string() }
    }

    pub fn deny(request_id: ApprovalRequestId, reason: &str) -> Self {
        Self { request_id, decision: ApprovalStatus::Denied, reason: reason.to_string() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_request_creation() {
        let r = ApprovalRequest::new("s1", "a1", "file.delete", "high", "delete file");
        assert!(r.is_pending());
        assert!(!r.is_approved());
        assert_eq!(r.action_name, "file.delete");
    }

    #[test]
    fn approve_sets_status() {
        let mut r = ApprovalRequest::new("s1", "a1", "file.delete", "high", "delete");
        r.approve();
        assert!(r.is_approved());
        assert!(r.resolved_at.is_some());
    }

    #[test]
    fn deny_sets_status() {
        let mut r = ApprovalRequest::new("s1", "a1", "file.delete", "high", "delete");
        r.deny();
        assert_eq!(r.status, ApprovalStatus::Denied);
    }

    #[test]
    fn approval_decision() {
        let rid = ApprovalRequestId::new();
        let d = ApprovalDecision::approve(rid, "user confirmed");
        assert_eq!(d.decision, ApprovalStatus::Approved);
    }

    #[test]
    fn deny_decision() {
        let rid = ApprovalRequestId::new();
        let d = ApprovalDecision::deny(rid, "too dangerous");
        assert_eq!(d.decision, ApprovalStatus::Denied);
    }
}
