// Aether Surface - client library for Aether applications.
//
//! Connects an application to the desktop shell's surface server, obtains
//! a window content rectangle, and delivers keyboard/close events.
//!
//! ```ignore
//! let mut surface = aether_surface::Surface::connect("calculator", "Calculator", 400, 480)?;
//! let rect = surface.rect();
//! // paint into fb0 within rect ...
//! while let Some(ev) = surface.poll() {
//!     match ev { aether_surface::SurfaceEvent::Key(c) => ..., SurfaceEvent::Closed => break }
//! }
//! ```

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Events delivered to a registered application.
pub enum SurfaceEvent {
    /// A printable character (or backspace marker) typed while focused.
    Key(char),
    Enter,
    Backspace,
    /// The shell asked this window to close.
    CloseRequested,
}

pub struct Surface {
    stream: TcpStream,
    pub window_id: u64,
    pub rect: Rect,
}

impl Surface {
    /// Registers a window with the desktop shell (port from env
    /// AETHER_SURFACE_PORT, default 4750).
    pub fn connect(app_id: &str, title: &str, w: u32, h: u32) -> Result<Self, String> {
        let port: u16 =
            std::env::var("AETHER_SURFACE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(4750);
        let stream = TcpStream::connect(("127.0.0.1", port))
            .map_err(|e| format!("surface server :{port}: {e}"))?;
        let mut stream = stream;
        let req = json!({ "op": "register", "app": app_id, "title": title, "w": w, "h": h });
        stream.write_all(format!("{req}\n").as_bytes()).map_err(|e| format!("register: {e}"))?;

        let mut line = String::new();
        std::io::BufReader::new(stream.try_clone().map_err(|e| e.to_string())?)
            .read_line(&mut line)
            .map_err(|e| format!("register recv: {e}"))?;
        let reply: Value =
            serde_json::from_str(line.trim()).map_err(|e| format!("bad register reply: {e}"))?;
        if reply["event"] != "registered" {
            return Err(format!("registration refused: {reply}"));
        }
        let window_id = reply["window_id"].as_u64().unwrap_or_default();
        let rect_arr = reply["rect"].as_array().cloned().unwrap_or_default();
        let nums = |i: usize| rect_arr.get(i).and_then(Value::as_i64).unwrap_or(0);
        Ok(Self {
            stream,
            window_id,
            rect: Rect {
                x: nums(0) as i32,
                y: nums(1) as i32,
                width: nums(2).max(1) as u32,
                height: nums(3).max(1) as u32,
            },
        })
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn window_id(&self) -> u64 {
        self.window_id
    }

    /// Blocking wait for the next event from the shell.
    pub fn poll(&mut self) -> Option<SurfaceEvent> {
        let mut line = String::new();
        let mut reader = BufReader::new(self.stream.try_clone().ok()?);
        reader.read_line(&mut line).ok()?;
        if line.trim().is_empty() {
            return None;
        }
        let v: Value = serde_json::from_str(line.trim()).ok()?;
        match v["event"].as_str() {
            Some("close") => Some(SurfaceEvent::CloseRequested),
            Some("key") => {
                let k = v["key"].as_str().unwrap_or_default();
                match k {
                    "\n" => Some(SurfaceEvent::Enter),
                    "\u{8}" => Some(SurfaceEvent::Backspace),
                    other if !other.is_empty() => Some(SurfaceEvent::Key(other.chars().next()?)),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
