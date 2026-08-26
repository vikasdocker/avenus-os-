// Aether Service Manifest types
use crate::error::AetherError;

/// Service manifest schema version.
pub const MANIFEST_SCHEMA_VERSION: &str = "1";

/// Service manifest - describes a service to the service manager.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceManifest {
    pub schema_version: String,
    pub service_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub service_type: ServiceType,
    pub command: Option<String>,
    pub dependencies: Vec<String>,
    pub startup_priority: u32,
    pub restart_policy: RestartPolicy,
    pub restart_limit: u32,
    pub restart_backoff_ms: u64,
    pub health_check: Option<String>,
    pub config_path: Option<String>,
    pub security_identity: String,
    pub ipc_endpoints: Vec<String>,
    pub capabilities: Vec<String>,
    pub resource_cpu_weight: f64,
    pub resource_memory_max_kib: u64,
    pub resource_process_limit: Option<u32>,
    pub resource_io_weight: f64,
    pub requires_root: bool,
    pub sandbox_profile: SandboxProfile,
    pub permission_profile: PermissionProfile,
    pub ipc_access: IpcAccessMode,
    pub shutdown_timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ServiceType {
    Internal,
    Process,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SandboxProfile {
    Internal,
    SystemService,
    RestrictedService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PermissionProfile {
    SystemInternal,
    ServiceRuntime,
    DeveloperControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IpcAccessMode {
    LocalPrivate,
    LocalPublic,
    Remote,
}

impl ServiceManifest {
    pub fn validate(&self) -> Result<(), AetherError> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(AetherError::new(
                crate::error::ErrorKind::InvalidInput,
                format!("Unsupported manifest schema version: {}", self.schema_version),
            ));
        }
        if self.service_id.is_empty() {
            return Err(AetherError::new(
                crate::error::ErrorKind::InvalidInput,
                "service_id is required",
            ));
        }
        if self.name.is_empty() {
            return Err(AetherError::new(
                crate::error::ErrorKind::InvalidInput,
                "name is required",
            ));
        }
        if self.service_type == ServiceType::Process && self.command.is_none() {
            return Err(AetherError::new(
                crate::error::ErrorKind::InvalidInput,
                "command is required for process services",
            ));
        }
        Ok(())
    }
}