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

pub mod action;
pub mod approval;
pub mod audit;
pub mod cancellation;
pub mod errors;
pub mod events;
pub mod executor;
pub mod host;
pub mod intent;
pub mod llm;
pub mod llm_provider;
pub mod memory;
pub mod memory_store;
pub mod observation;
pub mod planner;
pub mod proposal_generator;
pub mod proposal_runner;
pub mod recovery;
pub mod request;
pub mod session;
pub mod structured_intent;
pub mod task_to_action;
pub mod tool;
pub mod tools;
pub mod validator;

pub use action::{recovery_policy_for, Action, ActionId, ActionVariant};
pub use approval::{ApprovalDecision, ApprovalRequest, ApprovalStatus};
pub use cancellation::CancellationToken;
pub use errors::AgentError;
pub use executor::{ActionExecutor, ExecutionResult};
pub use host::{
    AgentRuntimeHost, EventPublisher, HostId, HostState, HostStatus, InMemoryEventBus,
    RequestOutcome,
};
pub use intent::{Confidence, Intent, IntentType};
pub use llm::{LlmProvider, LlmRequest, LlmResponse};
pub use memory::{ConversationMemory, SessionMemory};
pub use memory_store::{
    decode_persisted, encode_persisted, FileMemoryStore, InMemoryStore, MemoryStore,
    MemoryStoreError, Persisted,
};
pub use observation::{Observation, ObservationId, ObservationType};
pub use planner::{Plan, PlanId, PlanStep};
pub use recovery::{backoff_delay, decide_recovery, FailureKind, RecoveryAction, RecoveryPolicy};
pub use request::{RequestActor, UserRequest};
pub use session::{ActorType as SessionActorType, SessionActor};
pub use session::{AgentSession, SessionId, SessionState};
pub use structured_intent::{
    build_intent_prompt, parse_envelope, parse_intent, IntentEnvelope, StructuredIntentError,
    INTENT_SCHEMA,
};
pub use task_to_action::{task_to_action, TaskToActionError};
pub use tool::{ToolDefinition, ToolId, ToolRegistry};
pub use tools::{register_all_tools, routing_for};
pub use validator::{ValidationResult, Validator};
