// Process Security - policy enforcement and guard for process operations

use aether_core::error::AetherError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Categories of security violations raised by the process manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationKind {
    /// Execution path is not on the allowlist.
    DisallowedExecutable,
    /// User identity is not permitted to run this operation.
    UnauthorizedUser,
    /// Operation targets a protected process.
    ProtectedProcess,
    /// Resource limits would be exceeded.
    ResourceLimit,
    /// Shell or interpreter invocation detected where disallowed.
    ShellExecution,
}

impl std::fmt::Display for ViolationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DisallowedExecutable => write!(f, "DISALLOWED_EXECUTABLE"),
            Self::UnauthorizedUser => write!(f, "UNAUTHORIZED_USER"),
            Self::ProtectedProcess => write!(f, "PROTECTED_PROCESS"),
            Self::ResourceLimit => write!(f, "RESOURCE_LIMIT"),
            Self::ShellExecution => write!(f, "SHELL_EXECUTION"),
        }
    }
}

/// A single recorded security violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityViolation {
    pub kind: ViolationKind,
    pub process_id: u32,
    pub detail: String,
    pub timestamp_ms: u64,
}

impl std::fmt::Display for SecurityViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} on pid {}: {}",
            self.kind, self.process_id, self.detail
        )
    }
}

/// Policy describing what process operations are allowed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessPolicy {
    /// Allowlisted executable paths (exact match after canonicalization).
    pub allowed_executables: HashSet<PathBuf>,
    /// Users permitted to spawn/inspect processes.
    pub allowed_users: HashSet<String>,
    /// Process ids that may never be signaled or killed.
    pub protected_pids: HashSet<u32>,
    /// Maximum concurrent processes a user may own.
    pub max_processes_per_user: u32,
    /// Whether shell/interpreter execution (sh, bash, cmd) is denied outright.
    pub deny_shell_execution: bool,
}

impl ProcessPolicy {
    pub fn new() -> Self {
        Self {
            allowed_executables: HashSet::new(),
            allowed_users: HashSet::new(),
            protected_pids: HashSet::new(),
            max_processes_per_user: 0,
            deny_shell_execution: true,
        }
    }

    pub fn allow_executable(mut self, path: impl Into<PathBuf>) -> Self {
        self.allowed_executables.insert(path.into());
        self
    }

    pub fn allow_user(mut self, user: impl Into<String>) -> Self {
        self.allowed_users.insert(user.into());
        self
    }

    pub fn protect(mut self, pid: u32) -> Self {
        self.protected_pids.insert(pid);
        self
    }

    /// Returns true if the executable looks like a shell or generic interpreter.
    fn is_shell(executable: &Path) -> bool {
        const SHELL_NAMES: [&str; 6] = ["sh", "bash", "dash", "zsh", "cmd", "powershell"];
        executable
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|stem| {
                let stem = stem.trim_end_matches(".exe");
                SHELL_NAMES.contains(&stem)
            })
            .unwrap_or(false)
    }
}

impl Default for ProcessPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Guard that validates process operations against a policy and records violations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessGuard {
    pub policy: ProcessPolicy,
    /// Audit log of all violations observed by this guard.
    pub violations: Vec<SecurityViolation>,
}

impl ProcessGuard {
    pub fn new(policy: ProcessPolicy) -> Self {
        Self {
            policy,
            violations: Vec::new(),
        }
    }

    /// Validates spawning a new executable as `user`. Returns Err with a recorded violation.
    pub fn check_spawn(
        &mut self,
        process_id: u32,
        executable: &Path,
        user: &str,
        current_user_process_count: u32,
        timestamp_ms: u64,
    ) -> Result<(), AetherError> {
        if self.policy.deny_shell_execution && ProcessPolicy::is_shell(executable) {
            return Err(self.record(
                ViolationKind::ShellExecution,
                process_id,
                format!("shell-like executable denied: {}", executable.display()),
                timestamp_ms,
            ));
        }
        if !self.policy.allowed_executables.is_empty()
            && !self.policy.allowed_executables.contains(executable)
        {
            return Err(self.record(
                ViolationKind::DisallowedExecutable,
                process_id,
                format!("executable not allowlisted: {}", executable.display()),
                timestamp_ms,
            ));
        }
        if !self.policy.allowed_users.is_empty() && !self.policy.allowed_users.contains(user) {
            return Err(self.record(
                ViolationKind::UnauthorizedUser,
                process_id,
                format!("user not authorized: {user}"),
                timestamp_ms,
            ));
        }
        if self.policy.max_processes_per_user > 0
            && current_user_process_count >= self.policy.max_processes_per_user
        {
            return Err(self.record(
                ViolationKind::ResourceLimit,
                process_id,
                format!(
                    "user {user} at process limit ({})",
                    self.policy.max_processes_per_user
                ),
                timestamp_ms,
            ));
        }
        Ok(())
    }

    /// Validates signaling/killing an existing process.
    pub fn check_signal(&mut self, process_id: u32, timestamp_ms: u64) -> Result<(), AetherError> {
        if self.policy.protected_pids.contains(&process_id) {
            return Err(self.record(
                ViolationKind::ProtectedProcess,
                process_id,
                "target process is protected".to_string(),
                timestamp_ms,
            ));
        }
        Ok(())
    }

    fn record(
        &mut self,
        kind: ViolationKind,
        process_id: u32,
        detail: String,
        timestamp_ms: u64,
    ) -> AetherError {
        let violation = SecurityViolation {
            kind,
            process_id,
            detail,
            timestamp_ms,
        };
        let err = AetherError::permission_denied(violation.to_string());
        self.violations.push(violation);
        err
    }
}
