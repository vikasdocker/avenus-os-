// Agent commands: agent status, sessions, inspect
use async_trait::async_trait;
use serde_json::json;
use anyhow::Result;
use once_cell::sync::Lazy;

use crate::command::{Command, CommandMetadata};
use crate::history::ShellHistory;
use crate::output::OutputFormatter;
use crate::session::ShellSession;

static AGENT_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "agent".to_string(),
    description: "Agent runtime management".to_string(),
    usage: "agent <subcommand> [args]".to_string(),
    required_capability: Some("agent.read".to_string()),
    risk_level: "low".to_string(),
    requires_confirmation: false,
});

pub struct AgentCommand;

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
            return Err(anyhow::anyhow!("agent requires subcommand: status, sessions, inspect, submit, approve, deny, cancel"));
        }

        match args[0] {
            "status" => {
                let result = json!({
                    "command": "agent.status",
                    "runtime": "ready",
                    "active_sessions": 0,
                    "pending_approvals": 0,
                    "tools_registered": 14,
                });
                formatter.output(&result)?;
            }
            "sessions" => {
                let result = json!({
                    "command": "agent.sessions",
                    "sessions": [],
                    "total": 0,
                });
                formatter.output(&result)?;
            }
            "inspect" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("agent inspect requires a session ID"));
                }
                let session_id = args[1];
                let result = json!({
                    "command": "agent.inspect",
                    "session_id": session_id,
                    "status": "not_found",
                });
                formatter.output(&result)?;
            }
            "submit" => {
                if args.len() < 3 {
                    return Err(anyhow::anyhow!("agent submit requires <session_id> <request>"));
                }
                let session_id = args[1];
                let request = args[2..].join(" ");
                let result = json!({
                    "command": "agent.request.submit",
                    "session_id": session_id,
                    "request": request,
                    "status": "submitted",
                });
                formatter.output(&result)?;
            }
            "approve" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("agent approve requires an action ID"));
                }
                let action_id = args[1];
                let reason = if args.len() > 2 { args[2..].join(" ") } else { "approved".to_string() };
                let result = json!({
                    "command": "agent.action.approve",
                    "action_id": action_id,
                    "reason": reason,
                    "status": "approved",
                });
                formatter.output(&result)?;
            }
            "deny" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("agent deny requires an action ID"));
                }
                let action_id = args[1];
                let reason = if args.len() > 2 { args[2..].join(" ") } else { "denied".to_string() };
                let result = json!({
                    "command": "agent.action.deny",
                    "action_id": action_id,
                    "reason": reason,
                    "status": "denied",
                });
                formatter.output(&result)?;
            }
            "cancel" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("agent cancel requires a session ID"));
                }
                let session_id = args[1];
                let result = json!({
                    "command": "agent.session.cancel",
                    "session_id": session_id,
                    "status": "cancelled",
                });
                formatter.output(&result)?;
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown agent subcommand: {}. Use: status, sessions, inspect, submit, approve, deny, cancel", args[0]));
            }
        }

        Ok(())
    }
}
