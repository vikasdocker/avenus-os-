// Aether SDK - Rust client library for the Aether OS control plane.
//
// Speaks the newline-delimited JSON control protocol (IpcRequest /
// IpcResponse from aether-core) over TCP loopback, matching the protocol
// served by `aether-system-core`.

pub mod client;

pub use aether_core::ipc::{ActorTrust, IpcError, IpcRequest, IpcResponse};
pub use client::AetherClient;

/// SDK version reported to the control plane.
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");
