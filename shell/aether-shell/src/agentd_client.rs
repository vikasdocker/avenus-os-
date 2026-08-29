// Aether Shell - thin TCP client for the agentd daemon.
//
// Sends newline-delimited JSON `AgentRequest` payloads and reads one
// `AgentResponse` reply. The agentd service is reachable at
// `127.0.0.1:${AETHER_AGENT_PORT:-4748}` by default. Any
// connection / parse failure is surfaced as a structured error so the
// shell can render a useful message instead of crashing.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Mirrors `aether_agentd::AgentRequest` so the shell does not have
/// to pull aether-agentd as a library dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument: Option<String>,
}

impl AgentRequest {
    pub fn new(command: impl Into<String>) -> Self {
        Self { command: command.into(), argument: None }
    }

    pub fn with_argument(mut self, argument: impl Into<String>) -> Self {
        self.argument = Some(argument.into());
        self
    }
}

/// Mirrors `aether_agentd::AgentResponse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub ok: bool,
    pub result: Value,
}

/// A bounded client. Opens a fresh TCP connection per call so
/// short-lived shell REPL lines never block on a stuck server.
#[derive(Debug, Clone)]
pub struct AgentdClient {
    addr: String,
    timeout: Duration,
}

impl AgentdClient {
    /// Reads `AETHER_AGENT_PORT` (default 4748) and `AETHER_BIND`
    /// (default `127.0.0.1`) from the environment.
    pub fn from_env() -> Self {
        let port: u16 =
            std::env::var("AETHER_AGENT_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(4748);
        let bind = std::env::var("AETHER_BIND").unwrap_or_else(|_| "127.0.0.1".to_string());
        Self { addr: format!("{bind}:{port}"), timeout: Duration::from_secs(2) }
    }

    pub fn with_timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    pub fn address(&self) -> &str {
        &self.addr
    }

    /// Sends a single request and returns the response. Returns a
    /// structured error on connection or parse failure.
    pub fn call(&self, req: &AgentRequest) -> Result<AgentResponse, String> {
        let mut stream =
            TcpStream::connect(&self.addr).map_err(|e| format!("connect {}: {e}", self.addr))?;
        stream.set_read_timeout(Some(self.timeout)).map_err(|e| format!("read timeout: {e}"))?;
        stream.set_write_timeout(Some(self.timeout)).map_err(|e| format!("write timeout: {e}"))?;
        let body = serde_json::to_string(req).map_err(|e| format!("encode request: {e}"))?;
        stream.write_all(body.as_bytes()).map_err(|e| format!("send: {e}"))?;
        stream.write_all(b"\n").map_err(|e| format!("send nl: {e}"))?;
        stream.flush().map_err(|e| format!("flush: {e}"))?;
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).map_err(|e| format!("recv: {e}"))?;
        if buf.is_empty() {
            return Err("empty response from agentd".to_string());
        }
        let line = String::from_utf8_lossy(&buf)
            .lines()
            .next()
            .map(str::to_string)
            .ok_or_else(|| "no response line".to_string())?;
        serde_json::from_str::<AgentResponse>(&line)
            .map_err(|e| format!("parse agentd response: {e} (raw: {line})"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::sync::{Arc, Mutex};
    use std::thread;

    fn spawn_mock_agentd() -> (u16, thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("{e}"));
        let port = listener.local_addr().unwrap_or_else(|e| panic!("{e}")).port();
        let handle = thread::spawn(move || {
            for stream in listener.incoming().flatten().take(4) {
                let mut writer = stream.try_clone().unwrap_or_else(|e| panic!("{e}"));
                let mut reader = std::io::BufReader::new(stream);
                let mut line = String::new();
                let _ = reader.read_line(&mut line);
                let req: AgentRequest = serde_json::from_str(line.trim())
                    .unwrap_or_else(|_| AgentRequest::new("status"));
                let resp = AgentResponse {
                    ok: true,
                    result: serde_json::json!({
                        "command": req.command,
                        "echo_argument": req.argument,
                        "mocked": true,
                    }),
                };
                let mut body = serde_json::to_string(&resp).unwrap_or_default();
                body.push('\n');
                let _ = writer.write_all(body.as_bytes());
                let _ = writer.flush();
            }
        });
        std::thread::sleep(Duration::from_millis(20));
        (port, handle)
    }

    #[test]
    fn client_round_trips() {
        let (port, _h) = spawn_mock_agentd();
        let client =
            AgentdClient { addr: format!("127.0.0.1:{port}"), timeout: Duration::from_secs(2) };
        let req = AgentRequest::new("agent.status").with_argument("alice");
        let resp = client.call(&req).unwrap_or_else(|e| panic!("{e}"));
        assert!(resp.ok);
        assert_eq!(resp.result["command"], "agent.status");
        assert_eq!(resp.result["echo_argument"], "alice");
    }

    #[test]
    fn client_reports_connect_failure() {
        let client =
            AgentdClient { addr: "127.0.0.1:1".to_string(), timeout: Duration::from_millis(200) };
        let req = AgentRequest::new("agent.status");
        match client.call(&req) {
            Ok(resp) => panic!("should have failed, got {resp:?}"),
            Err(e) => assert!(e.contains("connect") || e.contains("Connection"), "got: {e}"),
        }
    }

    /// Suppresses a noisy clippy lint about unused Arc<Mutex<..>>.
    #[allow(dead_code)]
    fn _arc_mutex_used_in_other_tests() {
        let _x: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    }
}
