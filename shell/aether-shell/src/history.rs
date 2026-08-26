// Shell history management
use std::collections::VecDeque;
use std::path::PathBuf;
use anyhow::Result;

pub struct ShellHistory {
    entries: VecDeque<String>,
    max_size: usize,
    path: Option<PathBuf>,
}

impl ShellHistory {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            max_size: 1000,
            path: Self::history_path(),
        }
    }

    pub fn add(&mut self, command: &str) {
        // Don't store commands that look like they might contain secrets
        if self.is_sensitive(command) {
            return;
        }

        if self.entries.len() >= self.max_size {
            self.entries.pop_front();
        }
        self.entries.push_back(command.to_string());
    }

    pub fn get_all(&self) -> Vec<&String> {
        self.entries.iter().collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn save(&self) -> Result<()> {
        if let Some(path) = &self.path {
            let content = self.entries.iter()
                .map(|e| e.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(path, content)?;
        }
        Ok(())
    }

    pub fn load(&mut self) -> Result<()> {
        if let Some(path) = &self.path {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                for line in content.lines() {
                    if !line.is_empty() {
                        self.entries.push_back(line.to_string());
                    }
                }
            }
        }
        Ok(())
    }

    fn is_sensitive(&self, command: &str) -> bool {
        let sensitive_keywords = ["password", "token", "secret", "key", "credential"];
        sensitive_keywords.iter().any(|&kw| command.to_lowercase().contains(kw))
    }

    fn history_path() -> Option<PathBuf> {
        if let Ok(home) = std::env::var("HOME") {
            Some(PathBuf::from(home).join(".aether_shell_history"))
        } else if let Ok(home) = std::env::var("USERPROFILE") {
            Some(PathBuf::from(home).join(".aether_shell_history"))
        } else {
            None
        }
    }
}

impl Default for ShellHistory {
    fn default() -> Self {
        Self::new()
    }
}
