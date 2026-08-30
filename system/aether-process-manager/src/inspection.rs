// Process Inspection - detailed inspection and point-in-time snapshots of processes

use crate::discovery::ProcessRecord;
use crate::lifecycle::ProcessState;
use aether_core::error::AetherError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Detailed inspection data for a single process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    /// Unique process identifier.
    pub process_id: u32,
    /// Parent process identifier.
    pub parent_id: u32,
    /// Display name of the process.
    pub name: String,
    /// Full executable path.
    pub executable_path: String,
    /// Current lifecycle state.
    pub state: ProcessState,
    /// User identity running the process.
    pub user: String,
    /// Command line as originally invoked (arguments joined for display only).
    pub command_line: String,
    /// Working directory of the process.
    pub working_directory: String,
    /// Number of threads in the process.
    pub thread_count: u32,
    /// Cumulative CPU time in milliseconds.
    pub cpu_time_ms: u64,
    /// Peak resident memory in kilobytes.
    pub peak_memory_kb: u64,
}

impl ProcessInfo {
    /// Builds inspection info from a discovery record.
    pub fn from_record(record: &ProcessRecord) -> Self {
        Self {
            process_id: record.process_id,
            parent_id: record.parent_id,
            name: record.name.clone(),
            executable_path: record.executable_path.display().to_string(),
            state: record.state,
            user: record.user.clone(),
            command_line: record.arguments.join(" "),
            working_directory: String::new(),
            thread_count: 0,
            cpu_time_ms: 0,
            peak_memory_kb: record.memory_kb,
        }
    }
}

impl std::fmt::Display for ProcessInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [{}] {} ({}) user={} threads={} cpu={}ms",
            self.process_id,
            self.state,
            self.name,
            self.executable_path,
            if self.user.is_empty() { "?" } else { &self.user },
            self.thread_count,
            self.cpu_time_ms
        )
    }
}

/// Point-in-time snapshot of the full process table with index lookups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    /// Millisecond timestamp when the snapshot was captured.
    pub captured_at_ms: u64,
    /// All inspected processes.
    pub processes: Vec<ProcessInfo>,
}

impl ProcessSnapshot {
    pub fn new(captured_at_ms: u64) -> Self {
        Self { captured_at_ms, processes: Vec::new() }
    }

    /// Captures a snapshot from discovered records.
    pub fn capture(records: &[ProcessRecord], captured_at_ms: u64) -> Self {
        Self { captured_at_ms, processes: records.iter().map(ProcessInfo::from_record).collect() }
    }

    /// Look up a process by id.
    pub fn find(&self, process_id: u32) -> Option<&ProcessInfo> {
        self.processes.iter().find(|p| p.process_id == process_id)
    }

    /// Index of pid -> process id for fast parent-child walks.
    pub fn index_by_pid(&self) -> HashMap<u32, usize> {
        self.processes.iter().enumerate().map(|(i, p)| (p.process_id, i)).collect()
    }

    /// All children of the given parent id.
    pub fn children_of(&self, parent_id: u32) -> Vec<&ProcessInfo> {
        self.processes.iter().filter(|p| p.parent_id == parent_id).collect()
    }

    /// Resolves the ancestor chain of a process (nearest parent first).
    pub fn ancestors_of(&self, mut process_id: u32) -> Result<Vec<&ProcessInfo>, AetherError> {
        let index = self.index_by_pid();
        let mut chain = Vec::new();
        let mut guard = 0usize;
        while let Some(info) = self.find(process_id) {
            if info.parent_id == 0 {
                break;
            }
            guard += 1;
            if guard > self.processes.len() {
                return Err(AetherError::internal(
                    "ancestor walk exceeded process count (cycle suspected)",
                ));
            }
            match index.get(&info.parent_id) {
                Some(&i) => {
                    chain.push(&self.processes[i]);
                    process_id = info.parent_id;
                }
                None => break,
            }
        }
        Ok(chain)
    }

    /// Processes currently in an alive state.
    pub fn alive(&self) -> Vec<&ProcessInfo> {
        self.processes.iter().filter(|p| p.state.is_alive()).collect()
    }
}
