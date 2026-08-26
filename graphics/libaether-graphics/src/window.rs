// Aether Graphics - Window management for the graphics stack
//
// Tracks window lifecycle (create, focus, move, resize, close) and the
// stacking order consumed by the compositor.

use crate::error::GraphicsError;
use crate::types::{PixelFormat, Surface, WindowId};
use std::collections::HashMap;

/// Geometry of a window in compositor space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns true when the point lies inside the rectangle.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        let fx = self.x as f32;
        let fy = self.y as f32;
        px >= fx && py >= fy && px < fx + self.width as f32 && py < fy + self.height as f32
    }
}

/// State of a managed window.
#[derive(Debug, Clone)]
pub struct Window {
    pub id: WindowId,
    pub title: String,
    pub rect: Rect,
    pub surface: Surface,
    pub focused: bool,
    pub minimized: bool,
}

/// Manages the set of live windows and their stacking order.
pub struct WindowManager {
    windows: HashMap<WindowId, Window>,
    stack: Vec<WindowId>,
    focused: Option<WindowId>,
    next_z: u64,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            stack: Vec::new(),
            focused: None,
            next_z: 0,
        }
    }

    /// Creates a window with a fresh id, title, geometry, and pixel format.
    pub fn create_window(
        &mut self,
        title: &str,
        rect: Rect,
        format: PixelFormat,
    ) -> Result<WindowId, GraphicsError> {
        if rect.width == 0 || rect.height == 0 {
            return Err(GraphicsError::Window(
                "Window dimensions must be non-zero".to_string(),
            ));
        }
        let id = WindowId::new();
        let surface = Surface {
            id: id.as_uuid(),
            width: rect.width,
            height: rect.height,
            format,
        };
        self.windows.insert(
            id,
            Window {
                id,
                title: title.to_string(),
                rect,
                surface,
                focused: false,
                minimized: false,
            },
        );
        self.stack.push(id);
        self.next_z += 1;
        Ok(id)
    }

    /// Closes a window and removes it from the stack.
    pub fn close_window(&mut self, id: WindowId) -> Result<(), GraphicsError> {
        if self.windows.remove(&id).is_none() {
            return Err(GraphicsError::Window(format!("Window {} not found", id)));
        }
        self.stack.retain(|w| *w != id);
        if self.focused == Some(id) {
            self.focused = None;
            // Focus falls through to the top-most remaining window.
            if let Some(top) = self.stack.last() {
                if let Some(w) = self.windows.get_mut(top) {
                    w.focused = true;
                }
                self.focused = Some(*top);
            }
        }
        Ok(())
    }

    /// Moves a window to the top of the stacking order and focuses it.
    pub fn focus(&mut self, id: WindowId) -> Result<(), GraphicsError> {
        if !self.windows.contains_key(&id) {
            return Err(GraphicsError::Window(format!("Window {} not found", id)));
        }
        for w in self.windows.values_mut() {
            w.focused = false;
        }
        if let Some(pos) = self.stack.iter().position(|w| *w == id) {
            self.stack.remove(pos);
        }
        self.stack.push(id);
        self.next_z += 1;
        if let Some(w) = self.windows.get_mut(&id) {
            w.focused = true;
        }
        self.focused = Some(id);
        Ok(())
    }

    /// Returns the currently focused window id.
    pub fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    /// Resizes a window and its backing surface.
    pub fn resize(&mut self, id: WindowId, width: u32, height: u32) -> Result<(), GraphicsError> {
        if width == 0 || height == 0 {
            return Err(GraphicsError::Window(
                "Window dimensions must be non-zero".to_string(),
            ));
        }
        let w = self
            .windows
            .get_mut(&id)
            .ok_or_else(|| GraphicsError::Window(format!("Window {} not found", id)))?;
        w.rect.width = width;
        w.rect.height = height;
        w.surface.width = width;
        w.surface.height = height;
        Ok(())
    }

    /// Moves a window.
    pub fn move_to(&mut self, id: WindowId, x: i32, y: i32) -> Result<(), GraphicsError> {
        let w = self
            .windows
            .get_mut(&id)
            .ok_or_else(|| GraphicsError::Window(format!("Window {} not found", id)))?;
        w.rect.x = x;
        w.rect.y = y;
        Ok(())
    }

    /// Sets the minimized flag for a window.
    pub fn set_minimized(&mut self, id: WindowId, minimized: bool) -> Result<(), GraphicsError> {
        let w = self
            .windows
            .get_mut(&id)
            .ok_or_else(|| GraphicsError::Window(format!("Window {} not found", id)))?;
        w.minimized = minimized;
        Ok(())
    }

    /// Returns the top-most window whose rectangle contains the point.
    pub fn window_at(&self, px: f32, py: f32) -> Option<WindowId> {
        for id in self.stack.iter().rev() {
            if let Some(w) = self.windows.get(id) {
                if !w.minimized && w.rect.contains(px, py) {
                    return Some(*id);
                }
            }
        }
        None
    }

    /// Returns windows in bottom-to-top stacking order.
    pub fn stacked_windows(&self) -> Vec<&Window> {
        self.stack
            .iter()
            .filter_map(|id| self.windows.get(id))
            .collect()
    }

    /// Returns the number of live windows.
    pub fn count(&self) -> usize {
        self.windows.len()
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_focus_close_cycle() {
        let mut wm = WindowManager::new();
        let a = wm
            .create_window("a", Rect::new(0, 0, 100, 100), PixelFormat::Rgba8888)
            .unwrap_or_else(|e| panic!("{e}"));
        let b = wm
            .create_window("b", Rect::new(10, 10, 100, 100), PixelFormat::Rgba8888)
            .unwrap_or_else(|e| panic!("{e}"));
        wm.focus(a).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(wm.focused(), Some(a));
        assert_eq!(wm.stacked_windows().last().unwrap().id, a);
        wm.close_window(a).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(wm.count(), 1);
        // Focus fell through to b.
        assert_eq!(wm.focused(), Some(b));
    }

    #[test]
    fn hit_testing_respects_stack_and_minimize() {
        let mut wm = WindowManager::new();
        let a = wm
            .create_window("a", Rect::new(0, 0, 100, 100), PixelFormat::Rgb24)
            .unwrap_or_else(|e| panic!("{e}"));
        let _b = wm
            .create_window("b", Rect::new(10, 10, 100, 100), PixelFormat::Rgb24)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(wm.window_at(20.0, 20.0), Some(_b));
        assert_eq!(wm.window_at(5.0, 5.0), Some(a));
        wm.set_minimized(_b, true).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(wm.window_at(20.0, 20.0), Some(a));
    }

    #[test]
    fn zero_size_window_rejected() {
        let mut wm = WindowManager::new();
        assert!(wm
            .create_window("bad", Rect::new(0, 0, 0, 10), PixelFormat::Rgb24)
            .is_err());
    }
}
