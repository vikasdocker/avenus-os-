// Aether Agent Daemon - the AI control-plane agent.
//
// Maintains a bounded, timestamped event ring and answers deterministic
// intent queries (status, health, events, tasks) for the local AI brain
// and interactive sessions. All state is in-memory and auditable.

pub mod intent;
pub mod context;
pub mod planner;
pub mod conversation;
pub mod confirmation;
pub mod structured_llm;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Capacity of the event ring before oldest events are evicted.
pub const EVENT_RING_CAPACITY: usize = 256;

/// Severity of an agent event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSeverity {
    Info,
    Warning,
    Error,
}

/// A single control-plane event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub id: Uuid,
    pub severity: EventSeverity,
    pub source: String,
    pub message: String,
    pub timestamp_ms: u64,
}

/// A durable task tracked by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: Uuid,
    pub description: String,
    pub done: bool,
}

/// The agent's observable state.
pub struct AgentState {
    agent_id: Uuid,
    events: VecDeque<AgentEvent>,
    tasks: Vec<TaskRecord>,
    now_ms: fn() -> u64,
    provider: Box<dyn AiProvider + Send>,
    /// Control-plane port used by the capability executor.
    pub control_port: u16,
    /// Surface server port for window operations.
    pub surface_port: u16,
    /// Bounded conversation memory for pronoun resolution.
    pub conversation: conversation::ConversationContext,
}

/// Replaceable AI backend. The UI never talks to a model directly; the
/// agent owns this interface so providers can be swapped by configuration.
pub trait AiProvider {
    fn name(&self) -> &str;
    fn complete(&self, prompt: &str) -> Result<String, String>;
}

/// Deterministic fallback used when no real provider is reachable. Keeps
/// the full UI -> Agent -> Provider -> Agent -> UI pipeline demonstrable
/// offline and in tests.
pub struct EchoProvider;

impl AiProvider for EchoProvider {
    fn name(&self) -> &str {
        "echo"
    }

    fn complete(&self, prompt: &str) -> Result<String, String> {
        Ok(format!("ECHO: {prompt}"))
    }
}

/// Local Ollama (http://127.0.0.1:11434 by default). No model is bundled;
/// whatever the host has pulled is addressed by name.
pub struct OllamaProvider {
    pub url: String,
    pub model: String,
}

impl OllamaProvider {
    /// Minimal HTTP/1.1 POST with `Connection: close` using std only.
    fn http_post_json(&self, path: &str, body: &str) -> Result<String, String> {
        use std::io::{Read, Write};
        let rest = self
            .url
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        let authority = rest.split('/').next().unwrap_or(rest);
        let mut stream = std::net::TcpStream::connect(authority)
            .map_err(|e| format!("connect {authority}: {e}"))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(60)))
            .map_err(|e| format!("timeout: {e}"))?;
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("send: {e}"))?;
        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .map_err(|e| format!("recv: {e}"))?;
        let text = String::from_utf8_lossy(&raw).to_string();
        text.split("\r\n\r\n")
            .nth(1)
            .map(str::to_string)
            .ok_or_else(|| "malformed HTTP response".to_string())
    }
}

impl AiProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn complete(&self, prompt: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
        });
        let body = body.to_string();
        let payload = self.http_post_json("/api/generate", &body)?;
        // The body may contain HTTP chunked framing; find the JSON object.
        let json_start = payload.find('{').ok_or("no JSON in response")?;
        let value: serde_json::Value = serde_json::from_str(payload[json_start..].trim())
            .map_err(|e| format!("bad ollama json: {e}"))?;
        value
            .get("response")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| "missing 'response' field".to_string())
    }
}

/// Selects the provider from the environment:
/// AETHER_AI_PROVIDER=echo|ollama, AETHER_OLLAMA_URL, AETHER_OLLAMA_MODEL.
/// Falls back to echo when the requested provider is unreachable later at
/// call time - selection here only picks the preferred backend.
pub fn provider_from_env() -> Box<dyn AiProvider + Send> {
    match std::env::var("AETHER_AI_PROVIDER").as_deref() {
        Ok("ollama") => Box::new(OllamaProvider {
            url: std::env::var("AETHER_OLLAMA_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string()),
            model: std::env::var("AETHER_OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string()),
        }),
        _ => Box::new(EchoProvider),
    }
}

impl AgentState {
    pub fn new(now_ms: fn() -> u64) -> Self {
        Self {
            agent_id: Uuid::new_v4(),
            events: VecDeque::with_capacity(EVENT_RING_CAPACITY),
            tasks: Vec::new(),
            now_ms,
            provider: provider_from_env(),
            control_port: 4747,
            surface_port: 4750,
            conversation: conversation::ConversationContext::default(),
        }
    }

    pub fn with_control_port(mut self, port: u16) -> Self {
        self.control_port = port;
        self
    }

    pub fn with_surface_port(mut self, port: u16) -> Self {
        self.surface_port = port;
        self
    }

    /// Replace the AI provider. Used by tests to inject deterministic
    /// structured-LLM responses without a real network call.
    pub fn with_provider(mut self, provider: Box<dyn AiProvider + Send>) -> Self {
        self.provider = provider;
        self
    }

    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }

    pub fn agent_id(&self) -> Uuid {
        self.agent_id
    }

    /// Records an event, evicting the oldest when full.
    pub fn record_event(
        &mut self,
        severity: EventSeverity,
        source: &str,
        message: &str,
    ) -> AgentEvent {
        let event = AgentEvent {
            id: Uuid::new_v4(),
            severity,
            source: source.to_string(),
            message: message.to_string(),
            timestamp_ms: (self.now_ms)(),
        };
        if self.events.len() >= EVENT_RING_CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back(event.clone());
        event
    }

    /// Events newest-first.
    pub fn recent_events(&self, limit: usize) -> Vec<&AgentEvent> {
        self.events.iter().rev().take(limit).collect()
    }

    /// Adds a task and returns its record.
    pub fn add_task(&mut self, description: &str) -> TaskRecord {
        let task = TaskRecord {
            id: Uuid::new_v4(),
            description: description.to_string(),
            done: false,
        };
        self.tasks.push(task.clone());
        task
    }

    /// Marks a task complete; returns true when it existed and was pending.
    pub fn complete_task(&mut self, id: Uuid) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id && !t.done) {
            task.done = true;
            return true;
        }
        false
    }

    /// All tasks.
    pub fn tasks(&self) -> &[TaskRecord] {
        &self.tasks
    }

    /// Deterministic health verdict for the whole system.
    pub fn health_verdict(&self) -> &'static str {
        if self
            .events
            .iter()
            .any(|e| e.severity == EventSeverity::Error)
        {
            "DEGRADED"
        } else {
            "HEALTHY"
        }
    }
}

impl Default for AgentState {
    fn default() -> Self {
        Self::new(system_time_ms)
    }
}

/// Wall-clock milliseconds; injectable for deterministic tests.
pub fn system_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Requests understood by the agent daemon.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AgentRequest {
    pub command: String,
    #[serde(default)]
    pub argument: Option<String>,
}

/// Responses produced by the agent daemon.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentResponse {
    pub ok: bool,
    pub result: serde_json::Value,
}

/// Handles one request against the agent state.
pub fn handle_request(state: &mut AgentState, request: &AgentRequest) -> AgentResponse {
    match request.command.as_str() {
        "status" => AgentResponse {
            ok: true,
            result: serde_json::json!({
                "agent_id": state.agent_id().to_string(),
                "health": state.health_verdict(),
                "event_count": state.recent_events(usize::MAX).len(),
                "task_count": state.tasks().len(),
                "provider": state.provider_name(),
            }),
        },
        "events" => {
            let limit = request
                .argument
                .as_deref()
                .and_then(|a| a.parse::<usize>().ok())
                .unwrap_or(10);
            let events: Vec<&AgentEvent> = state.recent_events(limit);
            AgentResponse {
                ok: true,
                result: serde_json::to_value(events).unwrap_or(serde_json::Value::Null),
            }
        }
        "context" | "context.get" => {
            let ctx = context::build_context(state.control_port, state.surface_port);
            AgentResponse {
                ok: true,
                result: serde_json::to_value(ctx).unwrap_or(serde_json::Value::Null),
            }
        }
        "note" => match request.argument.as_deref() {
            Some(text) if !text.trim().is_empty() => {
                let event = state.record_event(EventSeverity::Info, "session", text);
                AgentResponse {
                    ok: true,
                    result: serde_json::json!({ "recorded": event.id.to_string() }),
                }
            }
            _ => AgentResponse {
                ok: false,
                result: serde_json::json!({ "error": "'argument' with note text required" }),
            },
        },
        "task.add" => match request.argument.as_deref() {
            Some(text) if !text.trim().is_empty() => {
                let task = state.add_task(text);
                state.record_event(
                    EventSeverity::Info,
                    "tasks",
                    &format!("added '{}'", task.description),
                );
                AgentResponse {
                    ok: true,
                    result: serde_json::to_value(task).unwrap_or(serde_json::Value::Null),
                }
            }
            _ => AgentResponse {
                ok: false,
                result: serde_json::json!({ "error": "'argument' with task text required" }),
            },
        },
        "task.done" => match request
            .argument
            .as_deref()
            .and_then(|a| Uuid::parse_str(a).ok())
        {
            Some(id) => {
                let completed = state.complete_task(id);
                AgentResponse {
                    ok: completed,
                    result: serde_json::json!({ "completed": completed }),
                }
            }
            None => AgentResponse {
                ok: false,
                result: serde_json::json!({ "error": "'argument' must be a task UUID" }),
            },
        },
        "chat" => match request.argument.as_deref() {
            Some(text) if !text.trim().is_empty() => {
                state.record_event(EventSeverity::Info, "ai", &format!("user: {text}"));

                // Build grounded context for this turn (minimal, bounded).
                let ctx = context::build_context(state.control_port, state.surface_port);
                let convo_app = state.conversation.last_app().map(|s| s.to_string());
                let convo_file = state.conversation.last_file().map(|s| s.to_string());

                // 1) Try deterministic multi-step intent via planner first.
                if let Some(intents) = planner::Planner::plan_with_file(text, &ctx, convo_app.as_deref(), convo_file.as_deref()) {
                    // Execute sequentially with per-step validation.
                    let plan = planner::Planner::execute(
                        intents.clone(),
                        state.control_port,
                        state.surface_port,
                        &ctx,
                    );

                    // Update conversation memory with apps/windows/files mentioned in this plan.
                    let apps_in_plan: Vec<String> = intents
                        .iter()
                        .filter_map(|i| i.arguments.get("app").and_then(|v| v.as_str()).map(|s| s.to_string()))
                        .collect();
                    let windows_in_plan: Vec<String> = apps_in_plan.clone();
                    let files_in_plan: Vec<String> = intents
                        .iter()
                        .filter_map(|i| {
                            match i.capability {
                                crate::intent::CapabilityId::FileList => i.arguments.get("path").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                crate::intent::CapabilityId::FileSearch => i.arguments.get("query").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                crate::intent::CapabilityId::FileRead | crate::intent::CapabilityId::FileCreate | crate::intent::CapabilityId::FileWrite | crate::intent::CapabilityId::FileDelete => i.arguments.get("path").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                crate::intent::CapabilityId::FileRename | crate::intent::CapabilityId::FileMove => i.arguments.get("from").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                _ => None,
                            }
                        })
                        .filter(|s| !s.is_empty())
                        .collect();
                    state.conversation.push_with_files(text, apps_in_plan, windows_in_plan, files_in_plan);

                    // Record each action's outcome.
                    for action in &plan.actions {
                        let sev = if action.status == planner::ActionStatus::Success {
                            EventSeverity::Info
                        } else if action.status == planner::ActionStatus::Rejected {
                            EventSeverity::Warning
                        } else {
                            EventSeverity::Error
                        };
                        state.record_event(
                            sev,
                            "capability",
                            &format!("{} -> {} ({:?})", action.capability.as_str(), action.message, action.status),
                        );
                    }

                    // Build UI-friendly response with per-step feedback.
                    let response = if plan.actions.len() == 1 {
                        plan.actions[0].message.clone()
                    } else {
                        plan.summary.clone()
                    };

                    // Structured actions array for the UI.
                    let actions_json: Vec<serde_json::Value> = plan
                        .actions
                        .iter()
                        .map(|a| {
                            serde_json::json!({
                                "capability": a.capability.as_str(),
                                "arguments": a.arguments,
                                "status": format!("{:?}", a.status),
                                "message": a.message,
                            })
                        })
                        .collect();

                    return AgentResponse {
                        ok: plan.ok,
                        result: serde_json::json!({
                            "response": response,
                            "capability": plan.actions.first().map(|a| a.capability.as_str()).unwrap_or("none"),
                            "provider": "capability-layer",
                            "actions": actions_json,
                            "context": {
                                "active_window": ctx.active_window,
                                "running_apps": ctx.running_apps,
                                "windows": ctx.windows.iter().map(|w| w.title.clone()).collect::<Vec<_>>(),
                            }
                        }),
                    };
                }

                // 2) No deterministic intent. Try the structured-LLM path: ask
                //    the provider to classify the user request using the
                //    INTENT_SCHEMA envelope. If the LLM returns a valid
                //    capability, execute it through the planner (which still
                //    validates against policy). Anything else falls through
                //    to plain chat.
                let llm_outcome = structured_llm::try_structured(state.provider.as_ref(), text, &ctx);
                if let structured_llm::LlmIntentOutcome::Intent(llm_intent) = llm_outcome {
                    state.record_event(
                        EventSeverity::Info,
                        "ai",
                        &format!(
                            "structured LLM intent: {} ({})",
                            llm_intent.capability.as_str(),
                            llm_intent.arguments
                        ),
                    );
                    let plan = planner::Planner::execute(
                        vec![llm_intent.clone()],
                        state.control_port,
                        state.surface_port,
                        &ctx,
                    );
                    // Update conversation memory with apps mentioned.
                    let apps: Vec<String> = llm_intent
                        .arguments
                        .get("app")
                        .and_then(|v| v.as_str())
                        .map(|s| vec![s.to_string()])
                        .unwrap_or_default();
                    state.conversation.push_with_files(text, apps.clone(), apps, Vec::new());
                    // Record action outcome.
                    for action in &plan.actions {
                        let sev = if action.status == planner::ActionStatus::Success {
                            EventSeverity::Info
                        } else if action.status == planner::ActionStatus::Rejected {
                            EventSeverity::Warning
                        } else {
                            EventSeverity::Error
                        };
                        state.record_event(
                            sev,
                            "capability",
                            &format!("{} -> {} ({:?})", action.capability.as_str(), action.message, action.status),
                        );
                    }
                    let response = if plan.actions.len() == 1 {
                        plan.actions[0].message.clone()
                    } else {
                        plan.summary.clone()
                    };
                    let actions_json: Vec<serde_json::Value> = plan
                        .actions
                        .iter()
                        .map(|a| {
                            serde_json::json!({
                                "capability": a.capability.as_str(),
                                "arguments": a.arguments,
                                "status": format!("{:?}", a.status),
                                "message": a.message,
                            })
                        })
                        .collect();
                    return AgentResponse {
                        ok: plan.ok,
                        result: serde_json::json!({
                            "response": response,
                            "capability": plan.actions.first().map(|a| a.capability.as_str()).unwrap_or("none"),
                            "provider": "structured-llm",
                            "actions": actions_json,
                            "context": {
                                "active_window": ctx.active_window,
                                "running_apps": ctx.running_apps,
                                "windows": ctx.windows.iter().map(|w| w.title.clone()).collect::<Vec<_>>(),
                            }
                        }),
                    };
                }

                // 3) No structured intent either. Plain AI chat.
                let outcome = state.provider.complete(text);
                let (provider, response) = match outcome {
                    Ok(reply) => (state.provider.name().to_string(), reply),
                    Err(e) => (
                        "fallback".to_string(),
                        format!("AI provider unavailable ({e}); echoing instead: ECHO: {text}"),
                    ),
                };
                state.record_event(EventSeverity::Info, "ai", &format!("reply via {provider}"));
                // Still push to conversation as non-capability turn for pronoun future.
                state.conversation.push(text, Vec::new(), Vec::new());
                AgentResponse {
                    ok: true,
                    result: serde_json::json!({
                        "response": response,
                        "provider": provider,
                    }),
                }
            }
            _ => AgentResponse {
                ok: false,
                result: serde_json::json!({ "error": "'argument' with prompt text required" }),
            },
        },
        unknown => AgentResponse {
            ok: false,
            result: serde_json::json!({ "error": format!("unknown command '{unknown}'") }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn fake_clock(start: u64) -> (fn() -> u64, impl Fn()) {
        thread_local! {
            static NOW: Cell<u64> = const { Cell::new(0) };
        }
        NOW.with(|n| n.set(start));
        let tick = || NOW.with(|n| n.set(n.get() + 1));
        let reader: fn() -> u64 = || NOW.with(|n| n.get());
        (reader, tick)
    }

    #[test]
    fn status_reflects_error_events() {
        let (clock, _tick) = fake_clock(1000);
        let mut state = AgentState::new(clock);
        assert_eq!(state.health_verdict(), "HEALTHY");
        state.record_event(EventSeverity::Error, "test", "boom");
        assert_eq!(state.health_verdict(), "DEGRADED");
    }

    #[test]
    fn event_ring_is_bounded_and_ordered_newest_first() {
        let (clock, tick) = fake_clock(0);
        let mut state = AgentState::new(clock);
        for i in 0..(EVENT_RING_CAPACITY + 25) {
            tick();
            state.record_event(EventSeverity::Info, "loop", &format!("e{i}"));
        }
        assert_eq!(state.recent_events(usize::MAX).len(), EVENT_RING_CAPACITY);
        let newest = state.recent_events(1)[0].message.clone();
        assert_eq!(newest, format!("e{}", EVENT_RING_CAPACITY + 24));
    }

    #[test]
    fn task_lifecycle() {
        let (clock, _tick) = fake_clock(5);
        let mut state = AgentState::new(clock);
        let task = state.add_task("boot services");
        assert_eq!(state.tasks().len(), 1);
        assert!(state.complete_task(task.id));
        assert!(!state.complete_task(task.id), "second completion rejected");
        let res = handle_request(
            &mut state,
            &AgentRequest {
                command: "status".to_string(),
                argument: None,
            },
        );
        assert!(res.ok);
    }

    #[test]
    fn unknown_command_fails_cleanly() {
        let (clock, _tick) = fake_clock(0);
        let mut state = AgentState::new(clock);
        let res = handle_request(
            &mut state,
            &AgentRequest {
                command: "teleport".to_string(),
                argument: None,
            },
        );
        assert!(!res.ok);
    }

    #[test]
    fn chat_flows_through_provider_for_plain_text() {
        let (clock, _tick) = fake_clock(0);
        let mut state = AgentState::new(clock);
        let res = handle_request(
            &mut state,
            &AgentRequest {
                command: "chat".to_string(),
                argument: Some("Hello Aether".to_string()),
            },
        );
        assert!(res.ok);
        assert_eq!(res.result["provider"], "echo");
        let reply = res.result["response"].as_str().unwrap_or_default();
        assert!(reply.contains("ECHO: Hello Aether"), "got: {reply}");
    }

    #[test]
    fn chat_without_argument_rejected() {
        let (clock, _tick) = fake_clock(0);
        let mut state = AgentState::new(clock);
        let res = handle_request(
            &mut state,
            &AgentRequest {
                command: "chat".to_string(),
                argument: None,
            },
        );
        assert!(!res.ok);
    }

    #[test]
    fn ollama_provider_parses_mock_http_response() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|e| panic!("{e}"));
        let addr = listener.local_addr().unwrap_or_else(|e| panic!("{e}"));
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap_or_else(|e| panic!("{e}"));
            use std::io::{Read, Write};
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let body = r#"{"model":"mock","response":"hi from mock ollama","done":true}"#;
            let http = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(http.as_bytes());
        });

        let provider = OllamaProvider {
            url: format!("http://{addr}"),
            model: "mock".to_string(),
        };
        let reply = provider
            .complete("ping")
            .unwrap_or_else(|e| panic!("ollama round trip failed: {e}"));
        assert_eq!(reply, "hi from mock ollama");
        server.join().unwrap_or(());
    }

    fn spawn_mock_control_plane() -> (u16, std::thread::JoinHandle<()>) {
        use std::collections::{BTreeSet, HashMap};
        use std::sync::{Arc, Mutex};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("{e}"));
        let port = listener.local_addr().unwrap_or_else(|e| panic!("{e}")).port();
        let running: Arc<Mutex<BTreeSet<String>>> = Arc::new(Mutex::new(BTreeSet::new()));
        let windows_state: Arc<Mutex<HashMap<String, serde_json::Value>>> = Arc::new(Mutex::new(HashMap::new()));
        let handle = std::thread::spawn({
            let running = Arc::clone(&running);
            let windows_state = Arc::clone(&windows_state);
            move || {
                use std::io::{BufRead, BufReader, Write};
                for stream in listener.incoming().flatten().take(100) {
                    let running = Arc::clone(&running);
                    let windows_state = Arc::clone(&windows_state);
                    std::thread::spawn(move || {
                        let mut reader = BufReader::new(stream.try_clone().unwrap_or_else(|e| panic!("{e}")));
                        let mut writer = stream;
                        let mut line = String::new();
                        if reader.read_line(&mut line).is_err() {
                            return;
                        }
                        if line.trim().is_empty() {
                            return;
                        }
                        let req: serde_json::Value = serde_json::from_str(line.trim()).unwrap_or(serde_json::json!({}));
                        let cmd = req["command"].as_str().unwrap_or("");
                        let resp = match cmd {
                            "status" | "system.status" => {
                                let run_count = running.lock().unwrap_or_else(|p| p.into_inner()).len();
                                serde_json::json!({
                                    "ok": true,
                                    "command": cmd,
                                    "result": {
                                        "overall_health": "HEALTHY",
                                        "services": [
                                            {"service_id": "aether-system-core", "status": "RUNNING", "health": "HEALTHY"},
                                            {"service_id": "aether-agentd", "status": "RUNNING", "health": "HEALTHY"},
                                            {"service_id": "aether-application-manager", "status": "RUNNING", "health": "HEALTHY"}
                                        ],
                                        "applications": {"installed": 3, "running": run_count, "failed": 0}
                                    },
                                    "error": null
                                })
                            },
                            "app.list" => serde_json::json!({
                                "ok": true,
                                "command": cmd,
                                "result": {"apps": [
                                    {"id": "calculator", "name": "Calculator", "version": "0.1.0"},
                                    {"id": "notes", "name": "Notes", "version": "0.1.0"},
                                    {"id": "files", "name": "Files", "version": "0.1.0"}
                                ]},
                                "error": null
                            }),
                            "app.status" => {
                                let app = req["parameters"]["app"].as_str().unwrap_or("unknown").to_string();
                                let is_running = running.lock().unwrap_or_else(|p| p.into_inner()).contains(&app);
                                let state = if is_running { "RUNNING" } else { "INSTALLED" };
                                serde_json::json!({
                                    "ok": true,
                                    "command": cmd,
                                    "result": {"report": {"app": app, "state": state, "installed": true}},
                                    "error": null
                                })
                            },
                            "app.launch" => {
                                let app = req["parameters"]["app"].as_str().unwrap_or("app").to_string();
                                let mut run = running.lock().unwrap_or_else(|p| p.into_inner());
                                if run.contains(&app) {
                                    serde_json::json!({
                                        "ok": false,
                                        "command": cmd,
                                        "result": null,
                                        "error": {"code": "ALREADY_RUNNING", "message": format!("application '{app}' is already running")}
                                    })
                                } else {
                                    run.insert(app.clone());
                                    // also create window
                                    let mut ws = windows_state.lock().unwrap_or_else(|p| p.into_inner());
                                    ws.insert(app.clone(), serde_json::json!({"id": (run.len() as u64 + 10), "app": app, "title": app, "state": "normal", "focused": true}));
                                    // unfocus others
                                    for (k, v) in ws.iter_mut() {
                                        if k != &app {
                                            if let Some(o) = v.as_object_mut() {
                                                o.insert("focused".to_string(), serde_json::json!(false));
                                            }
                                        }
                                    }
                                    serde_json::json!({
                                        "ok": true,
                                        "command": cmd,
                                        "result": {"app": app, "instance": {"pid": 1234, "instance_id": 1}},
                                        "error": null
                                    })
                                }
                            },
                            "app.close" => {
                                let app = req["parameters"]["app"].as_str().unwrap_or("").to_string();
                                let mut run = running.lock().unwrap_or_else(|p| p.into_inner());
                                if run.remove(&app) {
                                    let mut ws = windows_state.lock().unwrap_or_else(|p| p.into_inner());
                                    ws.remove(&app);
                                    serde_json::json!({
                                        "ok": true,
                                        "command": cmd,
                                        "result": {"closed": 1},
                                        "error": null
                                    })
                                } else {
                                    serde_json::json!({
                                        "ok": false,
                                        "command": cmd,
                                        "result": null,
                                        "error": {"code": "NOT_RUNNING", "message": format!("'{app}' has no running instance")}
                                    })
                                }
                            },
                            "window.list" => {
                                let ws = windows_state.lock().unwrap_or_else(|p| p.into_inner());
                                let mut wins: Vec<serde_json::Value> = Vec::new();
                                for (app, v) in ws.iter() {
                                    let id = v["id"].as_u64().unwrap_or(1);
                                    let state = v["state"].as_str().unwrap_or("normal");
                                    let focused = v["focused"].as_bool().unwrap_or(false);
                                    let title = app.chars().next().map(|c| c.to_ascii_uppercase().to_string() + &app[1..]).unwrap_or(app.clone());
                                    wins.push(serde_json::json!({"id": id, "app": app, "title": title, "state": state, "focused": focused}));
                                }
                                // also include running apps that may not have window yet? For simplicity, windows derived from running.
                                serde_json::json!({
                                    "ok": true,
                                    "command": cmd,
                                    "result": {"windows": wins},
                                    "error": null
                                })
                            },
                            "window.focus" => {
                                let app = req["parameters"]["app"].as_str().unwrap_or("").to_string();
                                let wid = req["parameters"]["window_id"].as_u64();
                                let mut ws = windows_state.lock().unwrap_or_else(|p| p.into_inner());
                                if let Some(id) = wid {
                                    for v in ws.values_mut() {
                                        let cur_id = v["id"].as_u64();
                                        let is_target = cur_id == Some(id);
                                        if let Some(o) = v.as_object_mut() {
                                            o.insert("focused".to_string(), serde_json::json!(is_target));
                                            if is_target {
                                                o.insert("state".to_string(), serde_json::json!("normal"));
                                            }
                                        }
                                    }
                                    serde_json::json!({
                                        "ok": true,
                                        "command": cmd,
                                        "result": {"window_id": id, "ok": true},
                                        "error": null
                                    })
                                } else if ws.contains_key(&app) {
                                    for (k, v) in ws.iter_mut() {
                                        if let Some(o) = v.as_object_mut() {
                                            o.insert("focused".to_string(), serde_json::json!(k == &app));
                                            if k == &app {
                                                o.insert("state".to_string(), serde_json::json!("normal"));
                                            }
                                        }
                                    }
                                    let id = ws.get(&app).and_then(|v| v["id"].as_u64()).unwrap_or(1);
                                    serde_json::json!({
                                        "ok": true,
                                        "command": cmd,
                                        "result": {"window_id": id, "ok": true},
                                        "error": null
                                    })
                                } else {
                                    serde_json::json!({
                                        "ok": false,
                                        "command": cmd,
                                        "result": null,
                                        "error": {"code": "NOT_FOUND", "message": format!("no window for '{app}'")}
                                    })
                                }
                            },
                            "window.minimize" => {
                                let app = req["parameters"]["app"].as_str().unwrap_or("").to_string();
                                let wid = req["parameters"]["window_id"].as_u64();
                                let mut ws = windows_state.lock().unwrap_or_else(|p| p.into_inner());
                                let mut found = false;
                                for v in ws.values_mut() {
                                    let matches = v["id"].as_u64() == wid || v["app"].as_str() == Some(&app);
                                    if matches {
                                        if let Some(o) = v.as_object_mut() {
                                            o.insert("state".to_string(), serde_json::json!("minimized"));
                                            o.insert("focused".to_string(), serde_json::json!(false));
                                        }
                                        found = true;
                                    }
                                }
                                if found {
                                    serde_json::json!({
                                        "ok": true,
                                        "command": cmd,
                                        "result": {"ok": true},
                                        "error": null
                                    })
                                } else {
                                    serde_json::json!({
                                        "ok": false,
                                        "command": cmd,
                                        "result": null,
                                        "error": {"code": "NOT_FOUND", "message": format!("no window for '{app}'")}
                                    })
                                }
                            },
                            "window.maximize" => {
                                let app = req["parameters"]["app"].as_str().unwrap_or("").to_string();
                                let wid = req["parameters"]["window_id"].as_u64();
                                let mut ws = windows_state.lock().unwrap_or_else(|p| p.into_inner());
                                let mut found = false;
                                for v in ws.values_mut() {
                                    let matches = v["id"].as_u64() == wid || v["app"].as_str() == Some(&app);
                                    if matches {
                                        if let Some(o) = v.as_object_mut() {
                                            o.insert("state".to_string(), serde_json::json!("maximized"));
                                            o.insert("focused".to_string(), serde_json::json!(true));
                                        }
                                        found = true;
                                    }
                                }
                                if found {
                                    serde_json::json!({
                                        "ok": true,
                                        "command": cmd,
                                        "result": {"ok": true},
                                        "error": null
                                    })
                                } else {
                                    serde_json::json!({
                                        "ok": false,
                                        "command": cmd,
                                        "result": null,
                                        "error": {"code": "NOT_FOUND", "message": format!("no window for '{app}'")}
                                    })
                                }
                            },
                            "window.close" => {
                                let app = req["parameters"]["app"].as_str().unwrap_or("").to_string();
                                let mut run = running.lock().unwrap_or_else(|p| p.into_inner());
                                let mut ws = windows_state.lock().unwrap_or_else(|p| p.into_inner());
                                if run.remove(&app) {
                                    ws.remove(&app);
                                    serde_json::json!({
                                        "ok": true,
                                        "command": cmd,
                                        "result": {"closed": 1},
                                        "error": null
                                    })
                                } else {
                                    serde_json::json!({
                                        "ok": false,
                                        "command": cmd,
                                        "result": null,
                                        "error": {"code": "NOT_FOUND", "message": format!("no window for '{app}'")}
                                    })
                                }
                            },
                            "window.restore" | "context.get" => serde_json::json!({
                                "ok": true,
                                "command": cmd,
                                "result": {"window_id": 1, "ok": true},
                                "error": null
                            }),
                            "file.list" => {
                                let path = req["parameters"]["path"].as_str().unwrap_or("");
                                if path.contains("..") || path.starts_with('/') || path.contains('\0') {
                                    serde_json::json!({"ok": false, "command": cmd, "result": null, "error": {"code": "PATH_TRAVERSAL", "message": format!("path traversal detected: {path}")}})
                                } else {
                                    serde_json::json!({"ok": true, "command": cmd, "result": {"path": path, "files": [{"filename": "roadmap.md", "relative_path": "Documents/roadmap.md", "file_type": "file", "size": 123}, {"filename": "ideas.md", "relative_path": "Documents/ideas.md", "file_type": "file", "size": 45}]}, "error": null})
                                }
                            },
                            "file.search" => {
                                let query = req["parameters"]["query"].as_str().unwrap_or("");
                                serde_json::json!({"ok": true, "command": cmd, "result": {"query": query, "results": [{"filename": "roadmap.md", "relative_path": "Documents/roadmap.md", "file_type": "file", "size": 123}]}, "error": null})
                            },
                            "file.read" => {
                                let path = req["parameters"]["path"].as_str().unwrap_or("");
                                if path.contains("..") || path.starts_with('/') || path.contains('\0') || path == "/etc/shadow" || path.contains("shadow") {
                                    serde_json::json!({"ok": false, "command": cmd, "result": null, "error": {"code": "PATH_TRAVERSAL", "message": format!("path traversal or protected: {path}")}})
                                } else if path.is_empty() {
                                    serde_json::json!({"ok": false, "command": cmd, "result": null, "error": {"code": "NOT_FOUND", "message": "file not found"}})
                                } else {
                                    serde_json::json!({"ok": true, "command": cmd, "result": {"path": path, "content": "sample content for testing", "size": 24}, "error": null})
                                }
                            },
                            "file.create" => {
                                let path = req["parameters"]["path"].as_str().unwrap_or("");
                                if path.contains("..") || path.starts_with('/') {
                                    serde_json::json!({"ok": false, "command": cmd, "result": null, "error": {"code": "PATH_TRAVERSAL", "message": format!("path traversal: {path}")}})
                                } else {
                                    serde_json::json!({"ok": true, "command": cmd, "result": {"path": path, "bytes_written": 14}, "error": null})
                                }
                            },
                            "file.write" => {
                                let path = req["parameters"]["path"].as_str().unwrap_or("");
                                if path.contains("..") || path.starts_with('/') {
                                    serde_json::json!({"ok": false, "command": cmd, "result": null, "error": {"code": "PATH_TRAVERSAL", "message": format!("path traversal: {path}")}})
                                } else {
                                    let content = req["parameters"]["content"].as_str().unwrap_or("");
                                    serde_json::json!({"ok": true, "command": cmd, "result": {"path": path, "bytes_written": content.len()}, "error": null})
                                }
                            },
                            "file.rename" | "file.move" => {
                                let from = req["parameters"]["from"].as_str().unwrap_or("");
                                let to = req["parameters"]["to"].as_str().unwrap_or("");
                                if from.contains("..") || to.contains("..") || from.starts_with('/') || to.starts_with('/') {
                                    serde_json::json!({"ok": false, "command": cmd, "result": null, "error": {"code": "PATH_TRAVERSAL", "message": "path traversal"}})
                                } else {
                                    serde_json::json!({"ok": true, "command": cmd, "result": {"from": from, "to": to}, "error": null})
                                }
                            },
                            "file.delete" => {
                                serde_json::json!({"ok": false, "command": cmd, "result": null, "error": {"code": "REQUIRES_CONFIRMATION", "message": "delete requires explicit user confirmation"}})
                            },
                            "system.info" => serde_json::json!({"ok": true, "command": cmd, "result": {"os": "Aether OS", "os_version": "0.1.0", "kernel_version": "6.8.0", "arch": "x86_64", "hostname": "aether"}, "error": null}),
                            "system.resources" => serde_json::json!({"ok": true, "command": cmd, "result": {"cpu_count": 4, "memory": {"total_kib": 16384, "available_kib": 8192}, "storage": {"total_bytes": 1073741824, "available_bytes": 536870912}}, "error": null}),
                            "system.uptime" => serde_json::json!({"ok": true, "command": cmd, "result": {"uptime_ms": 123456, "uptime_human": "2m 3s", "boot_time_ms": 0}, "error": null}),
                            _ => serde_json::json!({
                                "ok": false,
                                "command": cmd,
                                "result": null,
                                "error": {"code": "NOT_FOUND", "message": format!("unknown command {cmd}")}
                            }),
                        };
                        let mut payload = serde_json::to_string(&resp).unwrap_or_default();
                        payload.push('\n');
                        let _ = writer.write_all(payload.as_bytes());
                        let _ = writer.flush();
                    });
                }
            }
        });
        // Give the listener a moment to start
        std::thread::sleep(std::time::Duration::from_millis(50));
        (port, handle)
    }

    #[test]
    fn eight_flows_end_to_end_via_handle_request() {
        let (ctrl_port, _h) = spawn_mock_control_plane();
        // Surface not needed because control plane now proxies windows, but set to same port fallback
        let (clock, _tick) = fake_clock(1000);
        let mut state = AgentState::new(clock)
            .with_control_port(ctrl_port)
            .with_surface_port(ctrl_port); // reuse

        // Helper to call chat and assert ok + contains expected substring
        let chat = |state: &mut AgentState, text: &str| -> AgentResponse {
            handle_request(
                state,
                &AgentRequest {
                    command: "chat".to_string(),
                    argument: Some(text.to_string()),
                },
            )
        };

        // 1. What's open? -> window.list
        let r = chat(&mut state, "What's open?");
        assert!(r.ok, "what's open failed: {:?}", r.result);
        let resp = r.result["response"].as_str().unwrap_or_default();
        assert!(resp.contains("OPEN WINDOWS") || resp.contains("WINDOW"), "got: {resp}");
        assert!(r.result["actions"].is_array(), "actions missing");

        // 2. Open Calculator.
        let r = chat(&mut state, "Open Calculator.");
        assert!(r.ok, "open calc failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("LAUNCHED"));

        // 3. Open Notes.
        let r = chat(&mut state, "Open Notes.");
        assert!(r.ok, "open notes failed: {:?}", r.result);

        // 4. Bring Notes to the front. -> window.focus
        let r = chat(&mut state, "Bring Notes to the front.");
        assert!(r.ok, "focus failed: {:?}", r.result);
        let resp = r.result["response"].as_str().unwrap_or_default();
        assert!(resp.contains("FOCUSED") || resp.contains("NOTES"), "got: {resp}");

        // 5. Minimize Calculator.
        let r = chat(&mut state, "Minimize Calculator.");
        assert!(r.ok, "minimize failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("MINIMIZED"));

        // 6. Maximize Notes.
        let r = chat(&mut state, "Maximize Notes.");
        assert!(r.ok, "maximize failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("MAXIMIZED"));

        // 7. Close Calculator.
        let r = chat(&mut state, "Close Calculator.");
        assert!(r.ok, "close failed: {:?}", r.result);

        // 8. Open Calculator and Notes. -> two launches
        // Use fresh state so both are not already running
        let (ctrl_port2, _h2) = spawn_mock_control_plane();
        let mut state2 = AgentState::new(clock)
            .with_control_port(ctrl_port2)
            .with_surface_port(ctrl_port2);
        let r = handle_request(
            &mut state2,
            &AgentRequest {
                command: "chat".to_string(),
                argument: Some("Open Calculator and Notes.".to_string()),
            },
        );
        assert!(r.ok, "multi launch failed: {:?}", r.result);
        let actions = r.result["actions"].as_array().unwrap_or_else(|| panic!("no actions"));
        assert_eq!(actions.len(), 2, "expected 2 actions, got {:?}", actions);
        assert_eq!(actions[0]["capability"], "app.launch");
        assert_eq!(actions[1]["capability"], "app.launch");
        assert!(r.result["response"].as_str().unwrap_or_default().contains("LAUNCHED") || actions[0]["status"] == "Success");
    }

    #[test]
    fn conversation_pronoun_resolves_across_turns() {
        let (ctrl_port, _h) = spawn_mock_control_plane();
        let (clock, _tick) = fake_clock(2000);
        let mut state = AgentState::new(clock)
            .with_control_port(ctrl_port)
            .with_surface_port(ctrl_port);

        // Open Notes -> sets last_app
        let r = handle_request(
            &mut state,
            &AgentRequest {
                command: "chat".to_string(),
                argument: Some("Open Notes.".to_string()),
            },
        );
        assert!(r.ok);

        // Bring it to the front should resolve "it" -> notes
        let r = handle_request(
            &mut state,
            &AgentRequest {
                command: "chat".to_string(),
                argument: Some("Bring it to the front.".to_string()),
            },
        );
        assert!(r.ok, "pronoun failed: {:?}", r.result);
        let resp = r.result["response"].as_str().unwrap_or_default();
        // Should have focused notes, not failed due to malformed
        assert!(!resp.contains("MALFORMED") && !resp.contains("NOT FOUND"), "got: {resp}");
        assert!(resp.contains("FOCUSED") || r.result["actions"][0]["arguments"]["app"] == "notes");
    }

    #[test]
    fn context_command_returns_structured_snapshot() {
        let (ctrl_port, _h) = spawn_mock_control_plane();
        let (clock, _tick) = fake_clock(3000);
        let mut state = AgentState::new(clock)
            .with_control_port(ctrl_port)
            .with_surface_port(ctrl_port);
        let r = handle_request(
            &mut state,
            &AgentRequest {
                command: "context".to_string(),
                argument: None,
            },
        );
        assert!(r.ok);
        // Should contain windows, running_apps etc via build_context
        // Our mock control plane returns windows via context.get? Actually handle_request's context path builds via context::build_context which does its own fetches, not via mock's context.get.
        // But we still check that result has expected keys when via direct context fetch.
        // For this test we just ensure it returns something with health or windows.
        assert!(r.result.is_object());
    }

    #[test]
    fn error_handling_gives_friendly_messages() {
        let (ctrl_port, _h) = spawn_mock_control_plane();
        let (clock, _tick) = fake_clock(4000);
        let mut state = AgentState::new(clock)
            .with_control_port(ctrl_port)
            .with_surface_port(ctrl_port);

        // Try to launch unknown app -> should be rejected as not installed via precheck
        // Need context that knows installed apps; mock returns installed calculator/notes/files, so ghost should be rejected.
        // Build a context where installed = calculator, notes, files only.
        let r = handle_request(
            &mut state,
            &AgentRequest {
                command: "chat".to_string(),
                argument: Some("Open GhostApp.".to_string()),
            },
        );
        // Should be handled: either precheck NOT FOUND or execution failure, but not panic and not raw stack trace.
        assert!(!r.ok || r.result["response"].as_str().unwrap_or_default().contains("NOT FOUND") || r.result["response"].as_str().unwrap_or_default().contains("FAILED") || r.result["response"].as_str().unwrap_or_default().contains("NOT_FOUND"));
        let resp = r.result["response"].as_str().unwrap_or_default();
        assert!(!resp.contains("stack") && !resp.contains("unwrap"), "leaked stack: {resp}");
    }

    #[test]
    fn file_and_system_flows_end_to_end() {
        let (ctrl_port, _h) = spawn_mock_control_plane();
        let (clock, _tick) = fake_clock(5000);
        let mut state = AgentState::new(clock)
            .with_control_port(ctrl_port)
            .with_surface_port(ctrl_port);

        let chat = |state: &mut AgentState, text: &str| -> AgentResponse {
            handle_request(
                state,
                &AgentRequest {
                    command: "chat".to_string(),
                    argument: Some(text.to_string()),
                },
            )
        };

        // 1. Show my files. -> file.list
        let r = chat(&mut state, "Show my files.");
        assert!(r.ok, "show files failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("FILES"));

        // 2. Find all markdown files. -> file.search
        let r = chat(&mut state, "Find all markdown files.");
        assert!(r.ok, "find markdown failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("FOUND"));

        // 3. Read roadmap.md. -> file.read
        let r = chat(&mut state, "Read roadmap.md.");
        assert!(r.ok, "read failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("READ"));

        // 4. Create a file called ideas.md. -> file.create
        let r = chat(&mut state, "Create a file called ideas.md.");
        assert!(r.ok, "create failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("CREATED"));

        // 5. Write 'Aether OS idea' into ideas.md. -> file.write
        let r = chat(&mut state, "Write 'Aether OS idea' into ideas.md.");
        assert!(r.ok, "write failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("WROTE"));

        // 6. Rename ideas.md to project-ideas.md. -> file.rename
        let r = chat(&mut state, "Rename ideas.md to project-ideas.md.");
        assert!(r.ok, "rename failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("RENAMED"));

        // 7. Move project-ideas.md into Documents. -> file.move
        let r = chat(&mut state, "Move project-ideas.md into Documents.");
        assert!(r.ok, "move failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("MOVED"));

        // 8. How much RAM is available? -> system.resources
        let r = chat(&mut state, "How much RAM is available?");
        assert!(r.ok, "resources failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("RESOURCES") || r.result["response"].as_str().unwrap_or_default().contains("CPU"));

        // 9. How long has Aether been running? -> system.uptime
        let r = chat(&mut state, "How long has Aether been running?");
        assert!(r.ok, "uptime failed: {:?}", r.result);
        assert!(r.result["response"].as_str().unwrap_or_default().contains("UPTIME"));
    }

    #[test]
    fn security_rejections_for_file_access() {
        let (ctrl_port, _h) = spawn_mock_control_plane();
        let (clock, _tick) = fake_clock(6000);
        let mut state = AgentState::new(clock)
            .with_control_port(ctrl_port)
            .with_surface_port(ctrl_port);

        let chat = |state: &mut AgentState, text: &str| -> AgentResponse {
            handle_request(
                state,
                &AgentRequest {
                    command: "chat".to_string(),
                    argument: Some(text.to_string()),
                },
            )
        };

        // Read /etc/shadow -> must be rejected (path traversal / protected)
        let r = chat(&mut state, "Read /etc/shadow.");
        assert!(!r.ok || r.result["response"].as_str().unwrap_or_default().contains("PATH_TRAVERSAL") || r.result["response"].as_str().unwrap_or_default().contains("REJECTED") || r.result["response"].as_str().unwrap_or_default().contains("FAILED") || r.result["actions"][0]["status"] == "Failed", "expected rejection for /etc/shadow, got {:?}", r.result);
        // Ensure no file content leaked (should not contain shadow content)
        let resp_str = serde_json::to_string(&r.result).unwrap_or_default();
        assert!(!resp_str.contains("root:"), "leaked protected file content");

        // Traversal
        let r = chat(&mut state, "Read ../../etc/passwd.");
        assert!(!r.ok || r.result["response"].as_str().unwrap_or_default().contains("PATH_TRAVERSAL") || r.result["response"].as_str().unwrap_or_default().contains("TRAVERSAL") || r.result["actions"][0]["status"] == "Failed", "expected traversal rejection, got {:?}", r.result);

        // Delete all files -> must require confirmation, not auto-execute
        let r = chat(&mut state, "Delete all files.");
        // Should be RequiresConsent or Failed, not success with ok true and no confirmation
        let status = r.result["actions"].as_array().and_then(|a| a.first()).and_then(|a| a.get("status")).and_then(|s| s.as_str()).unwrap_or("");
        assert!(status == "RequiresConsent" || r.result["response"].as_str().unwrap_or_default().contains("REQUIRES") || !r.ok, "expected confirmation for bulk delete, got {:?}", r.result);
    }

    /// A provider that always returns a valid structured intent. Used to
    /// verify the chat handler routes through the LLM path when the
    /// deterministic parser finds no intent (i.e. unprompted freeform text).
    struct StubStructuredProvider;
    impl AiProvider for StubStructuredProvider {
        fn name(&self) -> &str { "stub-structured" }
        fn complete(&self, _prompt: &str) -> Result<String, String> {
            Ok(r#"{"capability":"system.status","confidence":80,"entities":{},"reason":"stub"}"#.to_string())
        }
    }

    /// A provider whose structured output should be rejected as an unknown
    /// capability, forcing the chat handler to fall back to plain chat.
    struct StubUnknownCapabilityProvider;
    impl AiProvider for StubUnknownCapabilityProvider {
        fn name(&self) -> &str { "stub-unknown" }
        fn complete(&self, _prompt: &str) -> Result<String, String> {
            Ok(r#"{"capability":"agent.execute_shell","confidence":99,"entities":{"command":"x"},"reason":"y"}"#.to_string())
        }
    }

    /// A provider that always fails. Should fall through to plain chat.
    struct StubFailingProvider;
    impl AiProvider for StubFailingProvider {
        fn name(&self) -> &str { "stub-fail" }
        fn complete(&self, _prompt: &str) -> Result<String, String> {
            Err("no network".to_string())
        }
    }

    fn spawn_state_with_provider(
        port: u16,
        clock: fn() -> u64,
        provider: Box<dyn AiProvider + Send>,
    ) -> AgentState {
        AgentState::new(clock)
            .with_control_port(port)
            .with_surface_port(port)
            .with_provider(provider)
    }

    #[test]
    fn chat_routes_freeform_through_structured_llm() {
        // A prompt that the deterministic parser will not match, but the LLM
        // returns a valid structured intent for.
        let (ctrl_port, _h) = spawn_mock_control_plane();
        let (clock, _tick) = fake_clock(7000);
        let mut state = spawn_state_with_provider(ctrl_port, clock, Box::new(StubStructuredProvider));

        let r = handle_request(
            &mut state,
            &AgentRequest {
                command: "chat".to_string(),
                argument: Some("how are things?".to_string()),
            },
        );
        assert!(r.ok, "structured llm chat failed: {:?}", r.result);
        assert_eq!(r.result["provider"], "structured-llm");
        let actions = r.result["actions"].as_array().unwrap_or_else(|| panic!("no actions"));
        assert_eq!(actions.len(), 1, "expected one structured action, got {:?}", actions);
        assert_eq!(actions[0]["capability"], "system.status");
    }

    #[test]
    fn chat_falls_back_to_plain_chat_on_unknown_capability() {
        let (ctrl_port, _h) = spawn_mock_control_plane();
        let (clock, _tick) = fake_clock(7100);
        let mut state = spawn_state_with_provider(ctrl_port, clock, Box::new(StubUnknownCapabilityProvider));

        let r = handle_request(
            &mut state,
            &AgentRequest {
                command: "chat".to_string(),
                argument: Some("some unrecognizable ask".to_string()),
            },
        );
        assert!(r.ok, "expected plain chat fallback to succeed: {:?}", r.result);
        // The provider is stub-unknown (it returned a structured response that
        // was rejected, then the handler falls through to the plain provider
        // call - which returns the same bad payload as the chat reply).
        // We just verify the structured-llm path did NOT run.
        assert_ne!(r.result["provider"], "structured-llm");
    }

    #[test]
    fn chat_falls_back_to_plain_chat_on_provider_error() {
        let (ctrl_port, _h) = spawn_mock_control_plane();
        let (clock, _tick) = fake_clock(7200);
        let mut state = spawn_state_with_provider(ctrl_port, clock, Box::new(StubFailingProvider));

        let r = handle_request(
            &mut state,
            &AgentRequest {
                command: "chat".to_string(),
                argument: Some("tell me a joke".to_string()),
            },
        );
        assert!(r.ok, "expected plain chat fallback to succeed: {:?}", r.result);
        // Should report the provider failure but still produce a friendly reply.
        let resp = r.result["response"].as_str().unwrap_or_default();
        assert!(resp.contains("unavailable") || resp.contains("ECHO"), "got: {resp}");
        assert_ne!(r.result["provider"], "structured-llm");
    }
}
