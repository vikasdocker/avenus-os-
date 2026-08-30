// Aether System Core - manifest loading.
//
// Reads service manifests from a directory. Every `*.json` file must
// contain exactly one manifest that validates against the shared schema.
//
// When a trust store is supplied, the loader also looks for a sibling
// `*.json.sig` file containing a `SignedManifest` envelope and verifies
// the manifest's signature against the store. The default
// `load_manifests_from_dir` keeps the historic unsigned behaviour
// (used by dev / test paths); production callers should use
// `load_manifests_with_trust`.

use aether_core::error::{AetherError, ErrorKind};
use aether_core::manifest::ServiceManifest;
use aether_security::manifest_signing::{verify_signed_manifest, SignedManifest, TrustStore};
use std::path::Path;

/// Loads and validates all manifests in a directory, sorted by
/// service id. No signature verification is performed.
pub fn load_manifests_from_dir(dir: &Path) -> Result<Vec<ServiceManifest>, AetherError> {
    load_manifests_with_trust(dir, None)
}

/// Loads and validates every manifest in a directory, sorted by
/// service id, and (if `trust` is `Some`) verifies each manifest's
/// signature against the supplied trust store.
///
/// When a trust store is supplied, every manifest MUST have a
/// companion `<name>.json.sig` envelope. A missing signature is
/// rejected, an untrusted signer is rejected, and a tampered
/// manifest is rejected — the failure happens at load time, before
/// any service is spawned.
///
/// When `trust` is `None`, signatures are skipped entirely (dev
/// mode).
pub fn load_manifests_with_trust(
    dir: &Path,
    trust: Option<&TrustStore>,
) -> Result<Vec<ServiceManifest>, AetherError> {
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
        // Avoid recursing into the signature files
        // themselves: they end in `.json.sig`, not `.json`.
        if path.to_string_lossy().ends_with(".json.sig") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            AetherError::new(ErrorKind::Io, format!("cannot read manifest {}: {e}", path.display()))
        })?;

        // Verify the signature before parsing the JSON.
        // A failed signature must not produce a partially
        // trusted manifest.
        if let Some(trust_store) = trust {
            let sig_path = sig_path_for(&path);
            let sig_text = std::fs::read_to_string(&sig_path).map_err(|e| {
                AetherError::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "missing signature file for manifest {}: expected {} ({e})",
                        path.display(),
                        sig_path.display()
                    ),
                )
            })?;
            let envelope: SignedManifest = serde_json::from_str(&sig_text).map_err(|e| {
                AetherError::new(
                    ErrorKind::InvalidInput,
                    format!("invalid signature envelope {}: {e}", sig_path.display()),
                )
            })?;
            // The signed manifest bytes must equal the
            // manifest bytes we just read. This guards
            // against an attacker who swaps the manifest
            // but reuses an old signature for a different
            // payload.
            if envelope.manifest_bytes != raw.as_bytes() {
                return Err(AetherError::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "signature envelope bytes do not match manifest bytes for {}",
                        path.display()
                    ),
                ));
            }
            verify_signed_manifest(&envelope, trust_store).map_err(|e| {
                AetherError::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "manifest {} failed signature verification: {e}",
                        path.display()
                    ),
                )
            })?;
        }

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

/// Returns the signature file path corresponding to
/// `manifest_path`. The path's extension is the literal
/// `.json.sig` (two extensions). Example: `aether-agentd.json`
/// becomes `aether-agentd.json.sig`.
fn sig_path_for(manifest_path: &Path) -> std::path::PathBuf {
    let mut s = manifest_path.as_os_str().to_os_string();
    s.push(".sig");
    std::path::PathBuf::from(s)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
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
        std::fs::write(tmp.path().join("notes.txt"), "ignored").unwrap_or_else(|e| panic!("{e}"));

        let loaded = load_manifests_from_dir(tmp.path()).unwrap_or_else(|e| panic!("{e}"));
        let ids: Vec<&str> = loaded.iter().map(|m| m.service_id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "bravo"]);
    }

    #[test]
    fn invalid_schema_rejected() {
        let tmp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let bad =
            manifest_json("bad").replace("\"schema_version\": \"1\"", "\"schema_version\": \"9\"");
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

    #[test]
    fn load_manifests_with_trust_accepts_signed_manifest() {
        use aether_security::manifest_signing::{Ed25519ManifestSigner, TrustStore};
        let signer = Ed25519ManifestSigner::generate();
        let mut trust = TrustStore::new();
        trust.trust(signer.fingerprint());

        let tmp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let json = manifest_json("alpha");
        let manifest_path = tmp.path().join("alpha.json");
        std::fs::write(&manifest_path, &json).unwrap_or_else(|e| panic!("{e}"));

        let signed = signer.sign(json.as_bytes());
        let sig_text = serde_json::to_string(&signed).expect("envelope serialises");
        let sig_path = {
            let mut s = manifest_path.as_os_str().to_os_string();
            s.push(".sig");
            std::path::PathBuf::from(s)
        };
        std::fs::write(&sig_path, sig_text).unwrap_or_else(|e| panic!("{e}"));

        let loaded =
            load_manifests_with_trust(tmp.path(), Some(&trust)).expect("signed manifest loads");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].service_id, "alpha");
    }

    #[test]
    fn load_manifests_with_trust_rejects_missing_signature() {
        let trust = TrustStore::new();
        let tmp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(tmp.path().join("a.json"), manifest_json("a")).unwrap();
        let err = load_manifests_with_trust(tmp.path(), Some(&trust)).unwrap_err();
        assert_eq!(err.code, ErrorKind::InvalidInput);
        assert!(err.message.contains("missing signature"));
    }

    #[test]
    fn load_manifests_with_trust_rejects_unknown_signer() {
        use aether_security::manifest_signing::{Ed25519ManifestSigner, TrustStore};
        let signer = Ed25519ManifestSigner::generate();
        // Trust store is empty — no signer is trusted.
        let trust = TrustStore::new();

        let tmp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let json = manifest_json("a");
        let manifest_path = tmp.path().join("a.json");
        std::fs::write(&manifest_path, &json).unwrap();
        let signed = signer.sign(json.as_bytes());
        let sig_text = serde_json::to_string(&signed).expect("envelope serialises");
        let sig_path = {
            let mut s = manifest_path.as_os_str().to_os_string();
            s.push(".sig");
            std::path::PathBuf::from(s)
        };
        std::fs::write(&sig_path, sig_text).unwrap();
        let err = load_manifests_with_trust(tmp.path(), Some(&trust)).unwrap_err();
        assert_eq!(err.code, ErrorKind::InvalidInput);
        assert!(err.message.contains("not in the trust store"));
    }

    #[test]
    fn load_manifests_with_trust_rejects_tampered_manifest() {
        use aether_security::manifest_signing::{Ed25519ManifestSigner, TrustStore};
        let signer = Ed25519ManifestSigner::generate();
        let mut trust = TrustStore::new();
        trust.trust(signer.fingerprint());

        let tmp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let json = manifest_json("a");
        let manifest_path = tmp.path().join("a.json");
        // Sign the original manifest.
        let signed = signer.sign(json.as_bytes());
        // Then mutate the manifest on disk without
        // re-signing.
        let tampered = json.replace("0.1.0", "9.9.9");
        std::fs::write(&manifest_path, &tampered).unwrap();
        let sig_text = serde_json::to_string(&signed).expect("envelope serialises");
        let sig_path = {
            let mut s = manifest_path.as_os_str().to_os_string();
            s.push(".sig");
            std::path::PathBuf::from(s)
        };
        std::fs::write(&sig_path, sig_text).unwrap();
        let err = load_manifests_with_trust(tmp.path(), Some(&trust)).unwrap_err();
        assert_eq!(err.code, ErrorKind::InvalidInput);
        // Either the manifest-bytes mismatch or the
        // signature mismatch will fire first; both
        // indicate tampering.
        assert!(
            err.message.contains("do not match") || err.message.contains("verification failed"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn load_manifests_with_trust_none_skips_verification() {
        // An unsigned manifest in trust=None mode
        // continues to load. This is the dev / test path.
        let tmp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(tmp.path().join("a.json"), manifest_json("a")).unwrap();
        let loaded =
            load_manifests_with_trust(tmp.path(), None).expect("unsigned manifest loads in dev");
        assert_eq!(loaded.len(), 1);
    }
}
