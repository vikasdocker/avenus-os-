// Aether Graphics - IPC module for the graphics stack
//
// Defines the structured commands exchanged between the desktop shell,
// compositor, and clients via the Aether IPC socket.

use crate::error::GraphicsError;
use crate::types::WindowId;
use serde::{Deserialize, Serialize};

/// Represents a command sent via Aether IPC to the graphics stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicsCommand {
    pub command: GraphicsCommandType,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphicsCommandType {
    // Window management
    CreateWindow,
    DestroyWindow(WindowId),
    UpdateSurface,

    // Input
    SetInputMode,

    // Sessions
    CreateSession,
    DestroySession,

    // Display
    GetDisplayModes,
    SetDisplayMode(u32, u32),
    SetCursorPosition { x: f32, y: f32 },

    // Window inspection and control (Phase 1.9 Part 2)
    WindowList,
    WindowInspect { window_id: u64 },
    WindowFocus { window_id: u64 },
    WindowClose { window_id: u64 },
    WindowMove { window_id: u64, x: i32, y: i32 },
    WindowResize { window_id: u64, width: u32, height: u32 },

    // Workspace management (Phase 1.9 Part 2)
    WorkspaceList,
    WorkspaceCreate { name: String },
    WorkspaceDestroy { id: u32 },
    WorkspaceActivate { id: u32 },

    // Desktop status (Phase 1.9 Part 2)
    DesktopStatus,
}

/// Represents a response from the graphics stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicsResponse {
    pub ok: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// The interface for communicating with the graphics stack.
pub trait GraphicsIpc {
    fn send_command(&self, cmd: GraphicsCommand) -> Result<GraphicsResponse, GraphicsError>;
    fn listen(&self) -> Result<(), GraphicsError>;
}

pub struct GraphicsIpcClient {
    pub socket_path: String,
}

impl GraphicsIpcClient {
    pub fn new(socket_path: &str) -> Self {
        Self {
            socket_path: socket_path.to_string(),
        }
    }
}

impl GraphicsIpc for GraphicsIpcClient {
    fn send_command(&self, _cmd: GraphicsCommand) -> Result<GraphicsResponse, GraphicsError> {
        // Placeholder implementation
        Err(GraphicsError::NotImplemented(
            "IPC Client implementation not yet complete".to_string(),
        ))
    }

    fn listen(&self) -> Result<(), GraphicsError> {
        // Placeholder implementation
        Err(GraphicsError::NotImplemented(
            "IPC Client listening not yet complete".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphics_command_roundtrips() {
        let cmd = GraphicsCommand {
            command: GraphicsCommandType::WindowList,
            params: serde_json::json!({}),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: GraphicsCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded.command, GraphicsCommandType::WindowList));
    }

    #[test]
    fn workspace_command_variants() {
        let cmds = [
            GraphicsCommandType::WorkspaceList,
            GraphicsCommandType::WorkspaceCreate {
                name: "Dev".to_string(),
            },
            GraphicsCommandType::WorkspaceDestroy { id: 1 },
            GraphicsCommandType::WorkspaceActivate { id: 2 },
        ];
        for cmd in cmds {
            let json = serde_json::to_string(&GraphicsCommand {
                command: cmd,
                params: serde_json::json!({}),
            })
            .unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn desktop_status_roundtrips() {
        let cmd = GraphicsCommand {
            command: GraphicsCommandType::DesktopStatus,
            params: serde_json::json!({}),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let decoded: GraphicsCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            decoded.command,
            GraphicsCommandType::DesktopStatus
        ));
    }
}
