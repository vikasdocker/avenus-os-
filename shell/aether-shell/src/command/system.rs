// System commands: help, version, status, health, services, events, audit, system control
use anyhow::Result;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::json;

use crate::command::{Command, CommandMetadata};
use crate::history::ShellHistory;
use crate::output::OutputFormatter;
use crate::session::ShellSession;

// Define static metadata for each command
static HELP_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "help".to_string(),
    description: "Display help information".to_string(),
    usage: "help [command]".to_string(),
    required_capability: None,
    risk_level: "low".to_string(),
    requires_confirmation: false,
});

static VERSION_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "version".to_string(),
    description: "Display shell version".to_string(),
    usage: "version".to_string(),
    required_capability: None,
    risk_level: "low".to_string(),
    requires_confirmation: false,
});

static STATUS_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "status".to_string(),
    description: "Show system status".to_string(),
    usage: "status".to_string(),
    required_capability: None,
    risk_level: "low".to_string(),
    requires_confirmation: false,
});

static HEALTH_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "health".to_string(),
    description: "Display system health".to_string(),
    usage: "health".to_string(),
    required_capability: None,
    risk_level: "low".to_string(),
    requires_confirmation: false,
});

static SERVICES_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "services".to_string(),
    description: "List all services".to_string(),
    usage: "services".to_string(),
    required_capability: None,
    risk_level: "low".to_string(),
    requires_confirmation: false,
});

static EVENTS_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "events".to_string(),
    description: "Show system events".to_string(),
    usage: "events [service]".to_string(),
    required_capability: None,
    risk_level: "low".to_string(),
    requires_confirmation: false,
});

static AUDIT_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "audit".to_string(),
    description: "Show audit log".to_string(),
    usage: "audit [--limit N]".to_string(),
    required_capability: Some("audit.read".to_string()),
    risk_level: "low".to_string(),
    requires_confirmation: false,
});

static SYSTEM_METADATA: Lazy<CommandMetadata> = Lazy::new(|| CommandMetadata {
    name: "system".to_string(),
    description: "System control commands".to_string(),
    usage: "system <shutdown|reboot>".to_string(),
    required_capability: Some("system.control".to_string()),
    risk_level: "critical".to_string(),
    requires_confirmation: true,
});

// Help Command
pub struct HelpCommand;

#[async_trait]
impl Command for HelpCommand {
    fn metadata(&self) -> &CommandMetadata {
        &HELP_METADATA
    }

    async fn execute(
        &self,
        args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        if args.is_empty() {
            let help_text = r#"Aether Shell Command Reference

System Commands:
  help [command]              - Show help information
  version                     - Display shell version
  status                      - Show system status
  health                      - Display system health
  services                    - List all services
  events [service]            - Show system events
  audit [--limit N]           - Show audit log
  system <subcommand>         - System control (shutdown, reboot)

Filesystem Commands:
  fs list <path>              - List directory contents
  fs stat <path>              - Show file statistics
  fs search <path> <pattern>  - Search for files
  fs storage                  - Show storage information
  fs mounts                   - Show mounted filesystems

Process Commands:
  process list                - List all processes
  process inspect <pid>       - Show process details
  process start <executable>  - Start a process
  process stop <pid>          - Stop a process
  process restart <pid>       - Restart a process

Application Commands:
  app list                    - List installed applications
  app inspect <id>            - Show application details
  app launch <id>             - Launch an application
  app close <id>              - Close an application

Network Commands:
  network status              - Show network status
  network interfaces          - List network interfaces
  network inspect <iface>     - Show interface details
  network addresses           - Show IP addresses
  network routes              - Show routing table
  network dns                 - Show DNS configuration
  network connectivity        - Check connectivity
  network stats               - Show network statistics

Agent Runtime Commands (talk to aether-agentd on :4748):
  agent status                - Show agent runtime host status
  agent sessions              - List all sessions
  agent inspect <sid>         - Inspect a single session
  agent intent <json>         - Submit a structured intent through the host
  agent cancel <sid>          - Cancel a session
  agent audit [N]             - Show last N audit entries (default 20)

Options:
  --json                      - Output in JSON format
  --help                      - Show command help

Type 'help <command>' for more information.
Type 'exit' or 'quit' to exit.
"#;
            formatter.print(help_text);
        } else {
            let cmd = args[0];
            formatter
                .print(&format!("Help for command: {}\n(Detailed help not yet implemented)", cmd));
        }
        Ok(())
    }
}

// Version Command
pub struct VersionCommand;

#[async_trait]
impl Command for VersionCommand {
    fn metadata(&self) -> &CommandMetadata {
        &VERSION_METADATA
    }

    async fn execute(
        &self,
        _args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        let version_info = json!({
            "name": "Aether Shell",
            "version": env!("CARGO_PKG_VERSION"),
            "phase": "1.8",
            "status": "development"
        });

        formatter.output(&version_info)?;
        Ok(())
    }
}

// Status Command
pub struct StatusCommand;

#[async_trait]
impl Command for StatusCommand {
    fn metadata(&self) -> &CommandMetadata {
        &STATUS_METADATA
    }

    async fn execute(
        &self,
        _args: &[&str],
        session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        let status = json!({
            "system_state": "READY",
            "session_id": session.session_id(),
            "actor": session.actor(),
            "uptime_seconds": 0,
            "services_running": 0,
            "services_total": 0,
        });

        formatter.output(&status)?;
        Ok(())
    }
}

// Health Command
pub struct HealthCommand;

#[async_trait]
impl Command for HealthCommand {
    fn metadata(&self) -> &CommandMetadata {
        &HEALTH_METADATA
    }

    async fn execute(
        &self,
        _args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        let health = json!({
            "overall_health": "HEALTHY",
            "system": "HEALTHY",
            "services": "HEALTHY",
        });

        formatter.output(&health)?;
        Ok(())
    }
}

// Services Command
pub struct ServicesCommand;

#[async_trait]
impl Command for ServicesCommand {
    fn metadata(&self) -> &CommandMetadata {
        &SERVICES_METADATA
    }

    async fn execute(
        &self,
        _args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        let services = json!({
            "services": []
        });

        formatter.output(&services)?;
        Ok(())
    }
}

// Events Command
pub struct EventsCommand;

#[async_trait]
impl Command for EventsCommand {
    fn metadata(&self) -> &CommandMetadata {
        &EVENTS_METADATA
    }

    async fn execute(
        &self,
        _args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        let events = json!({
            "events": []
        });

        formatter.output(&events)?;
        Ok(())
    }
}

// Audit Command
pub struct AuditCommand;

#[async_trait]
impl Command for AuditCommand {
    fn metadata(&self) -> &CommandMetadata {
        &AUDIT_METADATA
    }

    async fn execute(
        &self,
        _args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        let audit = json!({
            "audit_entries": []
        });

        formatter.output(&audit)?;
        Ok(())
    }
}

// System Command (shutdown, reboot)
pub struct SystemCommand;

#[async_trait]
impl Command for SystemCommand {
    fn metadata(&self) -> &CommandMetadata {
        &SYSTEM_METADATA
    }

    async fn execute(
        &self,
        args: &[&str],
        _session: &ShellSession,
        formatter: &mut OutputFormatter,
        _history: &ShellHistory,
    ) -> Result<()> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("system requires subcommand: shutdown or reboot"));
        }

        match args[0] {
            "shutdown" => {
                formatter.print("Shutting down system... (not implemented)");
            }
            "reboot" => {
                formatter.print("Rebooting system... (not implemented)");
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown system subcommand: {}", args[0]));
            }
        }

        Ok(())
    }
}
