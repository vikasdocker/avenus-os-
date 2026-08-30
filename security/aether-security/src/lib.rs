use aether_core::{Capability, RiskLevel};
use std::fmt::{self, Display};

pub mod audit;
pub mod credentials;
pub mod manifest_signing;
pub mod signed_update;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny,
    RequireConsent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDecision {
    decision: Decision,
    reason: String,
}

impl PermissionDecision {
    #[must_use]
    pub fn allow(reason: impl Into<String>) -> Self {
        Self { decision: Decision::Allow, reason: reason.into() }
    }

    #[must_use]
    pub fn deny(reason: impl Into<String>) -> Self {
        Self { decision: Decision::Deny, reason: reason.into() }
    }

    #[must_use]
    pub fn require_consent(reason: impl Into<String>) -> Self {
        Self { decision: Decision::RequireConsent, reason: reason.into() }
    }

    #[must_use]
    pub fn decision(&self) -> Decision {
        self.decision
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

pub trait PermissionPolicy {
    fn evaluate(&self, capability: &Capability) -> PermissionDecision;
}

#[derive(Debug, Copy, Clone, Default)]
pub struct DefaultPermissionPolicy;

impl PermissionPolicy for DefaultPermissionPolicy {
    fn evaluate(&self, capability: &Capability) -> PermissionDecision {
        match capability.risk_level {
            RiskLevel::Low => PermissionDecision::allow("low-risk local capability"),
            RiskLevel::Medium => PermissionDecision::require_consent("medium-risk capability"),
            RiskLevel::High | RiskLevel::Critical => {
                PermissionDecision::require_consent("elevated-risk capability")
            }
        }
    }
}

impl Display for Decision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::RequireConsent => "require-consent",
        };
        formatter.write_str(value)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{Decision, DefaultPermissionPolicy, PermissionPolicy};
    use aether_core::{Capability, CapabilityDomain, RiskLevel};

    #[test]
    fn low_risk_capability_is_allowed() {
        let capability = Capability::new(CapabilityDomain::System, "status.read", RiskLevel::Low);
        let decision = DefaultPermissionPolicy.evaluate(&capability);
        assert_eq!(decision.decision(), Decision::Allow);
    }
}
