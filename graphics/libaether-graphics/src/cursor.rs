// Aether Graphics - Cursor management for the graphics stack
//
// Owns the pointer sprite: position clamping to the virtual screen,
// visibility, and the active cursor shape.

use crate::error::GraphicsError;
use crate::types::PixelFormat;

/// Supported cursor shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Arrow,
    IBeam,
    Hand,
    Crosshair,
    ResizeNs,
    ResizeEw,
    Wait,
    Hidden,
}

impl CursorShape {
    /// Default hotspot offset (from top-left of the sprite) for this shape.
    pub fn hotspot(&self) -> (u32, u32) {
        match self {
            Self::Arrow | Self::Hand => (0, 0),
            Self::IBeam => (8, 8),
            Self::Crosshair | Self::Wait => (16, 16),
            Self::ResizeNs => (16, 16),
            Self::ResizeEw => (16, 16),
            Self::Hidden => (0, 0),
        }
    }
}

/// State of the pointer cursor.
#[derive(Debug, Clone)]
pub struct Cursor {
    pub shape: CursorShape,
    pub x: i32,
    pub y: i32,
    pub visible: bool,
    pub sprite_size: u32,
    pub sprite_format: PixelFormat,
}

/// Manages cursor position, visibility, and shape.
pub struct CursorManager {
    cursor: Cursor,
    screen_width: u32,
    screen_height: u32,
}

impl CursorManager {
    /// Creates a cursor manager bounded by the given virtual screen size.
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            cursor: Cursor {
                shape: CursorShape::Arrow,
                x: (screen_width / 2) as i32,
                y: (screen_height / 2) as i32,
                visible: true,
                sprite_size: 32,
                sprite_format: PixelFormat::Bgra8888,
            },
            screen_width,
            screen_height,
        }
    }

    /// Moves the cursor, clamping to the virtual screen bounds.
    pub fn move_by(&mut self, dx: i32, dy: i32) -> Result<(), GraphicsError> {
        self.cursor.x = (self.cursor.x + dx).clamp(0, self.screen_width as i32 - 1);
        self.cursor.y = (self.cursor.y + dy).clamp(0, self.screen_height as i32 - 1);
        Ok(())
    }

    /// Warps the cursor to an absolute position, clamped to bounds.
    pub fn warp_to(&mut self, x: i32, y: i32) -> Result<(), GraphicsError> {
        self.cursor.x = x.clamp(0, self.screen_width as i32 - 1);
        self.cursor.y = y.clamp(0, self.screen_height as i32 - 1);
        Ok(())
    }

    /// Shows or hides the cursor.
    pub fn set_visible(&mut self, visible: bool) -> Result<(), GraphicsError> {
        self.cursor.visible = visible;
        Ok(())
    }

    /// Changes the active cursor shape. `Hidden` also hides the sprite.
    pub fn set_shape(&mut self, shape: CursorShape) -> Result<(), GraphicsError> {
        if shape == CursorShape::Hidden {
            self.cursor.visible = false;
        }
        self.cursor.shape = shape;
        Ok(())
    }

    /// Returns the current cursor state.
    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// Updates the virtual screen bounds used for clamping.
    pub fn set_screen_bounds(&mut self, width: u32, height: u32) -> Result<(), GraphicsError> {
        if width == 0 || height == 0 {
            return Err(GraphicsError::Output("Screen bounds must be non-zero".to_string()));
        }
        self.screen_width = width;
        self.screen_height = height;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_clamps_to_screen() {
        let mut cm = CursorManager::new(800, 600);
        cm.move_by(-10_000, 10_000).unwrap_or_else(|e| panic!("{e}"));
        let c = cm.cursor();
        assert_eq!((c.x, c.y), (0, 599));
    }

    #[test]
    fn hidden_shape_hides_cursor() {
        let mut cm = CursorManager::new(800, 600);
        cm.set_shape(CursorShape::Hidden).unwrap_or_else(|e| panic!("{e}"));
        assert!(!cm.cursor().visible);
    }

    #[test]
    fn zero_bounds_rejected() {
        let mut cm = CursorManager::new(800, 600);
        assert!(cm.set_screen_bounds(0, 100).is_err());
    }
}
