// Agent Runtime - Action Validator
//
// Before execution, every action must pass through:
//   Schema validation → Capability validation → Policy validation
//   → Resource validation → Risk classification → Confirmation if required

use crate::action::{Action, ActionRisk};
use crate::errors::AgentError;

/// Result of validating an action.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub schema_ok: bool,
    pub capability_ok: bool,
    pub policy_ok: bool,
    pub risk_level: ActionRisk,
    pub requires_confirmation: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn passed(risk: ActionRisk, needs_confirm: bool) -> Self {
        Self {
            valid: true,
            schema_ok: true,
            capability_ok: true,
            policy_ok: true,
            risk_level: risk,
            requires_confirmation: needs_confirm,
            errors: Vec::new(),
        }
    }

    pub fn failed(errors: Vec<String>) -> Self {
        Self {
            valid: false,
            schema_ok: false,
            capability_ok: false,
            policy_ok: false,
            risk_level: ActionRisk::Low,
            requires_confirmation: false,
            errors,
        }
    }
}

/// Validates actions before execution.
pub struct Validator {
    granted_capabilities: Vec<String>,
}

impl Validator {
    pub fn new(granted_capabilities: Vec<String>) -> Self {
        Self { granted_capabilities }
    }

    /// Validates an action through all validation stages.
    pub fn validate(&self, action: &Action) -> Result<ValidationResult, AgentError> {
        let mut errors = Vec::new();

        // Stage 1: Schema validation (action structure is valid)
        let schema_ok = self.validate_schema(action, &mut errors);

        // Stage 2: Capability validation
        let capability_ok = self.validate_capabilities(action, &mut errors);

        // Stage 3: Policy validation
        let policy_ok = self.validate_policy(action, &mut errors);

        // Stage 4: Risk classification determines confirmation requirement
        let needs_confirmation =
            matches!(action.risk_level, ActionRisk::High | ActionRisk::Critical);

        let valid = schema_ok && capability_ok && policy_ok;

        Ok(ValidationResult {
            valid,
            schema_ok,
            capability_ok,
            policy_ok,
            risk_level: action.risk_level,
            requires_confirmation: needs_confirmation,
            errors,
        })
    }

    fn validate_schema(&self, action: &Action, errors: &mut Vec<String>) -> bool {
        // Validate action has required fields
        if action.session_id.is_empty() {
            errors.push("Action missing session ID".to_string());
            return false;
        }
        if action.reason.is_empty() {
            errors.push("Action missing reason".to_string());
            return false;
        }
        // Validate typed params have required fields
        match &action.variant {
            crate::action::ActionVariant::ApplicationLaunch(p) => {
                if p.application_id.is_empty() {
                    errors.push("Application launch missing application_id".to_string());
                    return false;
                }
            }
            crate::action::ActionVariant::FileRead(p) => {
                if p.path.is_empty() {
                    errors.push("File action missing path".to_string());
                    return false;
                }
            }
            crate::action::ActionVariant::FileList(p) => {
                if p.path.is_empty() {
                    errors.push("File action missing path".to_string());
                    return false;
                }
            }
            crate::action::ActionVariant::FileDelete(p) => {
                if p.path.is_empty() {
                    errors.push("File delete missing path".to_string());
                    return false;
                }
                // Extra validation: block dangerous paths
                if p.path == "/" || p.path == "/root" || p.path == "/etc" {
                    errors.push("Refusing to delete system directory".to_string());
                    return false;
                }
            }
            _ => {}
        }
        true
    }

    fn validate_capabilities(&self, action: &Action, errors: &mut Vec<String>) -> bool {
        let mut all_granted = true;
        for cap in &action.requested_capabilities {
            if !self.granted_capabilities.iter().any(|c| c == cap) {
                errors.push(format!("Missing capability: {cap}"));
                all_granted = false;
            }
        }
        all_granted
    }

    fn validate_policy(&self, action: &Action, errors: &mut Vec<String>) -> bool {
        // Basic policy: block raw shell execution attempts
        // This is a structural check — the ActionVariant enum doesn't have
        // a Shell variant, so this is defense-in-depth.
        let name = action.action_name();
        if name.contains("sh ") || name.contains("bash ") || name.contains("exec") {
            errors.push("Policy violation: raw shell execution blocked".to_string());
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{
        ActionVariant, ApplicationLaunchParams, CredentialSealParams, FileDeleteParams,
        FileReadParams, ServiceRestartParams, SystemRebootParams, SystemShutdownParams,
    };

    fn validator() -> Validator {
        Validator::new(vec![
            "application.launch".to_string(),
            "application.close".to_string(),
            "file.read".to_string(),
            "file.list".to_string(),
            "file.delete".to_string(),
            "system.status".to_string(),
            "window.list".to_string(),
        ])
    }

    fn full_validator() -> Validator {
        Validator::new(vec![
            "application.launch".to_string(),
            "application.close".to_string(),
            "file.read".to_string(),
            "file.list".to_string(),
            "file.delete".to_string(),
            "file.write".to_string(),
            "system.status".to_string(),
            "system.shutdown".to_string(),
            "system.reboot".to_string(),
            "service.restart".to_string(),
            "credential.seal".to_string(),
            "window.list".to_string(),
            "display.brightness".to_string(),
            "policy.reload".to_string(),
        ])
    }

    #[test]
    fn valid_action_passes() {
        let v = validator();
        let a = Action::new(
            "s1",
            ActionVariant::ApplicationLaunch(ApplicationLaunchParams {
                application_id: "calc".to_string(),
            }),
            "user asked",
        );
        let result = match v.validate(&a) {
            Ok(r) => r,
            Err(e) => panic!("validate failed: {e}"),
        };
        assert!(result.valid);
        assert!(result.schema_ok);
        assert!(result.capability_ok);
    }

    #[test]
    fn missing_capability_fails() {
        let v = validator();
        let a = Action::new(
            "s1",
            ActionVariant::FileWrite(crate::action::FileWriteParams {
                path: "/tmp/x".to_string(),
                content: "data".to_string(),
            }),
            "write",
        );
        let result = match v.validate(&a) {
            Ok(r) => r,
            Err(e) => panic!("validate failed: {e}"),
        };
        assert!(!result.valid);
        assert!(!result.capability_ok);
        assert!(result.errors.iter().any(|e| e.contains("file.write")));
    }

    #[test]
    fn empty_session_id_fails() {
        let v = validator();
        let mut a = Action::new("s1", ActionVariant::SystemStatus, "check");
        a.session_id = String::new();
        let result = match v.validate(&a) {
            Ok(r) => r,
            Err(e) => panic!("validate failed: {e}"),
        };
        assert!(!result.valid);
        assert!(!result.schema_ok);
    }

    #[test]
    fn empty_reason_fails() {
        let v = validator();
        let mut a = Action::new("s1", ActionVariant::SystemStatus, "");
        a.reason = String::new();
        let result = match v.validate(&a) {
            Ok(r) => r,
            Err(e) => panic!("validate failed: {e}"),
        };
        assert!(!result.valid);
    }

    #[test]
    fn high_risk_requires_confirmation() {
        let v = validator();
        let a = Action::new(
            "s1",
            ActionVariant::FileDelete(FileDeleteParams { path: "/tmp/test".to_string() }),
            "cleanup",
        );
        let result = match v.validate(&a) {
            Ok(r) => r,
            Err(e) => panic!("validate failed: {e}"),
        };
        assert!(result.valid);
        assert!(result.requires_confirmation);
    }

    #[test]
    fn low_risk_does_not_require_confirmation() {
        let v = validator();
        let a = Action::new("s1", ActionVariant::SystemStatus, "check");
        let result = match v.validate(&a) {
            Ok(r) => r,
            Err(e) => panic!("validate failed: {e}"),
        };
        assert!(!result.requires_confirmation);
    }

    #[test]
    fn empty_application_id_fails() {
        let v = validator();
        let a = Action::new(
            "s1",
            ActionVariant::ApplicationLaunch(ApplicationLaunchParams {
                application_id: String::new(),
            }),
            "launch",
        );
        let result = match v.validate(&a) {
            Ok(r) => r,
            Err(e) => panic!("validate failed: {e}"),
        };
        assert!(!result.valid);
        assert!(!result.schema_ok);
    }

    #[test]
    fn system_directory_delete_blocked() {
        let v = validator();
        let a = Action::new(
            "s1",
            ActionVariant::FileDelete(FileDeleteParams { path: "/".to_string() }),
            "nuke",
        );
        let result = match v.validate(&a) {
            Ok(r) => r,
            Err(e) => panic!("validate failed: {e}"),
        };
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("system directory")));
    }

    // === Phase 11.10: High-risk action gating validation ===

    #[test]
    fn system_shutdown_requires_confirmation() {
        let v = full_validator();
        let a = Action::new(
            "s1",
            ActionVariant::SystemShutdown(SystemShutdownParams { delay_ms: 5000 }),
            "shutdown",
        );
        let result = v.validate(&a).unwrap();
        assert!(result.valid);
        assert!(result.requires_confirmation, "SystemShutdown must require user consent");
    }

    #[test]
    fn system_reboot_requires_confirmation() {
        let v = full_validator();
        let a = Action::new(
            "s1",
            ActionVariant::SystemReboot(SystemRebootParams { delay_ms: 5000 }),
            "reboot",
        );
        let result = v.validate(&a).unwrap();
        assert!(result.valid);
        assert!(result.requires_confirmation, "SystemReboot must require user consent");
    }

    #[test]
    fn credential_seal_requires_confirmation() {
        let v = full_validator();
        let a = Action::new(
            "s1",
            ActionVariant::CredentialSeal(CredentialSealParams {
                name: "test-key".to_string(),
                plaintext: "secret-data".to_string(),
            }),
            "seal credential",
        );
        let result = v.validate(&a).unwrap();
        assert!(result.valid);
        assert!(result.requires_confirmation, "CredentialSeal must require user consent");
    }

    #[test]
    fn service_restart_does_not_require_confirmation() {
        let v = full_validator();
        let a = Action::new(
            "s1",
            ActionVariant::ServiceRestart(ServiceRestartParams { service_id: "nginx".to_string() }),
            "restart service",
        );
        let result = v.validate(&a).unwrap();
        assert!(result.valid);
        // ServiceRestart is Medium risk — only High/Critical require confirmation.
        assert!(
            !result.requires_confirmation,
            "ServiceRestart is Medium risk, no confirmation needed"
        );
    }

    #[test]
    fn file_read_does_not_require_confirmation() {
        let v = validator();
        let a = Action::new(
            "s1",
            ActionVariant::FileRead(FileReadParams { path: "/tmp/file.txt".to_string() }),
            "read file",
        );
        let result = v.validate(&a).unwrap();
        assert!(result.valid);
        assert!(!result.requires_confirmation, "FileRead must NOT require user consent");
    }

    #[test]
    fn system_status_does_not_require_confirmation() {
        let v = validator();
        let a = Action::new("s1", ActionVariant::SystemStatus, "check status");
        let result = v.validate(&a).unwrap();
        assert!(!result.requires_confirmation);
    }

    #[test]
    fn application_launch_does_not_require_confirmation() {
        let v = validator();
        let a = Action::new(
            "s1",
            ActionVariant::ApplicationLaunch(ApplicationLaunchParams {
                application_id: "calculator".to_string(),
            }),
            "launch app",
        );
        let result = v.validate(&a).unwrap();
        assert!(result.valid);
        assert!(!result.requires_confirmation, "ApplicationLaunch must NOT require user consent");
    }

    #[test]
    fn policy_validation_blocks_untrusted_actions() {
        // Validator with no capabilities should block capability-gated actions.
        let v = Validator::new(vec![]);
        let a = Action::new(
            "s1",
            ActionVariant::FileDelete(FileDeleteParams { path: "/tmp/test".to_string() }),
            "delete",
        );
        let result = v.validate(&a).unwrap();
        assert!(!result.valid);
        assert!(!result.capability_ok);
    }

    #[test]
    fn policy_validation_passes_trusted_actions() {
        let v = Validator::new(vec!["file.delete".to_string()]);
        let a = Action::new(
            "s1",
            ActionVariant::FileDelete(FileDeleteParams { path: "/tmp/test".to_string() }),
            "delete",
        );
        let result = v.validate(&a).unwrap();
        assert!(result.valid);
        assert!(result.capability_ok);
        assert!(result.requires_confirmation, "Even trusted high-risk needs consent");
    }

    #[test]
    fn proposal_risk_gating_blocks_low_risk_for_propose_update() {
        use crate::llm::MockLlmProvider;
        use crate::proposal_generator::ProposalGenerator;
        use aether_agent_core::{Observation, ObservationSeverity, ProposalRisk, TaskKind};

        let json = r#"[
            {
                "kind": "ProposeUpdate",
                "title": "System update",
                "description": "Update available",
                "reasoning": "...",
                "risk": "Low"
            }
        ]"#;
        let fake = MockLlmProvider::single(json);
        let gen = ProposalGenerator::new(&fake);
        let obs = vec![Observation {
            id: "obs-1".to_string(),
            component: "system".to_string(),
            summary: "update available".to_string(),
            detail: None,
            severity: ObservationSeverity::Notice,
            timestamp_ms: 1000,
            data: None,
        }];
        let (_, rejected) = gen.generate(&obs, 2000).unwrap();
        // ProposeUpdate requires High risk; Low should be rejected.
        assert_eq!(rejected.len(), 1);
    }

    #[test]
    fn proposal_risk_gating_passes_high_risk_for_propose_update() {
        use crate::llm::MockLlmProvider;
        use crate::proposal_generator::ProposalGenerator;
        use aether_agent_core::{Observation, ObservationSeverity};

        let json = r#"[
            {
                "kind": "ProposeUpdate",
                "title": "System update",
                "description": "Update available",
                "reasoning": "...",
                "risk": "High"
            }
        ]"#;
        let fake = MockLlmProvider::single(json);
        let gen = ProposalGenerator::new(&fake);
        let obs = vec![Observation {
            id: "obs-1".to_string(),
            component: "system".to_string(),
            summary: "update available".to_string(),
            detail: None,
            severity: ObservationSeverity::Notice,
            timestamp_ms: 1000,
            data: None,
        }];
        let (accepted, rejected) = gen.generate(&obs, 2000).unwrap();
        assert_eq!(accepted.len(), 1);
        assert!(rejected.is_empty());
    }

    #[test]
    fn approval_request_lifecycle() {
        use crate::approval::{ApprovalDecision, ApprovalRequest, ApprovalStatus};

        let req =
            ApprovalRequest::new("session-1", "prop-1", "Restart nginx", "high", "service down");
        assert_eq!(req.status, ApprovalStatus::Pending);
        assert!(req.is_pending());

        let decision = ApprovalDecision::approve(req.id.clone(), "user confirmed");
        assert_eq!(decision.decision, ApprovalStatus::Approved);
        assert_eq!(decision.request_id, req.id);
    }

    #[test]
    fn deny_decision_records_correct_status() {
        use crate::approval::{ApprovalDecision, ApprovalRequest, ApprovalStatus};

        let req =
            ApprovalRequest::new("session-1", "prop-1", "Delete file", "critical", "user action");
        let decision = ApprovalDecision::deny(req.id.clone(), "too dangerous");
        assert_eq!(decision.decision, ApprovalStatus::Denied);
        assert!(decision.reason.contains("dangerous"));
    }

    #[test]
    fn policy_reload_requires_confirmation() {
        let v = full_validator();
        let a = Action::new("s1", ActionVariant::PolicyReload, "reload policy");
        let result = match v.validate(&a) {
            Ok(r) => r,
            Err(e) => panic!("validate failed: {e}"),
        };
        assert!(result.valid);
        assert!(result.requires_confirmation);
    }

    #[test]
    fn proposal_runner_triage_separates_by_risk() {
        use crate::llm::MockLlmProvider;
        use crate::proposal_runner::ProposalRunner;
        use aether_agent_core::{Proposal, ProposalRisk, TaskKind};

        let low = Proposal::new(
            "p1",
            TaskKind::Notify,
            "Notify",
            "desc",
            "reason",
            ProposalRisk::Low,
            1000,
        )
        .unwrap();
        let high = Proposal::new(
            "p2",
            TaskKind::ProposeUpdate,
            "Update",
            "desc",
            "reason",
            ProposalRisk::High,
            1000,
        )
        .unwrap();
        let critical = Proposal::new(
            "p3",
            TaskKind::SecurityControl,
            "Scan",
            "desc",
            "reason",
            ProposalRisk::Critical,
            1000,
        )
        .unwrap();

        let fake = MockLlmProvider::single("[]");
        let executor = crate::executor::ActionExecutor::new(4747, 4750);
        let runner = ProposalRunner::new(&fake, &executor, "test");

        let (auto, consent) = runner.triage_proposals(&[low, high, critical]);
        // Notify is Low → auto-approved
        assert_eq!(auto.len(), 1);
        assert_eq!(auto[0].id.as_str(), "p1");
        // ProposeUpdate (High) and SecurityControl (Critical) need consent
        assert_eq!(consent.len(), 2);
    }
}
