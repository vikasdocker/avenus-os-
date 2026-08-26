// Aether Graphics - Foundation types and traits for the Aether OS graphical stack
// This crate provides shared types for the entire graphics pipeline:
// display, renderer, compositor, input, window, cursor, output, session, workspace, IPC, security.

pub mod types;
pub mod error;
pub mod ipc;
pub mod security;
pub mod session;
pub mod workspace;
pub mod display;
pub mod renderer;
pub mod input;
pub mod window;
pub mod cursor;
pub mod output;
pub mod compositor;
pub mod desktop_shell;

pub use types::*;
pub use error::GraphicsError;
pub use ipc::{GraphicsCommand, GraphicsCommandType, GraphicsIpc, GraphicsResponse};
pub use security::GraphicsSecurity;
pub use session::GraphicsSessionManager;
pub use workspace::WorkspaceManager;
pub use display::DisplayManager;
pub use renderer::Renderer;
pub use input::InputManager;
pub use window::WindowManager;
pub use cursor::CursorManager;
pub use output::OutputManager;
pub use compositor::Compositor;
pub use desktop_shell::DesktopShell;
