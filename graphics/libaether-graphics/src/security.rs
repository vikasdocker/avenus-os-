// Aether Graphics - Security module for the graphics stack
//
// Enforces window isolation, surface isolation, application isolation,
// and input isolation. Every privileged operation must pass through these checks.

use crate::error::GraphicsError;
use aether_core::capability::{Capability, RiskLevel};
use aether_core::identity::AetherIdentity;

/// Security boundaries for the graphics stack.
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

    /// Verifies if a specific capability is granted.
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

    /// Verifies if the current identity has sufficient permissions.
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

    // ---- window/surface isolation checks ----

    /// Checks if the client can inspect a specific window.
    /// Clients can always inspect their own windows; inspecting others requires
    /// the `window.control` capability.
    pub fn can_inspect(
        &self,
        client_app_id: &str,
        target_app_id: &str,
    ) -> Result<(), GraphicsError> {
        if client_app_id == target_app_id {
            return Ok(());
        }
        let req = Capability::new(
            aether_core::capability::CapabilityDomain::Application,
            "window.control",
            RiskLevel::High,
        );
        self.verify_capability(&req)
    }

    /// Checks if the client can control (focus/move/resize/close) a window.
    /// Only the window's owner or a privileged desktop shell may control it.
    pub fn can_control(
        &self,
        client_app_id: &str,
        target_app_id: &str,
    ) -> Result<(), GraphicsError> {
        if client_app_id == target_app_id {
            return Ok(());
        }
        let req = Capability::new(
            aether_core::capability::CapabilityDomain::Application,
            "window.control",
            RiskLevel::High,
        );
        self.verify_capability(&req)
    }

    /// Checks if the client can inject keyboard input into a window.
    pub fn can_inject(
        &self,
        client_app_id: &str,
        target_app_id: &str,
    ) -> Result<(), GraphicsError> {
        if client_app_id == target_app_id {
            return Ok(());
        }
        let req = Capability::new(
            aether_core::capability::CapabilityDomain::Application,
            "input.injection",
            RiskLevel::Critical,
        );
        self.verify_capability(&req)
    }

    /// Checks if the client can capture arbitrary screen contents.
    pub fn can_capture(&self) -> Result<(), GraphicsError> {
        let req = Capability::new(
            aether_core::capability::CapabilityDomain::Application,
            "screen.capture",
            RiskLevel::Critical,
        );
        self.verify_capability(&req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::identity::AetherIdentity;

    fn identity(name: &str) -> AetherIdentity {
        AetherIdentity::new(name, "graphics")
    }

    fn desktop_security() -> GraphicsSecurity {
        GraphicsSecurity::new(
            identity("aether-desktop"),
            vec![Capability::new(
                aether_core::capability::CapabilityDomain::Application,
                "window.control",
                RiskLevel::High,
            )],
        )
    }

    fn app_security() -> GraphicsSecurity {
        GraphicsSecurity::new(identity("calculator"), vec![])
    }

    #[test]
    fn same_app_can_inspect() {
        let sec = app_security();
        assert!(sec.can_inspect("calculator", "calculator").is_ok());
    }

    #[test]
    fn cross_app_inspect_requires_capability() {
        let sec = app_security();
        assert!(sec.can_inspect("calculator", "notes").is_err());
    }

    #[test]
    fn privileged_can_inspect_cross_app() {
        let sec = desktop_security();
        assert!(sec.can_inspect("desktop", "notes").is_ok());
    }

    #[test]
    fn same_app_can_control() {
        let sec = app_security();
        assert!(sec.can_control("calculator", "calculator").is_ok());
    }

    #[test]
    fn cross_app_control_requires_capability() {
        let sec = app_security();
        assert!(sec.can_control("calculator", "notes").is_err());
    }

    #[test]
    fn input_injection_requires_critical() {
        let sec = app_security();
        assert!(sec.can_inject("calculator", "notes").is_err());
    }

    #[test]
    fn screen_capture_requires_critical() {
        let sec = app_security();
        assert!(sec.can_capture().is_err());
    }

    #[test]
    fn desktop_has_high_risk_action() {
        let sec = desktop_security();
        let req = Capability::new(
            aether_core::capability::CapabilityDomain::Application,
            "window.control",
            RiskLevel::High,
        );
        assert!(sec.is_high_risk_action(&req));
    }
}
