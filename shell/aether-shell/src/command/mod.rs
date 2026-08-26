// Command registry and command execution framework
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

use crate::history::ShellHistory;
use crate::output::OutputFormatter;
use crate::session::ShellSession;

pub mod system;
pub mod filesystem;
pub mod process;
pub mod application;
pub mod network;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub required_capability: Option<String>,
    pub risk_level: String,
    pub requires_confirmation: bool,
}

#[async_trait::async_trait]
pub trait Command: Send + Sync {
    fn metadata(&self) -> &CommandMetadata;
    async fn execute(
        &self,
        args: &[&str],
        session: &ShellSession,
        formatter: &mut OutputFormatter,
        history: &ShellHistory,
    ) -> Result<()>;
}

pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut commands: HashMap<String, Box<dyn Command>> = HashMap::new();

        // System commands
        commands.insert("help".to_string(), Box::new(system::HelpCommand));
        commands.insert("version".to_string(), Box::new(system::VersionCommand));
        commands.insert("status".to_string(), Box::new(system::StatusCommand));
        commands.insert("health".to_string(), Box::new(system::HealthCommand));
        commands.insert("services".to_string(), Box::new(system::ServicesCommand));
        commands.insert("events".to_string(), Box::new(system::EventsCommand));
        commands.insert("audit".to_string(), Box::new(system::AuditCommand));
        commands.insert("system".to_string(), Box::new(system::SystemCommand));

        // Filesystem commands
        commands.insert("fs".to_string(), Box::new(filesystem::FilesystemCommand));

        // Process commands
        commands.insert("process".to_string(), Box::new(process::ProcessCommand));

        // Application commands
        commands.insert("app".to_string(), Box::new(application::ApplicationCommand));

        // Network commands
        commands.insert("network".to_string(), Box::new(network::NetworkCommand));

        Self { commands }
    }

    pub async fn execute(
        &self,
        command_name: &str,
        args: &[&str],
        session: &ShellSession,
        formatter: &mut OutputFormatter,
        history: &ShellHistory,
    ) -> Result<()> {
        match self.commands.get(command_name) {
            Some(cmd) => {
                info!("Executing command: {}", command_name);
                cmd.execute(args, session, formatter, history).await
            }
            None => Err(anyhow!("Unknown command: {}. Type 'help' for available commands.", command_name)),
        }
    }

    pub fn list_commands(&self) -> Vec<(&String, &CommandMetadata)> {
        self.commands
            .iter()
            .map(|(name, cmd)| (name, cmd.metadata()))
            .collect()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
