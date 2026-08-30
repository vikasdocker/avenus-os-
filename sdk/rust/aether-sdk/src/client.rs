// Aether SDK - TCP control-plane client.

use crate::{IpcRequest, IpcResponse};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Client bound to a control-plane endpoint.
pub struct AetherClient {
    addr: String,
    timeout: Duration,
}

impl AetherClient {
    /// Creates a client for `host:port` with a per-request timeout.
    pub fn new(addr: impl Into<String>, timeout: Duration) -> Self {
        Self { addr: addr.into(), timeout }
    }

    pub fn endpoint(&self) -> &str {
        &self.addr
    }

    /// Sends one request and reads exactly one JSON-line response.
    pub fn request(&self, req: &IpcRequest) -> Result<IpcResponse, String> {
        let stream =
            TcpStream::connect(&self.addr).map_err(|e| format!("connect {}: {e}", self.addr))?;
        stream.set_read_timeout(Some(self.timeout)).map_err(|e| format!("set timeout: {e}"))?;
        let mut writer = stream.try_clone().map_err(|e| format!("clone stream: {e}"))?;
        let mut payload = serde_json::to_string(req).map_err(|e| format!("encode: {e}"))?;
        payload.push('\n');
        writer.write_all(payload.as_bytes()).map_err(|e| format!("send: {e}"))?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| format!("recv: {e}"))?;
        if line.trim().is_empty() {
            return Err("empty response from control plane".to_string());
        }
        serde_json::from_str::<IpcResponse>(line.trim())
            .map_err(|e| format!("decode response: {e}"))
    }

    /// Convenience: `status` command.
    pub fn status(&self) -> Result<IpcResponse, String> {
        self.request(&IpcRequest {
            service_id: "aether-system-core".to_string(),
            command: "status".to_string(),
            parameters: serde_json::json!({}),
        })
    }

    /// Convenience: lifecycle commands `start|stop|restart` on a service.
    pub fn service_control(&self, action: &str, service_id: &str) -> Result<IpcResponse, String> {
        self.request(&IpcRequest {
            service_id: "aether-system-core".to_string(),
            command: action.to_string(),
            parameters: serde_json::json!({ "service": service_id }),
        })
    }

    /// Convenience: graceful shutdown of the whole system.
    pub fn shutdown(&self) -> Result<IpcResponse, String> {
        self.request(&IpcRequest {
            service_id: "aether-system-core".to_string(),
            command: "shutdown".to_string(),
            parameters: serde_json::json!({}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IpcError;
    use std::io::Read;

    #[test]
    fn round_trip_against_mock_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("{e}"));
        let port = listener.local_addr().unwrap_or_else(|e| panic!("{e}")).port();

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap_or_else(|e| panic!("{e}"));
            let mut stream = stream;
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap_or(0);
            let text = String::from_utf8_lossy(&buf[..n]).to_string();
            assert!(text.contains("\"status\""));
            let reply = IpcResponse::ok("status", serde_json::json!({ "health": "HEALTHY" }));
            let mut out = serde_json::to_string(&reply).unwrap_or_default();
            out.push('\n');
            let _ = std::io::Write::write_all(&mut stream, out.as_bytes());
        });

        let client = AetherClient::new(format!("127.0.0.1:{port}"), Duration::from_secs(2));
        let res = client.status().unwrap_or_else(|e| panic!("{e}"));
        assert!(res.ok);
        assert_eq!(res.command, "status");
        server.join().unwrap_or(());
    }

    #[test]
    fn connect_failure_is_reported() {
        // Port 1 on loopback is never our mock server.
        let client = AetherClient::new("127.0.0.1:1", Duration::from_millis(300));
        assert!(client.status().is_err());
    }

    #[test]
    fn error_response_shape_round_trips() {
        let err =
            IpcError { code: "NOT_FOUND".to_string(), message: "no such service".to_string() };
        let res = IpcResponse::err("start", err);
        assert!(!res.ok);
        assert_eq!(res.error.map(|e| e.code).as_deref(), Some("NOT_FOUND"));
    }
}
