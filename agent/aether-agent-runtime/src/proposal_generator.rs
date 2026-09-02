// Phase 13.3 — LLM-driven proposal generator.
//
// Takes a batch of `Observation`s, sends them to an LLM, parses the
// structured response into `Proposal` drafts, and validates them
// through `propose_from_observations`.

use crate::llm::{LlmProvider, LlmRequest};
use aether_agent_core::{Observation, Proposal, ProposalError, TaskKind};

/// Schema the LLM must conform to for proposal generation.
const PROPOSAL_SCHEMA: &str = r#"{
  "type": "array",
  "items": {
    "type": "object",
    "required": ["kind", "title", "description", "reasoning", "risk"],
    "properties": {
      "kind": { "type": "string", "enum": ["RestartService","Notify","ProposeUpdate","ProposeInstall","ProposeCleanup","ProposeSecurityScan","DeviceControl","DisplayControl","PowerControl","SecurityControl","Custom"] },
      "title": { "type": "string", "minLength": 1 },
      "description": { "type": "string", "minLength": 1 },
      "reasoning": { "type": "string", "minLength": 1 },
      "risk": { "type": "string", "enum": ["Low","Medium","High","Critical"] },
      "target": { "type": "string" },
      "arguments": { "type": "object" }
    }
  }
}"#;

/// System prompt for the proposal generator.
const SYSTEM_PROMPT: &str = r#"You are the Aether OS autonomous agent proposal generator.

You receive a batch of system observations (sensor readings, anomaly detections, status reports).
Your job is to propose concrete actions to address them.

Rules:
- Each proposal MUST reference at least one observation id from the input as evidence.
- Risk must match the TaskKind's default floor: ProposeUpdate, ProposeInstall, ProposeSecurityScan require High; Custom requires High; others default to Low.
- Title and description must be concise and actionable.
- Reasoning must explain why this action is warranted by the evidence.
- Target and arguments are optional; use them when the TaskKind requires a specific target (e.g., service name for RestartService).

Output a JSON array of proposal objects. Do not include any text outside the JSON array."#;

/// A raw proposal draft as returned by the LLM (before validation).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ProposalDraft {
    kind: String,
    title: String,
    description: String,
    reasoning: String,
    risk: String,
    target: Option<String>,
    arguments: Option<serde_json::Value>,
}

/// The LLM proposal generator.
pub struct ProposalGenerator<'a> {
    provider: &'a dyn LlmProvider,
}

impl<'a> ProposalGenerator<'a> {
    /// Create a new generator using the given LLM provider.
    pub fn new(provider: &'a dyn LlmProvider) -> Self {
        Self { provider }
    }

    /// Generate proposals from a batch of observations.
    ///
    /// Returns `(accepted, rejected)` where `accepted` are validated proposals
    /// and `rejected` are the validation errors for proposals that didn't pass.
    pub fn generate(
        &self,
        observations: &[Observation],
        now_ms: u64,
    ) -> Result<(Vec<Proposal>, Vec<ProposalError>), String> {
        if observations.is_empty() {
            return Ok((vec![], vec![]));
        }

        let prompt = self.build_prompt(observations);
        let request = LlmRequest {
            prompt,
            system_prompt: Some(SYSTEM_PROMPT.to_string()),
            max_tokens: Some(4096),
            temperature: Some(0.3),
            structured_output: Some(
                serde_json::from_str(PROPOSAL_SCHEMA).map_err(|e| format!("schema parse: {e}"))?,
            ),
        };

        let response = self.provider.structured_output(
            &request,
            &serde_json::from_str(PROPOSAL_SCHEMA).map_err(|e| format!("schema parse: {e}"))?,
        )?;

        let drafts: Vec<ProposalDraft> =
            serde_json::from_value(response).map_err(|e| format!("parse drafts: {e}"))?;

        let proposals = self.drafts_to_proposals(drafts, now_ms, observations)?;
        let (accepted, rejected) =
            aether_agent_core::propose_from_observations(&proposals, observations);

        Ok((accepted, rejected))
    }

    /// Build the user prompt from observations.
    fn build_prompt(&self, observations: &[Observation]) -> String {
        let obs_json =
            serde_json::to_string_pretty(observations).unwrap_or_else(|_| "[]".to_string());

        format!(
            "Current system observations:\n```json\n{obs_json}\n```\n\n\
             Propose actions to address these observations. Return a JSON array of proposals."
        )
    }

    /// Convert raw LLM drafts into typed Proposals with generated ids.
    /// Attaches all observation ids as evidence.
    fn drafts_to_proposals(
        &self,
        drafts: Vec<ProposalDraft>,
        now_ms: u64,
        observations: &[Observation],
    ) -> Result<Vec<Proposal>, String> {
        let obs_ids: Vec<String> = observations.iter().map(|o| o.id.clone()).collect();
        let mut proposals = Vec::with_capacity(drafts.len());

        for (i, draft) in drafts.into_iter().enumerate() {
            let kind = parse_task_kind(&draft.kind)
                .ok_or_else(|| format!("unknown kind: {}", draft.kind))?;
            let risk =
                parse_risk(&draft.risk).ok_or_else(|| format!("unknown risk: {}", draft.risk))?;

            let id = format!("prop-llm-{now_ms}-{i}");
            let mut proposal = Proposal::new(
                id,
                kind,
                draft.title,
                draft.description,
                draft.reasoning,
                risk,
                now_ms,
            )
            .ok_or_else(|| format!("invalid proposal at index {i}"))?;
            proposal.target = draft.target;
            proposal.arguments = draft.arguments;
            proposal.evidence = obs_ids.clone();
            proposals.push(proposal);
        }

        Ok(proposals)
    }
}

fn parse_task_kind(s: &str) -> Option<TaskKind> {
    match s {
        "RestartService" => Some(TaskKind::RestartService),
        "Notify" => Some(TaskKind::Notify),
        "ProposeUpdate" => Some(TaskKind::ProposeUpdate),
        "ProposeInstall" => Some(TaskKind::ProposeInstall),
        "ProposeCleanup" => Some(TaskKind::ProposeCleanup),
        "ProposeSecurityScan" => Some(TaskKind::ProposeSecurityScan),
        "DeviceControl" => Some(TaskKind::DeviceControl),
        "DisplayControl" => Some(TaskKind::DisplayControl),
        "PowerControl" => Some(TaskKind::PowerControl),
        "SecurityControl" => Some(TaskKind::SecurityControl),
        "Custom" => Some(TaskKind::Custom),
        _ => None,
    }
}

fn parse_risk(s: &str) -> Option<aether_agent_core::ProposalRisk> {
    match s {
        "Low" => Some(aether_agent_core::ProposalRisk::Low),
        "Medium" => Some(aether_agent_core::ProposalRisk::Medium),
        "High" => Some(aether_agent_core::ProposalRisk::High),
        "Critical" => Some(aether_agent_core::ProposalRisk::Critical),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_agent_core::{ObservationSeverity, ProposalRisk, TaskKind};

    /// A fake LLM provider that returns a fixed JSON array.
    struct FakeLlm {
        response: String,
    }

    impl LlmProvider for FakeLlm {
        fn name(&self) -> &str {
            "fake"
        }
        fn generate(&self, _request: &LlmRequest) -> Result<crate::llm::LlmResponse, String> {
            Ok(crate::llm::LlmResponse {
                content: self.response.clone(),
                model: "fake".to_string(),
                tokens_used: None,
                finish_reason: "stop".to_string(),
                parsed_output: None,
            })
        }
        fn structured_output(
            &self,
            _request: &LlmRequest,
            _schema: &serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            serde_json::from_str(&self.response).map_err(|e| format!("parse error: {e}"))
        }
    }

    fn make_observation(id: &str, component: &str, severity: ObservationSeverity) -> Observation {
        Observation {
            id: id.to_string(),
            component: component.to_string(),
            summary: format!("test observation {id}"),
            detail: None,
            severity,
            timestamp_ms: 1000,
            data: None,
        }
    }

    #[test]
    fn empty_observations_returns_empty() {
        let fake = FakeLlm { response: "[]".to_string() };
        let gen = ProposalGenerator::new(&fake);
        let (accepted, rejected) = gen.generate(&[], 1000).unwrap();
        assert!(accepted.is_empty());
        assert!(rejected.is_empty());
    }

    #[test]
    fn parses_valid_proposals() {
        let json = r#"[
            {
                "kind": "RestartService",
                "title": "Restart network service",
                "description": "Network service appears unresponsive",
                "reasoning": "Observation shows network unreachable",
                "risk": "Medium",
                "target": "network",
                "arguments": null
            }
        ]"#;
        let fake = FakeLlm { response: json.to_string() };
        let gen = ProposalGenerator::new(&fake);
        let obs = vec![make_observation("obs-net-1", "network", ObservationSeverity::Warning)];
        let (accepted, rejected) = gen.generate(&obs, 2000).unwrap();
        assert_eq!(accepted.len(), 1);
        assert!(rejected.is_empty());
        assert_eq!(accepted[0].kind, TaskKind::RestartService);
        assert_eq!(accepted[0].risk, ProposalRisk::Medium);
    }

    #[test]
    fn rejects_invalid_kind() {
        let json = r#"[
            {
                "kind": "BogusKind",
                "title": "Do something",
                "description": "...",
                "reasoning": "...",
                "risk": "Low"
            }
        ]"#;
        let fake = FakeLlm { response: json.to_string() };
        let gen = ProposalGenerator::new(&fake);
        let obs = vec![make_observation("obs-1", "test", ObservationSeverity::Info)];
        let result = gen.generate(&obs, 3000);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown kind"));
    }

    #[test]
    fn rejects_invalid_risk() {
        let json = r#"[
            {
                "kind": "RestartService",
                "title": "Restart",
                "description": "...",
                "reasoning": "...",
                "risk": "Banana"
            }
        ]"#;
        let fake = FakeLlm { response: json.to_string() };
        let gen = ProposalGenerator::new(&fake);
        let obs = vec![make_observation("obs-1", "test", ObservationSeverity::Info)];
        let result = gen.generate(&obs, 4000);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown risk"));
    }

    #[test]
    fn parses_multiple_proposals() {
        let json = r#"[
            {
                "kind": "RestartService",
                "title": "Restart A",
                "description": "...",
                "reasoning": "...",
                "risk": "Medium"
            },
            {
                "kind": "ProposeCleanup",
                "title": "Free disk",
                "description": "...",
                "reasoning": "...",
                "risk": "Medium"
            }
        ]"#;
        let fake = FakeLlm { response: json.to_string() };
        let gen = ProposalGenerator::new(&fake);
        let obs = vec![
            make_observation("obs-1", "service", ObservationSeverity::Warning),
            make_observation("obs-2", "storage", ObservationSeverity::Critical),
        ];
        let (accepted, _rejected) = gen.generate(&obs, 5000).unwrap();
        assert_eq!(accepted.len(), 2);
    }
}
