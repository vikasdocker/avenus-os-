// Agent commands: agent status, sessions, inspect, intent
//
// Routes every subcommand through the aether-agentd TCP service
// (port 4748 by default). The shell is a thin client; all state,
// capability checks, audit, and execution live inside agentd.
use anyhow::Result;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::json;

use crate::agentd_client::{AgentRequest, AgentdClient};
use crate::command::{Command, CommandMetadata};
use crate::history::ShellHistory;
use crate::output::OutputFormatter;
use crate::session::ShellSession;

static AGENT_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "agent".to_string(),
    description: "Agent runtime management (talks to aether-agentd)".to_string(),
    usage: "agent <status|sessions|inspect|intent|cancel|approve|deny|progress|audit> [args]"
        .to_string(),
    required_capability: Some("agent.read".to_string()),
    risk_level: "low".to_string(),
    requires_confirmation: false,
});

pub struct AgentCommand;

fn client() -> AgentdClient {
    AgentdClient::from_env()
}

#[async_trait]
impl Command for AgentCommand {
    fn metadata(&self) -> &CommandMetadata {
        &AGENT_METADATA
    }

    async fn execute(
        &self,
        args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        if args.is_empty() {
            return Err(anyhow::anyhow!(
                "agent requires subcommand: status, sessions, inspect, intent, cancel, approve, deny, progress, audit"
            ));
        }

        let c = client();
        match args[0] {
            "status" => {
                let resp = c
                    .call(&AgentRequest::new("agent.status"))
                    .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                formatter.print(&format_agent_response("agent.status", &resp)?);
            }
            "sessions" => {
                let resp = c
                    .call(&AgentRequest::new("agent.session.list"))
                    .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                formatter.print(&format_agent_response("agent.session.list", &resp)?);
            }
            "inspect" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("agent inspect requires a session ID"));
                }
                let resp = c
                    .call(&AgentRequest::new("agent.session.status").with_argument(args[1]))
                    .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                formatter.print(&format_agent_response("agent.session.status", &resp)?);
            }
            "intent" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!(
                        "agent intent requires a JSON envelope as argument"
                    ));
                }
                let payload = args[1..].join(" ");
                let resp = c
                    .call(&AgentRequest::new("agent.intent").with_argument(payload))
                    .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                formatter.print(&format_agent_response("agent.intent", &resp)?);
            }
            "cancel" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("agent cancel requires a session ID"));
                }
                let resp = c
                    .call(&AgentRequest::new("agent.session.cancel").with_argument(args[1]))
                    .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                formatter.print(&format_agent_response("agent.session.cancel", &resp)?);
            }
            "audit" => {
                let count = args.get(1).copied().unwrap_or("20");
                let resp = c
                    .call(&AgentRequest::new("agent.audit.recent").with_argument(count))
                    .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                formatter.print(&format_agent_response("agent.audit.recent", &resp)?);
            }
            "approvals" => {
                let resp = c
                    .call(&AgentRequest::new("agent.approval.list"))
                    .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                formatter.print(&format_agent_response("agent.approval.list", &resp)?);
            }
            "approve" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("agent approve requires an approval_id"));
                }
                let resp = c
                    .call(&AgentRequest::new("agent.approval.grant").with_argument(args[1]))
                    .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                formatter.print(&format_agent_response("agent.approval.grant", &resp)?);
            }
            "deny" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!(
                        "agent deny requires an approval_id and optional reason: agent deny <id> [reason]"
                    ));
                }
                let approval_id = args[1];
                let reason =
                    if args.len() >= 3 { args[2..].join(" ") } else { "user denied".to_string() };
                let payload = json!({
                    "approval_id": approval_id,
                    "reason": reason,
                })
                .to_string();
                let resp = c
                    .call(&AgentRequest::new("agent.approval.deny").with_argument(&payload))
                    .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                formatter.print(&format_agent_response("agent.approval.deny", &resp)?);
            }
            "progress" => {
                let what = args.get(1).copied().unwrap_or("current");
                match what {
                    "current" => {
                        let resp = c
                            .call(&AgentRequest::new("agent.progress.current"))
                            .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                        formatter.print(&format_agent_response("agent.progress.current", &resp)?);
                    }
                    "history" => {
                        let limit = args.get(2).copied().unwrap_or("10");
                        let resp = c
                            .call(&AgentRequest::new("agent.progress.history").with_argument(limit))
                            .map_err(|e| anyhow::anyhow!("agentd unreachable: {e}"))?;
                        formatter.print(&format_agent_response("agent.progress.history", &resp)?);
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "agent progress: unknown subcommand '{what}'. Use: current, history [N]"
                        ));
                    }
                }
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unknown agent subcommand: {}. Use: status, sessions, inspect, intent, cancel, audit, approvals, approve, deny, progress",
                    args[0]
                ));
            }
        }

        Ok(())
    }
}

fn format_agent_response(
    command: &str,
    resp: &crate::agentd_client::AgentResponse,
) -> Result<String> {
    let payload = json!({
        "command": command,
        "ok": resp.ok,
        "result": resp.result,
    });
    serde_json::to_string_pretty(&payload).map_err(|e| anyhow::anyhow!("encode: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;

    fn spawn_mock_agentd() -> (u16, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("{e}"));
        let port = listener.local_addr().unwrap_or_else(|e| panic!("{e}")).port();
        let handle = thread::spawn(move || {
            for stream in listener.incoming().flatten().take(20) {
                let mut writer = stream.try_clone().unwrap_or_else(|e| panic!("{e}"));
                let mut reader = std::io::BufReader::new(stream);
                let mut line = String::new();
                let _ = reader.read_line(&mut line);
                let req: AgentRequest = serde_json::from_str(line.trim())
                    .unwrap_or_else(|_| AgentRequest::new("unknown"));
                // Echo back a synthetic response for any request.
                let resp = crate::agentd_client::AgentResponse {
                    ok: true,
                    result: json!({
                        "command": req.command,
                        "argument": req.argument,
                        "mocked": true,
                    }),
                };
                let mut body = serde_json::to_string(&resp).unwrap_or_default();
                body.push('\n');
                let _ = writer.write_all(body.as_bytes());
                let _ = writer.flush();
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
        (port, handle)
    }

    fn set_port(port: u16) {
        // The client reads AETHER_AGENT_PORT at construction time.
        // SAFETY: shell tests are the only writer; tests run on a thread
        // and the env var is read before the next assertion.
        unsafe {
            std::env::set_var("AETHER_AGENT_PORT", port.to_string());
        }
    }

    #[test]
    fn shell_agent_status_proxies_to_agentd() {
        let (port, _h) = spawn_mock_agentd();
        set_port(port);
        let c = AgentdClient::from_env();
        let resp = c.call(&AgentRequest::new("agent.status")).unwrap_or_else(|e| panic!("{e}"));
        assert!(resp.ok);
        assert_eq!(resp.result["command"], "agent.status");
        assert_eq!(resp.result["mocked"], true);
        let _: Arc<()> = Arc::new(());
    }

    #[test]
    fn shell_agent_intent_round_trips_payload() {
        let (port, _h) = spawn_mock_agentd();
        set_port(port);
        let c = AgentdClient::from_env();
        let payload = json!({
            "session_id": "s1",
            "capability": "system.status",
            "arguments": {},
        })
        .to_string();
        let resp = c
            .call(&AgentRequest::new("agent.intent").with_argument(payload))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(resp.ok);
        assert_eq!(resp.result["command"], "agent.intent");
        // The argument should round-trip back.
        let arg = resp.result["argument"].as_str().unwrap_or_default();
        assert!(arg.contains("system.status"), "got: {arg}");
    }

    #[test]
    fn shell_agent_approval_list_round_trips() {
        let (port, _h) = spawn_mock_agentd();
        set_port(port);
        let c = AgentdClient::from_env();
        let resp =
            c.call(&AgentRequest::new("agent.approval.list")).unwrap_or_else(|e| panic!("{e}"));
        assert!(resp.ok);
        assert_eq!(resp.result["command"], "agent.approval.list");
    }

    #[test]
    fn shell_agent_approve_round_trips_id() {
        let (port, _h) = spawn_mock_agentd();
        set_port(port);
        let c = AgentdClient::from_env();
        let resp = c
            .call(&AgentRequest::new("agent.approval.grant").with_argument("approval-123"))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(resp.ok);
        assert_eq!(resp.result["command"], "agent.approval.grant");
        let arg = resp.result["argument"].as_str().unwrap_or_default();
        assert_eq!(arg, "approval-123");
    }

    #[test]
    fn shell_agent_deny_round_trips_payload() {
        let (port, _h) = spawn_mock_agentd();
        set_port(port);
        let c = AgentdClient::from_env();
        let payload = json!({
            "approval_id": "approval-456",
            "reason": "user said no",
        })
        .to_string();
        let resp = c
            .call(&AgentRequest::new("agent.approval.deny").with_argument(&payload))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(resp.ok);
        assert_eq!(resp.result["command"], "agent.approval.deny");
        let arg = resp.result["argument"].as_str().unwrap_or_default();
        assert!(arg.contains("approval-456"));
        assert!(arg.contains("user said no"));
    }

    #[test]
    fn shell_agent_progress_current_round_trips() {
        let (port, _h) = spawn_mock_agentd();
        set_port(port);
        let c = AgentdClient::from_env();
        let resp =
            c.call(&AgentRequest::new("agent.progress.current")).unwrap_or_else(|e| panic!("{e}"));
        assert!(resp.ok);
        assert_eq!(resp.result["command"], "agent.progress.current");
    }

    #[test]
    fn shell_agent_progress_history_round_trips_limit() {
        let (port, _h) = spawn_mock_agentd();
        set_port(port);
        let c = AgentdClient::from_env();
        let resp = c
            .call(&AgentRequest::new("agent.progress.history").with_argument("5"))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(resp.ok);
        assert_eq!(resp.result["command"], "agent.progress.history");
        let arg = resp.result["argument"].as_str().unwrap_or_default();
        assert_eq!(arg, "5");
    }
}
