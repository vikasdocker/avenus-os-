// Aether System Core - permission policy gate
//
// Wires the cross-domain `DefaultPermissionPolicy` into the
// system-core dispatcher. Every incoming IPC request is mapped to
// a `Capability`; the policy decision is then combined with the
// request's `actor_trust` to produce a final verdict.
//
// Rules:
//   * `Untrusted` actors are denied for every capability, period.
//     The dispatch path is the second line of defence: a
//     privileged agent (e.g. aether-system-core) must not
//     execute capabilities for a peer that has not been
//     authenticated.
//   * `Trusted` actors follow the policy:
//        Allow -> execute
//        RequireConsent -> in a single-shot daemon context this
//          means deny with a clear reason; the
//          approval-gated flow lives in the agentd / shell.
//        Deny -> deny

use aether_core::ipc::ActorTrust;
use aether_core::{Capability, CapabilityDomain, RiskLevel};
use aether_security::{Decision, DefaultPermissionPolicy, PermissionDecision, PermissionPolicy};

/// Result of evaluating a request against the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyVerdict {
    pub decision: Decision,
    pub reason: String,
}

impl PolicyVerdict {
    pub fn allow(reason: &str) -> Self {
        Self { decision: Decision::Allow, reason: reason.to_string() }
    }
    pub fn deny(reason: &str) -> Self {
        Self { decision: Decision::Deny, reason: reason.to_string() }
    }
    pub fn require_consent(reason: &str) -> Self {
        Self { decision: Decision::RequireConsent, reason: reason.to_string() }
    }
    pub fn is_allow(&self) -> bool {
        self.decision == Decision::Allow
    }
}

/// Maps an incoming IPC `command` to a `Capability`. Returns
/// `None` for commands that are not gated by the policy (status
/// reads, dispatcher-internal commands, etc.).
pub fn command_to_capability(command: &str) -> Option<Capability> {
    let (domain, name, risk) = match command {
        // Process control.
        "process.list" => (CapabilityDomain::Process, "list", RiskLevel::Low),
        "process.inspect" => (CapabilityDomain::Process, "inspect", RiskLevel::Low),

        // Storage reads.
        "storage.status" => (CapabilityDomain::Storage, "status", RiskLevel::Low),
        "file.list" => (CapabilityDomain::Filesystem, "list", RiskLevel::Low),
        "file.read" => (CapabilityDomain::Filesystem, "read", RiskLevel::Low),
        "file.search" => (CapabilityDomain::Filesystem, "search", RiskLevel::Low),

        // Filesystem mutations.
        "file.create" => (CapabilityDomain::Filesystem, "create", RiskLevel::Medium),
        "file.write" => (CapabilityDomain::Filesystem, "write", RiskLevel::Medium),
        "file.rename" => (CapabilityDomain::Filesystem, "rename", RiskLevel::Medium),
        "file.move" => (CapabilityDomain::Filesystem, "move", RiskLevel::Medium),
        "file.delete" => (CapabilityDomain::Filesystem, "delete", RiskLevel::High),

        // System control.
        "system.status" => (CapabilityDomain::System, "status", RiskLevel::Low),
        "system.info" => (CapabilityDomain::System, "info", RiskLevel::Low),
        "system.resources" => (CapabilityDomain::System, "resources", RiskLevel::Low),
        "system.uptime" => (CapabilityDomain::System, "uptime", RiskLevel::Low),
        "shutdown" | "system.shutdown" => {
            (CapabilityDomain::System, "shutdown", RiskLevel::Critical)
        }

        // Network reads (low risk).
        "network.status" => (CapabilityDomain::Network, "status", RiskLevel::Low),
        "network.interfaces" => (CapabilityDomain::Network, "interfaces", RiskLevel::Low),

        _ => return None,
    };
    Some(Capability::new(domain, name, risk))
}

/// Evaluates the policy for a given command and trust level.
pub fn evaluate(command: &str, trust: ActorTrust) -> PolicyVerdict {
    // Defence-in-depth: an untrusted actor is denied before the
    // policy is consulted. The system-core dispatcher never
    // executes a capability for a peer that has not been
    // authenticated.
    if trust == ActorTrust::Untrusted {
        return PolicyVerdict::deny("untrusted actor: capability denied by policy");
    }
    let Some(cap) = command_to_capability(command) else {
        // Commands that don't map to a capability are allowed
        // (they are dispatcher-internal or non-privileged).
        return PolicyVerdict::allow("command is not policy-gated");
    };
    let decision: PermissionDecision = DefaultPermissionPolicy.evaluate(&cap);
    match decision.decision() {
        Decision::Allow => PolicyVerdict::allow(decision.reason()),
        Decision::Deny => PolicyVerdict::deny(decision.reason()),
        Decision::RequireConsent => {
            // In the system-core single-shot daemon, consent
            // requires an interactive step that we do not have
            // here. The agentd's approval-gated flow is the
            // right place for that; here we surface it as a
            // deny-with-reason so the calling agent is told to
            // re-issue through the approval path.
            PolicyVerdict::require_consent(decision.reason())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_risk_capability_allows_trusted() {
        let v = evaluate("system.status", ActorTrust::Trusted);
        assert!(v.is_allow());
    }

    #[test]
    fn high_risk_capability_requires_consent_for_trusted() {
        let v = evaluate("file.delete", ActorTrust::Trusted);
        assert_eq!(v.decision, Decision::RequireConsent);
    }

    #[test]
    fn critical_capability_requires_consent_for_trusted() {
        let v = evaluate("system.shutdown", ActorTrust::Trusted);
        assert_eq!(v.decision, Decision::RequireConsent);
    }

    #[test]
    fn untrusted_actor_is_denied_for_low_risk_too() {
        let v = evaluate("system.status", ActorTrust::Untrusted);
        assert_eq!(v.decision, Decision::Deny);
        assert!(v.reason.contains("untrusted actor"));
    }

    #[test]
    fn untrusted_actor_is_denied_for_critical_shutdown() {
        let v = evaluate("system.shutdown", ActorTrust::Untrusted);
        assert_eq!(v.decision, Decision::Deny);
        assert!(v.reason.contains("untrusted actor"));
    }

    #[test]
    fn unknown_command_is_allowed_as_dispatcher_internal() {
        let v = evaluate("status", ActorTrust::Trusted);
        assert!(v.is_allow());
    }

    #[test]
    fn command_to_capability_maps_each_domain() {
        assert!(command_to_capability("file.read").is_some());
        assert!(command_to_capability("process.inspect").is_some());
        assert!(command_to_capability("network.status").is_some());
        assert!(command_to_capability("storage.status").is_some());
        assert!(command_to_capability("system.shutdown").is_some());
    }

    #[test]
    fn command_to_capability_returns_none_for_unknown() {
        assert!(command_to_capability("not.a.command").is_none());
    }
}
