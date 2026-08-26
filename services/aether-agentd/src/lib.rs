// Aether Agent Daemon - the AI control-plane agent.
//
// Maintains a bounded, timestamped event ring and answers deterministic
// intent queries (status, health, events, tasks) for the local AI brain
// and interactive sessions. All state is in-memory and auditable.

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
}

impl AgentState {
    pub fn new(now_ms: fn() -> u64) -> Self {
        Self {
            agent_id: Uuid::new_v4(),
            events: VecDeque::with_capacity(EVENT_RING_CAPACITY),
            tasks: Vec::new(),
            now_ms,
        }
    }

    pub fn agent_id(&self) -> Uuid {
        self.agent_id
    }

    /// Records an event, evicting the oldest when full.
    pub fn record_event(&mut self, severity: EventSeverity, source: &str, message: &str) -> AgentEvent {
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
                state.record_event(EventSeverity::Info, "tasks", &format!("added '{}'", task.description));
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
        "task.done" => match request.argument.as_deref().and_then(|a| Uuid::parse_str(a).ok()) {
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
        // Deterministic monotonic clock for tests.
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
}
