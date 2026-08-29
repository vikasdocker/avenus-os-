// Confirmation framework - deterministic policy for safe multi-step actions.
//
// For this phase all safe window/app actions auto-execute (Low/Medium).
// The framework is built so future high-risk operations will require
// explicit user confirmation: AI -> proposed -> confirmation -> execution.

use aether_core::capability::{Capability, RiskLevel};

/// Outcome of a confirmation check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirmation {
    /// Execute immediately.
    AutoExecute,
    /// Would require user approval in a future phase.
    RequiresConsent { reason: String },
    /// Denied by policy.
    Denied { reason: String },
}

/// Central policy mapping risk to confirmation.
pub struct ConfirmationPolicy;

impl ConfirmationPolicy {
    /// Decide whether a capability needs consent.
    /// Safe desktop actions (app.launch/close, window.*) are Low/Medium and auto-execute.
    /// High/Critical would require consent later.
    pub fn decide(capability: &Capability) -> Confirmation {
        match capability.risk_level {
            RiskLevel::Low => Confirmation::AutoExecute,
            RiskLevel::Medium => {
                // For now, safe desktop mutations are still auto-executed.
                // If the capability name suggests destructive risk, require consent later.
                // Keep deterministic: allow all Medium in this phase.
                Confirmation::AutoExecute
            }
            RiskLevel::High | RiskLevel::Critical => Confirmation::RequiresConsent {
                reason: format!("{} requires user confirmation", capability.qualified_name()),
            },
        }
    }

    /// Check if a capability is auto-executable.
    pub fn is_auto(capability: &Capability) -> bool {
        matches!(Self::decide(capability), Confirmation::AutoExecute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::capability::{CapabilityDomain, RiskLevel};

    #[test]
    fn low_risk_auto_executes() {
        let cap = Capability::new(CapabilityDomain::Application, "list", RiskLevel::Low);
        assert_eq!(ConfirmationPolicy::decide(&cap), Confirmation::AutoExecute);
    }

    #[test]
    fn medium_risk_auto_executes_in_this_phase() {
        let cap = Capability::new(CapabilityDomain::Application, "launch", RiskLevel::Medium);
        assert_eq!(ConfirmationPolicy::decide(&cap), Confirmation::AutoExecute);
    }

    #[test]
    fn high_risk_requires_consent() {
        let cap = Capability::new(CapabilityDomain::System, "shutdown", RiskLevel::Critical);
        assert!(matches!(
            ConfirmationPolicy::decide(&cap),
            Confirmation::RequiresConsent { .. }
        ));
    }
}
