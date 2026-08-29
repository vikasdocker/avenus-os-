// Agent Runtime - Request model
//
// Structured representation of user requests. Input is never assumed
// to be trustworthy — it is validated before processing.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique request identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(Uuid);

impl RequestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Actor that submitted the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestActor {
    pub actor_type: ActorType,
    pub identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorType {
    Human,
    System,
    Agent,
}

/// A structured user request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRequest {
    pub id: RequestId,
    pub session_id: String,
    pub actor: RequestActor,
    pub input: String,
    pub timestamp: u64,
    pub metadata: serde_json::Value,
}

impl UserRequest {
    pub fn new(
        session_id: &str,
        actor: RequestActor,
        input: &str,
        metadata: serde_json::Value,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            id: RequestId::new(),
            session_id: session_id.to_string(),
            actor,
            input: input.to_string(),
            timestamp: now,
            metadata,
        }
    }

    /// Validates the request structure (not the content trustworthiness).
    pub fn validate(&self) -> Result<(), String> {
        if self.input.is_empty() {
            return Err("Request input must not be empty".to_string());
        }
        if self.input.len() > 10_000 {
            return Err("Request input exceeds maximum length".to_string());
        }
        if self.session_id.is_empty() {
            return Err("Session ID must not be empty".to_string());
        }
        Ok(())
    }

    /// Sanitizes the input for safe logging (redacts potential sensitive content).
    pub fn sanitized_input(&self) -> String {
        let input = &self.input;
        if input.len() > 200 {
            format!("{}...", &input[..197])
        } else {
            input.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_actor() -> RequestActor {
        RequestActor { actor_type: ActorType::Human, identity: "test-user".to_string() }
    }

    #[test]
    fn request_creation() {
        let r = UserRequest::new("sess-1", test_actor(), "open calculator", serde_json::json!({}));
        assert_eq!(r.session_id, "sess-1");
        assert_eq!(r.input, "open calculator");
        assert!(r.timestamp > 0);
    }

    #[test]
    fn validate_rejects_empty_input() {
        let r = UserRequest::new("s", test_actor(), "", serde_json::json!({}));
        assert!(r.validate().is_err());
    }

    #[test]
    fn validate_rejects_oversized_input() {
        let long = "x".repeat(10_001);
        let r = UserRequest::new("s", test_actor(), &long, serde_json::json!({}));
        assert!(r.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_session() {
        let r = UserRequest::new("", test_actor(), "hello", serde_json::json!({}));
        assert!(r.validate().is_err());
    }

    #[test]
    fn sanitized_input_truncates() {
        let long = "a".repeat(300);
        let r = UserRequest::new("s", test_actor(), &long, serde_json::json!({}));
        assert!(r.sanitized_input().ends_with("..."));
        assert!(r.sanitized_input().len() < 300);
    }

    #[test]
    fn sanitized_input_short_not_truncated() {
        let r = UserRequest::new("s", test_actor(), "hello", serde_json::json!({}));
        assert_eq!(r.sanitized_input(), "hello");
    }
}
