// Process Discovery - identifies and enumerates processes on the system

use crate::lifecycle::ProcessState;
use aether_core::error::AetherError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a discovered process on the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessRecord {
    /// Unique process identifier.
    pub process_id: u32,
    /// Parent process identifier (0 if orphan).
    pub parent_id: u32,
    /// Display name of the process.
    pub name: String,
    /// Full executable path.
    pub executable_path: PathBuf,
    /// Command line arguments (as array, no shell interpretation).
    pub arguments: Vec<String>,
    /// Current state of the process.
    pub state: ProcessState,
    /// User identity running the process.
    pub user: String,
    /// CPU usage percentage (approximate).
    pub cpu_usage_pct: f64,
    /// Memory usage in kilobytes.
    pub memory_kb: u64,
    /// Open file descriptor count.
    pub open_fds: u32,
    /// Environment variables snapshot.
    pub environment: Vec<String>,
    /// Timestamp when the process was started.
    pub start_time_ms: u64,
    /// Timestamp of last status update.
    pub last_update_ms: u64,
    /// Whether this process is critical to system operation.
    pub is_critical: bool,
    /// Whether this process belongs to a protected set.
    pub is_protected: bool,
}

impl ProcessRecord {
    pub fn new(
        process_id: u32,
        parent_id: u32,
        name: impl Into<String>,
        executable_path: PathBuf,
        arguments: Vec<String>,
    ) -> Self {
        Self {
            process_id,
            parent_id,
            name: name.into(),
            executable_path,
            arguments,
            state: ProcessState::Running,
            user: String::new(),
            cpu_usage_pct: 0.0,
            memory_kb: 0,
            open_fds: 0,
            environment: Vec::new(),
            start_time_ms: 0,
            last_update_ms: 0,
            is_critical: false,
            is_protected: false,
        }
    }

    pub fn has_ancestor(&self, target_id: u32) -> bool {
        self.process_id == target_id || self.parent_id == target_id
    }
}

impl std::fmt::Display for ProcessRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PID {} ({}): {} {} - {}KB mem, {} fds",
            self.process_id,
            self.state,
            self.name,
            self.cpu_usage_pct,
            self.memory_kb,
            self.open_fds
        )
    }
}

/// Discovery context for process enumeration.
pub struct ProcessDiscovery {
    /// Base directory for process metadata storage.
    pub metadata_dir: PathBuf,
    /// Whether to include child processes (recursive).
    pub recursive: bool,
}

impl ProcessDiscovery {
    pub fn new(metadata_dir: impl Into<PathBuf>) -> Self {
        Self { metadata_dir: metadata_dir.into(), recursive: true }
    }

    pub fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Discover all processes on the system.
    /// Returns a vector of process records.
    pub fn discover(&self) -> Result<Vec<ProcessRecord>, AetherError> {
        // In a real implementation, this would:
        // 1. Enumerate process list from OS API
        // 2. For each process, gather metadata
        // 3. Check parent-child relationships
        // 4. Classify critical/protected processes
        // 5. Validate process legitimacy (no shell execution)

        // Placeholder: return empty vector (will be populated by OS integration)
        Ok(Vec::new())
    }

    /// Discover processes matching a specific name pattern.
    pub fn discover_by_name(&self, name_pattern: &str) -> Result<Vec<ProcessRecord>, AetherError> {
        let all_processes = self.discover()?;
        Ok(all_processes.into_iter().filter(|p| p.name.contains(name_pattern)).collect())
    }

    /// Discover processes running under a specific user.
    pub fn discover_by_user(&self, user: &str) -> Result<Vec<ProcessRecord>, AetherError> {
        let all_processes = self.discover()?;
        Ok(all_processes.into_iter().filter(|p| p.user == user).collect())
    }

    /// Discover processes with resource usage above threshold.
    pub fn discover_high_resource(
        &self,
        cpu_threshold: f64,
        memory_threshold_kb: u64,
    ) -> Result<Vec<ProcessRecord>, AetherError> {
        let all_processes = self.discover()?;
        Ok(all_processes
            .into_iter()
            .filter(|p| p.cpu_usage_pct > cpu_threshold || p.memory_kb > memory_threshold_kb)
            .collect())
    }

    /// Discover child processes of a given parent.
    pub fn discover_children(&self, parent_id: u32) -> Result<Vec<ProcessRecord>, AetherError> {
        let all_processes = self.discover()?;
        Ok(all_processes.into_iter().filter(|p| p.parent_id == parent_id).collect())
    }

    /// Get metadata storage path for a specific process.
    pub fn metadata_path(&self, process_id: u32) -> PathBuf {
        self.metadata_dir.join(format!("pid_{}", process_id))
    }
}

impl Default for ProcessDiscovery {
    fn default() -> Self {
        Self::new("/tmp/aether/processes")
    }
}
