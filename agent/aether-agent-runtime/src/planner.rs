// Agent Runtime - Planner
//
// Converts intents into ordered plans. Each step references a structured
// action. Planning is deterministic where possible.

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub step_index: u32,
    pub action_name: String,
    pub parameters: serde_json::Value,
    pub depends_on: Vec<u32>,
    pub required_capabilities: Vec<String>,
    pub risk_level: String,
    pub optional: bool,
}

/// A plan consisting of ordered steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: PlanId,
    pub session_id: String,
    pub intent_summary: String,
    pub steps: Vec<PlanStep>,
    pub estimated_risk: String,
    pub requires_approval: bool,
    pub created_at: u64,
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
                    return Err(format!(
                        "Step {} depends on future step {}",
                        step.step_index, dep
                    ));
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
        });
        plan
    }

    /// Creates a multi-step plan.
    pub fn plan_multi(
        &self,
        session_id: &str,
        intent_summary: &str,
        steps: Vec<PlanStep>,
    ) -> Plan {
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
                },
                PlanStep {
                    step_index: 1,
                    action_name: "b".to_string(),
                    parameters: serde_json::json!({}),
                    depends_on: vec![],
                    required_capabilities: vec![],
                    risk_level: "low".to_string(),
                    optional: false,
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
                },
                PlanStep {
                    step_index: 1,
                    action_name: "b".to_string(),
                    parameters: serde_json::json!({}),
                    depends_on: vec![0],
                    required_capabilities: vec![],
                    risk_level: "low".to_string(),
                    optional: false,
                },
            ],
        );
        assert!(plan.validate().is_ok());
    }
}
