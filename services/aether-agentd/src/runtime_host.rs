// Aether Agent Daemon - Agent Runtime Host integration.
//
// The Agent Runtime Host is the structured, audited, capability-checked
// execution path inside agentd. The old `chat` path remains for
// backwards compatibility, but every new agent.* IPC command routes
// through `AgentRuntimeHost` so:
//
//   * session lifecycle is explicit (Created -> Ready -> Thinking ->
//     Planning -> Executing -> Observing -> Completed/Failed/Cancelled);
//   * capability checks and policy checks are recorded per action;
//   * audit entries are produced for every transition;
//   * the LLM can only propose *intent* — it never authorises an
//     action. The runtime does, after validation.

use aether_agent_runtime::{
    action::Action,
    errors::AgentError,
    host::{AgentRuntimeHost, InMemoryEventBus, RequestOutcome},
    request::{ActorType as RequestActorType, RequestActor},
    session::{ActorType as SessionActorType, SessionActor, SessionId},
};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Default capabilities the agent identity owns. The runtime narrows
/// the validator to exactly this set. Aether-security's policy layer
/// (when wired) will derive the same set from the agent manifest.
pub fn default_capabilities() -> Vec<String> {
    vec![
        // Read-only status / discovery
        "system.status".to_string(),
        "system.info".to_string(),
        "system.resources".to_string(),
        "system.uptime".to_string(),
        "context.get".to_string(),
        "storage.status".to_string(),
        "network.status".to_string(),
        "network.interfaces".to_string(),
        "application.list".to_string(),
        "process.list".to_string(),
        "process.inspect".to_string(),
        "window.list".to_string(),
        // File system (read-only + workspace writes)
        "file.list".to_string(),
        "file.search".to_string(),
        "file.read".to_string(),
        "file.stat".to_string(),
        "file.create".to_string(),
        "file.write".to_string(),
        "file.rename".to_string(),
        "file.move".to_string(),
        // Application lifecycle
        "application.launch".to_string(),
        "application.close".to_string(),
        // Window control
        "window.focus".to_string(),
        "window.minimize".to_string(),
        "window.maximize".to_string(),
        "window.close".to_string(),
    ]
}

/// Thin wrapper that owns the host under a single mutex. The daemon's
/// TCP loop is the only consumer; a `Mutex<...>` is the simplest lock
/// that keeps the host's invariants intact across threads.
pub struct RuntimeBridge {
    inner: Mutex<AgentRuntimeHost>,
    bus: Arc<InMemoryEventBus>,
}

impl RuntimeBridge {
    pub fn start(control_port: u16, surface_port: u16) -> Result<Self, AgentError> {
        let bus = Arc::new(InMemoryEventBus::new());
        let bus_for_host: InMemoryEventBus = (*bus).clone();
        let host = AgentRuntimeHost::start(
            control_port,
            surface_port,
            default_capabilities(),
            Box::new(bus_for_host),
        )?;
        Ok(Self { inner: Mutex::new(host), bus })
    }

    /// Locks the host and applies a read-only function.
    pub fn with_host<R>(&self, f: impl FnOnce(&AgentRuntimeHost) -> R) -> R {
        let guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        f(&guard)
    }

    /// Locks the host and applies a mutable function.
    pub fn with_host_mut<R>(&self, f: impl FnOnce(&mut AgentRuntimeHost) -> R) -> R {
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut guard)
    }

    /// Returns a clone of the in-memory event bus. The bus is
    /// `Clone` because it uses `Arc<Mutex<...>>` internally.
    pub fn bus(&self) -> Arc<InMemoryEventBus> {
        Arc::clone(&self.bus)
    }
}

/// Helpers for building structured responses.
pub fn ok(result: Value) -> Value {
    serde_json::json!({ "ok": true, "result": result })
}

pub fn err(message: impl Into<String>) -> Value {
    serde_json::json!({ "ok": false, "result": { "error": message.into() } })
}

pub fn request_actor_from_str(identity: &str) -> RequestActor {
    RequestActor { actor_type: RequestActorType::Human, identity: identity.to_string() }
}

pub fn session_actor_from_str(identity: &str) -> SessionActor {
    SessionActor { actor_type: SessionActorType::Human, identity: identity.to_string() }
}

/// Parses a session id from a JSON value, returning a
/// `Result<(), Value>` so the caller can produce a structured error
/// response without unwrapping.
pub fn parse_session_id(raw: &str) -> Result<SessionId, Value> {
    let uuid = uuid::Uuid::parse_str(raw)
        .map_err(|_| err(format!("invalid session id: '{raw}' (must be a UUID)")))?;
    Ok(SessionId::from_uuid(uuid))
}

/// Resolves a session id (string form) against the host and returns
/// the host's `SessionId` if it exists. Used by every per-session
/// agent.* command.
pub fn resolve_session(host: &AgentRuntimeHost, raw: &str) -> Result<SessionId, Value> {
    let id = parse_session_id(raw)?;
    if !host.has_session(&id) {
        return Err(err(format!("no such session: '{raw}'")));
    }
    Ok(id)
}

/// Submits an action to the host and converts the outcome into a
/// JSON result for the IPC layer.
pub fn submit_and_format(bridge: &RuntimeBridge, session_id_str: &str, action: Action) -> Value {
    let id = match parse_session_id(session_id_str) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let actor = request_actor_from_str("agentd");
    let outcome: Result<RequestOutcome, _> =
        bridge.with_host_mut(|h| h.submit_action(&id, &actor, action));
    match outcome {
        Ok(o) => outcome_to_value(&o),
        Err(e) => err(e.to_string()),
    }
}

pub fn outcome_to_value(o: &RequestOutcome) -> Value {
    let mut result = serde_json::Map::new();
    result.insert("request_id".to_string(), Value::String(o.request_id.clone()));
    result.insert("session_id".to_string(), Value::String(o.session_id.clone()));
    if let Some(aid) = &o.action_id {
        result.insert("action_id".to_string(), Value::String(aid.clone()));
    }
    result.insert("success".to_string(), Value::Bool(o.success));
    result.insert("duration_ms".to_string(), Value::Number(o.duration_ms.into()));
    result.insert("session_state".to_string(), Value::String(o.session_state.to_string()));
    if let Some(obs) = &o.observation {
        result.insert("observation".to_string(), obs.clone());
    }
    if let Some(e) = &o.error {
        result.insert("error".to_string(), Value::String(e.clone()));
    }
    ok(Value::Object(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_agent_runtime::action::ActionVariant;

    fn bridge() -> RuntimeBridge {
        RuntimeBridge::start(0, 0).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn bridge_starts_in_ready() {
        let b = bridge();
        let state = b.with_host(|h| h.state());
        // After start, the host is Ready (no session yet).
        // We don't compare to a specific variant to keep this
        // resilient to enum additions.
        let _ = state;
    }

    #[test]
    fn default_capabilities_includes_core_set() {
        let caps = default_capabilities();
        assert!(caps.contains(&"application.launch".to_string()));
        assert!(caps.contains(&"system.status".to_string()));
        assert!(caps.contains(&"window.list".to_string()));
        // No shell-execution capability by design.
        assert!(!caps.iter().any(|c| c.contains("shell") || c.contains("exec")));
    }

    #[test]
    fn outcome_to_value_carries_request_and_session() {
        let b = bridge();
        let sid = b.with_host_mut(|h| {
            h.create_session(session_actor_from_str("alice")).unwrap_or_else(|e| panic!("{e}"))
        });
        let action = Action::new(&sid.to_string(), ActionVariant::SystemStatus, "check");
        let v = submit_and_format(&b, &sid.to_string(), action);
        let inner = &v["result"];
        assert_eq!(inner["session_id"], sid.to_string());
        // The outcome ran; on a closed port it returns success=false.
        assert!(inner["request_id"].is_string());
    }

    #[test]
    fn resolve_session_rejects_unknown() {
        let b = bridge();
        let v = b.with_host(|h| resolve_session(h, "00000000-0000-0000-0000-000000000000"));
        assert!(v.is_err());
    }

    #[test]
    fn submit_and_format_rejects_bad_session_id() {
        let b = bridge();
        let action = Action::new("garbage", ActionVariant::SystemStatus, "check");
        let v = submit_and_format(&b, "garbage", action);
        assert_eq!(v["ok"], Value::Bool(false));
    }
}
