// Aether Graphics - Display management for the graphics stack
//
// Owns the logical display topology: connected displays, active display
// mode selection, and mode transitions. Hardware backends plug in behind
// this surface.

use crate::error::GraphicsError;
use crate::types::DisplayMode;

/// Logical identifier for a connected display.
pub type DisplayId = u32;

/// A single connected display and its supported modes.
#[derive(Debug, Clone)]
pub struct Display {
    pub id: DisplayId,
    pub name: String,
    pub modes: Vec<DisplayMode>,
    pub active_mode: Option<DisplayMode>,
    pub enabled: bool,
}

/// Manages displays attached to the graphics stack.
pub struct DisplayManager {
    displays: Vec<Display>,
}

impl DisplayManager {
    pub fn new() -> Self {
        Self {
            displays: Vec::new(),
        }
    }

    /// Registers a display with its list of supported modes.
    pub fn attach(&mut self, id: DisplayId, name: &str, modes: Vec<DisplayMode>) {
        self.displays.push(Display {
            id,
            name: name.to_string(),
            modes,
            active_mode: None,
            enabled: false,
        });
    }

    /// Detaches a display by id.
    pub fn detach(&mut self, id: DisplayId) -> Result<(), GraphicsError> {
        let len_before = self.displays.len();
        self.displays.retain(|d| d.id != id);
        if self.displays.len() == len_before {
            return Err(GraphicsError::Display(format!("Display {} not found", id)));
        }
        Ok(())
    }

    /// Enables a display and activates the requested mode.
    pub fn enable(&mut self, id: DisplayId, mode_index: usize) -> Result<(), GraphicsError> {
        let display = self
            .displays
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or_else(|| GraphicsError::Display(format!("Display {} not found", id)))?;
        if display.modes.is_empty() {
            return Err(GraphicsError::Mode(format!(
                "Display {} has no modes",
                id
            )));
        }
        if mode_index >= display.modes.len() {
            return Err(GraphicsError::Mode(format!(
                "Mode index {} out of range for display {}",
                mode_index, id
            )));
        }
        display.active_mode = Some(display.modes[mode_index]);
        display.enabled = true;
        Ok(())
    }

    /// Disables a display.
    pub fn disable(&mut self, id: DisplayId) -> Result<(), GraphicsError> {
        let display = self
            .displays
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or_else(|| GraphicsError::Display(format!("Display {} not found", id)))?;
        display.enabled = false;
        display.active_mode = None;
        Ok(())
    }

    /// Returns all registered displays.
    pub fn displays(&self) -> &[Display] {
        &self.displays
    }

    /// Returns the currently active (enabled) displays.
    pub fn active_displays(&self) -> Vec<&Display> {
        self.displays.iter().filter(|d| d.enabled).collect()
    }
}

impl Default for DisplayManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(w: u32, h: u32) -> DisplayMode {
        DisplayMode {
            width: w,
            height: h,
            refresh_rate: 60,
        }
    }

    #[test]
    fn attach_and_enable_display() {
        let mut dm = DisplayManager::new();
        dm.attach(0, "Virtual-1", vec![mode(1920, 1080), mode(1280, 720)]);
        dm.enable(0, 0).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(dm.active_displays().len(), 1);
        let active = dm.active_displays()[0];
        assert_eq!(active.active_mode.as_ref().unwrap().width, 1920);
    }

    #[test]
    fn enable_out_of_range_mode_fails() {
        let mut dm = DisplayManager::new();
        dm.attach(1, "Virtual-1", vec![mode(800, 600)]);
        assert!(dm.enable(1, 5).is_err());
    }

    #[test]
    fn detach_unknown_display_fails() {
        let mut dm = DisplayManager::new();
        assert!(dm.detach(99).is_err());
    }
}
