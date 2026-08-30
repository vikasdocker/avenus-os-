// Aether Graphics - Foundation types and traits for the Aether OS graphical stack
// This crate provides shared types for the entire graphics pipeline:
// display, renderer, compositor, input, window, cursor, output, session, workspace, IPC, security.

pub mod compositor;
pub mod cursor;
pub mod desktop_shell;
pub mod display;
pub mod error;
pub mod input;
pub mod ipc;
pub mod output;
pub mod renderer;
pub mod security;
pub mod session;
pub mod types;
pub mod window;
pub mod workspace;

pub use compositor::Compositor;
pub use cursor::CursorManager;
pub use desktop_shell::DesktopShell;
pub use display::DisplayManager;
pub use error::GraphicsError;
pub use input::InputManager;
pub use ipc::{GraphicsCommand, GraphicsCommandType, GraphicsIpc, GraphicsResponse};
pub use output::OutputManager;
pub use renderer::Renderer;
pub use security::GraphicsSecurity;
pub use session::GraphicsSessionManager;
pub use types::*;
pub use window::WindowManager;
pub use workspace::WorkspaceManager;
