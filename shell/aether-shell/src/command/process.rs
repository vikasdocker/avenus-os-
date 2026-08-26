// Process commands: process list, inspect, start, stop, restart
use async_trait::async_trait;
use serde_json::json;
use anyhow::Result;
use once_cell::sync::Lazy;

use crate::command::{Command, CommandMetadata};
use crate::history::ShellHistory;
use crate::output::OutputFormatter;
use crate::session::ShellSession;

static PROCESS_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "process".to_string(),
    description: "Process management".to_string(),
    usage: "process <subcommand> [args]".to_string(),
    required_capability: Some("process.read".to_string()),
    risk_level: "medium".to_string(),
    requires_confirmation: false,
});

pub struct ProcessCommand;

#[async_trait]
impl Command for ProcessCommand {
    fn metadata(&self) -> &CommandMetadata {
        &PROCESS_METADATA
    }

    async fn execute(
        &self,
        args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("process requires subcommand: list, inspect, start, stop, restart"));
        }

        match args[0] {
            "list" => {
                let result = json!({
                    "command": "process.list",
                    "processes": []
                });
                formatter.output(&result)?;
            }
            "inspect" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("process inspect requires a PID"));
                }
                let pid = args[1];
                let result = json!({
                    "command": "process.inspect",
                    "pid": pid,
                });
                formatter.output(&result)?;
            }
            "start" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("process start requires an executable"));
                }
                let executable = args[1];
                let result = json!({
                    "command": "process.start",
                    "executable": executable,
                });
                formatter.output(&result)?;
            }
            "stop" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("process stop requires a PID"));
                }
                let pid = args[1];
                let result = json!({
                    "command": "process.stop",
                    "pid": pid,
                });
                formatter.output(&result)?;
            }
            "restart" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("process restart requires a PID"));
                }
                let pid = args[1];
                let result = json!({
                    "command": "process.restart",
                    "pid": pid,
                });
                formatter.output(&result)?;
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown process subcommand: {}", args[0]));
            }
        }

        Ok(())
    }
}
