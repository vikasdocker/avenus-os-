// Aether Application Manager - registry, runtime, and lifecycle.
//
// Aether applications are first-class OS objects: they are DISCOVERED in
// a structured registry, LAUNCHED through whitelisted entry points,
// RUN as tracked child processes, QUERIED for live state, and CLOSED
// cleanly. Commands are spawned directly as argv vectors - never through
// a shell - so nothing here can execute arbitrary text.

use aether_apps::AppManifestError;
use aether_core::ComponentId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::process::{Child, Command, Stdio};

// ------------------------------------------------------------- definitions

/// How the application integrates with Aether.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppType {
    /// Ships with the OS image.
    Builtin,
    /// Installed separately at runtime.
    Native,
}

impl std::fmt::Display for AppType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin => write!(f, "builtin"),
            Self::Native => write!(f, "native"),
        }
    }
}

/// Structured application definition (registry entry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppDefinition {
    pub id: String,
    pub display_name: String,
    pub version: String,
    /// Entry point as an argv string; spawned directly, never via shell.
    pub command: String,
    pub app_type: AppType,
    /// Declared permissions (e.g. "display"). Enforcement arrives with the
    /// sandbox phase; declaration is required today.
    pub permissions: Vec<String>,
}

impl AppDefinition {
    /// Convenience builder used by seeds and simple registrations.
    pub fn new(
        id: &str,
        display_name: &str,
        version: &str,
        command: &str,
        permissions: &[&str],
    ) -> Result<Self, AppManagerError> {
        Ok(Self {
            id: ComponentId::new(id)
                .map_err(|_| AppManagerError::InvalidId(id.to_string()))?
                .as_str()
                .to_string(),
            display_name: display_name.to_string(),
            version: version.to_string(),
            command: command.to_string(),
            app_type: AppType::Builtin,
            permissions: permissions.iter().map(|s| s.to_string()).collect(),
        })
    }
}

// ----------------------------------------------------------------- runtime

/// Lifecycle state of one launched instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstanceState {
    Running,
    Exited,
    Failed,
    Closed,
}

impl std::fmt::Display for InstanceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Running => "RUNNING",
            Self::Exited => "EXITED",
            Self::Failed => "FAILED",
            Self::Closed => "CLOSED",
        };
        write!(f, "{s}")
    }
}

/// One launched instance of an application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub instance_id: u64,
    pub app_id: String,
    pub pid: Option<u32>,
    pub state: InstanceState,
}

/// Aggregated per-application report (registry + runtime view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateReport {
    pub app_id: String,
    pub installed: bool,
    pub state: String,
    pub instances: Vec<Instance>,
}

#[derive(Debug)]
struct InstanceRecord {
    instance: Instance,
    child: Option<Child>,
}

// ------------------------------------------------------------------ errors

/// Errors surfaced by the application manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppManagerError {
    UnknownApp(String),
    AlreadyRunning(String),
    NotRunning(u64),
    InvalidId(String),
    Manifest(AppManifestError),
    LaunchFailed(String),
}

impl std::fmt::Display for AppManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownApp(id) => write!(f, "unknown application '{id}'"),
            Self::AlreadyRunning(id) => write!(f, "application '{id}' is already running"),
            Self::NotRunning(instance) => write!(f, "instance {instance} is not running"),
            Self::InvalidId(id) => {
                write!(f, "invalid application id '{id}'; allowed: [a-z0-9._-]")
            }
            Self::Manifest(e) => write!(f, "invalid manifest: {e}"),
            Self::LaunchFailed(e) => write!(f, "launch failed: {e}"),
        }
    }
}

impl std::error::Error for AppManagerError {}

impl From<AppManifestError> for AppManagerError {
    fn from(e: AppManifestError) -> Self {
        Self::Manifest(e)
    }
}

// ------------------------------------------------------------------ stdio

/// Opens per-app output files as stdio redirections so failures remain
/// diagnosable without a controlling terminal.
fn app_stdio(out_path: &str) -> Option<(Stdio, Stdio)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let out = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o644)
            .open(out_path)
            .ok()?;
        let err = out.try_clone().ok()?;
        Some((Stdio::from(out), Stdio::from(err)))
    }
    #[cfg(not(unix))]
    {
        let _ = out_path;
        None
    }
}

// ----------------------------------------------------------------- manager

/// Registry + runtime + lifecycle owner.
#[derive(Default)]
pub struct ApplicationManager {
    defs: BTreeMap<String, AppDefinition>,
    instances: BTreeMap<u64, InstanceRecord>,
    next_instance: u64,
}

impl ApplicationManager {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- registry ----

    /// Registers a full definition (discover/register part of the lifecycle).
    pub fn register(&mut self, def: AppDefinition) -> Result<(), AppManagerError> {
        self.defs.insert(def.id.clone(), def);
        Ok(())
    }

    /// Compatibility helper: registers from raw fields with defaults.
    pub fn register_json(
        &mut self,
        id: &str,
        display_name: &str,
        command: &str,
    ) -> Result<(), AppManagerError> {
        self.register(AppDefinition::new(
            id,
            display_name,
            "0.1.0",
            command,
            &["display"],
        )?)
    }

    /// DISCOVER: all registered definitions sorted by id.
    pub fn discover(&self) -> Vec<&AppDefinition> {
        self.defs.values().collect()
    }

    /// Number of registered applications.
    pub fn registered_count(&self) -> usize {
        self.defs.len()
    }

    // ---- lifecycle ----

    /// Reaps finished children, updating their recorded states.
    fn reap(&mut self) {
        let ids: Vec<u64> = self.instances.keys().copied().collect();
        for id in ids {
            let Some(record) = self.instances.get_mut(&id) else { continue };
            if record.instance.state != InstanceState::Running {
                continue;
            }
            let mut child_slot = record.child.take();
            if let Some(child) = child_slot.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        record.instance.state = if status.success() {
                            InstanceState::Exited
                        } else {
                            InstanceState::Failed
                        };
                    }
                    Ok(None) => {
                        record.child = child_slot;
                    }
                    Err(_) => {
                        record.instance.state = InstanceState::Failed;
                    }
                }
            }
        }
    }

    /// LAUNCH: starts a registered app by id.
    pub fn launch(&mut self, app_id: &str) -> Result<Instance, AppManagerError> {
        self.reap();
        let def = self
            .defs
            .get(app_id)
            .ok_or_else(|| AppManagerError::UnknownApp(app_id.to_string()))?
            .clone();

        if self.instances.values().any(|r| {
            r.instance.app_id == def.id && r.instance.state == InstanceState::Running
        }) {
            return Err(AppManagerError::AlreadyRunning(def.id.clone()));
        }

        self.next_instance += 1;
        let instance_id = self.next_instance;

        let mut parts = def.command.split_whitespace();
        let Some(program) = parts.next() else {
            return Err(AppManagerError::LaunchFailed("empty command".to_string()));
        };
        let args: Vec<&str> = parts.collect();
        let app_name = program.rsplit('/').next().unwrap_or("app");
        let stdio_pair = app_stdio(&format!("/tmp/{app_name}.out"));
        let (out_stdio, err_stdio) = match stdio_pair {
            Some((o, e)) => (Some(o), Some(e)),
            None => (None, None),
        };

        let spawn_result = Command::new(program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(out_stdio.unwrap_or(Stdio::null()))
            .stderr(err_stdio.unwrap_or(Stdio::null()))
            .spawn();

        let (child_handle, pid) = match spawn_result {
            Ok(child) => {
                let pid = Some(child.id());
                (Some(child), pid)
            }
            Err(_) => (None, None),
        };

        let state = if child_handle.is_some() {
            InstanceState::Running
        } else {
            InstanceState::Failed
        };
        let instance = Instance {
            instance_id,
            app_id: def.id.clone(),
            pid,
            state,
        };
        self.instances.insert(
            instance_id,
            InstanceRecord {
                instance: instance.clone(),
                child: child_handle,
            },
        );

        if state == InstanceState::Failed {
            return Err(AppManagerError::LaunchFailed(format!(
                "spawn failed; recorded as FAILED instance {instance_id}"
            )));
        }
        Ok(instance)
    }

    /// RUNNING: live instances (after reaping).
    pub fn running(&mut self) -> Vec<Instance> {
        self.reap();
        self.instances
            .values()
            .filter(|r| r.instance.state == InstanceState::Running)
            .map(|r| r.instance.clone())
            .collect()
    }

    /// STOP/CLOSE: terminates a running instance cleanly.
    pub fn close(&mut self, instance_id: u64) -> Result<Instance, AppManagerError> {
        self.reap();
        let record = self
            .instances
            .get_mut(&instance_id)
            .ok_or(AppManagerError::NotRunning(instance_id))?;
        if let Some(child) = record.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        record.child = None;
        record.instance.state = InstanceState::Closed;
        Ok(record.instance.clone())
    }

    /// QUERY: aggregated state for one application id.
    pub fn app_state(&mut self, app_id: &str) -> AppStateReport {
        self.reap();
        let installed = self.defs.contains_key(app_id);
        let instances: Vec<Instance> = self
            .instances
            .values()
            .filter(|r| r.instance.app_id == app_id)
            .map(|r| r.instance.clone())
            .collect();

        let state = if instances
            .iter()
            .any(|i| i.state == InstanceState::Running)
        {
            "RUNNING"
        } else if instances.iter().any(|i| i.state == InstanceState::Failed) {
            "FAILED"
        } else if let Some(last) = instances.last() {
            match last.state {
                InstanceState::Exited => "EXITED",
                InstanceState::Closed => "CLOSED",
                _ => "INSTALLED",
            }
        } else if installed {
            "INSTALLED"
        } else {
            "UNKNOWN"
        };

        AppStateReport {
            app_id: app_id.to_string(),
            installed,
            state: state.to_string(),
            instances,
        }
    }

    /// Aggregate counts for system status surfaces.
    pub fn stats(&mut self) -> (usize, usize, usize) {
        self.reap();
        let installed = self.defs.len();
        let running = self
            .instances
            .values()
            .filter(|r| r.instance.state == InstanceState::Running)
            .count();
        let failed = self
            .instances
            .values()
            .filter(|r| r.instance.state == InstanceState::Failed)
            .count();
        (installed, running, failed)
    }
}

// ------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_app_rejected() {
        let mut am = ApplicationManager::new();
        assert!(matches!(
            am.launch("ghost"),
            Err(AppManagerError::UnknownApp(_))
        ));
    }

    #[test]
    fn failed_spawn_is_recorded_as_failed_instance() {
        let mut am = ApplicationManager::new();
        am.register(
            AppDefinition::new(
                "badapp",
                "Bad",
                "0.1.0",
                "/definitely/not/a/binary",
                &["display"],
            )
            .unwrap_or_else(|e| panic!("{e}")),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let err = am
            .launch("badapp")
            .err()
            .unwrap_or_else(|| panic!("expected launch failure"));
        assert!(matches!(err, AppManagerError::LaunchFailed(_)));
        let report = am.app_state("badapp");
        assert_eq!(report.state, "FAILED");
        assert_eq!(report.instances.len(), 1);
        assert_eq!(am.stats(), (1, 0, 1));
    }

    // Spawn-dependent lifecycle tests require a unix userspace (/bin/sleep).
    #[cfg(unix)]
    #[test]
    fn single_running_instance_policy() {
        let mut am = ApplicationManager::new();
        am.register(
            AppDefinition::new("dup", "Dup", "0.1.0", "/bin/sleep 5", &["display"])
                .unwrap_or_else(|e| panic!("{e}")),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let first = am.launch("dup").unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(
            am.launch("dup"),
            Err(AppManagerError::AlreadyRunning(_))
        ));
        let closed = am.close(first.instance_id).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(closed.state, InstanceState::Closed);
        assert!(am.launch("dup").is_ok());
        am.close(first.instance_id + 1).unwrap_or_else(|e| panic!("{e}"));
    }

    #[test]
    fn discovery_lists_definitions_sorted() {
        let mut am = ApplicationManager::new();
        am.register(
            AppDefinition::new("zeta", "Zeta", "0.2.0", "/bin/true", &["display"])
                .unwrap_or_else(|e| panic!("{e}")),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        am.register(
            AppDefinition::new("alpha", "Alpha", "0.1.0", "/bin/true", &["display"])
                .unwrap_or_else(|e| panic!("{e}")),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let ids: Vec<String> = am.discover().into_iter().map(|d| d.id.clone()).collect();
        assert_eq!(ids, vec!["alpha".to_string(), "zeta".to_string()]);
    }

    // Spawn-dependent lifecycle tests require a unix userspace (/bin/sleep).
    #[cfg(unix)]
    #[test]
    fn status_query_tracks_lifecycle() {
        let mut am = ApplicationManager::new();
        am.register(
            AppDefinition::new("lifecycle", "LC", "0.1.0", "/bin/sleep 30", &["display"])
                .unwrap_or_else(|e| panic!("{e}")),
        )
        .unwrap_or_else(|e| panic!("{e}"));

        let before = am.app_state("lifecycle");
        assert_eq!(before.state, "INSTALLED");
        assert!(before.instances.is_empty());

        let inst = am.launch("lifecycle").unwrap_or_else(|e| panic!("{e}"));
        let during = am.app_state("lifecycle");
        assert_eq!(during.state, "RUNNING");
        assert_eq!(during.instances[0].state, InstanceState::Running);

        am.close(inst.instance_id).unwrap_or_else(|e| panic!("{e}"));
        let after = am.app_state("lifecycle");
        assert_eq!(after.state, "CLOSED");
        assert_eq!(after.instances[0].state, InstanceState::Closed);
    }

    #[test]
    fn invalid_ids_rejected_at_definition_time() {
        assert!(AppDefinition::new("BAD ID", "x", "0.1.0", "/bin/true", &[]).is_err());
    }
}
