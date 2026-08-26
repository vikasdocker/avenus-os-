// Network commands: network status, interfaces, inspect, addresses, routes, dns, connectivity, stats
use async_trait::async_trait;
use serde_json::json;
use anyhow::Result;
use once_cell::sync::Lazy;

use crate::command::{Command, CommandMetadata};
use crate::history::ShellHistory;
use crate::output::OutputFormatter;
use crate::session::ShellSession;

static NETWORK_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "network".to_string(),
    description: "Network management".to_string(),
    usage: "network <subcommand> [args]".to_string(),
    required_capability: Some("network.read".to_string()),
    risk_level: "medium".to_string(),
    requires_confirmation: false,
});

pub struct NetworkCommand;

#[async_trait]
impl Command for NetworkCommand {
    fn metadata(&self) -> &CommandMetadata {
        &NETWORK_METADATA
    }

    async fn execute(
        &self,
        args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("network requires subcommand: status, interfaces, inspect, addresses, routes, dns, connectivity, stats"));
        }

        match args[0] {
            "status" => {
                let result = json!({
                    "command": "network.status",
                });
                formatter.output(&result)?;
            }
            "interfaces" => {
                let result = json!({
                    "command": "network.interfaces",
                    "interfaces": []
                });
                formatter.output(&result)?;
            }
            "inspect" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("network inspect requires an interface name"));
                }
                let iface = args[1];
                let result = json!({
                    "command": "network.inspect",
                    "interface": iface,
                });
                formatter.output(&result)?;
            }
            "addresses" => {
                let result = json!({
                    "command": "network.addresses",
                    "addresses": []
                });
                formatter.output(&result)?;
            }
            "routes" => {
                let result = json!({
                    "command": "network.routes",
                    "routes": []
                });
                formatter.output(&result)?;
            }
            "dns" => {
                let result = json!({
                    "command": "network.dns",
                });
                formatter.output(&result)?;
            }
            "connectivity" => {
                let result = json!({
                    "command": "network.connectivity",
                });
                formatter.output(&result)?;
            }
            "stats" => {
                let result = json!({
                    "command": "network.stats",
                });
                formatter.output(&result)?;
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown network subcommand: {}", args[0]));
            }
        }

        Ok(())
    }
}
