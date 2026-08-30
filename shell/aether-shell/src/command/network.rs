// Network commands: status, interfaces, inspect, addresses, routes, dns,
// connectivity, stats, events.
//
// Backed by the dedicated `aether-network` service crate. The shell
// asks the service for the answer and hands the JSON straight to the
// existing OutputFormatter — no new wire format.

use aether_network::NetworkManager;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::json;

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

/// Build a fresh manager for one command invocation. The default
/// backend is `auto`: ProcBackend on Linux when /proc/net is
/// reachable, else StubBackend.
fn fresh_manager() -> NetworkManager {
    let backend = aether_network::select_backend("auto");
    let mut m = NetworkManager::new_with_backend(backend);
    m.refresh();
    m
}

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
    ) -> anyhow::Result<()> {
        if args.is_empty() {
            return Err(anyhow::anyhow!(
                "network requires subcommand: status, interfaces, inspect, addresses, routes, dns, connectivity, stats, events"
            ));
        }

        let manager = fresh_manager();

        match args[0] {
            "status" => {
                let result = json!({
                    "command": "network.status",
                    "result": manager.status(),
                });
                formatter.output(&result)?;
            }
            "interfaces" => {
                let result = json!({
                    "command": "network.interfaces",
                    "result": manager.interfaces(),
                });
                formatter.output(&result)?;
            }
            "inspect" => {
                if args.len() < 2 {
                    return Err(anyhow::anyhow!("network inspect requires an interface name"));
                }
                let iface = args[1];
                match manager.inspect(iface) {
                    Ok(found) => {
                        let result = json!({
                            "command": "network.inspect",
                            "interface": iface,
                            "result": found,
                        });
                        formatter.output(&result)?;
                    }
                    Err(e) => {
                        let result = json!({
                            "command": "network.inspect",
                            "interface": iface,
                            "ok": false,
                            "error": e.to_string(),
                        });
                        formatter.output(&result)?;
                    }
                }
            }
            "addresses" => {
                let result = json!({
                    "command": "network.addresses",
                    "result": manager.addresses(),
                });
                formatter.output(&result)?;
            }
            "routes" => {
                let result = json!({
                    "command": "network.routes",
                    "result": manager.routes(),
                });
                formatter.output(&result)?;
            }
            "dns" => {
                let result = json!({
                    "command": "network.dns",
                    "result": manager.dns(),
                });
                formatter.output(&result)?;
            }
            "connectivity" => {
                let result = json!({
                    "command": "network.connectivity",
                    "result": manager.connectivity(),
                });
                formatter.output(&result)?;
            }
            "stats" => {
                let result = json!({
                    "command": "network.stats",
                    "result": manager.stats(),
                });
                formatter.output(&result)?;
            }
            "events" => {
                let result = json!({
                    "command": "network.events",
                    "result": manager.events(),
                });
                formatter.output(&result)?;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unknown network subcommand: {}. Use status, interfaces, inspect, addresses, routes, dns, connectivity, stats, events.",
                    args[0]
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::ShellHistory;
    use crate::output::OutputFormatter;
    use crate::session::ShellSession;

    async fn run(args: &[&str]) -> anyhow::Result<()> {
        let cmd = NetworkCommand;
        let session = ShellSession::default();
        let history = ShellHistory::default();
        let mut formatter = OutputFormatter::new();
        cmd.execute(args, &session, &mut formatter, &history).await
    }

    #[tokio::test]
    async fn status_runs() {
        assert!(run(&["status"]).await.is_ok());
    }

    #[tokio::test]
    async fn interfaces_runs() {
        assert!(run(&["interfaces"]).await.is_ok());
    }

    #[tokio::test]
    async fn inspect_existing_runs() {
        assert!(run(&["inspect", "lo"]).await.is_ok());
    }

    #[tokio::test]
    async fn inspect_missing_reports_error() {
        // The command reports the error through the formatter, so
        // it does not fail the call — it returns ok with an error
        // in the JSON payload.
        assert!(run(&["inspect", "ghost"]).await.is_ok());
    }

    #[tokio::test]
    async fn inspect_without_name_errors() {
        assert!(run(&["inspect"]).await.is_err());
    }

    #[tokio::test]
    async fn addresses_runs() {
        assert!(run(&["addresses"]).await.is_ok());
    }

    #[tokio::test]
    async fn routes_runs() {
        assert!(run(&["routes"]).await.is_ok());
    }

    #[tokio::test]
    async fn dns_runs() {
        assert!(run(&["dns"]).await.is_ok());
    }

    #[tokio::test]
    async fn connectivity_runs() {
        assert!(run(&["connectivity"]).await.is_ok());
    }

    #[tokio::test]
    async fn stats_runs() {
        assert!(run(&["stats"]).await.is_ok());
    }

    #[tokio::test]
    async fn events_runs() {
        assert!(run(&["events"]).await.is_ok());
    }

    #[tokio::test]
    async fn no_subcommand_errors() {
        assert!(run(&[]).await.is_err());
    }

    #[tokio::test]
    async fn unknown_subcommand_errors() {
        assert!(run(&["frobnicate"]).await.is_err());
    }

    #[test]
    fn metadata_is_stable() {
        let cmd = NetworkCommand;
        let md = cmd.metadata();
        assert_eq!(md.name, "network");
        assert_eq!(md.required_capability.as_deref(), Some("network.read"));
    }
}
