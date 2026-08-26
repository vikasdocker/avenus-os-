// Aether Agent Daemon - REPL-style daemon entry point.
//
// Reads newline-delimited JSON requests on stdin and writes JSON
// responses on stdout, making the agent drivable from scripts, tests,
// and (later) the AI brain over any transport.

use std::io::{BufRead, Write};

fn main() {
    let mut state = aether_agentd::AgentState::new(aether_agentd::system_time_ms);
    state.record_event(
        aether_agentd::EventSeverity::Info,
        "agentd",
        "agent started",
    );
    eprintln!(
        "[agentd] ready id={} protocol=ndjson stdin->stdout",
        state.agent_id()
    );

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<aether_agentd::AgentRequest>(&line) {
            Ok(req) => aether_agentd::handle_request(&mut state, &req),
            Err(e) => aether_agentd::AgentResponse {
                ok: false,
                result: serde_json::json!({ "error": format!("bad request: {e}") }),
            },
        };
        let mut payload =
            serde_json::to_string(&response).unwrap_or_else(|_| "{\"ok\":false}".to_string());
        payload.push('\n');
        let mut out = stdout.lock();
        if out.write_all(payload.as_bytes()).is_err() || out.flush().is_err() {
            break;
        }
    }

    state.record_event(
        aether_agentd::EventSeverity::Info,
        "agentd",
        "agent stopping",
    );
    eprintln!("[agentd] exited cleanly");
}
