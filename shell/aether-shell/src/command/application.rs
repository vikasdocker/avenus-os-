// Application commands: app list, inspect, launch, close
use anyhow::Result;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::json;

use crate::command::{Command, CommandMetadata};
use crate::history::ShellHistory;
use crate::output::OutputFormatter;
use crate::session::ShellSession;

static APP_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "app".to_string(),
    description: "Application management".to_string(),
    usage: "app <subcommand> [args]".to_string(),
    required_capability: Some("application.read".to_string()),
    risk_level: "medium".to_string(),
    requires_confirmation: false,
});

pub struct ApplicationCommand;

#[async_trait]
impl Command for ApplicationCommand {
    fn metadata(&self) -> &CommandMetadata {
        &APP_METADATA
    }

    async fn execute(
        &self,
        args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("app requires subcommand: list, inspect, launch, close"));
        }

        match args[0] {
            "list" => {
                let result = json!({
                    "command": "application.list",
                    "applications": []
                });
                formatter.output(&result)?;
            }
            "inspect" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("app inspect requires an application ID"));
                }
                let app_id = args[1];
                let result = json!({
                    "command": "application.inspect",
                    "application_id": app_id,
                });
                formatter.output(&result)?;
            }
            "launch" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("app launch requires an application ID"));
                }
                let app_id = args[1];
                let result = json!({
                    "command": "application.launch",
                    "application_id": app_id,
                });
                formatter.output(&result)?;
            }
            "close" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("app close requires an application ID"));
                }
                let app_id = args[1];
                let result = json!({
                    "command": "application.close",
                    "application_id": app_id,
                });
                formatter.output(&result)?;
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown app subcommand: {}", args[0]));
            }
        }

        Ok(())
    }
}
