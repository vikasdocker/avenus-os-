// Aether Graphics - Desktop shell module for the graphics stack

use crate::error::GraphicsError;

/// The desktop shell manages the shell environment, application integration, and user interactions.
pub struct DesktopShell {
    /// Active workspace manager.
    pub workspace: crate::WorkspaceManager,
    /// Current display mode.
    pub display_mode: (u32, u32),
    /// Session manager.
    pub session: crate::GraphicsSessionManager,
}

impl DesktopShell {
    /// Creates a new desktop shell with default workspace.
    pub fn new() -> Self {
        let session = crate::GraphicsSessionManager::new();
        Self { workspace: crate::WorkspaceManager::new(), display_mode: (1920, 1080), session }
    }

    /// Returns the current display resolution.
    pub fn get_display_mode(&self) -> (u32, u32) {
        self.display_mode
    }

    /// Sets the display resolution.
    pub fn set_display_mode(&mut self, width: u32, height: u32) -> Result<(), GraphicsError> {
        self.display_mode = (width, height);
        Ok(())
    }

    /// Returns the active workspace ID.
    pub fn get_active_workspace(&self) -> u32 {
        self.workspace.active_workspace_id
    }

    /// Gets the current session.
    pub fn get_session(&self) -> &crate::GraphicsSessionManager {
        &self.session
    }
}

impl Default for DesktopShell {
    fn default() -> Self {
        Self::new()
    }
}
