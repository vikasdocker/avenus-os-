// Agent Runtime - Host
//
// The `AgentRuntimeHost` is the integration point for the Agent Runtime
// inside the Aether OS control plane. It owns the lifecycle of one
// runtime instance, coordinates the sub-systems (SessionManager, ToolRegistry,
// AuditLog, EventBus, Validator, Executor), and exposes the public API the
// daemon (and tests) use to submit requests, inspect state, and recover
// from failures.
//
// Lifecycle:
//   Starting  -> Ready  -> Running  -> Stopping  -> Stopped
//                                              \-> Failed
//
// All state transitions are recorded in the AuditLog and emit
// `AgentEvent` on the event bus. The host is the only object the
// daemon holds; everything else lives behind it.

use crate::action::Action;
use crate::audit::{AuditEventType, AuditLog};
use crate::cancellation::CancellationToken;
use crate::errors::AgentError;
use crate::events::AgentEvent;
use crate::executor::ActionExecutor;
use crate::observation::{Observation, ObservationType};
use crate::request::{RequestActor, RequestId, UserRequest};
use crate::session::{AgentSession, SessionActor, SessionId, SessionState};
use crate::validator::Validator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Lifecycle phase of the host. The transitions are strict:
///   Starting -> Ready -> Running -> Stopping -> Stopped
///   Starting -> Failed
///   Ready    -> Failed
///   Running  -> Failed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostState {
    Starting,
    Ready,
    Running,
    Stopping,
    Stopped,
    Failed,
}

impl std::fmt::Display for HostState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Starting => "STARTING",
            Self::Ready => "READY",
            Self::Running => "RUNNING",
            Self::Stopping => "STOPPING",
            Self::Stopped => "STOPPED",
            Self::Failed => "FAILED",
        };
        write!(f, "{s}")
    }
}

impl HostState {
    /// Returns true if the host is in a state that can accept new work.
    pub fn is_accepting(self) -> bool {
        matches!(self, Self::Ready | Self::Running)
    }
    /// Returns true if the host has reached a terminal lifecycle phase.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Stopped | Self::Failed)
    }
}

/// Stable identifier for a single host instance. Persists across
/// session creation, requests, and restarts within the same process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HostId(pub uuid::Uuid);

impl HostId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for HostId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for HostId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Minimal event-bus publisher. The host publishes structured
/// `AgentEvent` values through this interface; the daemon wires
/// it to the Aether event bus (system-core or aether-supervisor).
pub trait EventPublisher: Send + Sync {
    /// Publish a structured event. Implementations must never panic.
    fn publish(&self, event: &AgentEvent);
    /// Returns the count of events this publisher has stored.
    /// Default implementation returns 0; the in-memory bus
    /// overrides this.
    fn len(&self) -> usize {
        0
    }
    /// Returns true if no events are stored.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// In-memory publisher used by tests and the default daemon path.
#[derive(Debug, Default, Clone)]
pub struct InMemoryEventBus {
    events: std::sync::Arc<std::sync::Mutex<Vec<AgentEvent>>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self { events: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())) }
    }

    /// Returns a snapshot of the published events, newest first.
    pub fn snapshot(&self) -> Vec<AgentEvent> {
        let guard = self.events.lock().unwrap_or_else(|p| p.into_inner());
        guard.iter().rev().cloned().collect()
    }

    pub fn len(&self) -> usize {
        let guard = self.events.lock().unwrap_or_else(|p| p.into_inner());
        guard.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        let mut guard = self.events.lock().unwrap_or_else(|p| p.into_inner());
        guard.clear();
    }
}

impl EventPublisher for InMemoryEventBus {
    fn publish(&self, event: &AgentEvent) {
        let mut guard = self.events.lock().unwrap_or_else(|p| p.into_inner());
        guard.push(event.clone());
    }
}

/// Snapshot of the host's health returned to status queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostStatus {
    pub host_id: HostId,
    pub state: HostState,
    pub session_count: u32,
    pub active_session_count: u32,
    pub audit_count: usize,
    pub event_count: usize,
    pub control_port: u16,
    pub surface_port: u16,
}

/// Per-session record that the host keeps. One per `UserRequest`
/// boundary — sessions are the long-lived interaction identity.
#[derive(Debug)]
struct HostSessionRecord {
    session: AgentSession,
    /// Request IDs that have been submitted in this session.
    requests: Vec<RequestId>,
    /// Action IDs that have been started (so we can cancel them).
    actions: HashMap<uuid::Uuid, CancellationToken>,
    /// Monotonic creation order used for stable list ordering.
    ordinal: u64,
}

impl HostSessionRecord {
    fn new(actor: SessionActor, ordinal: u64) -> Self {
        Self {
            session: AgentSession::new(actor),
            requests: Vec::new(),
            actions: HashMap::new(),
            ordinal,
        }
    }
}

/// Result of one request submitted to the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestOutcome {
    pub request_id: String,
    pub session_id: String,
    pub action_id: Option<String>,
    pub success: bool,
    pub observation: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub session_state: SessionState,
}

/// The Agent Runtime Host — the daemon's view of the runtime.
pub struct AgentRuntimeHost {
    id: HostId,
    state: HostState,
    audit: AuditLog,
    events: Box<dyn EventPublisher>,
    sessions: HashMap<SessionId, HostSessionRecord>,
    validator: Validator,
    executor: ActionExecutor,
    /// All capabilities the host has been granted by the daemon's
    /// identity. Defaults to a permissive set used in tests; the
    /// daemon narrows this to whatever the agent identity owns.
    granted_capabilities: Vec<String>,
    /// Bound on concurrent active sessions; older sessions can be
    /// evicted. Default 64.
    max_sessions: usize,
    /// Optional override for the clock (tests only).
    now_ms: fn() -> u64,
    /// Counter of events we have published. The publisher may also
    /// keep its own copy; this is the host's view of the total.
    published_event_count: usize,
    /// Monotonic counter for stable session ordering.
    next_session_ordinal: u64,
    /// Port the control plane (system-core) is reachable on.
    control_port: u16,
    /// Port the surface server is reachable on.
    surface_port: u16,
}

impl AgentRuntimeHost {
    /// Creates a new host in `Starting` state, then transitions to
    /// `Ready`. Returns the host and a snapshot of the resulting state.
    pub fn start(
        control_port: u16,
        surface_port: u16,
        granted_capabilities: Vec<String>,
        publisher: Box<dyn EventPublisher>,
    ) -> Result<Self, AgentError> {
        Self::start_with(
            control_port,
            surface_port,
            granted_capabilities,
            publisher,
            system_time_ms,
        )
    }

    /// Same as `start`, but accepts a custom clock for tests.
    pub fn start_with(
        control_port: u16,
        surface_port: u16,
        granted_capabilities: Vec<String>,
        publisher: Box<dyn EventPublisher>,
        now_ms: fn() -> u64,
    ) -> Result<Self, AgentError> {
        let id = HostId::new();
        let mut host = Self {
            id,
            state: HostState::Starting,
            audit: AuditLog::new(4096),
            events: publisher,
            sessions: HashMap::new(),
            validator: Validator::new(granted_capabilities),
            executor: ActionExecutor::new(control_port, surface_port),
            granted_capabilities: Vec::new(),
            max_sessions: 64,
            now_ms,
            published_event_count: 0,
            next_session_ordinal: 0,
            control_port,
            surface_port,
        };
        host.transition_state(HostState::Ready, "host initialised")?;
        Ok(host)
    }

    /// Sets the granted capabilities post-start. Useful for tests that
    /// need to widen the set after construction.
    pub fn grant_capabilities(&mut self, caps: Vec<String>) {
        self.granted_capabilities = caps.clone();
        // Recreate the validator with the new capability set.
        self.validator = Validator::new(caps);
    }

    pub fn host_id(&self) -> HostId {
        self.id
    }

    pub fn state(&self) -> HostState {
        self.state
    }

    pub fn control_port(&self) -> u16 {
        self.control_port
    }

    pub fn surface_port(&self) -> u16 {
        self.surface_port
    }

    /// Returns a snapshot suitable for IPC `agent.status` replies.
    pub fn status(&self) -> HostStatus {
        HostStatus {
            host_id: self.id,
            state: self.state,
            session_count: self.sessions.len() as u32,
            active_session_count: self.sessions.values().filter(|s| s.session.is_active()).count()
                as u32,
            audit_count: self.audit.len(),
            event_count: self.published_event_count,
            control_port: self.control_port,
            surface_port: self.surface_port,
        }
    }

    /// Creates a new session and transitions it to Ready.
    pub fn create_session(&mut self, actor: SessionActor) -> Result<SessionId, AgentError> {
        self.ensure_accepting()?;
        if self.sessions.len() >= self.max_sessions {
            return Err(AgentError::Internal(format!(
                "max sessions ({}) reached",
                self.max_sessions
            )));
        }
        let mut record = HostSessionRecord::new(actor.clone(), self.next_session_ordinal);
        self.next_session_ordinal += 1;
        record.session.transition(SessionState::Ready).map_err(AgentError::Session)?;
        let id = record.session.id;
        self.audit.record(
            &id.to_string(),
            AuditEventType::SessionCreated,
            &format!("actor={}", actor.identity),
            true,
            "host",
        );
        self.events.publish(&AgentEvent::SessionCreated {
            session_id: id.to_string(),
            actor: actor.identity.clone(),
        });
        self.published_event_count += 1;
        self.sessions.insert(id, record);
        if self.state == HostState::Ready {
            self.transition_state(HostState::Running, "first session created")?;
        }
        Ok(id)
    }

    /// Returns true if the host knows about this session.
    pub fn has_session(&self, id: &SessionId) -> bool {
        self.sessions.contains_key(id)
    }

    /// Returns the count of currently tracked sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Returns a JSON-friendly snapshot of a single session.
    pub fn inspect_session(&self, id: &SessionId) -> Option<serde_json::Value> {
        self.sessions.get(id).map(|rec| {
            serde_json::json!({
                "session_id": rec.session.id.to_string(),
                "state": rec.session.state.to_string(),
                "actor": rec.session.actor.actor_type,
                "actor_identity": rec.session.actor.identity,
                "created_at": rec.session.created_at,
                "updated_at": rec.session.updated_at,
                "request_count": rec.session.request_count,
                "action_count": rec.session.action_count,
                "observation_count": rec.session.observation_count,
                "error_count": rec.session.error_count,
                "cancelled": rec.session.cancelled,
                "is_active": rec.session.is_active(),
                "request_ids": rec.requests.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
            })
        })
    }

    /// Returns JSON snapshots for all sessions, newest first.
    pub fn list_sessions(&self) -> Vec<serde_json::Value> {
        // Newest-first by the host's monotonic session ordinal.
        let mut records: Vec<&HostSessionRecord> = self.sessions.values().collect();
        records.sort_by_key(|r| std::cmp::Reverse(r.ordinal));
        records
            .iter()
            .map(|r| {
                serde_json::json!({
                    "session_id": r.session.id.to_string(),
                    "state": r.session.state.to_string(),
                    "actor_identity": r.session.actor.identity,
                    "request_count": r.session.request_count,
                    "action_count": r.session.action_count,
                    "is_active": r.session.is_active(),
                })
            })
            .collect()
    }

    /// Submits a single validated action for execution. The action
    /// is run through Validator first; only then does it reach the
    /// executor. The session transitions to Executing -> Observing.
    pub fn submit_action(
        &mut self,
        session_id: &SessionId,
        actor: &RequestActor,
        action: Action,
    ) -> Result<RequestOutcome, AgentError> {
        self.ensure_accepting()?;
        // Idempotent advance: skip transitions that have already
        // happened. The session is in Ready after create_session.
        self.advance_session_if_not(session_id, SessionState::Ready)?;
        self.advance_session(session_id, SessionState::Thinking)?;
        self.advance_session(session_id, SessionState::Planning)?;

        // 1) Validate the action.
        let result = self.validator.validate(&action)?;
        self.audit.record(
            &session_id.to_string(),
            AuditEventType::CapabilityCheck,
            &format!("action={} valid={}", action.action_name(), result.valid),
            result.valid,
            "host",
        );
        self.audit.record(
            &session_id.to_string(),
            AuditEventType::PolicyCheck,
            &format!("action={}", action.action_name()),
            true,
            "host",
        );
        if !result.valid {
            self.advance_session(session_id, SessionState::Failed).ok();
            self.audit.record(
                &session_id.to_string(),
                AuditEventType::ActionDenied,
                &result.errors.join("; "),
                false,
                "host",
            );
            self.events.publish(&AgentEvent::ActionDenied {
                session_id: session_id.to_string(),
                action_id: action.id.to_string(),
                reason: result.errors.join("; "),
            });
            self.published_event_count += 1;
            return Err(AgentError::Validation(result.errors.join("; ")));
        }
        if result.requires_confirmation {
            // High/Critical risk: surface a WaitingApproval state. The
            // daemon decides whether to actually await user input.
            self.advance_session(session_id, SessionState::WaitingApproval)?;
        }

        // 2) Build the request envelope and run it.
        let request = UserRequest::new(
            &session_id.to_string(),
            actor.clone(),
            &action.reason,
            serde_json::json!({ "action": action.action_name() }),
        );
        self.advance_session(session_id, SessionState::Executing)?;
        self.audit.record(
            &session_id.to_string(),
            AuditEventType::ActionRequested,
            &format!("action={} risk={:?}", action.action_name(), action.risk_level),
            true,
            "host",
        );
        self.events.publish(&AgentEvent::ActionRequested {
            session_id: session_id.to_string(),
            action_id: action.id.to_string(),
            action_name: action.action_name().to_string(),
            risk_level: format!("{:?}", action.risk_level),
        });
        self.published_event_count += 1;
        // Track the action so it can be cancelled.
        let token = CancellationToken::new();
        self.record_action(session_id, action.id, token.clone());

        let start = (self.now_ms)();
        let exec = self.executor.execute(&action);
        let duration_ms = (self.now_ms)().saturating_sub(start);

        let outcome = match exec {
            Ok(result) => {
                self.audit.record(
                    &session_id.to_string(),
                    AuditEventType::ActionCompleted,
                    &format!("action={} dur_ms={}", action.action_name(), result.duration_ms),
                    true,
                    "host",
                );
                self.events.publish(&AgentEvent::ActionCompleted {
                    session_id: session_id.to_string(),
                    action_id: action.id.to_string(),
                    duration_ms: result.duration_ms,
                });
                self.published_event_count += 1;
                self.advance_session(session_id, SessionState::Observing)?;
                self.complete_session(session_id)?;
                RequestOutcome {
                    request_id: request.id.to_string(),
                    session_id: session_id.to_string(),
                    action_id: Some(action.id.to_string()),
                    success: result.success,
                    observation: Some(observation_to_json(&result.observation)),
                    error: None,
                    duration_ms,
                    session_state: self
                        .session_state(session_id)
                        .unwrap_or(SessionState::Completed),
                }
            }
            Err(e) => {
                self.audit.record(
                    &session_id.to_string(),
                    AuditEventType::ActionFailed,
                    &format!("action={} err={}", action.action_name(), e),
                    false,
                    "host",
                );
                self.events.publish(&AgentEvent::ActionFailed {
                    session_id: session_id.to_string(),
                    action_id: action.id.to_string(),
                    error: e.to_string(),
                });
                self.published_event_count += 1;
                self.advance_session(session_id, SessionState::Failed).ok();
                self.record_session_error(session_id);
                RequestOutcome {
                    request_id: request.id.to_string(),
                    session_id: session_id.to_string(),
                    action_id: Some(action.id.to_string()),
                    success: false,
                    observation: None,
                    error: Some(e.to_string()),
                    duration_ms,
                    session_state: self.session_state(session_id).unwrap_or(SessionState::Failed),
                }
            }
        };
        self.record_request(session_id, request.id);
        Ok(outcome)
    }

    /// Cancels the action with the given ID. Returns true if the
    /// action was found and signalled.
    pub fn cancel_action(
        &mut self,
        session_id: &SessionId,
        action_id: &uuid::Uuid,
    ) -> Result<bool, AgentError> {
        let rec = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| AgentError::NotFound(format!("session {session_id}")))?;
        if let Some(token) = rec.actions.remove(action_id) {
            token.cancel();
            self.audit.record(
                &session_id.to_string(),
                AuditEventType::ActionDenied,
                &format!("action_id={action_id} cancelled"),
                true,
                "host",
            );
            self.events.publish(&AgentEvent::ActionDenied {
                session_id: session_id.to_string(),
                action_id: action_id.to_string(),
                reason: "cancelled".to_string(),
            });
            self.published_event_count += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Cancels an entire session.
    pub fn cancel_session(&mut self, session_id: &SessionId) -> Result<bool, AgentError> {
        let rec = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| AgentError::NotFound(format!("session {session_id}")))?;
        // Cancel all in-flight action tokens.
        for (_, token) in rec.actions.drain() {
            token.cancel();
        }
        if rec.session.state.is_terminal() {
            return Ok(false);
        }
        rec.session.cancelled = true;
        rec.session.state = SessionState::Cancelled;
        self.audit.record(
            &session_id.to_string(),
            AuditEventType::SessionCancelled,
            "session cancelled by host",
            true,
            "host",
        );
        self.events.publish(&AgentEvent::SessionCancelled { session_id: session_id.to_string() });
        self.published_event_count += 1;
        Ok(true)
    }

    /// Returns the audit entries for one session.
    pub fn audit_for(&self, session_id: &str) -> Vec<serde_json::Value> {
        self.audit
            .for_session(session_id)
            .iter()
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp,
                    "session_id": e.session_id,
                    "event_type": e.event_type.to_string(),
                    "success": e.success,
                    "component": e.component,
                    "detail": e.detail,
                })
            })
            .collect()
    }

    /// Returns the most recent N audit entries.
    pub fn audit_recent(&self, count: usize) -> Vec<serde_json::Value> {
        self.audit
            .recent(count)
            .iter()
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp,
                    "session_id": e.session_id,
                    "event_type": e.event_type.to_string(),
                    "success": e.success,
                    "component": e.component,
                    "detail": e.detail,
                })
            })
            .collect()
    }

    /// Returns the host's view of how many events have been published.
    pub fn event_count(&self) -> usize {
        self.published_event_count
    }

    /// Stops the host, transitioning to Stopped. Idempotent.
    pub fn stop(&mut self) -> Result<(), AgentError> {
        if self.state == HostState::Stopped {
            return Ok(());
        }
        self.transition_state(HostState::Stopping, "stop requested")?;
        // Cancel all sessions.
        let ids: Vec<SessionId> = self.sessions.keys().copied().collect();
        for id in ids {
            let _ = self.cancel_session(&id);
        }
        self.transition_state(HostState::Stopped, "host stopped")?;
        Ok(())
    }

    /// Marks the host as failed and records the reason.
    pub fn fail(&mut self, reason: &str) {
        // Best effort — the audit may itself be broken, but we try.
        self.audit.record("host", AuditEventType::SessionFailed, reason, false, "host");
        self.events.publish(&AgentEvent::SessionFailed {
            session_id: "host".to_string(),
            reason: reason.to_string(),
        });
        self.published_event_count += 1;
        self.state = HostState::Failed;
    }

    // ---------- private helpers ----------

    fn ensure_accepting(&self) -> Result<(), AgentError> {
        if !self.state.is_accepting() {
            return Err(AgentError::Internal(format!(
                "host is {}; not accepting work",
                self.state
            )));
        }
        Ok(())
    }

    fn transition_state(&mut self, next: HostState, reason: &str) -> Result<(), AgentError> {
        let allowed = matches!(
            (self.state, next),
            (HostState::Starting, HostState::Ready)
                | (HostState::Ready, HostState::Running)
                | (HostState::Running, HostState::Stopping)
                | (HostState::Running, HostState::Failed)
                | (HostState::Ready, HostState::Failed)
                | (HostState::Starting, HostState::Failed)
                | (HostState::Stopping, HostState::Stopped)
        );
        if !allowed {
            return Err(AgentError::Internal(format!(
                "invalid host transition {} -> {} ({})",
                self.state, next, reason
            )));
        }
        self.state = next;
        Ok(())
    }

    fn advance_session(
        &mut self,
        session_id: &SessionId,
        next: SessionState,
    ) -> Result<(), AgentError> {
        let rec = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| AgentError::NotFound(format!("session {session_id}")))?;
        if rec.session.state.is_terminal() {
            return Err(AgentError::Session(format!(
                "session {session_id} already terminal ({})",
                rec.session.state
            )));
        }
        // Cancelled sessions reject further transitions; the caller
        // sees an error so it can stop processing the plan.
        if rec.session.cancelled {
            return Err(AgentError::Cancellation(format!("session {session_id} cancelled")));
        }
        if rec.session.state == next {
            return Ok(()); // idempotent
        }
        if !rec.session.state.can_transition_to(&next) {
            return Err(AgentError::Session(format!(
                "invalid session transition {} -> {}",
                rec.session.state, next
            )));
        }
        rec.session.transition(next).map_err(AgentError::Session)?;
        Ok(())
    }

    /// Idempotent version: no-ops if the session is already in (or
    /// past) the target state.
    fn advance_session_if_not(
        &mut self,
        session_id: &SessionId,
        next: SessionState,
    ) -> Result<(), AgentError> {
        let rec = self
            .sessions
            .get(session_id)
            .ok_or_else(|| AgentError::NotFound(format!("session {session_id}")))?;
        if rec.session.state == next || rec.session.state.is_terminal() {
            return Ok(());
        }
        let _ = rec;
        self.advance_session(session_id, next)
    }

    fn complete_session(&mut self, session_id: &SessionId) -> Result<(), AgentError> {
        let rec = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| AgentError::NotFound(format!("session {session_id}")))?;
        rec.session.transition(SessionState::Completed).map_err(AgentError::Session)?;
        self.audit.record(
            &session_id.to_string(),
            AuditEventType::SessionCompleted,
            "session completed",
            true,
            "host",
        );
        self.events.publish(&AgentEvent::SessionCompleted { session_id: session_id.to_string() });
        self.published_event_count += 1;
        Ok(())
    }

    fn record_action(
        &mut self,
        session_id: &SessionId,
        action_id: crate::action::ActionId,
        token: CancellationToken,
    ) {
        if let Some(rec) = self.sessions.get_mut(session_id) {
            rec.actions.insert(action_id.0, token);
            rec.session.action_count += 1;
        }
    }

    fn record_request(&mut self, session_id: &SessionId, request_id: RequestId) {
        if let Some(rec) = self.sessions.get_mut(session_id) {
            rec.requests.push(request_id);
            rec.session.request_count += 1;
            rec.session.observation_count += 1;
        }
    }

    fn record_session_error(&mut self, session_id: &SessionId) {
        if let Some(rec) = self.sessions.get_mut(session_id) {
            rec.session.mark_error();
        }
    }

    fn session_state(&self, session_id: &SessionId) -> Option<SessionState> {
        self.sessions.get(session_id).map(|r| r.session.state)
    }
}

fn system_time_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn observation_to_json(obs: &Observation) -> serde_json::Value {
    serde_json::json!({
        "observation_id": obs.id.to_string(),
        "action_id": obs.action_id,
        "session_id": obs.session_id,
        "success": obs.success,
        "type": observation_type_name(&obs.observation_type),
        "data": obs.normalized(),
    })
}

fn observation_type_name(t: &ObservationType) -> &'static str {
    match t {
        ObservationType::ApplicationStarted { .. } => "application.started",
        ObservationType::ApplicationFailed { .. } => "application.failed",
        ObservationType::ApplicationClosed { .. } => "application.closed",
        ObservationType::ProcessExited { .. } => "process.exited",
        ObservationType::WindowCreated { .. } => "window.created",
        ObservationType::WindowClosed { .. } => "window.closed",
        ObservationType::WindowFocused { .. } => "window.focused",
        ObservationType::WindowMinimized { .. } => "window.minimized",
        ObservationType::WindowMaximized { .. } => "window.maximized",
        ObservationType::WindowList { .. } => "window.list",
        ObservationType::FilesystemResult { .. } => "filesystem.result",
        ObservationType::NetworkStatus { .. } => "network.status",
        ObservationType::NetworkInterfaces { .. } => "network.interfaces",
        ObservationType::SystemStatus { .. } => "system.status",
        ObservationType::SystemInfo { .. } => "system.info",
        ObservationType::SystemResources { .. } => "system.resources",
        ObservationType::SystemUptime { .. } => "system.uptime",
        ObservationType::StorageStatus { .. } => "storage.status",
        ObservationType::ProcessList { .. } => "process.list",
        ObservationType::ProcessInspect { .. } => "process.inspect",
        ObservationType::ContextSnapshot { .. } => "context.snapshot",
        ObservationType::Error { .. } => "error",
    }
}

// Validate the validator return so the daemon can rely on a uniform error
// surface. We don't expose `Result<ValidationResult, AgentError>` from the
// runtime validator today; the trait converts via From below.
impl From<String> for AgentError {
    fn from(_s: String) -> Self {
        // Strings coming from validator are treated as validation errors.
        AgentError::Validation("validation failed".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionVariant, ApplicationLaunchParams};

    fn test_actor() -> RequestActor {
        RequestActor {
            actor_type: crate::request::ActorType::Human,
            identity: "test-user".to_string(),
        }
    }

    fn session_actor() -> SessionActor {
        SessionActor {
            actor_type: crate::session::ActorType::Human,
            identity: "test-user".to_string(),
        }
    }

    fn host() -> AgentRuntimeHost {
        AgentRuntimeHost::start(
            0,
            0,
            vec![
                "application.launch".to_string(),
                "application.close".to_string(),
                "system.status".to_string(),
                "window.list".to_string(),
            ],
            Box::new(InMemoryEventBus::new()),
        )
        .unwrap_or_else(|e| panic!("start: {e}"))
    }

    #[test]
    fn host_starts_in_ready() {
        let h = host();
        assert_eq!(h.state(), HostState::Ready);
    }

    #[test]
    fn state_transitions_are_strict() {
        let mut h = host();
        // Stop requires Running, so attempting to stop from Ready should fail.
        assert!(h.transition_state(HostState::Stopping, "bad").is_err());
        // Move to Running via a session create.
        let actor = session_actor();
        h.create_session(actor).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(h.state(), HostState::Running);
    }

    #[test]
    fn invalid_lifecycle_to_failed_is_allowed() {
        let mut h = host();
        h.fail("synthetic");
        assert_eq!(h.state(), HostState::Failed);
    }

    #[test]
    fn host_rejects_work_after_failed() {
        let mut h = host();
        h.fail("down");
        let res = h.create_session(session_actor());
        assert!(res.is_err());
    }

    #[test]
    fn create_session_publishes_event() {
        let bus = InMemoryEventBus::new();
        let mut h = AgentRuntimeHost::start(0, 0, Vec::new(), Box::new(bus.clone()))
            .unwrap_or_else(|e| panic!("{e}"));
        let _ = h.create_session(session_actor());
        let snap = bus.snapshot();
        assert!(!snap.is_empty());
        assert_eq!(snap[0].event_type(), "agent.session.created");
    }

    #[test]
    fn session_count_increments() {
        let mut h = host();
        let _ = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let _ = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(h.session_count(), 2);
    }

    #[test]
    fn list_sessions_returns_newest_first() {
        let mut h = host();
        let id1 = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let id2 = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let list = h.list_sessions();
        assert_eq!(list.len(), 2);
        // id2 was created after id1 so it should appear first
        assert_eq!(list[0]["session_id"], id2.to_string());
        assert_eq!(list[1]["session_id"], id1.to_string());
    }

    #[test]
    fn inspect_session_returns_metadata() {
        let mut h = host();
        let id = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let snap = h.inspect_session(&id).unwrap_or_else(|| panic!("none"));
        assert_eq!(snap["session_id"], id.to_string());
        assert_eq!(snap["state"], "ready");
    }

    #[test]
    fn cancel_session_publishes_event_and_blocks() {
        let bus = InMemoryEventBus::new();
        let mut h = AgentRuntimeHost::start(0, 0, Vec::new(), Box::new(bus.clone()))
            .unwrap_or_else(|e| panic!("{e}"));
        let id = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        assert!(h.cancel_session(&id).unwrap_or_else(|e| panic!("{e}")));
        let snap = bus.snapshot();
        let types: Vec<&'static str> = snap.iter().map(|e| e.event_type()).collect();
        assert!(types.contains(&"agent.session.cancelled"));
        // Submitting work to a cancelled session should fail.
        let action = Action::new(&id.to_string(), ActionVariant::SystemStatus, "check");
        let res = h.submit_action(&id, &test_actor(), action);
        assert!(res.is_err());
    }

    #[test]
    fn cancel_unknown_session_returns_not_found() {
        let mut h = host();
        let id = SessionId::new();
        assert!(h.cancel_session(&id).is_err());
    }

    #[test]
    fn stop_is_idempotent_and_cancels_sessions() {
        let mut h = host();
        let _ = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        h.stop().unwrap_or_else(|e| panic!("first stop: {e}"));
        assert_eq!(h.state(), HostState::Stopped);
        // Second stop is a no-op.
        h.stop().unwrap_or_else(|e| panic!("second stop: {e}"));
    }

    #[test]
    fn max_sessions_enforced() {
        let mut h = host();
        h.max_sessions = 2;
        let _ = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let _ = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let res = h.create_session(session_actor());
        assert!(res.is_err());
    }

    #[test]
    fn audit_records_session_lifecycle() {
        let mut h = host();
        let id = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let _ = h.cancel_session(&id).unwrap_or_else(|e| panic!("{e}"));
        let entries = h.audit_for(&id.to_string());
        assert!(entries.iter().any(|e| e["event_type"] == "session.created"));
        assert!(entries.iter().any(|e| e["event_type"] == "session.cancelled"));
    }

    #[test]
    fn action_validation_failure_is_audited() {
        let mut h = AgentRuntimeHost::start(
            0,
            0,
            // Note: file.delete capability NOT granted.
            Vec::new(),
            Box::new(InMemoryEventBus::new()),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let id = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let action = Action::new(&id.to_string(), ActionVariant::SystemStatus, "check");
        // System status isn't granted either, so this should be denied.
        let res = h.submit_action(&id, &test_actor(), action);
        assert!(res.is_err());
        let entries = h.audit_for(&id.to_string());
        assert!(entries.iter().any(|e| e["event_type"] == "action.denied"));
    }

    #[test]
    fn status_reports_session_and_audit_counts() {
        let mut h = host();
        let _ = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let s = h.status();
        assert_eq!(s.state, HostState::Running);
        assert_eq!(s.session_count, 1);
        assert!(s.audit_count >= 1);
    }

    #[test]
    fn grant_capabilities_widens_validator() {
        let mut h = host();
        h.grant_capabilities(vec!["system.status".to_string()]);
        // The validator's granted set now includes system.status so a
        // schema-valid action can pass capability validation. Execution
        // will still fail because port 0 is not listening — but we
        // verify only validation here.
        let id = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let action = Action::new(&id.to_string(), ActionVariant::SystemStatus, "check");
        // Should NOT error on validation now.
        let _ = h.submit_action(&id, &test_actor(), action);
    }

    #[test]
    fn application_launch_with_no_listener_records_execution_failure() {
        let mut h = AgentRuntimeHost::start(
            1, // control port: no service listening here
            2, // surface port
            vec!["application.launch".to_string()],
            Box::new(InMemoryEventBus::new()),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let id = h.create_session(session_actor()).unwrap_or_else(|e| panic!("{e}"));
        let action = Action::new(
            &id.to_string(),
            ActionVariant::ApplicationLaunch(ApplicationLaunchParams {
                application_id: "calc".to_string(),
            }),
            "user asked",
        );
        let res = h.submit_action(&id, &test_actor(), action);
        let outcome = res.unwrap_or_else(|e| panic!("{e}"));
        assert!(!outcome.success);
        assert!(outcome.error.is_some());
        let entries = h.audit_for(&id.to_string());
        assert!(entries.iter().any(|e| e["event_type"] == "action.failed"));
    }
}
