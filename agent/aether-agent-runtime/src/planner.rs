// Agent Runtime - Planner
//
// Converts intents into ordered plans. Each step references a structured
// action. Planning is deterministic where possible.

use crate::recovery::RecoveryPolicy;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique plan identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlanId(Uuid);

impl PlanId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PlanId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PlanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A single step in a plan.
///
/// `deny_unknown_fields` is the security boundary: a model-produced
/// step cannot smuggle `root`, `admin`, `allow`, `skip_policy`,
/// `trusted`, or any other privilege-escalation field past the
/// deserializer. The risk level is set by trusted planner code.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanStep {
    pub step_index: u32,
    pub action_name: String,
    pub parameters: serde_json::Value,
    pub depends_on: Vec<u32>,
    pub required_capabilities: Vec<String>,
    pub risk_level: String,
    pub optional: bool,
    /// Bounded recovery policy for this step. Defaults to no-retry.
    /// The executor and the daemon planner both consult this when
    /// an attempt fails.
    #[serde(default)]
    pub recovery: RecoveryPolicy,
}

impl PlanStep {
    /// Sets the recovery policy in a builder style.
    pub fn with_recovery(mut self, recovery: RecoveryPolicy) -> Self {
        self.recovery = recovery;
        self
    }
}

/// A plan consisting of ordered steps.
///
/// `deny_unknown_fields` is the security boundary: a model-produced
/// plan cannot smuggle in extra fields like `root: true`,
/// `skip_policy: true`, etc. Plan-level retry count is bounded by
/// `max_plan_retries`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub id: PlanId,
    pub session_id: String,
    pub intent_summary: String,
    pub steps: Vec<PlanStep>,
    pub estimated_risk: String,
    pub requires_approval: bool,
    pub created_at: u64,
    /// Maximum number of times the whole plan may be retried after
    /// a partial failure. Defaults to 0 (no plan-level retry; each
    /// step decides for itself).
    #[serde(default)]
    pub max_plan_retries: u32,
}

impl Plan {
    pub fn new(session_id: &str, intent_summary: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            id: PlanId::new(),
            session_id: session_id.to_string(),
            intent_summary: intent_summary.to_string(),
            steps: Vec::new(),
            estimated_risk: "low".to_string(),
            requires_approval: false,
            created_at: now,
            max_plan_retries: 0,
        }
    }

    pub fn add_step(&mut self, step: PlanStep) {
        self.steps.push(step);
        self.recalculate_risk();
    }

    fn recalculate_risk(&mut self) {
        let max_risk = self.steps.iter().map(|s| s.risk_level.as_str()).max();
        if let Some(risk) = max_risk {
            self.estimated_risk = risk.to_string();
            self.requires_approval = risk == "high" || risk == "critical";
        }
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Validates plan structure (no circular dependencies).
    pub fn validate(&self) -> Result<(), String> {
        for step in &self.steps {
            for dep in &step.depends_on {
                if *dep >= step.step_index {
                    return Err(format!("Step {} depends on future step {}", step.step_index, dep));
                }
            }
        }
        Ok(())
    }
}

/// Builds plans from intents.
pub struct Planner;

impl Planner {
    pub fn new() -> Self {
        Self
    }

    /// Creates a simple single-step plan.
    pub fn plan_single(
        &self,
        session_id: &str,
        action_name: &str,
        params: serde_json::Value,
        capabilities: Vec<String>,
        risk_level: &str,
    ) -> Plan {
        let mut plan = Plan::new(session_id, action_name);
        plan.add_step(PlanStep {
            step_index: 0,
            action_name: action_name.to_string(),
            parameters: params,
            depends_on: Vec::new(),
            required_capabilities: capabilities,
            risk_level: risk_level.to_string(),
            optional: false,
            recovery: RecoveryPolicy::default(),
        });
        plan
    }

    /// Creates a single-step plan with a custom recovery policy.
    pub fn plan_single_with_recovery(
        &self,
        session_id: &str,
        action_name: &str,
        params: serde_json::Value,
        capabilities: Vec<String>,
        risk_level: &str,
        recovery: RecoveryPolicy,
    ) -> Plan {
        let mut plan = Plan::new(session_id, action_name);
        plan.add_step(PlanStep {
            step_index: 0,
            action_name: action_name.to_string(),
            parameters: params,
            depends_on: Vec::new(),
            required_capabilities: capabilities,
            risk_level: risk_level.to_string(),
            optional: false,
            recovery,
        });
        plan
    }

    /// Creates a single-step plan from an `Action`, using the
    /// trusted `recovery_policy_for(&action.variant)` mapping as
    /// the recovery policy. This is the recommended entry point
    /// for new code: it pulls the policy from the trusted
    /// classification table rather than letting the LLM or the
    /// caller pick a retry budget.
    pub fn plan_for_action(&self, action: &crate::action::Action) -> Plan {
        use crate::action::{recovery_policy_for, ActionRisk};
        let risk = match action.risk_level {
            ActionRisk::Low => "low",
            ActionRisk::Medium => "medium",
            ActionRisk::High => "high",
            ActionRisk::Critical => "critical",
        };
        let recovery = recovery_policy_for(&action.variant);
        let mut plan = Plan::new(&action.session_id, action.action_name());
        plan.add_step(PlanStep {
            step_index: 0,
            action_name: action.action_name().to_string(),
            parameters: serde_json::to_value(&action.variant)
                .unwrap_or_else(|_| serde_json::json!({})),
            depends_on: Vec::new(),
            required_capabilities: action.requested_capabilities.clone(),
            risk_level: risk.to_string(),
            optional: false,
            recovery,
        });
        plan
    }

    /// Creates a multi-step plan.
    pub fn plan_multi(&self, session_id: &str, intent_summary: &str, steps: Vec<PlanStep>) -> Plan {
        let mut plan = Plan::new(session_id, intent_summary);
        for step in steps {
            plan.add_step(step);
        }
        plan
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_step_plan() {
        let p = Planner::new();
        let plan = p.plan_single(
            "s1",
            "application.launch",
            serde_json::json!({"app": "calc"}),
            vec!["application.launch".to_string()],
            "medium",
        );
        assert_eq!(plan.step_count(), 1);
        assert_eq!(plan.estimated_risk, "medium");
        assert!(!plan.requires_approval);
    }

    #[test]
    fn high_risk_plan_requires_approval() {
        let p = Planner::new();
        let plan = p.plan_single(
            "s1",
            "file.delete",
            serde_json::json!({"path": "/tmp/x"}),
            vec!["file.delete".to_string()],
            "high",
        );
        assert!(plan.requires_approval);
    }

    #[test]
    fn plan_validate_catches_forward_dependency() {
        let p = Planner::new();
        let plan = p.plan_multi(
            "s1",
            "test",
            vec![
                PlanStep {
                    step_index: 0,
                    action_name: "a".to_string(),
                    parameters: serde_json::json!({}),
                    depends_on: vec![1], // depends on future step
                    required_capabilities: vec![],
                    risk_level: "low".to_string(),
                    optional: false,
                    recovery: RecoveryPolicy::default(),
                },
                PlanStep {
                    step_index: 1,
                    action_name: "b".to_string(),
                    parameters: serde_json::json!({}),
                    depends_on: vec![],
                    required_capabilities: vec![],
                    risk_level: "low".to_string(),
                    optional: false,
                    recovery: RecoveryPolicy::default(),
                },
            ],
        );
        assert!(plan.validate().is_err());
    }

    #[test]
    fn plan_validate_passes_valid() {
        let p = Planner::new();
        let plan = p.plan_multi(
            "s1",
            "test",
            vec![
                PlanStep {
                    step_index: 0,
                    action_name: "a".to_string(),
                    parameters: serde_json::json!({}),
                    depends_on: vec![],
                    required_capabilities: vec![],
                    risk_level: "low".to_string(),
                    optional: false,
                    recovery: RecoveryPolicy::default(),
                },
                PlanStep {
                    step_index: 1,
                    action_name: "b".to_string(),
                    parameters: serde_json::json!({}),
                    depends_on: vec![0],
                    required_capabilities: vec![],
                    risk_level: "low".to_string(),
                    optional: false,
                    recovery: RecoveryPolicy::default(),
                },
            ],
        );
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn plan_step_recovery_defaults_to_no_retry() {
        let p = Planner::new();
        let plan = p.plan_single("s1", "system.status", serde_json::json!({}), vec![], "low");
        assert_eq!(plan.steps[0].recovery.max_retries, 0);
    }

    #[test]
    fn plan_step_with_recovery_attaches_policy() {
        let p = Planner::new();
        let plan = p.plan_single_with_recovery(
            "s1",
            "network.status",
            serde_json::json!({}),
            vec![],
            "low",
            RecoveryPolicy::transient_default(),
        );
        assert_eq!(plan.steps[0].recovery.max_retries, 3);
        assert_eq!(plan.steps[0].recovery.timeout_ms, Some(2_000));
    }

    #[test]
    fn plan_step_serialization_round_trip() {
        let p = Planner::new();
        let plan = p.plan_single_with_recovery(
            "s1",
            "network.status",
            serde_json::json!({}),
            vec![],
            "low",
            RecoveryPolicy::transient_default(),
        );
        let json = serde_json::to_string(&plan).unwrap_or_default();
        let back: Plan = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back.steps[0].recovery.max_retries, 3);
        assert_eq!(back.max_plan_retries, 0);
    }

    #[test]
    fn legacy_plan_step_without_recovery_field_deserializes() {
        // Older serialized plans (or hand-written test fixtures) may
        // not include the `recovery` field. The default must apply.
        let legacy = serde_json::json!({
            "step_index": 0,
            "action_name": "a",
            "parameters": {},
            "depends_on": [],
            "required_capabilities": [],
            "risk_level": "low",
            "optional": false,
        });
        let step: PlanStep = serde_json::from_value(legacy).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(step.recovery.max_retries, 0);
    }
}
