// Aether Core - Foundation types and traits for the Aether OS
// This crate provides the shared type system used across all Aether OS components.

pub mod capability;
pub mod error;
pub mod identity;
pub mod ipc;
pub mod manifest;
pub mod sandbox;
pub mod types;

pub use capability::{Capability, CapabilityDomain, RiskLevel};
pub use error::AetherError;
pub use identity::{AetherIdentity, ComponentId};
pub use manifest::ServiceManifest;
pub use sandbox::{plan_sandbox, LinuxCapability, LinuxNamespace, ResourceLimits, SandboxPlan, SeccompFilterTag};
pub use types::{HealthStatus, ServiceStatus};
