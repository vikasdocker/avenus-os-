// Aether Graphics - Workspace management for the graphics stack

use crate::error::GraphicsError;
use std::collections::HashMap;

/// Manages multiple graphical workspaces.
pub struct WorkspaceManager {
    pub workspaces: HashMap<u32, Workspace>,
    pub active_workspace_id: u32,
}

/// Represents a single graphical workspace.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: u32,
    pub windows: Vec<crate::types::WindowId>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        let mut workspaces = HashMap::new();
        workspaces.insert(0, Workspace { id: 0, windows: Vec::new() });

        Self { workspaces, active_workspace_id: 0 }
    }

    /// Adds a new workspace.
    pub fn create_workspace(&mut self, id: u32) -> Result<(), GraphicsError> {
        if self.workspaces.contains_key(&id) {
            return Err(GraphicsError::Workspace(format!("Workspace {} already exists", id)));
        }
        self.workspaces.insert(id, Workspace { id, windows: Vec::new() });
        Ok(())
    }

    /// Removes a workspace.
    pub fn remove_workspace(&mut self, id: u32) -> Result<(), GraphicsError> {
        if id == 0 {
            return Err(GraphicsError::Workspace("Cannot remove workspace 0".to_string()));
        }
        if self.workspaces.remove(&id).is_some() {
            if self.active_workspace_id == id {
                self.active_workspace_id = 0;
            }
            Ok(())
        } else {
            Err(GraphicsError::Workspace(format!("Workspace {} not found", id)))
        }
    }

    /// Switches the active workspace.
    pub fn switch_to_workspace(&mut self, id: u32) -> Result<(), GraphicsError> {
        if self.workspaces.contains_key(&id) {
            self.active_workspace_id = id;
            Ok(())
        } else {
            Err(GraphicsError::Workspace(format!("Workspace {} does not exist", id)))
        }
    }

    /// Adds a window to the active workspace.
    pub fn add_window_to_active(
        &mut self,
        window_id: crate::types::WindowId,
    ) -> Result<(), GraphicsError> {
        if let Some(workspace) = self.workspaces.get_mut(&self.active_workspace_id) {
            workspace.windows.push(window_id);
            Ok(())
        } else {
            Err(GraphicsError::Workspace("Active workspace not found".to_string()))
        }
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}
