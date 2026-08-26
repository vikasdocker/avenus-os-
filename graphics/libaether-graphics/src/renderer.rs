// Aether Graphics - Renderer interface for the graphics stack
//
// Defines the rendering contract shared by all Aether render backends
// (software, GPU). Backends receive a framebuffer and produce pixels;
// scheduling and damage tracking live in the compositor.

use crate::error::GraphicsError;
use crate::types::{Framebuffer, PixelFormat};

/// Statistics about a rendered frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStats {
    pub frame_number: u64,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
}

/// The renderer trait implemented by every Aether render backend.
pub trait RenderBackend {
    /// Human readable backend name (e.g. "softpipe", "vulkan").
    fn name(&self) -> &str;

    /// Prepares the backend to render into a framebuffer of the given geometry.
    fn configure(&mut self, width: u32, height: u32, format: PixelFormat) -> Result<(), GraphicsError>;

    /// Performs a full-frame clear using the given RGBA color.
    fn clear(&mut self, rgba: [u8; 4]) -> Result<(), GraphicsError>;

    /// Flushes pending work and returns statistics for the completed frame.
    fn submit(&mut self) -> Result<FrameStats, GraphicsError>;
}

/// Software reference renderer. Deterministic, dependency-free backend used
/// for tests, headless sessions, and as a fallback when no GPU is present.
pub struct Renderer {
    target: Option<Framebuffer>,
    frames_rendered: u64,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            target: None,
            frames_rendered: 0,
        }
    }

    /// Returns the number of successfully submitted frames.
    pub fn frames_rendered(&self) -> u64 {
        self.frames_rendered
    }

    /// Returns a mutable reference to the bound framebuffer, if configured.
    pub fn target(&mut self) -> Option<&mut Framebuffer> {
        self.target.as_mut()
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderBackend for Renderer {
    fn name(&self) -> &str {
        "aether-softpipe"
    }

    fn configure(&mut self, width: u32, height: u32, format: PixelFormat) -> Result<(), GraphicsError> {
        if width == 0 || height == 0 {
            return Err(GraphicsError::InvalidParameter(
                "Render target dimensions must be non-zero".to_string(),
            ));
        }
        self.target = Some(Framebuffer::new(width, height, format));
        Ok(())
    }

    fn clear(&mut self, rgba: [u8; 4]) -> Result<(), GraphicsError> {
        let bpp = self
            .target
            .as_ref()
            .ok_or_else(|| GraphicsError::Renderer("No render target configured".to_string()))?
            .format
            .bytes_per_pixel();
        let fb = self
            .target
            .as_mut()
            .ok_or_else(|| GraphicsError::Renderer("No render target configured".to_string()))?;
        let row = rgba.as_ref();
        for chunk in fb.data.chunks_exact_mut(bpp as usize) {
            let src: &[u8] = match fb.format {
                PixelFormat::Rgb24 | PixelFormat::Rgb565 => &[row[0], row[1], row[2]],
                _ => &rgba,
            };
            let n = chunk.len().min(src.len());
            // Rgb565 packs two pixels per 4 bytes; software fallback writes raw bytes.
            if fb.format == PixelFormat::Rgb565 && chunk.len() == 4 {
                let px = [src[0], src[1]];
                chunk.copy_from_slice(&[px[0], px[1], px[0], px[1]]);
                continue;
            }
            chunk[..n].copy_from_slice(&src[..n]);
        }
        Ok(())
    }

    fn submit(&mut self) -> Result<FrameStats, GraphicsError> {
        let fb = self
            .target
            .as_ref()
            .ok_or_else(|| GraphicsError::Renderer("No render target configured".to_string()))?;
        if !fb.is_consistent() {
            return Err(GraphicsError::Framebuffer(
                "Framebuffer size inconsistent with geometry".to_string(),
            ));
        }
        self.frames_rendered += 1;
        Ok(FrameStats {
            frame_number: self.frames_rendered,
            width: fb.width,
            height: fb.height,
            pixel_format: fb.format,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_and_submit_frame() {
        let mut r = Renderer::new();
        r.configure(64, 32, PixelFormat::Rgba8888)
            .unwrap_or_else(|e| panic!("{e}"));
        r.clear([10, 20, 30, 255]).unwrap_or_else(|e| panic!("{e}"));
        let stats = r.submit().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(stats.frame_number, 1);
        assert_eq!(stats.width, 64);
        assert_eq!(r.frames_rendered(), 1);
    }

    #[test]
    fn submit_without_target_fails() {
        let mut r = Renderer::new();
        assert!(r.submit().is_err());
    }

    #[test]
    fn zero_dimensions_rejected() {
        let mut r = Renderer::new();
        assert!(r.configure(0, 100, PixelFormat::Rgb24).is_err());
    }
}
