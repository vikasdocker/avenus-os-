// Proposal: what the agent wants to do.
//
// A `Proposal` is the agent's recommendation to the
// user. It is always paired with one or more
// `Observation`s (the evidence the user can review
// to decide whether to approve the proposal).
//
// The IPC layer uses the proposal's `risk` to decide
// whether user consent is required before the
// proposal can be turned into an `AgentTask` and
// executed. Low-risk proposals (e.g. "let me know
// the network is up") can proceed without explicit
// consent; high-risk proposals (e.g. "delete these
// files", "install this update") must collect
// explicit user consent via the shell.

use serde::{Deserialize, Serialize};

use aether_core::RiskLevel;


use crate::observation::Observation;
use crate::task::{AgentTask, TaskId, TaskKind, TaskRisk};

/// The risk a proposal exposes. Maps to the
/// existing `aether_core::types::RiskLevel` so the
/// IPC layer can reuse the Phase 11.3 policy gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProposalRisk {
    /// The proposal is informational. The agent
    /// can act on it without user consent
    /// (e.g. surface a notification).
    Low,
    /// The proposal is mildly consequential. The
    /// agent may act on it but should log the
    /// action.
    Medium,
    /// The proposal is consequential. Requires
    /// explicit user consent.
    High,
    /// The proposal is dangerous. Requires
    /// explicit user consent AND a second
    /// confirmation (e.g. "are you really sure
    /// you want to delete this?").
    Critical,
}

impl ProposalRisk {
    /// Returns the canonical kebab-case name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    /// Maps to the corresponding `aether_core::types::RiskLevel`.
    #[must_use]
    pub fn to_risk_level(&self) -> RiskLevel {
        match self {
            Self::Low => RiskLevel::Low,
            Self::Medium => RiskLevel::Medium,
            Self::High => RiskLevel::High,
            Self::Critical => RiskLevel::Critical,
        }
    }

    /// Returns `true` if the proposal requires
    /// explicit user consent.
    #[must_use]
    pub fn requires_consent(&self) -> bool {
        !matches!(self, Self::Low)
    }
}

impl std::fmt::Display for ProposalRisk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A unique identifier for a proposal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProposalId(String);

impl ProposalId {
    /// Creates a new `ProposalId` from a non-empty
    /// string.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let s: String = value.into();
        if s.is_empty() {
            return None;
        }
        Some(Self(s))
    }

    /// Returns the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProposalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A proposal: what the agent wants to do, why,
/// and what evidence supports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub id: ProposalId,
    /// The kind of action the agent is proposing.
    pub kind: TaskKind,
    /// A short summary ("delete cached files",
    /// "install update 1.2.0").
    pub title: String,
    /// A longer description of the proposal.
    pub description: String,
    /// The risk the proposal exposes.
    pub risk: ProposalRisk,
    /// The reasoning the agent offers for the
    /// proposal. The future model produces this
    /// alongside the action; the shell stores it
    /// for the user to read.
    pub reasoning: String,
    /// The observations that support the
    /// proposal. The shell records the full
    /// observation ids so the user can drill
    /// down into the evidence; the IPC layer
    /// returns the full observation objects
    /// on demand.
    pub evidence: Vec<String>,
    /// Wall-clock timestamp when the proposal
    /// was created.
    pub timestamp_ms: u64,
    /// Optional structured target id (service
    /// name, app id, etc).
    pub target: Option<String>,
    /// Optional structured arguments for the
    /// future executor.
    pub arguments: Option<serde_json::Value>,
}

impl Proposal {
    /// Creates a new proposal. `id`, `title`,
    /// `description`, and `reasoning` must be
    /// non-empty.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        kind: TaskKind,
        title: impl Into<String>,
        description: impl Into<String>,
        reasoning: impl Into<String>,
        risk: ProposalRisk,
        timestamp_ms: u64,
    ) -> Option<Self> {
        let id = ProposalId::new(id)?;
        let title: String = title.into();
        let description: String = description.into();
        let reasoning: String = reasoning.into();
        if title.is_empty() || description.is_empty() || reasoning.is_empty() {
            return None;
        }
        Some(Self {
            id,
            kind,
            title,
            description,
            risk,
            reasoning,
            evidence: Vec::new(),
            timestamp_ms,
            target: None,
            arguments: None,
        })
    }

    /// Attaches the ids of supporting observations.
    #[must_use]
    pub fn with_evidence(mut self, ids: impl IntoIterator<Item = String>) -> Self {
        self.evidence.extend(ids);
        self
    }

    /// Attaches a target id.
    #[must_use]
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Attaches structured arguments.
    #[must_use]
    pub fn with_arguments(mut self, args: serde_json::Value) -> Self {
        self.arguments = Some(args);
        self
    }

    /// Returns `true` if the proposal requires
    /// explicit user consent.
    #[must_use]
    pub fn requires_consent(&self) -> bool {
        self.risk.requires_consent()
    }
}

/// Reasons a `Proposal` is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalError {
    /// The proposal's id is empty.
    EmptyId,
    /// The proposal is missing a title,
    /// description, or reasoning.
    IncompleteDescription,
    /// The proposal's evidence references an
    /// observation id that does not exist in the
    /// supplied observation list.
    UnknownEvidence { proposal: String, missing: String },
    /// The proposal's risk is `Low` but its
    /// kind is one that always requires consent
    /// (ProposeUpdate / ProposeInstall / Custom).
    RiskTooLowForKind { kind: TaskKind, risk: ProposalRisk },
}

impl std::fmt::Display for ProposalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyId => f.write_str("proposal id is empty"),
            Self::IncompleteDescription => {
                f.write_str("proposal is missing title, description, or reasoning")
            }
            Self::UnknownEvidence { proposal, missing } => write!(
                f,
                "proposal '{proposal}' references unknown observation '{missing}'"
            ),
            Self::RiskTooLowForKind { kind, risk } => write!(
                f,
                "proposal kind '{}' requires consent, but risk is '{}'",
                kind.as_str(),
                risk.as_str()
            ),
        }
    }
}

impl std::error::Error for ProposalError {}

/// Validates a proposal. Returns the proposal on
/// success, or a `ProposalError` describing the
/// first issue. The validation enforces:
///
///   * non-empty id, title, description, reasoning
///   * every evidence id refers to an observation
///     in `observations`
///   * the proposal's `risk` is at least
///     `kind.default_risk()` (so e.g. a
///     `ProposeUpdate` cannot be classified as
///     `Low`)
pub fn validate_proposal(
    proposal: &Proposal,
    observations: &[Observation],
) -> Result<(), ProposalError> {
    if proposal.id.as_str().is_empty() {
        return Err(ProposalError::EmptyId);
    }
    if proposal.title.is_empty()
        || proposal.description.is_empty()
        || proposal.reasoning.is_empty()
    {
        return Err(ProposalError::IncompleteDescription);
    }
    for ev in &proposal.evidence {
        if !observations.iter().any(|o| &o.id == ev) {
            return Err(ProposalError::UnknownEvidence {
                proposal: proposal.id.to_string(),
                missing: ev.clone(),
            });
        }
    }
    let min_risk = match proposal.kind {
        TaskKind::Notify => ProposalRisk::Low,
        TaskKind::RestartService | TaskKind::ProposeCleanup | TaskKind::ProposeSecurityScan => {
            ProposalRisk::Medium
        }
        TaskKind::ProposeUpdate | TaskKind::ProposeInstall | TaskKind::Custom => {
            ProposalRisk::High
        }
    };
    if proposal.risk < min_risk {
        return Err(ProposalError::RiskTooLowForKind {
            kind: proposal.kind,
            risk: proposal.risk,
        });
    }
    Ok(())
}

/// The bridge from observations to proposals. The
/// caller is the future agent runtime (or a test);
/// it supplies a list of observations and a list of
/// proposal drafts, and this function validates the
/// whole batch. Returns the validated proposals
/// (those that pass) in the same order.
///
/// This is a static contract: it does not call the
/// model. The future runtime is the only thing
/// allowed to produce `Proposal`s; the shell only
/// validates them.
pub fn propose_from_observations(
    drafts: &[Proposal],
    observations: &[Observation],
) -> (Vec<Proposal>, Vec<ProposalError>) {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for draft in drafts {
        match validate_proposal(draft, observations) {
            Ok(()) => accepted.push(draft.clone()),
            Err(e) => rejected.push(e),
        }
    }
    (accepted, rejected)
}

/// Turns a proposal into a task graph. The caller
/// is the future agent runtime; this helper lives
/// here so the test surface and the IPC layer both
/// use the same conversion.
///
/// `task_id` is supplied by the caller (the future
/// runtime generates UUIDv7-style ids). The
/// returned `AgentTask` carries the proposal's
/// risk, target, and arguments.
#[must_use]
pub fn proposal_to_task(proposal: &Proposal, task_id: TaskId) -> Option<AgentTask> {
    let id_str = task_id.as_str().to_string();
    let mut t = AgentTask::new(
        id_str,
        proposal.kind,
        proposal.title.clone(),
        proposal.description.clone(),
    )?;
    t.risk = TaskRisk::from(proposal.risk.to_risk_level());
    if let Some(target) = &proposal.target {
        t = t.with_target(target.clone());
    }
    if let Some(args) = &proposal.arguments {
        t = t.with_arguments(args.clone());
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::ObservationSeverity;

    fn observation(id: &str, severity: ObservationSeverity) -> Observation {
        Observation::new(id, "storage", "summary", severity, 1).expect("valid")
    }

    fn proposal(kind: TaskKind, risk: ProposalRisk) -> Proposal {
        Proposal::new("p1", kind, "title", "description", "reasoning", risk, 1)
            .expect("valid")
    }

    #[test]
    fn proposal_new_rejects_empty_fields() {
        assert!(Proposal::new("p1", TaskKind::Notify, "", "d", "r", ProposalRisk::Low, 1).is_none());
        assert!(Proposal::new("p1", TaskKind::Notify, "t", "", "r", ProposalRisk::Low, 1).is_none());
        assert!(Proposal::new("p1", TaskKind::Notify, "t", "d", "", ProposalRisk::Low, 1).is_none());
        assert!(Proposal::new("", TaskKind::Notify, "t", "d", "r", ProposalRisk::Low, 1).is_none());
    }

    #[test]
    fn proposal_requires_consent_for_medium_and_above() {
        assert!(!ProposalRisk::Low.requires_consent());
        assert!(ProposalRisk::Medium.requires_consent());
        assert!(ProposalRisk::High.requires_consent());
        assert!(ProposalRisk::Critical.requires_consent());
    }

    #[test]
    fn risk_maps_to_core_risk_level() {
        assert_eq!(ProposalRisk::Low.to_risk_level(), RiskLevel::Low);
        assert_eq!(ProposalRisk::Medium.to_risk_level(), RiskLevel::Medium);
        assert_eq!(ProposalRisk::High.to_risk_level(), RiskLevel::High);
        assert_eq!(ProposalRisk::Critical.to_risk_level(), RiskLevel::Critical);
    }

    #[test]
    fn validate_proposal_accepts_well_formed_proposal() {
        let p = proposal(TaskKind::ProposeUpdate, ProposalRisk::High)
            .with_evidence(vec!["o1".to_string()]);
        let obs = vec![observation("o1", ObservationSeverity::Warning)];
        assert!(validate_proposal(&p, &obs).is_ok());
    }

    #[test]
    fn validate_proposal_rejects_unknown_evidence() {
        let p = proposal(TaskKind::ProposeUpdate, ProposalRisk::High)
            .with_evidence(vec!["o1".to_string()]);
        let obs = vec![];
        let err = validate_proposal(&p, &obs).unwrap_err();
        assert!(matches!(err, ProposalError::UnknownEvidence { .. }));
    }

    #[test]
    fn validate_proposal_rejects_low_risk_for_propose_update() {
        let p = proposal(TaskKind::ProposeUpdate, ProposalRisk::Low);
        let err = validate_proposal(&p, &[]).unwrap_err();
        assert!(matches!(err, ProposalError::RiskTooLowForKind { .. }));
    }

    #[test]
    fn validate_proposal_rejects_low_risk_for_propose_install() {
        let p = proposal(TaskKind::ProposeInstall, ProposalRisk::Low);
        let err = validate_proposal(&p, &[]).unwrap_err();
        assert!(matches!(err, ProposalError::RiskTooLowForKind { .. }));
    }

    #[test]
    fn validate_proposal_rejects_low_risk_for_custom() {
        let p = proposal(TaskKind::Custom, ProposalRisk::Low);
        let err = validate_proposal(&p, &[]).unwrap_err();
        assert!(matches!(err, ProposalError::RiskTooLowForKind { .. }));
    }

    #[test]
    fn validate_proposal_rejects_medium_for_propose_update() {
        // Medium is below High, the minimum for
        // ProposeUpdate.
        let p = proposal(TaskKind::ProposeUpdate, ProposalRisk::Medium);
        let err = validate_proposal(&p, &[]).unwrap_err();
        assert!(matches!(err, ProposalError::RiskTooLowForKind { .. }));
    }

    #[test]
    fn validate_proposal_accepts_low_for_notify() {
        let p = proposal(TaskKind::Notify, ProposalRisk::Low);
        assert!(validate_proposal(&p, &[]).is_ok());
    }

    #[test]
    fn validate_proposal_accepts_medium_for_restart_service() {
        let p = proposal(TaskKind::RestartService, ProposalRisk::Medium);
        assert!(validate_proposal(&p, &[]).is_ok());
    }

    #[test]
    fn propose_from_observations_partitions_results() {
        let p1 = proposal(TaskKind::Notify, ProposalRisk::Low);
        let p2 = proposal(TaskKind::ProposeUpdate, ProposalRisk::Low); // invalid
        let p3 = proposal(TaskKind::ProposeUpdate, ProposalRisk::High)
            .with_evidence(vec!["missing".to_string()]); // invalid
        let (ok, errs) = propose_from_observations(&[p1.clone(), p2, p3], &[]);
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].id, p1.id);
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn proposal_to_task_carries_risk_target_and_args() {
        let p = proposal(TaskKind::ProposeCleanup, ProposalRisk::Medium)
            .with_target("aether-agentd")
            .with_arguments(serde_json::json!({"max_age_ms": 60000}));
        let t = proposal_to_task(&p, TaskId::new("t1").unwrap()).expect("task");
        assert_eq!(t.risk, TaskRisk::Medium);
        assert_eq!(t.target.as_deref(), Some("aether-agentd"));
        assert_eq!(t.arguments, Some(serde_json::json!({"max_age_ms": 60000})));
    }

    #[test]
    fn risk_as_str_is_stable() {
        assert_eq!(ProposalRisk::Low.as_str(), "low");
        assert_eq!(ProposalRisk::Medium.as_str(), "medium");
        assert_eq!(ProposalRisk::High.as_str(), "high");
        assert_eq!(ProposalRisk::Critical.as_str(), "critical");
    }
}
