// Process Lifecycle - state machine and lifecycle management for processes

use aether_core::error::AetherError;
use serde::{Deserialize, Serialize};

/// Runtime state of a managed process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessState {
    /// Process has been created but not yet scheduled.
    Created,
    /// Process is actively executing.
    Running,
    /// Process is waiting on an event or resource.
    Sleeping,
    /// Process has been stopped by a signal.
    Stopped,
    /// Process has exited but not yet been reaped.
    Zombie,
    /// Process has terminated and been fully reaped.
    Terminated,
}

impl ProcessState {
    /// Returns true if the process is considered alive.
    pub fn is_alive(&self) -> bool {
        matches!(
            self,
            Self::Created | Self::Running | Self::Sleeping | Self::Stopped
        )
    }

    /// Validates whether a transition from this state to another is legal.
    pub fn can_transition_to(&self, next: &ProcessState) -> bool {
        use ProcessState::*;
        matches!(
            (self, next),
            (Created, Running)
                | (Created, Terminated)
                | (Running, Sleeping)
                | (Running, Stopped)
                | (Running, Zombie)
                | (Sleeping, Running)
                | (Sleeping, Stopped)
                | (Sleeping, Zombie)
                | (Stopped, Running)
                | (Stopped, Zombie)
                | (Zombie, Terminated)
        )
    }
}

impl std::fmt::Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "CREATED"),
            Self::Running => write!(f, "RUNNING"),
            Self::Sleeping => write!(f, "SLEEPING"),
            Self::Stopped => write!(f, "STOPPED"),
            Self::Zombie => write!(f, "ZOMBIE"),
            Self::Terminated => write!(f, "TERMINATED"),
        }
    }
}

/// A single recorded state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub process_id: u32,
    pub from: ProcessState,
    pub to: ProcessState,
    pub timestamp_ms: u64,
    pub reason: String,
}

/// Manages process lifecycle transitions with validation and audit trail.
pub struct ProcessLifecycle {
    /// Current state per process id.
    states: std::collections::HashMap<u32, ProcessState>,
    /// Audit log of all transitions.
    transitions: Vec<StateTransition>,
}

impl ProcessLifecycle {
    pub fn new() -> Self {
        Self {
            states: std::collections::HashMap::new(),
            transitions: Vec::new(),
        }
    }

    /// Registers a new process in the Created state.
    pub fn register(&mut self, process_id: u32, timestamp_ms: u64) -> Result<(), AetherError> {
        if self.states.contains_key(&process_id) {
            return Err(AetherError::invalid_input(format!(
                "Process {} already registered",
                process_id
            )));
        }
        self.states.insert(process_id, ProcessState::Created);
        self.transitions.push(StateTransition {
            process_id,
            from: ProcessState::Terminated,
            to: ProcessState::Created,
            timestamp_ms,
            reason: "registered".to_string(),
        });
        Ok(())
    }

    /// Transitions a process to a new state after validating the move.
    pub fn transition(
        &mut self,
        process_id: u32,
        next: ProcessState,
        timestamp_ms: u64,
        reason: impl Into<String>,
    ) -> Result<(), AetherError> {
        let current = *self.states.get(&process_id).ok_or_else(|| {
            AetherError::not_found(&format!("process {}", process_id))
        })?;
        if !current.can_transition_to(&next) {
            return Err(AetherError::invalid_input(format!(
                "Illegal transition for process {}: {} -> {}",
                process_id, current, next
            )));
        }
        self.states.insert(process_id, next);
        self.transitions.push(StateTransition {
            process_id,
            from: current,
            to: next,
            timestamp_ms,
            reason: reason.into(),
        });
        Ok(())
    }

    /// Returns the current state of a process.
    pub fn state(&self, process_id: u32) -> Option<ProcessState> {
        self.states.get(&process_id).copied()
    }

    /// Reaps a zombie process, moving it to Terminated.
    pub fn reap(&mut self, process_id: u32, timestamp_ms: u64) -> Result<(), AetherError> {
        self.transition(process_id, ProcessState::Terminated, timestamp_ms, "reaped")
    }

    /// Audit trail of all recorded transitions.
    pub fn history(&self) -> &[StateTransition] {
        &self.transitions
    }

    /// All live process ids currently tracked.
    pub fn live_processes(&self) -> Vec<u32> {
        self.states
            .iter()
            .filter(|(_, s)| s.is_alive())
            .map(|(pid, _)| *pid)
            .collect()
    }
}

impl Default for ProcessLifecycle {
    fn default() -> Self {
        Self::new()
    }
}
