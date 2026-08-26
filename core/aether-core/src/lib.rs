// Aether Core - Foundation types and traits for the Aether OS
// This crate provides the shared type system used across all Aether OS components.

pub mod error;
pub mod ipc;
pub mod manifest;
pub mod identity;
pub mod capability;
pub mod types;

pub use error::AetherError;
pub use identity::{AetherIdentity, ComponentId};
pub use capability::{Capability, CapabilityDomain, RiskLevel};
pub use manifest::ServiceManifest;
pub use types::{ServiceStatus, HealthStatus};