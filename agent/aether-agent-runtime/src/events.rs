// Agent Runtime - Event types for the event bus
//
// Publishes structured events for the Aether event bus.
// Uses existing patterns from aether-agentd.

use serde::{Deserialize, Serialize};

/// Agent runtime events for the event bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
    SessionCreated {
        session_id: String,
        actor: String,
    },
    SessionStarted {
        session_id: String,
    },
    SessionCompleted {
        session_id: String,
    },
    SessionFailed {
        session_id: String,
        reason: String,
    },
    SessionCancelled {
        session_id: String,
    },
    IntentCreated {
        session_id: String,
        intent_type: String,
        confidence: u8,
    },
    PlanCreated {
        session_id: String,
        plan_id: String,
        step_count: u32,
    },
    ActionRequested {
        session_id: String,
        action_id: String,
        action_name: String,
        risk_level: String,
    },
    ActionApproved {
        session_id: String,
        action_id: String,
    },
    ActionDenied {
        session_id: String,
        action_id: String,
        reason: String,
    },
    ActionStarted {
        session_id: String,
        action_id: String,
    },
    ActionCompleted {
        session_id: String,
        action_id: String,
        duration_ms: u64,
    },
    ActionFailed {
        session_id: String,
        action_id: String,
        error: String,
    },
    ObservationCreated {
        session_id: String,
        observation_id: String,
        success: bool,
    },
}

impl AgentEvent {
    /// Returns the event type as a string for the event bus.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::SessionCreated { .. } => "agent.session.created",
            Self::SessionStarted { .. } => "agent.session.started",
            Self::SessionCompleted { .. } => "agent.session.completed",
            Self::SessionFailed { .. } => "agent.session.failed",
            Self::SessionCancelled { .. } => "agent.session.cancelled",
            Self::IntentCreated { .. } => "agent.intent.created",
            Self::PlanCreated { .. } => "agent.plan.created",
            Self::ActionRequested { .. } => "agent.action.requested",
            Self::ActionApproved { .. } => "agent.action.approved",
            Self::ActionDenied { .. } => "agent.action.denied",
            Self::ActionStarted { .. } => "agent.action.started",
            Self::ActionCompleted { .. } => "agent.action.completed",
            Self::ActionFailed { .. } => "agent.action.failed",
            Self::ObservationCreated { .. } => "agent.observation.created",
        }
    }

    /// Serializes the event for publishing.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::json!({"error": "serialization failed"}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_type_strings() {
        let e = AgentEvent::SessionCreated {
            session_id: "s1".to_string(),
            actor: "user".to_string(),
        };
        assert_eq!(e.event_type(), "agent.session.created");
    }

    #[test]
    fn event_serializes() {
        let e = AgentEvent::ActionCompleted {
            session_id: "s1".to_string(),
            action_id: "a1".to_string(),
            duration_ms: 42,
        };
        let json = e.to_json();
        // Enum serializes with variant key
        let inner = &json["ActionCompleted"];
        assert!(inner["session_id"].as_str().is_some());
        assert!(inner["duration_ms"].as_u64().is_some());
    }

    #[test]
    fn all_event_types_have_strings() {
        let events = vec![
            AgentEvent::SessionCreated { session_id: "s".into(), actor: "u".into() },
            AgentEvent::SessionStarted { session_id: "s".into() },
            AgentEvent::SessionCompleted { session_id: "s".into() },
            AgentEvent::SessionFailed { session_id: "s".into(), reason: "r".into() },
            AgentEvent::SessionCancelled { session_id: "s".into() },
            AgentEvent::IntentCreated { session_id: "s".into(), intent_type: "i".into(), confidence: 80 },
            AgentEvent::PlanCreated { session_id: "s".into(), plan_id: "p".into(), step_count: 1 },
            AgentEvent::ActionRequested { session_id: "s".into(), action_id: "a".into(), action_name: "n".into(), risk_level: "low".into() },
            AgentEvent::ActionApproved { session_id: "s".into(), action_id: "a".into() },
            AgentEvent::ActionDenied { session_id: "s".into(), action_id: "a".into(), reason: "r".into() },
            AgentEvent::ActionStarted { session_id: "s".into(), action_id: "a".into() },
            AgentEvent::ActionCompleted { session_id: "s".into(), action_id: "a".into(), duration_ms: 0 },
            AgentEvent::ActionFailed { session_id: "s".into(), action_id: "a".into(), error: "e".into() },
            AgentEvent::ObservationCreated { session_id: "s".into(), observation_id: "o".into(), success: true },
        ];
        for e in &events {
            assert!(!e.event_type().is_empty());
            assert!(e.to_json().is_object());
        }
    }
}
