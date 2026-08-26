// Aether Application Manager - registry and launch lifecycle.
//
// Owns the installed-application registry (AppManifests) and the set of
// running application instances. Launch policy is deterministic: an app
// id must be registered, and duplicate running instances are rejected.

use aether_apps::{AppManifest, AppManifestError};
use aether_core::ComponentId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A running application instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunningApp {
    pub instance_id: u64,
    pub app_id: String,
}

/// Errors surfaced by the application manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppManagerError {
    UnknownApp(String),
    AlreadyRunning(String),
    NotRunning(u64),
    InvalidId(String),
    Manifest(AppManifestError),
}

impl std::fmt::Display for AppManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownApp(id) => write!(f, "unknown application '{id}'"),
            Self::AlreadyRunning(id) => write!(f, "application '{id}' is already running"),
            Self::NotRunning(instance) => write!(f, "instance {instance} is not running"),
            Self::InvalidId(id) => write!(f, "invalid application id '{id}'; allowed: [a-z0-9._-]"),
            Self::Manifest(e) => write!(f, "invalid manifest: {e}"),
        }
    }
}

impl std::error::Error for AppManagerError {}

impl From<AppManifestError> for AppManagerError {
    fn from(e: AppManifestError) -> Self {
        Self::Manifest(e)
    }
}

/// Registry + running set. Deterministic ordering via BTree collections.
#[derive(Default)]
pub struct ApplicationManager {
    registry: BTreeMap<String, AppManifest>,
    running: BTreeMap<u64, RunningApp>,
    running_ids: BTreeSet<String>,
    next_instance: u64,
}

impl ApplicationManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an application manifest.
    pub fn register(&mut self, manifest: AppManifest) -> Result<(), AppManagerError> {
        self.registry
            .insert(manifest.id().as_str().to_string(), manifest);
        Ok(())
    }

    /// Registers from raw fields, validating the id and manifest.
    pub fn register_json(&mut self, id: &str, display_name: &str, command: &str) -> Result<(), AppManagerError> {
        let component = ComponentId::new(id)
            .map_err(|_| AppManagerError::InvalidId(id.to_string()))?;
        let manifest = AppManifest::new(component, display_name, command)?;
        self.register(manifest)
    }

    /// Lists registered apps sorted by id: `(id, display_name, command)`.
    pub fn list(&self) -> Vec<(String, String, String)> {
        self.registry
            .values()
            .map(|m| {
                (
                    m.id().as_str().to_string(),
                    m.display_name().to_string(),
                    m.command().to_string(),
                )
            })
            .collect()
    }

    /// Launches an app by id, returning its new instance.
    pub fn launch(&mut self, app_id: &str) -> Result<RunningApp, AppManagerError> {
        if !self.registry.contains_key(app_id) {
            return Err(AppManagerError::UnknownApp(app_id.to_string()));
        }
        if self.running_ids.contains(app_id) {
            return Err(AppManagerError::AlreadyRunning(app_id.to_string()));
        }
        self.next_instance += 1;
        let instance = RunningApp {
            instance_id: self.next_instance,
            app_id: app_id.to_string(),
        };
        self.running.insert(instance.instance_id, instance.clone());
        self.running_ids.insert(app_id.to_string());
        Ok(instance)
    }

    /// Closes a running instance by id.
    pub fn close(&mut self, instance_id: u64) -> Result<RunningApp, AppManagerError> {
        let instance = self
            .running
            .remove(&instance_id)
            .ok_or(AppManagerError::NotRunning(instance_id))?;
        self.running_ids.remove(&instance.app_id);
        Ok(instance)
    }

    /// All running instances sorted by instance id.
    pub fn running(&self) -> Vec<&RunningApp> {
        self.running.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_requires_registered_app() {
        let mut am = ApplicationManager::new();
        assert!(matches!(
            am.launch("ghost"),
            Err(AppManagerError::UnknownApp(_))
        ));
    }

    #[test]
    fn single_instance_policy() {
        let mut am = ApplicationManager::new();
        am.register_json("terminal", "Terminal", "/bin/term")
            .unwrap_or_else(|e| panic!("{e}"));
        let first = am.launch("terminal").unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(
            am.launch("terminal"),
            Err(AppManagerError::AlreadyRunning(_))
        ));
        let closed = am.close(first.instance_id).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(closed.app_id, "terminal");
        // Relaunch works after close.
        assert!(am.launch("terminal").is_ok());
    }

    #[test]
    fn list_is_sorted() {
        let mut am = ApplicationManager::new();
        am.register_json("zeta", "Zeta", "/z").unwrap_or_else(|e| panic!("{e}"));
        am.register_json("alpha", "Alpha", "/a").unwrap_or_else(|e| panic!("{e}"));
        let ids: Vec<String> = am.list().into_iter().map(|(id, _, _)| id).collect();
        assert_eq!(ids, vec!["alpha".to_string(), "zeta".to_string()]);
    }

    #[test]
    fn close_unknown_instance_fails() {
        let mut am = ApplicationManager::new();
        assert!(matches!(am.close(42), Err(AppManagerError::NotRunning(42))));
    }
}
