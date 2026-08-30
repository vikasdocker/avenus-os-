// Aether System Core - service lifecycle owner for Aether OS.
//
// Loads machine-readable service manifests, resolves the dependency graph,
// and owns the start/stop/restart/supervise lifecycle of every service.

pub mod graph;
pub mod loader;
pub mod manager;
pub mod policy;

pub use graph::{DependencyGraph, GraphError};
pub use loader::load_manifests_from_dir;
pub use manager::{build_manager, ServiceExecutor, ServiceHandle, ServiceManager};
pub use policy::{command_to_capability, evaluate, PolicyVerdict};

/// Protocol version of the system-core control interface.
pub const CONTROL_PROTOCOL_VERSION: &str = "1";
