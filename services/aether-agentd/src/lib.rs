// Aether Agent Daemon - the AI control-plane agent.
//
// Maintains a bounded, timestamped event ring and answers deterministic
// intent queries (status, health, events, tasks) for the local AI brain
// and interactive sessions. All state is in-memory and auditable.

pub mod intent;

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
        }
    }

    pub fn with_control_port(mut self, port: u16) -> Self {
        self.control_port = port;
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

                // Structured intent first; plain AI chat only when the text
                // carries no capability request.
                if let Some(intent) = intent::parse_intent(text) {
                    let capability = intent.capability;
                    let reply = match intent::validate(&intent) {
                        Err(rejection) => {
                            state.record_event(
                                EventSeverity::Warning,
                                "capability",
                                &format!("rejected: {rejection} ({})", capability.as_str()),
                            );
                            format!("REQUEST REJECTED - {rejection}")
                        }
                        Ok(()) => {
                            let client = intent::control_client(state.control_port);
                            match intent::execute(&intent, &client) {
                                Ok(result) => {
                                    let formatted =
                                        intent::format_result(capability, &result);
                                    state.record_event(
                                        EventSeverity::Info,
                                        "capability",
                                        &format!(
                                            "{} -> {}",
                                            capability.as_str(),
                                            formatted
                                        ),
                                    );
                                    formatted
                                }
                                Err(e) => {
                                    state.record_event(
                                        EventSeverity::Error,
                                        "capability",
                                        &format!("{} failed: {e}", capability.as_str()),
                                    );
                                    format!("ACTION FAILED - {e}")
                                }
                            }
                        }
                    };
                    return AgentResponse {
                        ok: true,
                        result: serde_json::json!({
                            "response": reply,
                            "capability": capability.as_str(),
                            "provider": "capability-layer",
                        }),
                    };
                }

                let outcome = state.provider.complete(text);
                let (provider, response) = match outcome {
                    Ok(reply) => (state.provider.name().to_string(), reply),
                    Err(e) => (
                        "fallback".to_string(),
                        format!("AI provider unavailable ({e}); echoing instead: ECHO: {text}"),
                    ),
                };
                state.record_event(EventSeverity::Info, "ai", &format!("reply via {provider}"));
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
}
