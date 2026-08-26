// Shell session management and state
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSession {
    session_id: String,
    actor: String,
    authentication_state: AuthenticationState,
    capabilities: Vec<String>,
    shell_version: String,
    startup_time: SystemTime,
    current_context: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthenticationState {
    Unauthenticated,
    Authenticated,
    Privileged,
}

impl ShellSession {
    pub fn new() -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            actor: std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "unknown".to_string()),
            authentication_state: AuthenticationState::Authenticated,
            capabilities: vec![
                "shell.basic".to_string(),
                "filesystem.read".to_string(),
                "process.read".to_string(),
                "network.read".to_string(),
                "application.read".to_string(),
            ],
            shell_version: env!("CARGO_PKG_VERSION").to_string(),
            startup_time: SystemTime::now(),
            current_context: "/".to_string(),
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn actor(&self) -> &str {
        &self.actor
    }

    pub fn authentication_state(&self) -> AuthenticationState {
        self.authentication_state
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    pub fn shell_version(&self) -> &str {
        &self.shell_version
    }

    pub fn startup_time(&self) -> SystemTime {
        self.startup_time
    }

    pub fn current_context(&self) -> &str {
        &self.current_context
    }

    pub fn set_context(&mut self, context: String) {
        self.current_context = context;
    }

    pub fn set_authentication_state(&mut self, state: AuthenticationState) {
        self.authentication_state = state;
    }

    pub fn add_capability(&mut self, capability: String) {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
    }
}

impl Default for ShellSession {
    fn default() -> Self {
        Self::new()
    }
}
