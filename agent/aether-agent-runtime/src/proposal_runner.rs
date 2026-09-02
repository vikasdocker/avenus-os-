// Phase 13.4 — Proposal executor pipeline.
//
// Chains: approved Proposal → AgentTask → Action → IPC execution → result recording.
// Handles approval gating for Medium+ risk proposals and records history.

use crate::approval::ApprovalRequest;
use crate::executor::{ActionExecutor, ExecutionResult};
use crate::llm::LlmProvider;
use crate::proposal_generator::ProposalGenerator;
use crate::task_to_action::{task_to_action, TaskToActionError};
use aether_agent_core::{AgentStatus, Proposal, TaskId, TaskStage};

/// Error type for the proposal runner pipeline.
#[derive(Debug, Clone)]
pub enum RunnerError {
    /// The proposal id was not found in the agent status.
    ProposalNotFound(String),
    /// The proposal was not in an approvable state.
    ProposalNotApprovable { id: String, stage: String },
    /// Converting proposal to task failed.
    ProposalToTaskFailed,
    /// Converting task to action failed.
    TaskToAction(TaskToActionError),
    /// The action executor returned an error.
    ExecutionFailed(String),
    /// The action requires user consent before proceeding.
    RequiresConsent(ApprovalRequest),
    /// The proposal was rejected during validation.
    ValidationFailed(Vec<aether_agent_core::ProposalError>),
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProposalNotFound(id) => write!(f, "proposal not found: {id}"),
            Self::ProposalNotApprovable { id, stage } => {
                write!(f, "proposal {id} not approvable in stage {stage}")
            }
            Self::ProposalToTaskFailed => write!(f, "proposal-to-task conversion failed"),
            Self::TaskToAction(e) => write!(f, "task-to-action: {e}"),
            Self::ExecutionFailed(e) => write!(f, "execution failed: {e}"),
            Self::RequiresConsent(req) => {
                write!(f, "requires consent: {}", req.reason)
            }
            Self::ValidationFailed(errs) => {
                write!(f, "validation failed: {} errors", errs.len())
            }
        }
    }
}

impl std::error::Error for RunnerError {}

/// Result of executing a single proposal through the full pipeline.
#[derive(Debug)]
pub struct RunResult {
    pub proposal_id: String,
    pub task_id: Option<String>,
    pub execution: Option<ExecutionResult>,
    pub error: Option<RunnerError>,
}

/// The proposal executor pipeline.
///
/// Orchestrates: generate proposals → approve → convert → execute → record.
pub struct ProposalRunner<'a> {
    generator: ProposalGenerator<'a>,
    executor: &'a ActionExecutor,
    session_id: String,
}

impl<'a> ProposalRunner<'a> {
    /// Create a new proposal runner.
    pub fn new(
        llm: &'a dyn LlmProvider,
        executor: &'a ActionExecutor,
        session_id: impl Into<String>,
    ) -> Self {
        Self { generator: ProposalGenerator::new(llm), executor, session_id: session_id.into() }
    }

    /// Step 1: Generate proposals from observations via LLM.
    pub fn generate_proposals(
        &self,
        observations: &[aether_agent_core::Observation],
        now_ms: u64,
    ) -> Result<(Vec<Proposal>, Vec<aether_agent_core::ProposalError>), String> {
        self.generator.generate(observations, now_ms)
    }

    /// Step 2: Auto-approve low-risk proposals, gate high-risk ones.
    ///
    /// Returns `(auto_approved, needs_consent)` — the caller must present
    /// `needs_consent` items to the user and call `approve_proposal` or
    /// `deny_proposal` for each.
    pub fn triage_proposals(
        &self,
        proposals: &[Proposal],
    ) -> (Vec<Proposal>, Vec<ApprovalRequest>) {
        let mut auto_approved = Vec::new();
        let mut needs_consent = Vec::new();

        for proposal in proposals {
            if proposal.risk.requires_consent() {
                let request = ApprovalRequest::new(
                    &self.session_id,
                    proposal.id.as_str(),
                    &proposal.title,
                    proposal.risk.as_str(),
                    &proposal.reasoning,
                );
                needs_consent.push(request);
            } else {
                auto_approved.push(proposal.clone());
            }
        }

        (auto_approved, needs_consent)
    }

    /// Step 3: Execute a single approved proposal through the full pipeline.
    pub fn execute_proposal(
        &self,
        status: &mut AgentStatus,
        proposal: &Proposal,
        now_ms: u64,
    ) -> RunResult {
        let proposal_id = proposal.id.as_str().to_string();

        // Convert proposal → task.
        let task_id =
            TaskId::new(format!("task-{now_ms}-{}", &proposal_id[..8.min(proposal_id.len())]))
                .unwrap();
        let task = match aether_agent_core::proposal::proposal_to_task(proposal, task_id.clone()) {
            Some(t) => t,
            None => {
                return RunResult {
                    proposal_id,
                    task_id: Some(task_id.as_str().to_string()),
                    execution: None,
                    error: Some(RunnerError::ProposalToTaskFailed),
                };
            }
        };

        // Insert task into graph.
        if status.insert_task(task).is_err() {
            return RunResult {
                proposal_id,
                task_id: Some(task_id.as_str().to_string()),
                execution: None,
                error: Some(RunnerError::ProposalToTaskFailed),
            };
        }

        // Look up the task in the graph for conversion.
        let task_obj = match status.task(&task_id) {
            Some(t) => t.clone(),
            None => {
                return RunResult {
                    proposal_id,
                    task_id: Some(task_id.as_str().to_string()),
                    execution: None,
                    error: Some(RunnerError::ProposalToTaskFailed),
                };
            }
        };

        // Convert task → action.
        let action = match task_to_action(&self.session_id, &task_obj) {
            Ok(a) => a,
            Err(e) => {
                let _ = status.complete_task(
                    &task_id,
                    TaskStage::Failed,
                    now_ms,
                    Some(format!("task_to_action failed: {e}")),
                );
                return RunResult {
                    proposal_id,
                    task_id: Some(task_id.as_str().to_string()),
                    execution: None,
                    error: Some(RunnerError::TaskToAction(e)),
                };
            }
        };

        // Execute.
        let result = match self.executor.execute_with_recovery(&action) {
            Ok(r) => r,
            Err(e) => {
                let _ = status.complete_task(
                    &task_id,
                    TaskStage::Failed,
                    now_ms,
                    Some(format!("execution failed: {e}")),
                );
                return RunResult {
                    proposal_id,
                    task_id: Some(task_id.as_str().to_string()),
                    execution: None,
                    error: Some(RunnerError::ExecutionFailed(format!("{e}"))),
                };
            }
        };

        // Record success.
        let stage = if result.success { TaskStage::Done } else { TaskStage::Failed };
        let note = Some(format!("action completed in {}ms", result.duration_ms));
        let _ = status.complete_task(&task_id, stage, now_ms, note);

        RunResult {
            proposal_id,
            task_id: Some(task_id.as_str().to_string()),
            execution: Some(result),
            error: None,
        }
    }

    /// Execute a batch of proposals (auto-approved ones).
    pub fn execute_batch(
        &self,
        status: &mut AgentStatus,
        proposals: &[Proposal],
        now_ms: u64,
    ) -> Vec<RunResult> {
        proposals.iter().map(|p| self.execute_proposal(status, p, now_ms)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_agent_core::{Observation, ObservationSeverity, ProposalRisk, TaskKind};

    fn make_obs(id: &str) -> Observation {
        Observation {
            id: id.to_string(),
            component: "test".to_string(),
            summary: format!("obs {id}"),
            detail: None,
            severity: ObservationSeverity::Warning,
            timestamp_ms: 1000,
            data: None,
        }
    }

    #[test]
    fn triage_separates_risk_levels() {
        let low = Proposal::new(
            "p1",
            TaskKind::RestartService,
            "Restart",
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

        let fake_llm = crate::llm::MockLlmProvider::single("[]");
        let executor = ActionExecutor::new(4747, 4750);
        let runner = ProposalRunner::new(&fake_llm, &executor, "test-session");

        let (auto, consent) = runner.triage_proposals(&[low, high]);
        assert_eq!(auto.len(), 1);
        assert_eq!(auto[0].id.as_str(), "p1");
        assert_eq!(consent.len(), 1);
        assert_eq!(consent[0].action_id.as_str(), "p2");
    }

    #[test]
    fn execute_proposal_converts_to_task_and_action() {
        let proposal = Proposal::new(
            "p-exec",
            TaskKind::Notify,
            "Notify",
            "notification description",
            "test reasoning",
            ProposalRisk::Low,
            2000,
        )
        .unwrap();

        let fake_llm = crate::llm::MockLlmProvider::single("[]");
        let executor = ActionExecutor::new(4747, 4750);
        let runner = ProposalRunner::new(&fake_llm, &executor, "test-session");

        let mut status = AgentStatus::new();
        let result = runner.execute_proposal(&mut status, &proposal, 3000);

        // Notify → ContextGet which is a no-op against a dead server,
        // but the pipeline should complete without panicking.
        assert_eq!(result.proposal_id, "p-exec");
        assert!(result.task_id.is_some());
    }

    #[test]
    fn runner_error_display() {
        let err = RunnerError::ProposalNotFound("abc".into());
        assert!(err.to_string().contains("abc"));

        let err = RunnerError::ProposalToTaskFailed;
        assert!(err.to_string().contains("proposal-to-task"));
    }
}
