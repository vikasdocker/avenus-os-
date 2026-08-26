// Aether System Core - manifest loading.
//
// Reads service manifests from a directory. Every `*.json` file must
// contain exactly one manifest that validates against the shared schema.

use aether_core::error::{AetherError, ErrorKind};
use aether_core::manifest::ServiceManifest;
use std::path::Path;

/// Loads and validates all manifests in a directory, sorted by service id.
pub fn load_manifests_from_dir(dir: &Path) -> Result<Vec<ServiceManifest>, AetherError> {
    let mut manifests: Vec<ServiceManifest> = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        AetherError::new(
            ErrorKind::NotFound,
            format!("cannot read manifest directory {}: {e}", dir.display()),
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(AetherError::from)?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            AetherError::new(
                ErrorKind::Io,
                format!("cannot read manifest {}: {e}", path.display()),
            )
        })?;
        let manifest: ServiceManifest = serde_json::from_str(&raw).map_err(|e| {
            AetherError::new(
                ErrorKind::InvalidInput,
                format!("invalid manifest {}: {e}", path.display()),
            )
        })?;
        manifest.validate()?;
        manifests.push(manifest);
    }

    manifests.sort_by(|a, b| a.service_id.cmp(&b.service_id));
    Ok(manifests)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_json(id: &str) -> String {
        format!(
            r#"{{
                "schema_version": "1",
                "service_id": "{id}",
                "name": "{id}",
                "version": "0.1.0",
                "description": "test",
                "service_type": "Internal",
                "command": null,
                "dependencies": [],
                "startup_priority": 10,
                "restart_policy": "OnFailure",
                "restart_limit": 3,
                "restart_backoff_ms": 10,
                "health_check": null,
                "config_path": null,
                "security_identity": "{id}.aether",
                "ipc_endpoints": [],
                "capabilities": [],
                "resource_cpu_weight": 1.0,
                "resource_memory_max_kib": 1024,
                "resource_process_limit": null,
                "resource_io_weight": 1.0,
                "requires_root": false,
                "sandbox_profile": "Internal",
                "permission_profile": "SystemInternal",
                "ipc_access": "LocalPrivate",
                "shutdown_timeout_ms": 100
            }}"#
        )
    }

    #[test]
    fn loads_and_sorts_manifests() {
        let tmp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(tmp.path().join("b.json"), manifest_json("bravo"))
            .unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(tmp.path().join("a.json"), manifest_json("alpha"))
            .unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(tmp.path().join("notes.txt"), "ignored")
            .unwrap_or_else(|e| panic!("{e}"));

        let loaded = load_manifests_from_dir(tmp.path()).unwrap_or_else(|e| panic!("{e}"));
        let ids: Vec<&str> = loaded.iter().map(|m| m.service_id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "bravo"]);
    }

    #[test]
    fn invalid_schema_rejected() {
        let tmp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let bad = manifest_json("bad").replace("\"schema_version\": \"1\"", "\"schema_version\": \"9\"");
        std::fs::write(tmp.path().join("bad.json"), bad).unwrap_or_else(|e| panic!("{e}"));
        match load_manifests_from_dir(tmp.path()) {
            Err(err) => assert_eq!(err.code, ErrorKind::InvalidInput),
            Ok(m) => panic!("expected invalid input error, got {m:?}"),
        }
    }

    #[test]
    fn missing_directory_rejected() {
        match load_manifests_from_dir(Path::new("/definitely/not/here")) {
            Err(err) => assert_eq!(err.code, ErrorKind::NotFound),
            Ok(m) => panic!("expected not-found error, got {m:?}"),
        }
    }
}
