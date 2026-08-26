// Aether Graphics - Session management for the graphics stack

use crate::error::GraphicsError;
use crate::types::GraphicsSession;
use uuid::Uuid;

/// Manages the lifecycle of a graphical session.
pub struct GraphicsSessionManager {
    pub active_session: Option<GraphicsSession>,
}

impl GraphicsSessionManager {
    pub fn new() -> Self {
        Self {
            active_session: None,
        }
    }

    /// Starts a new graphical session.
    pub fn start_session(&mut self, _user: &str) -> Result<GraphicsSession, GraphicsError> {
        let session = GraphicsSession {
            session_id: Uuid::new_v4().to_string(),
            active_workspace: 0,
            connected_devices: Vec::new(),
        };

        self.active_session = Some(session.clone());
        Ok(session)
    }

    /// Ends the current graphical session.
    pub fn end_session(&mut self) -> Result<(), GraphicsError> {
        self.active_session = None;
        Ok(())
    }

    /// Switches to a different workspace.
    pub fn switch_workspace(&mut self, workspace_id: u32) -> Result<(), GraphicsError> {
        if let Some(ref mut session) = self.active_session {
            session.active_workspace = workspace_id;
            Ok(())
        } else {
            Err(GraphicsError::Session("No active session to switch workspace".to_string()))
        }
    }
}

impl Default for GraphicsSessionManager {
    fn default() -> Self {
        Self::new()
    }
}
