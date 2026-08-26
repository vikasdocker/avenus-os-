// Aether Graphics - Error types for the graphics stack

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur in the Aether graphics stack.
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum GraphicsError {
    #[error("Display error: {0}")]
    Display(String),

    #[error("Renderer error: {0}")]
    Renderer(String),

    #[error("Wayland error: {0}")]
    Wayland(String),

    #[error("Input error: {0}")]
    Input(String),

    #[error("Window error: {0}")]
    Window(String),

    #[error("Cursor error: {0}")]
    Cursor(String),

    #[error("Output error: {0}")]
    Output(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Workspace error: {0}")]
    Workspace(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Security error: {0}")]
    Security(String),

    #[error("Compositor error: {0}")]
    Compositor(String),

    #[error("Shell error: {0}")]
    Shell(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("GPU unavailable")]
    GpuUnavailable,

    #[error("Drm unavailable")]
    DrmUnavailable,

    #[error("Wayland initialization failed: {0}")]
    WaylandInitFailed(String),

    #[error("Input backend unavailable")]
    InputBackendUnavailable,

    #[error("Renderer initialization failed: {0}")]
    RendererInitFailed(String),

    #[error("Framebuffer error: {0}")]
    Framebuffer(String),

    #[error("Mode error: {0}")]
    Mode(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Graphics stack error: {0}")]
    Stack(String),
}

impl GraphicsError {
    /// Returns the error code as a string for IPC reporting.
    pub fn code(&self) -> &str {
        match self {
            Self::Display(_) => "display",
            Self::Renderer(_) => "renderer",
            Self::Wayland(_) => "wayland",
            Self::Input(_) => "input",
            Self::Window(_) => "window",
            Self::Cursor(_) => "cursor",
            Self::Output(_) => "output",
            Self::Session(_) => "session",
            Self::Workspace(_) => "workspace",
            Self::Ipc(_) => "ipc",
            Self::Security(_) => "security",
            Self::Compositor(_) => "compositor",
            Self::Shell(_) => "shell",
            Self::Configuration(_) => "configuration",
            Self::NotImplemented(_) => "not_implemented",
            Self::GpuUnavailable => "gpu_unavailable",
            Self::DrmUnavailable => "drm_unavailable",
            Self::WaylandInitFailed(_) => "wayland_init_failed",
            Self::InputBackendUnavailable => "input_backend_unavailable",
            Self::RendererInitFailed(_) => "renderer_init_failed",
            Self::Framebuffer(_) => "framebuffer",
            Self::Mode(_) => "mode",
            Self::PermissionDenied(_) => "permission_denied",
            Self::InvalidParameter(_) => "invalid_parameter",
            Self::Connection(_) => "connection",
            Self::InvalidState(_) => "invalid_state",
            Self::Timeout(_) => "timeout",
            Self::Stack(_) => "stack",
        }
    }

    /// Maps a graphics error to the shared Aether core error kind.
    pub fn kind(&self) -> aether_core::error::ErrorKind {
        use aether_core::error::ErrorKind;
        match self {
            Self::PermissionDenied(_) | Self::Security(_) => ErrorKind::PermissionDenied,
            Self::InvalidParameter(_) | Self::Configuration(_) | Self::Mode(_) => {
                ErrorKind::InvalidInput
            }
            Self::Timeout(_) | Self::GpuUnavailable | Self::DrmUnavailable
            | Self::InputBackendUnavailable => ErrorKind::ResourceExhausted,
            _ => ErrorKind::ServiceFailed,
        }
    }
}

impl From<GraphicsError> for aether_core::error::AetherError {
    fn from(err: GraphicsError) -> Self {
        aether_core::error::AetherError::new(err.kind(), err.to_string())
    }
}
