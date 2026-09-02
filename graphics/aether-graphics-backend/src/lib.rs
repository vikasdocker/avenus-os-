//! Graphics backend abstraction for Aether OS.
//!
//! Provides a trait-based abstraction over display output
//! backends (DRM/KMS, framebuffer, virtio-gpu). The real
//! kernel backend lives in a separate driver crate; this
//! crate defines the interface the shell consumes.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use std::fmt;

// ------------------------------------------------------------------- errors

/// Graphics backend error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsError {
    /// Backend not available.
    BackendUnavailable,
    /// Mode setting failed.
    ModeSettingFailed(String),
    /// Buffer allocation failed.
    BufferAllocationFailed,
    /// Page flip failed.
    PageFlipFailed,
    /// The requested resolution is not supported.
    UnsupportedResolution {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// I/O error.
    IoError(String),
}

impl fmt::Display for GraphicsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendUnavailable => write!(f, "graphics backend unavailable"),
            Self::ModeSettingFailed(s) => write!(f, "mode setting failed: {s}"),
            Self::BufferAllocationFailed => write!(f, "buffer allocation failed"),
            Self::PageFlipFailed => write!(f, "page flip failed"),
            Self::UnsupportedResolution { width, height } => {
                write!(f, "unsupported resolution: {width}x{height}")
            }
            Self::IoError(s) => write!(f, "I/O error: {s}"),
        }
    }
}

impl std::error::Error for GraphicsError {}

/// Convenience result type.
pub type GraphicsResult<T> = Result<T, GraphicsError>;

// ------------------------------------------------------------------ modes

/// A display mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayMode {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Refresh rate in Hz.
    pub refresh_mhz: u32,
    /// Bits per pixel.
    pub bpp: u8,
}

impl DisplayMode {
    /// Create a new display mode.
    #[must_use]
    pub fn new(width: u32, height: u32, refresh_hz: u32, bpp: u8) -> Self {
        Self { width, height, refresh_mhz: refresh_hz * 1000, bpp }
    }

    /// Refresh rate in Hz (rounded).
    #[must_use]
    pub fn refresh_hz(&self) -> u32 {
        self.refresh_mhz / 1000
    }

    /// Stride in bytes for this mode.
    #[must_use]
    pub fn stride(&self) -> u32 {
        self.width * (self.bpp as u32 / 8)
    }

    /// Total buffer size in bytes.
    #[must_use]
    pub fn buffer_size(&self) -> usize {
        (self.stride() * self.height) as usize
    }
}

impl fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}@{}Hz {}bpp", self.width, self.height, self.refresh_hz(), self.bpp)
    }
}

// --------------------------------------------------------------- connector

/// A display connector (output port).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connector {
    /// Connector ID.
    pub id: u32,
    /// Connector name (e.g. "HDMI-A-1").
    pub name: String,
    /// Connector type.
    pub kind: ConnectorType,
    /// Whether a display is connected.
    pub connected: bool,
    /// Supported modes.
    pub modes: Vec<DisplayMode>,
    /// Current mode, if active.
    pub current_mode: Option<DisplayMode>,
    /// Physical size in mm (width, height), if known.
    pub physical_size: Option<(u32, u32)>,
}

/// Connector type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectorType {
    /// VGA.
    Vga,
    /// DVI.
    Dvi,
    /// HDMI.
    Hdmi,
    /// DisplayPort.
    DisplayPort,
    /// USB-C / Thunderbolt.
    UsbC,
    /// Built-in panel (eDP, LVDS).
    Internal,
    /// Virtual / software framebuffer.
    Virtual,
}

impl ConnectorType {
    /// Kebab-case identifier.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vga => "vga",
            Self::Dvi => "dvi",
            Self::Hdmi => "hdmi",
            Self::DisplayPort => "displayport",
            Self::UsbC => "usb-c",
            Self::Internal => "internal",
            Self::Virtual => "virtual",
        }
    }
}

// ----------------------------------------------------------------- crtc

/// A CRTC (CRT Controller) — scans out a framebuffer to a connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crtc {
    /// CRTC ID.
    pub id: u32,
    /// Bound connector ID, if any.
    pub connector_id: Option<u32>,
    /// Current mode.
    pub mode: Option<DisplayMode>,
    /// Current framebuffer ID.
    pub framebuffer_id: Option<u64>,
}

// ----------------------------------------------------------------- buffer

/// A framebuffer (DRM framebuffer / scanout buffer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Framebuffer {
    /// Buffer ID.
    pub id: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Stride in bytes.
    pub stride: u32,
    /// Bits per pixel.
    pub bpp: u8,
    /// Buffer handle (opaque, backend-specific).
    pub handle: u64,
}

// ========================================================== backend trait

/// The graphics backend trait. Implementations provide
/// display output through DRM/KMS, framebuffer, or software
/// rendering.
pub trait GraphicsBackend: Send + Sync {
    /// Get the backend name.
    fn name(&self) -> &str;

    /// Initialize the backend.
    fn init(&mut self) -> GraphicsResult<()>;

    /// Enumerate available connectors.
    fn connectors(&self) -> GraphicsResult<Vec<Connector>>;

    /// Enumerate available CRTCs.
    fn crtcs(&self) -> GraphicsResult<Vec<Crtc>>;

    /// Get the current mode for a CRTC.
    fn current_mode(&self, crtc_id: u32) -> GraphicsResult<Option<DisplayMode>>;

    /// Set the mode for a CRTC (mode setting).
    fn set_mode(&mut self, crtc_id: u32, mode: &DisplayMode) -> GraphicsResult<()>;

    /// Allocate a framebuffer.
    fn allocate_framebuffer(
        &mut self,
        width: u32,
        height: u32,
        bpp: u8,
    ) -> GraphicsResult<Framebuffer>;

    /// Deallocate a framebuffer.
    fn deallocate_framebuffer(&mut self, id: u64) -> GraphicsResult<()>;

    /// Map a framebuffer for CPU writing (returns a mutable byte slice).
    fn map_buffer(&mut self, id: u64) -> GraphicsResult<*mut u8>;

    /// Unmap a previously mapped buffer.
    fn unmap_buffer(&mut self, id: u64) -> GraphicsResult<()>;

    /// Schedule a page flip (atomic or legacy).
    fn page_flip(&mut self, crtc_id: u32, framebuffer_id: u64) -> GraphicsResult<()>;

    /// Check if the backend supports hardware acceleration.
    fn has_hardware_acceleration(&self) -> bool;

    /// Get the maximum supported resolution.
    fn max_resolution(&self) -> GraphicsResult<(u32, u32)>;
}

// ========================================================== mock backend

/// Mock graphics backend for QEMU testing.
pub struct MockGraphicsBackend {
    connectors: Vec<Connector>,
    crtcs: Vec<Crtc>,
    framebuffers: Vec<Framebuffer>,
    next_fb_id: u64,
    initialized: bool,
}

impl MockGraphicsBackend {
    /// Create a new mock backend with a standard QEMU display.
    #[must_use]
    pub fn new() -> Self {
        let modes = vec![
            DisplayMode::new(1280, 800, 60, 32),
            DisplayMode::new(1920, 1080, 60, 32),
            DisplayMode::new(1024, 768, 60, 32),
        ];
        Self {
            connectors: vec![Connector {
                id: 1,
                name: "Virtual-1".into(),
                kind: ConnectorType::Virtual,
                connected: true,
                modes: modes.clone(),
                current_mode: Some(modes[0].clone()),
                physical_size: None,
            }],
            crtcs: vec![Crtc {
                id: 1,
                connector_id: Some(1),
                mode: Some(modes[0].clone()),
                framebuffer_id: None,
            }],
            framebuffers: Vec::new(),
            next_fb_id: 1,
            initialized: false,
        }
    }
}

impl Default for MockGraphicsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphicsBackend for MockGraphicsBackend {
    fn name(&self) -> &str {
        "mock-graphics"
    }

    fn init(&mut self) -> GraphicsResult<()> {
        self.initialized = true;
        Ok(())
    }

    fn connectors(&self) -> GraphicsResult<Vec<Connector>> {
        Ok(self.connectors.clone())
    }

    fn crtcs(&self) -> GraphicsResult<Vec<Crtc>> {
        Ok(self.crtcs.clone())
    }

    fn current_mode(&self, crtc_id: u32) -> GraphicsResult<Option<DisplayMode>> {
        self.crtcs
            .iter()
            .find(|c| c.id == crtc_id)
            .map(|c| c.mode.clone())
            .ok_or(GraphicsError::IoError("CRTC not found".into()))
    }

    fn set_mode(&mut self, crtc_id: u32, mode: &DisplayMode) -> GraphicsResult<()> {
        let crtc = self
            .crtcs
            .iter_mut()
            .find(|c| c.id == crtc_id)
            .ok_or_else(|| GraphicsError::IoError("CRTC not found".into()))?;

        // Validate mode against connector.
        if let Some(conn_id) = crtc.connector_id {
            let conn = self.connectors.iter().find(|c| c.id == conn_id);
            if let Some(conn) = conn {
                if !conn.modes.iter().any(|m| m.width == mode.width && m.height == mode.height) {
                    return Err(GraphicsError::UnsupportedResolution {
                        width: mode.width,
                        height: mode.height,
                    });
                }
            }
        }

        crtc.mode = Some(mode.clone());

        // Update connector current mode.
        if let Some(conn_id) = crtc.connector_id {
            if let Some(conn) = self.connectors.iter_mut().find(|c| c.id == conn_id) {
                conn.current_mode = Some(mode.clone());
            }
        }

        Ok(())
    }

    fn allocate_framebuffer(
        &mut self,
        width: u32,
        height: u32,
        bpp: u8,
    ) -> GraphicsResult<Framebuffer> {
        let id = self.next_fb_id;
        self.next_fb_id += 1;
        let stride = width * (bpp as u32 / 8);
        let fb = Framebuffer { id, width, height, stride, bpp, handle: id };
        self.framebuffers.push(fb.clone());
        Ok(fb)
    }

    fn deallocate_framebuffer(&mut self, id: u64) -> GraphicsResult<()> {
        self.framebuffers.retain(|f| f.id != id);
        Ok(())
    }

    fn map_buffer(&mut self, _id: u64) -> GraphicsResult<*mut u8> {
        // Mock: return a dummy pointer (not actually writable).
        // In real usage, this would mmap the buffer.
        Ok(std::ptr::null_mut())
    }

    fn unmap_buffer(&mut self, _id: u64) -> GraphicsResult<()> {
        Ok(())
    }

    fn page_flip(&mut self, crtc_id: u32, framebuffer_id: u64) -> GraphicsResult<()> {
        let crtc = self
            .crtcs
            .iter_mut()
            .find(|c| c.id == crtc_id)
            .ok_or_else(|| GraphicsError::IoError("CRTC not found".into()))?;
        crtc.framebuffer_id = Some(framebuffer_id);
        Ok(())
    }

    fn has_hardware_acceleration(&self) -> bool {
        false
    }

    fn max_resolution(&self) -> GraphicsResult<(u32, u32)> {
        let max_w =
            self.connectors.iter().flat_map(|c| &c.modes).map(|m| m.width).max().unwrap_or(1920);
        let max_h =
            self.connectors.iter().flat_map(|c| &c.modes).map(|m| m.height).max().unwrap_or(1080);
        Ok((max_w, max_h))
    }
}

/// Software framebuffer backend — no DRM/KMS, just a
/// malloc'd pixel buffer. Useful for headless testing.
pub struct SoftwareFramebuffer {
    width: u32,
    height: u32,
    stride: u32,
    bpp: u8,
    buffer: Vec<u8>,
}

impl SoftwareFramebuffer {
    /// Create a new software framebuffer.
    #[must_use]
    pub fn new(width: u32, height: u32, bpp: u8) -> Self {
        let stride = width * (bpp as u32 / 8);
        let size = (stride * height) as usize;
        Self { width, height, stride, bpp, buffer: vec![0; size] }
    }

    /// Get the pixel buffer as a byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }

    /// Get the pixel buffer as a mutable byte slice.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    /// Get the width.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the height.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the stride.
    #[must_use]
    pub fn stride(&self) -> u32 {
        self.stride
    }

    /// Get the bits per pixel.
    #[must_use]
    pub fn bpp(&self) -> u8 {
        self.bpp
    }

    /// Fill the entire buffer with a color (RGBX).
    pub fn fill(&mut self, r: u8, g: u8, b: u8) {
        for chunk in self.buffer.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = 0xFF;
        }
    }

    /// Set a single pixel.
    pub fn set_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8) {
        if x < self.width && y < self.height {
            let offset = (y * self.stride + x * 4) as usize;
            if offset + 4 <= self.buffer.len() {
                self.buffer[offset] = r;
                self.buffer[offset + 1] = g;
                self.buffer[offset + 2] = b;
                self.buffer[offset + 3] = 0xFF;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn display_mode_construction() {
        let m = DisplayMode::new(1920, 1080, 60, 32);
        assert_eq!(m.width, 1920);
        assert_eq!(m.height, 1080);
        assert_eq!(m.refresh_hz(), 60);
        assert_eq!(m.bpp, 32);
    }

    #[test]
    fn display_mode_stride() {
        let m = DisplayMode::new(1920, 1080, 60, 32);
        assert_eq!(m.stride(), 1920 * 4);
    }

    #[test]
    fn display_mode_buffer_size() {
        let m = DisplayMode::new(1920, 1080, 60, 32);
        assert_eq!(m.buffer_size(), 1920 * 1080 * 4);
    }

    #[test]
    fn display_mode_display() {
        let m = DisplayMode::new(1920, 1080, 60, 32);
        assert_eq!(m.to_string(), "1920x1080@60Hz 32bpp");
    }

    #[test]
    fn mock_backend_init() {
        let mut backend = MockGraphicsBackend::new();
        backend.init().unwrap();
        let connectors = backend.connectors().unwrap();
        assert_eq!(connectors.len(), 1);
        assert!(connectors[0].connected);
    }

    #[test]
    fn mock_backend_set_mode() {
        let mut backend = MockGraphicsBackend::new();
        backend.init().unwrap();
        let mode = DisplayMode::new(1920, 1080, 60, 32);
        backend.set_mode(1, &mode).unwrap();
        let current = backend.current_mode(1).unwrap().unwrap();
        assert_eq!(current.width, 1920);
    }

    #[test]
    fn mock_backend_allocate_framebuffer() {
        let mut backend = MockGraphicsBackend::new();
        backend.init().unwrap();
        let fb = backend.allocate_framebuffer(800, 600, 32).unwrap();
        assert_eq!(fb.width, 800);
        assert_eq!(fb.height, 600);
        assert_eq!(fb.stride, 800 * 4);
    }

    #[test]
    fn mock_backend_max_resolution() {
        let mut backend = MockGraphicsBackend::new();
        backend.init().unwrap();
        let (w, h) = backend.max_resolution().unwrap();
        assert!(w >= 1024);
        assert!(h >= 768);
    }

    #[test]
    fn connector_type_as_str() {
        assert_eq!(ConnectorType::Hdmi.as_str(), "hdmi");
        assert_eq!(ConnectorType::Virtual.as_str(), "virtual");
    }

    #[test]
    fn software_framebuffer_fill() {
        let mut fb = SoftwareFramebuffer::new(4, 4, 32);
        fb.fill(255, 128, 0);
        let bytes = fb.as_bytes();
        assert_eq!(bytes[0], 255);
        assert_eq!(bytes[1], 128);
        assert_eq!(bytes[2], 0);
        assert_eq!(bytes[3], 0xFF);
    }

    #[test]
    fn software_framebuffer_set_pixel() {
        let mut fb = SoftwareFramebuffer::new(4, 4, 32);
        fb.set_pixel(1, 1, 100, 200, 50);
        let bytes = fb.as_bytes();
        let offset = 1 * 4 * 4 + 1 * 4; // y=1, x=1
        assert_eq!(bytes[offset], 100);
        assert_eq!(bytes[offset + 1], 200);
        assert_eq!(bytes[offset + 2], 50);
    }

    #[test]
    fn graphics_error_display() {
        let e = GraphicsError::UnsupportedResolution { width: 800, height: 600 };
        assert!(e.to_string().contains("800x600"));
    }

    #[test]
    fn mock_page_flip() {
        let mut backend = MockGraphicsBackend::new();
        backend.init().unwrap();
        let fb = backend.allocate_framebuffer(800, 600, 32).unwrap();
        backend.page_flip(1, fb.id).unwrap();
        let crtc = backend.crtcs().unwrap();
        assert_eq!(crtc[0].framebuffer_id, Some(fb.id));
    }
}
