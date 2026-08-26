// Aether Graphics - Security module for the graphics stack

use crate::error::GraphicsError;
use aether_core::capability::{Capability, RiskLevel};
use aether_core::identity::AetherIdentity;

/// Defines the security boundaries for the graphics stack.
pub struct GraphicsSecurity {
    pub identity: AetherIdentity,
    pub capabilities: Vec<Capability>,
}

impl GraphicsSecurity {
    pub fn new(identity: AetherIdentity, capabilities: Vec<Capability>) -> Self {
        Self {
            identity,
            capabilities,
        }
    }

    /// Verifies if a specific capability is granted to the current session.
    pub fn verify_capability(&self, required: &Capability) -> Result<(), GraphicsError> {
        if self.capabilities.iter().any(|c| c == required) {
            Ok(())
        } else {
            Err(GraphicsError::Security(format!(
                "Missing required capability: {}",
                required
            )))
        }
    }

    /// Verifies if the current identity has sufficient permissions for an action.
    pub fn verify_permission(&self, permission: &str) -> Result<(), GraphicsError> {
        if self.identity.has_permission(permission) {
            Ok(())
        } else {
            Err(GraphicsError::Security(format!(
                "Identity '{}' lacks permission: '{}'",
                self.identity.name, permission
            )))
        }
    }

    /// Checks if an action is considered high risk.
    pub fn is_high_risk_action(&self, required: &Capability) -> bool {
        required.risk_level >= RiskLevel::High
    }
}
