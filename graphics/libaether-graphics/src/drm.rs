// Aether Graphics - DRM/KMS backend for native display output
//
// Provides direct GPU scanout via the Linux DRM subsystem.
// Falls back to fbdev (`/dev/fb0`) when DRM is unavailable.
//
// Phase 1.9 Part A: DRM/KMS / GPU detection.

use crate::error::GraphicsError;
use crate::types::DisplayMode;

/// DRM connector status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrmConnectionStatus {
    Connected,
    Disconnected,
    Unknown,
}

/// A DRM mode (resolution + refresh).
#[derive(Debug, Clone, Copy)]
pub struct DrmMode {
    /// Mode ID (DRM-internal).
    pub mode_id: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Refresh rate in mHz (e.g. 60000 = 60 Hz).
    pub refresh_mhz: u32,
}

impl DrmMode {
    /// Convert to a `DisplayMode`.
    #[must_use]
    pub fn to_display_mode(self) -> DisplayMode {
        DisplayMode {
            width: self.width,
            height: self.height,
            refresh_rate: self.refresh_mhz / 1000,
        }
    }
}

/// A DRM connector (physical output port).
#[derive(Debug, Clone)]
pub struct DrmConnector {
    /// Connector ID.
    pub connector_id: u32,
    /// Human-readable name (e.g. "HDMI-A-1").
    pub name: String,
    /// Connection status.
    pub status: DrmConnectionStatus,
    /// Supported modes.
    pub modes: Vec<DrmMode>,
}

/// A DRM CRTC (scanout pipeline).
#[derive(Debug, Clone)]
pub struct DrmCrtc {
    /// CRTC ID.
    pub crtc_id: u32,
    /// Currently active mode, if any.
    pub mode: Option<DrmMode>,
    /// Whether this CRTC is active.
    pub active: bool,
}

/// A DRM encoder (maps connectors to CRTCs).
#[derive(Debug, Clone)]
pub struct DrmEncoder {
    /// Encoder ID.
    pub encoder_id: u32,
    /// Supported CRTCs (bitmask).
    pub possible_crtcs: u32,
}

/// DRM framebuffer info.
#[derive(Debug, Clone, Copy)]
pub struct DrmFb {
    /// Framebuffer ID.
    pub fb_id: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Stride in bytes.
    pub stride: u32,
    /// Bits per pixel (always 32).
    pub bpp: u32,
}

/// DRM device capabilities discovered at init.
#[derive(Debug, Clone)]
pub struct DrmDeviceInfo {
    /// Device path (e.g. "/dev/dri/card0").
    pub path: String,
    /// Available connectors.
    pub connectors: Vec<DrmConnector>,
    /// Available CRTCs.
    pub crtcs: Vec<DrmCrtc>,
    /// Available encoders.
    pub encoders: Vec<DrmEncoder>,
}

impl DrmDeviceInfo {
    /// Find the best mode for a connected output: prefer
    /// the highest resolution, then highest refresh.
    #[must_use]
    pub fn best_mode_for_connector(&self, connector_id: u32) -> Option<&DrmMode> {
        let conn = self.connectors.iter().find(|c| c.connector_id == connector_id)?;
        if conn.status != DrmConnectionStatus::Connected {
            return None;
        }
        conn.modes.iter().max_by_key(|m| (m.width * m.height, m.refresh_mhz))
    }

    /// Find a connected connector, if any.
    #[must_use]
    pub fn first_connected(&self) -> Option<&DrmConnector> {
        self.connectors.iter().find(|c| c.status == DrmConnectionStatus::Connected)
    }
}

// ------------------------------------------------------- DRM backend trait

/// Trait for DRM/KMS backends. Implementations talk to
/// the real DRM subsystem or provide a stub for testing.
pub trait DrmBackend {
    /// Discover available DRM devices and their capabilities.
    fn discover(&self) -> Result<DrmDeviceInfo, GraphicsError>;

    /// Set the mode on a CRTC + connector pair.
    fn set_mode(
        &self,
        connector_id: u32,
        crtc_id: u32,
        mode: &DrmMode,
    ) -> Result<(), GraphicsError>;

    /// Allocate a dumb buffer for software rendering.
    fn allocate_buffer(&self, width: u32, height: u32) -> Result<DrmFb, GraphicsError>;

    /// Map a dumb buffer for CPU access (returns a byte
    /// slice the caller can render into).
    fn map_buffer(&self, fb: &DrmFb) -> Result<Vec<u8>, GraphicsError>;

    /// Flip the framebuffer to display the given buffer.
    fn page_flip(&self, fb: &DrmFb) -> Result<(), GraphicsError>;
}

// ------------------------------------------------------- stub backend (for tests / QEMU without DRM)

/// Stub DRM backend that returns no devices. Used when
/// DRM is not available and the shell falls back to fbdev.
pub struct StubDrmBackend;

impl DrmBackend for StubDrmBackend {
    fn discover(&self) -> Result<DrmDeviceInfo, GraphicsError> {
        Err(GraphicsError::DrmUnavailable)
    }

    fn set_mode(
        &self,
        _connector_id: u32,
        _crtc_id: u32,
        _mode: &DrmMode,
    ) -> Result<(), GraphicsError> {
        Err(GraphicsError::DrmUnavailable)
    }

    fn allocate_buffer(&self, _width: u32, _height: u32) -> Result<DrmFb, GraphicsError> {
        Err(GraphicsError::DrmUnavailable)
    }

    fn map_buffer(&self, _fb: &DrmFb) -> Result<Vec<u8>, GraphicsError> {
        Err(GraphicsError::DrmUnavailable)
    }

    fn page_flip(&self, _fb: &DrmFb) -> Result<(), GraphicsError> {
        Err(GraphicsError::DrmUnavailable)
    }
}

// ------------------------------------------------------- probe helper

/// Probe for DRM devices at standard paths. Returns the
/// first available device info, or `Err(DrmUnavailable)`.
#[must_use]
pub fn probe_drm_device() -> Result<DrmDeviceInfo, GraphicsError> {
    let paths = ["/dev/dri/card0", "/dev/dri/card1"];
    for path in &paths {
        if std::path::Path::new(path).exists() {
            // In a real implementation, this would open the
            // DRM fd and issue ioctls. For now, return a
            // stub that indicates DRM is available but we
            // haven't implemented the ioctl layer yet.
            return Err(GraphicsError::NotImplemented(format!(
                "DRM device found at {path} but ioctl layer not yet implemented"
            )));
        }
    }
    Err(GraphicsError::DrmUnavailable)
}

// ------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_unavailable() {
        let stub = StubDrmBackend;
        assert!(stub.discover().is_err());
    }

    #[test]
    fn best_mode_picks_highest_res() {
        let info = DrmDeviceInfo {
            path: "/dev/dri/card0".to_string(),
            connectors: vec![DrmConnector {
                connector_id: 1,
                name: "HDMI-A-1".to_string(),
                status: DrmConnectionStatus::Connected,
                modes: vec![
                    DrmMode { mode_id: 1, width: 1280, height: 720, refresh_mhz: 60000 },
                    DrmMode { mode_id: 2, width: 1920, height: 1080, refresh_mhz: 60000 },
                    DrmMode { mode_id: 3, width: 1920, height: 1080, refresh_mhz: 144000 },
                ],
            }],
            crtcs: vec![],
            encoders: vec![],
        };
        let best = info.best_mode_for_connector(1).unwrap();
        assert_eq!(best.width, 1920);
        assert_eq!(best.height, 1080);
        assert_eq!(best.refresh_mhz, 144000);
    }

    #[test]
    fn best_mode_none_for_disconnected() {
        let info = DrmDeviceInfo {
            path: "/dev/dri/card0".to_string(),
            connectors: vec![DrmConnector {
                connector_id: 1,
                name: "DP-1".to_string(),
                status: DrmConnectionStatus::Disconnected,
                modes: vec![DrmMode { mode_id: 1, width: 1920, height: 1080, refresh_mhz: 60000 }],
            }],
            crtcs: vec![],
            encoders: vec![],
        };
        assert!(info.best_mode_for_connector(1).is_none());
    }

    #[test]
    fn first_connected_finds_one() {
        let info = DrmDeviceInfo {
            path: "/dev/dri/card0".to_string(),
            connectors: vec![
                DrmConnector {
                    connector_id: 1,
                    name: "DP-1".to_string(),
                    status: DrmConnectionStatus::Disconnected,
                    modes: vec![],
                },
                DrmConnector {
                    connector_id: 2,
                    name: "HDMI-A-1".to_string(),
                    status: DrmConnectionStatus::Connected,
                    modes: vec![DrmMode {
                        mode_id: 1,
                        width: 1280,
                        height: 800,
                        refresh_mhz: 60000,
                    }],
                },
            ],
            crtcs: vec![],
            encoders: vec![],
        };
        assert_eq!(info.first_connected().unwrap().connector_id, 2);
    }

    #[test]
    fn mode_to_display_mode() {
        let m = DrmMode { mode_id: 1, width: 1920, height: 1080, refresh_mhz: 60000 };
        let dm = m.to_display_mode();
        assert_eq!(dm.width, 1920);
        assert_eq!(dm.height, 1080);
        assert_eq!(dm.refresh_rate, 60);
    }

    #[test]
    fn probe_drm_no_device() {
        // Unless /dev/dri/card0 exists on the test host,
        // this returns DrmUnavailable.
        let result = probe_drm_device();
        assert!(result.is_err());
    }
}
