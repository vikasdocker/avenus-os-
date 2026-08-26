// Aether Graphics - Compositor module for the graphics stack

use crate::error::GraphicsError;
use crate::types::*;
use std::collections::HashMap;

/// The compositor manages window compositing, layer ordering, and frame rendering.
pub struct Compositor {
    /// Active windows and their surfaces.
    pub windows: HashMap<WindowId, Surface>,
    /// Frame counters for synchronization.
    pub frame_counter: u64,
}

impl Compositor {
    /// Creates a new compositor instance.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            frame_counter: 0,
        }
    }

    /// Registers a new window with its surface.
    pub fn register_window(&mut self, window_id: WindowId, surface: Surface) -> Result<(), GraphicsError> {
        self.windows.insert(window_id, surface);
        Ok(())
    }

    /// Removes a window from the compositor.
    pub fn remove_window(&mut self, window_id: WindowId) -> Result<(), GraphicsError> {
        if self.windows.remove(&window_id).is_some() {
            Ok(())
        } else {
            Err(GraphicsError::Window(format!(
                "Window {} not found",
                window_id
            )))
        }
    }

    /// Returns the current frame counter.
    pub fn get_frame(&self) -> u64 {
        self.frame_counter
    }

    /// Returns the number of active windows.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Returns a reference to a window's surface.
    pub fn get_surface(&self, window_id: WindowId) -> Option<&Surface> {
        self.windows.get(&window_id)
    }
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}
