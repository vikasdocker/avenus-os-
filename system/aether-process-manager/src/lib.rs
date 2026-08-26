// Aether Process Manager - Process discovery, inspection, lifecycle, and security
// Provides the foundation for managing OS-level processes within Aether OS.

pub mod discovery;
pub mod inspection;
pub mod lifecycle;
pub mod security;

pub use discovery::ProcessDiscovery;
pub use inspection::{ProcessInfo, ProcessSnapshot};
pub use lifecycle::{ProcessLifecycle, ProcessState};
pub use security::{ProcessGuard, ProcessPolicy, SecurityViolation};
