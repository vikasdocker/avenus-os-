// Agent Runtime - Session model
//
// Tracks the full lifecycle of an agent session from creation through
// completion or failure.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Unique session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Created,
    Ready,
    Thinking,
    Planning,
    WaitingApproval,
    Executing,
    Observing,
    Completed,
    Failed,
    Cancelled,
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Created => "created",
            Self::Ready => "ready",
            Self::Thinking => "thinking",
            Self::Planning => "planning",
            Self::WaitingApproval => "waiting_approval",
            Self::Executing => "executing",
            Self::Observing => "observing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        };
        write!(f, "{s}")
    }
}

impl SessionState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled
        )
    }

    pub fn can_transition_to(&self, next: &SessionState) -> bool {
        matches!(
            (self, next),
            (Self::Created, Self::Ready)
                | (Self::Ready, Self::Thinking)
                | (Self::Thinking, Self::Planning)
                | (Self::Planning, Self::Executing)
                | (Self::Planning, Self::WaitingApproval)
                | (Self::WaitingApproval, Self::Executing)
                | (Self::WaitingApproval, Self::Cancelled)
                | (Self::Executing, Self::Observing)
                | (Self::Executing, Self::Failed)
                | (Self::Observing, Self::Completed)
                | (Self::Observing, Self::Thinking)
                | (Self::Observing, Self::Failed)
        )
    }
}

/// Actor that initiated the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionActor {
    pub actor_type: ActorType,
    pub identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorType {
    Human,
    System,
    Agent,
}

/// An agent session tracking full lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: SessionId,
    pub state: SessionState,
    pub actor: SessionActor,
    pub created_at: u64,
    pub updated_at: u64,
    pub request_count: u32,
    pub action_count: u32,
    pub observation_count: u32,
    pub error_count: u32,
    pub cancelled: bool,
}

impl AgentSession {
    pub fn new(actor: SessionActor) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            id: SessionId::new(),
            state: SessionState::Created,
            actor,
            created_at: now,
            updated_at: now,
            request_count: 0,
            action_count: 0,
            observation_count: 0,
            error_count: 0,
            cancelled: false,
        }
    }

    /// Transitions to a new state if the transition is valid.
    pub fn transition(&mut self, new_state: SessionState) -> Result<(), String> {
        if self.state.is_terminal() {
            return Err(format!(
                "Cannot transition from terminal state {}",
                self.state
            ));
        }
        if !self.state.can_transition_to(&new_state) {
            return Err(format!(
                "Invalid transition: {} -> {}",
                self.state, new_state
            ));
        }
        self.state = new_state;
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        !self.state.is_terminal()
    }

    pub fn mark_error(&mut self) {
        self.error_count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn human_actor() -> SessionActor {
        SessionActor {
            actor_type: ActorType::Human,
            identity: "user".to_string(),
        }
    }

    #[test]
    fn session_starts_in_created_state() {
        let s = AgentSession::new(human_actor());
        assert_eq!(s.state, SessionState::Created);
        assert!(s.is_active());
        assert_eq!(s.request_count, 0);
    }

    #[test]
    fn valid_transition_created_to_ready() {
        let mut s = AgentSession::new(human_actor());
        assert!(s.transition(SessionState::Ready).is_ok());
        assert_eq!(s.state, SessionState::Ready);
    }

    #[test]
    fn invalid_transition_skips_ready() {
        let mut s = AgentSession::new(human_actor());
        assert!(s.transition(SessionState::Thinking).is_err());
        assert_eq!(s.state, SessionState::Created);
    }

    #[test]
    fn terminal_states_block_transitions() {
        let mut s = AgentSession::new(human_actor());
        s.state = SessionState::Completed;
        assert!(!s.is_active());
        assert!(s.transition(SessionState::Ready).is_err());
    }

    #[test]
    fn full_lifecycle() {
        let mut s = AgentSession::new(human_actor());
        assert!(s.transition(SessionState::Ready).is_ok());
        assert!(s.transition(SessionState::Thinking).is_ok());
        assert!(s.transition(SessionState::Planning).is_ok());
        assert!(s.transition(SessionState::Executing).is_ok());
        assert!(s.transition(SessionState::Observing).is_ok());
        assert!(s.transition(SessionState::Completed).is_ok());
        assert!(!s.is_active());
    }

    #[test]
    fn cancellation_from_waiting_approval() {
        let mut s = AgentSession::new(human_actor());
        s.state = SessionState::WaitingApproval;
        assert!(s.transition(SessionState::Cancelled).is_ok());
        assert!(!s.is_active());
    }

    #[test]
    fn session_id_display() {
        let id = SessionId::new();
        let display = format!("{id}");
        assert_eq!(display.len(), 36); // UUID format
    }

    #[test]
    fn error_count_increments() {
        let mut s = AgentSession::new(human_actor());
        assert_eq!(s.error_count, 0);
        s.mark_error();
        assert_eq!(s.error_count, 1);
    }
}
