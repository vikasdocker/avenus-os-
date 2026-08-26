// Filesystem commands: fs list, fs stat, fs search, fs storage, fs mounts
use async_trait::async_trait;
use serde_json::json;
use anyhow::Result;
use once_cell::sync::Lazy;

use crate::command::{Command, CommandMetadata};
use crate::history::ShellHistory;
use crate::output::OutputFormatter;
use crate::session::ShellSession;

static FS_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "fs".to_string(),
    description: "Filesystem operations".to_string(),
    usage: "fs <subcommand> [args]".to_string(),
    required_capability: Some("filesystem.read".to_string()),
    risk_level: "medium".to_string(),
    requires_confirmation: false,
});

pub struct FilesystemCommand;

#[async_trait]
impl Command for FilesystemCommand {
    fn metadata(&self) -> &CommandMetadata {
        &FS_METADATA
    }

    async fn execute(
        &self,
        args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("fs requires subcommand: list, stat, search, storage, mounts"));
        }

        match args[0] {
            "list" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("fs list requires a path"));
                }
                let path = args[1];
                let result = json!({
                    "command": "fs.list",
                    "path": path,
                    "entries": []
                });
                formatter.output(&result)?;
            }
            "stat" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("fs stat requires a path"));
                }
                let path = args[1];
                let result = json!({
                    "command": "fs.stat",
                    "path": path,
                });
                formatter.output(&result)?;
            }
            "search" => {
                if args.len() < 3 {
                    return Err(anyhow::anyhow!("fs search requires path and pattern"));
                }
                let path = args[1];
                let pattern = args[2];
                let result = json!({
                    "command": "fs.search",
                    "path": path,
                    "pattern": pattern,
                    "results": []
                });
                formatter.output(&result)?;
            }
            "storage" => {
                let result = json!({
                    "command": "fs.storage",
                });
                formatter.output(&result)?;
            }
            "mounts" => {
                let result = json!({
                    "command": "fs.mounts",
                    "mounts": []
                });
                formatter.output(&result)?;
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown fs subcommand: {}", args[0]));
            }
        }

        Ok(())
    }
}
