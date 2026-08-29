// Aether Agent Daemon entry point.
//
// Default mode: TCP server on 127.0.0.1:${AETHER_AGENT_PORT:-4748} speaking
// newline-delimited JSON (AgentRequest -> AgentResponse) so graphical and
// remote surfaces reach the agent without touching Linux directly.
//
// `--stdio` keeps the original stdin/stdout REPL for scripts and tests.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

fn handle_line(state: &Arc<Mutex<aether_agentd::AgentState>>, line: &str) -> Option<String> {
    if line.trim().is_empty() {
        return None;
    }
    let response = match serde_json::from_str::<aether_agentd::AgentRequest>(line) {
        Ok(req) => {
            let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
            aether_agentd::handle_request(&mut guard, &req)
        }
        Err(e) => aether_agentd::AgentResponse {
            ok: false,
            result: serde_json::json!({ "error": format!("bad request: {e}") }),
        },
    };
    Some(
        serde_json::to_string(&response)
            .unwrap_or_else(|_| "{\"ok\":false}".to_string()),
    )
}

fn serve_tcp(state: Arc<Mutex<aether_agentd::AgentState>>) {
    let port: u16 = std::env::var("AETHER_AGENT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4748);
    let bind_addr =
        std::env::var("AETHER_BIND").unwrap_or_else(|_| "127.0.0.1".to_string());
    let listener = match TcpListener::bind((bind_addr.as_str(), port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[agentd] fatal: cannot bind agent port {port}: {e}");
            std::process::exit(1);
        }
    };
      {
          // Single lock scope: two locks in one eprintln! would self-deadlock
          // (the first guard lives until the end of the statement).
          let guard = state.lock().unwrap_or_else(|p| p.into_inner());
          eprintln!(
              "[agentd] ready id={} provider={} protocol=ndjson-tcp 127.0.0.1:{port}",
              guard.agent_id(),
              guard.provider_name(),
          );
      }

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let state = Arc::clone(&state);
        std::thread::spawn(move || {
            let Ok(write_half) = stream.try_clone() else {
                return;
            };
            let mut writer = write_half;
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else { break };
                if let Some(reply) = handle_line(&state, &line) {
                    let mut payload = reply;
                    payload.push('\n');
                    if writer.write_all(payload.as_bytes()).is_err()
                        || writer.flush().is_err()
                    {
                        break;
                    }
                }
            }
        });
    }
}

fn main() {
    let control_port = std::env::var("AETHER_CONTROL_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4747);
    let surface_port = std::env::var("AETHER_SURFACE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4750);
    let state = Arc::new(Mutex::new(
        aether_agentd::AgentState::new(aether_agentd::system_time_ms)
            .with_control_port(control_port)
            .with_surface_port(surface_port),
    ));

    if std::env::args().any(|a| a == "--stdio") {
        state.lock().unwrap_or_else(|p| p.into_inner()).record_event(
            aether_agentd::EventSeverity::Info,
            "agentd",
            "agent started (stdio)",
        );
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let Some(reply) = handle_line(&state, &line) else { continue };
            let mut payload = reply;
            payload.push('\n');
            let mut out = stdout.lock();
            if out.write_all(payload.as_bytes()).is_err() || out.flush().is_err() {
                break;
            }
        }
        return;
    }

    serve_tcp(state);
}

