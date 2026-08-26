// Aether Graphics - Core types and structures

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(Uuid);

impl WindowId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for WindowId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents a graphical surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Surface {
    pub id: Uuid,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
}

/// Pixel format for surfaces and framebuffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    Rgb24,
    Rgba8888,
    Bgra8888,
    Rgb565,
}

impl PixelFormat {
    /// Returns the number of bytes per pixel for this format.
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            Self::Rgb24 => 3,
            Self::Rgba8888 | Self::Bgra8888 => 4,
            Self::Rgb565 => 2,
        }
    }
}

impl fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rgb24 => write!(f, "RGB24"),
            Self::Rgba8888 => write!(f, "RGBA8888"),
            Self::Bgra8888 => write!(f, "BGRA8888"),
            Self::Rgb565 => write!(f, "RGB565"),
        }
    }
}

/// Represents a framebuffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Box<[u8]>,
    pub format: PixelFormat,
}

impl Framebuffer {
    /// Allocates a new zeroed framebuffer with a tightly packed stride.
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        let bpp = format.bytes_per_pixel();
        let stride = width * bpp;
        let size = (stride as usize) * (height as usize);
        Self {
            width,
            height,
            stride,
            data: vec![0u8; size].into_boxed_slice(),
            format,
        }
    }

    /// Returns true if the buffer size matches width/height/format expectations.
    pub fn is_consistent(&self) -> bool {
        self.data.len() == (self.stride as usize) * (self.height as usize)
    }

    pub fn clear(&mut self) {
        for byte in self.data.iter_mut() {
            *byte = 0;
        }
    }
}

/// Represents an input event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    MouseMove { x: f32, y: f32 },
    MouseButton { button: MouseButton, state: ButtonState },
    KeyInput { key: String, state: ButtonState },
    Scroll { delta_x: f32, delta_y: f32 },
    Touch { id: u32, x: f32, y: f32, state: TouchPhase },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchPhase {
    Began,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicsSession {
    pub session_id: String,
    pub active_workspace: u32,
    pub connected_devices: Vec<String>,
}

pub type GPUContext = String;
