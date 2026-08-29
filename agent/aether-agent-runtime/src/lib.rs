// Aether Agent Runtime - structured agent execution foundation
//
// The Agent Runtime bridges user requests to system services through
// structured, validated, capability-controlled actions. The LLM is a
// reasoning component — never the operating system itself.
//
// Architecture:
//   User Request → Agent Runtime → Context → LLM → Structured Plan
//   → Action Validator → Capability/Policy → IPC → System Service
//   → Result → Observation → Agent

pub mod session;
pub mod request;
pub mod intent;
pub mod action;
pub mod tool;
pub mod validator;
pub mod executor;
pub mod observation;
pub mod planner;
pub mod approval;
pub mod cancellation;
pub mod memory;
pub mod llm;
pub mod errors;
pub mod audit;
pub mod events;

pub use session::{AgentSession, SessionId, SessionState};
pub use request::{RequestActor, UserRequest};
pub use intent::{Confidence, Intent, IntentType};
pub use action::{Action, ActionId, ActionVariant};
pub use tool::{ToolDefinition, ToolId, ToolRegistry};
pub use validator::{ValidationResult, Validator};
pub use executor::{ActionExecutor, ExecutionResult};
pub use observation::{Observation, ObservationId, ObservationType};
pub use planner::{Plan, PlanId, PlanStep};
pub use approval::{ApprovalDecision, ApprovalRequest, ApprovalStatus};
pub use cancellation::CancellationToken;
pub use memory::{ConversationMemory, SessionMemory};
pub use llm::{LlmProvider, LlmRequest, LlmResponse};
pub use errors::AgentError;
