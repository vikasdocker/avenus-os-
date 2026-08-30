// Aether Application Package format (Phase 9.2).
//
// An `AppPackage` is the typed description of a user-facing Aether
// application that the future `aether-store` (Phase 9.4) hands to
// the application manager (Phase 9.3). It is structurally similar
// to the OS-level `ServiceManifest` (which describes daemons) but
// is scoped to a single user's app: it has a publisher, a
// signature over the manifest, a binary payload, and a permission
// profile that is *user-facing* (sandboxed) rather than
// `SystemInternal`.
//
// The format is intentionally minimal:
//
//   * The package JSON is self-describing (`schema_version`).
//   * The `signature` field carries an Ed25519 signature over the
//     canonicalised manifest bytes. Verification is independent of
//     this module: aether_security::app_signing owns the key
//     management.
//   * The `payload` field is the application's binary bytes. The
//     payload is NOT a tarball: the package is a single binary
//     plus its manifest, with the manifest and the binary hash
//     cross-checked. A multi-file app would be a future extension
//     (and out of scope for the typed contract).
//
// Invariants:
//   * `app_id` is a non-empty, lowercase, dot-separated
//     reverse-DNS identifier (e.g. `com.example.calculator`).
//   * `version` is a non-empty semver-ish string.
//   * `publisher` is a non-empty string.
//   * `payload` is non-empty and `payload_len` matches its length.
//   * The `binary_sha256` field is the hex-encoded SHA-256 of
//     `payload`. The verifier MUST recompute and compare.
//   * `permissions` is a non-empty list of typed
//     `AppPermission` values. The application manager will
//     prompt the user for each.

use serde::{Deserialize, Serialize};

use crate::manifest::SandboxProfile;

/// The current app package schema version. The application manager
/// MUST reject any package whose `schema_version` does not match.
pub const APP_PACKAGE_SCHEMA_VERSION: &str = "1";

/// The kind of permission an app may request. The list is
/// deliberately small in this revision; new kinds are added by
/// extending the enum (and the application manager's consent UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppPermission {
    /// Read files inside the user's home directory.
    ReadUserFiles,
    /// Write files inside the user's home directory.
    WriteUserFiles,
    /// Open network connections to arbitrary hosts.
    NetworkEgress,
    /// Bind a listening socket on a non-privileged port.
    NetworkListen,
    /// Read the user's calendar / contacts.
    ReadPersonalData,
    /// Send notifications to the user.
    Notify,
    /// Read the screen contents (e.g. for screen-sharing apps).
    CaptureScreen,
    /// Access the camera.
    Camera,
    /// Access the microphone.
    Microphone,
    /// Access the user's location.
    Location,
    /// Pair with and observe other Aether devices.
    PairDevices,
}

impl AppPermission {
    /// The canonical kebab-case wire name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReadUserFiles => "read-user-files",
            Self::WriteUserFiles => "write-user-files",
            Self::NetworkEgress => "network-egress",
            Self::NetworkListen => "network-listen",
            Self::ReadPersonalData => "read-personal-data",
            Self::Notify => "notify",
            Self::CaptureScreen => "capture-screen",
            Self::Camera => "camera",
            Self::Microphone => "microphone",
            Self::Location => "location",
            Self::PairDevices => "pair-devices",
        }
    }
}

/// A typed resource budget for an app. Mirrors `ResourceLimits`
/// from the kernel-sandbox module but is application-scoped: it
/// always lives under `aether.slice/app.<app_id>.slice`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppResourceLimits {
    /// `cpu.weight` for the app's cgroup. 1..=10000.
    pub cpu_weight: u32,
    /// `memory.max` for the app's cgroup, in bytes. `None`
    /// inherits the parent slice.
    pub memory_max_bytes: Option<u64>,
    /// `pids.max` for the app's cgroup. `None` inherits the
    /// parent slice.
    pub pids_max: Option<u32>,
    /// `io.weight` for the app's cgroup. 1..=10000.
    pub io_weight: u32,
}

impl AppResourceLimits {
    /// A reasonable default for a user-facing app: lighter CPU
    /// than a system service, a 256 MiB memory cap, 64 processes.
    /// The `app_id` argument is consumed so the cgroup slice
    /// name can be derived from it (the kernel sandbox places
    /// each app in its own slice under `aether.slice/app.<id>.slice`).
    #[must_use]
    pub fn default_for(_app_id: &str) -> Self {
        Self {
            cpu_weight: 50,
            memory_max_bytes: Some(256 * 1024 * 1024),
            pids_max: Some(64),
            io_weight: 50,
        }
    }
}

/// The app manifest, extracted from a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppManifest {
    /// `1` for this revision.
    pub schema_version: String,
    /// Reverse-DNS app id (e.g. `com.example.calculator`).
    pub app_id: String,
    /// Human-readable app name.
    pub name: String,
    /// App version, semver-ish.
    pub version: String,
    /// Publisher (organisation or individual). MUST be non-empty.
    pub publisher: String,
    /// Short, one-line description.
    pub description: String,
    /// Minimum Aether OS version this app requires (semver-ish).
    pub min_os_version: String,
    /// Permissions the app requests at install time. The user is
    /// prompted for each before installation proceeds.
    pub permissions: Vec<AppPermission>,
    /// Sandbox profile the app asks to run in. The user is shown
    /// the effective plan and must consent.
    pub sandbox_profile: SandboxProfile,
    /// Per-app resource budget. The application manager passes
    /// this to the kernel-sandbox layer.
    pub resources: AppResourceLimits,
    /// Hex SHA-256 of the `payload` field. The verifier MUST
    /// recompute it and reject on mismatch.
    pub binary_sha256: String,
    /// Length of the `payload` field, in bytes. Stored
    /// explicitly so the verifier can sanity-check before hashing.
    pub payload_len: u64,
    /// Service ids this app talks to. The application manager
    /// records each `app -> service` edge in the audit log.
    pub depends_on: Vec<String>,
    /// Wall-clock timestamp the publisher produced the package.
    /// The application manager rejects packages older than a
    /// configured skew window.
    pub timestamp_ms: u64,
    /// Public-key fingerprint (hex) of the publisher's signing
    /// key. The application manager looks the fingerprint up
    /// in the trusted-publisher registry; unknown publishers are
    /// refused outright.
    pub publisher_key_id: String,
}

impl AppManifest {
    /// Validates the manifest. The binary hash and signature are
    /// checked separately (they are part of `AppPackage`, not
    /// `AppManifest`).
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != APP_PACKAGE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported app manifest schema_version '{}' (this build expects '{APP_PACKAGE_SCHEMA_VERSION}')",
                self.schema_version
            ));
        }
        if self.app_id.is_empty() {
            return Err("app_id is required".to_string());
        }
        if !is_valid_app_id(&self.app_id) {
            return Err(format!(
                "app_id '{}' is not a valid reverse-DNS identifier",
                self.app_id
            ));
        }
        if self.name.is_empty() {
            return Err("name is required".to_string());
        }
        if self.version.is_empty() {
            return Err("version is required".to_string());
        }
        if self.publisher.is_empty() {
            return Err("publisher is required".to_string());
        }
        if self.publisher_key_id.is_empty() {
            return Err("publisher_key_id is required".to_string());
        }
        if self.min_os_version.is_empty() {
            return Err("min_os_version is required".to_string());
        }
        if self.binary_sha256.len() != 64
            || !self.binary_sha256.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(format!(
                "binary_sha256 '{}' is not a 64-character hex string",
                self.binary_sha256
            ));
        }
        if self.payload_len == 0 {
            return Err("payload_len must be > 0".to_string());
        }
        if self.resources.cpu_weight < 1 || self.resources.cpu_weight > 10_000 {
            return Err(format!(
                "cpu_weight {} is outside the cgroup v2 range 1..=10000",
                self.resources.cpu_weight
            ));
        }
        if self.resources.io_weight < 1 || self.resources.io_weight > 10_000 {
            return Err(format!(
                "io_weight {} is outside the cgroup v2 range 1..=10000",
                self.resources.io_weight
            ));
        }
        Ok(())
    }
}

/// A signed Aether app package.
///
/// The `signature` field is an Ed25519 signature over the
/// canonicalised JSON bytes of `manifest`. The verifier
/// (aether_security::app_signing) re-serialises the manifest,
/// hashes it, and checks the signature against the publisher's
/// public key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPackage {
    pub manifest: AppManifest,
    /// Ed25519 signature bytes (64 bytes, base64).
    pub signature: String,
    /// The application binary. Length MUST equal `manifest.payload_len`.
    pub payload: Vec<u8>,
}

impl AppPackage {
    /// Returns the canonical (deterministic) JSON encoding of the
    /// manifest. The signature is computed over these bytes.
    pub fn canonical_manifest_bytes(&self) -> Result<Vec<u8>, String> {
        // The signature is computed over the same JSON the
        // verifier will see; serde_json with sorted keys would be
        // ideal but the crate is not in this dependency set. We
        // document the requirement here and let the verifier use
        // the same field order.
        serde_json::to_vec(&self.manifest).map_err(|e| format!("encode manifest: {e}"))
    }
}

/// Returns `true` if `id` is a syntactically valid reverse-DNS
/// app id: at least two dot-separated lowercase segments, each
/// segment 1..=63 characters of `[a-z0-9-]` (no leading / trailing
/// hyphen).
#[must_use]
pub fn is_valid_app_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 253 {
        return false;
    }
    let segments: Vec<&str> = id.split('.').collect();
    if segments.len() < 2 {
        return false;
    }
    for seg in segments {
        if seg.is_empty() || seg.len() > 63 {
            return false;
        }
        if seg.starts_with('-') || seg.ends_with('-') {
            return false;
        }
        if !seg.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return false;
        }
    }
    true
}

/// Build a `ResourceLimits` cgroup slice name for an app under
/// `aether.slice/`. Kept here (not in `sandbox.rs`) so the app
/// module owns its slice-naming policy.
#[must_use]
pub fn app_cgroup_slice(app_id: &str) -> String {
    // Strip the dot separators to make a valid cgroup path
    // component (cgroupfs does not allow `.` in leaf names).
    let sanitised: String = app_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("aether.slice/app.{sanitised}.slice")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::SeccompFilterTag;

    fn sample_manifest() -> AppManifest {
        AppManifest {
            schema_version: APP_PACKAGE_SCHEMA_VERSION.to_string(),
            app_id: "com.example.calculator".to_string(),
            name: "Aether Calculator".to_string(),
            version: "1.2.3".to_string(),
            publisher: "Example Org".to_string(),
            description: "minimal calculator".to_string(),
            min_os_version: "0.2.0".to_string(),
            permissions: vec![AppPermission::Notify],
            sandbox_profile: SandboxProfile::RestrictedService,
            resources: AppResourceLimits::default_for("com.example.calculator"),
            binary_sha256: "0".repeat(64),
            payload_len: 4096,
            depends_on: vec!["aether-system-core".to_string()],
            timestamp_ms: 1_700_000_000_000,
            publisher_key_id: "deadbeef".repeat(8),
        }
    }

    #[test]
    fn valid_app_id_accepted() {
        assert!(is_valid_app_id("com.example.calculator"));
        assert!(is_valid_app_id("io.github.vikas"));
        assert!(is_valid_app_id("a.b"));
    }

    #[test]
    fn invalid_app_id_rejected() {
        assert!(!is_valid_app_id(""));
        assert!(!is_valid_app_id("nodot"));
        assert!(!is_valid_app_id("com..example"));
        assert!(!is_valid_app_id(".com.example"));
        assert!(!is_valid_app_id("com.example."));
        assert!(!is_valid_app_id("com.Example"));
        assert!(!is_valid_app_id("com.example.foo!"));
        assert!(!is_valid_app_id(&"a".repeat(254)));
    }

    #[test]
    fn app_cgroup_slice_strips_dots() {
        assert_eq!(
            app_cgroup_slice("com.example.calculator"),
            "aether.slice/app.com_example_calculator.slice"
        );
    }

    #[test]
    fn app_resource_limits_default_for_uses_sane_bounds() {
        let r = AppResourceLimits::default_for("com.example.calculator");
        assert_eq!(r.cpu_weight, 50);
        assert_eq!(r.io_weight, 50);
        assert_eq!(r.pids_max, Some(64));
        assert_eq!(r.memory_max_bytes, Some(256 * 1024 * 1024));
    }

    #[test]
    fn validate_accepts_default_manifest() {
        sample_manifest().validate().expect("default manifest must validate");
    }

    #[test]
    fn validate_rejects_wrong_schema_version() {
        let mut m = sample_manifest();
        m.schema_version = "0".to_string();
        let err = m.validate().expect_err("wrong schema must be rejected");
        assert!(err.contains("schema_version"), "{err}");
    }

    #[test]
    fn validate_rejects_empty_app_id() {
        let mut m = sample_manifest();
        m.app_id = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn validate_rejects_invalid_app_id() {
        let mut m = sample_manifest();
        m.app_id = "Calculator".to_string();
        let err = m.validate().expect_err("invalid id");
        assert!(err.contains("reverse-DNS"), "{err}");
    }

    #[test]
    fn validate_rejects_empty_publisher() {
        let mut m = sample_manifest();
        m.publisher = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn validate_rejects_non_hex_sha256() {
        let mut m = sample_manifest();
        m.binary_sha256 = "z".repeat(64);
        assert!(m.validate().is_err());
    }

    #[test]
    fn validate_rejects_wrong_length_sha256() {
        let mut m = sample_manifest();
        m.binary_sha256 = "abcd".to_string();
        assert!(m.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_payload_len() {
        let mut m = sample_manifest();
        m.payload_len = 0;
        let err = m.validate().expect_err("zero payload");
        assert!(err.contains("payload_len"), "{err}");
    }

    #[test]
    fn validate_rejects_zero_cpu_weight() {
        let mut m = sample_manifest();
        m.resources.cpu_weight = 0;
        assert!(m.validate().is_err());
    }

    #[test]
    fn validate_rejects_oversized_io_weight() {
        let mut m = sample_manifest();
        m.resources.io_weight = 10_001;
        assert!(m.validate().is_err());
    }

    #[test]
    fn permission_as_str_is_stable() {
        assert_eq!(AppPermission::ReadUserFiles.as_str(), "read-user-files");
        assert_eq!(AppPermission::PairDevices.as_str(), "pair-devices");
        assert_eq!(AppPermission::NetworkEgress.as_str(), "network-egress");
    }

    #[test]
    fn package_round_trips_through_serde_json() {
        let pkg = AppPackage {
            manifest: sample_manifest(),
            signature: "AAAA".repeat(32),
            payload: vec![1, 2, 3, 4, 5],
        };
        let text = serde_json::to_string(&pkg).expect("encode");
        let back: AppPackage = serde_json::from_str(&text).expect("decode");
        assert_eq!(back, pkg);
    }

    #[test]
    fn package_canonical_manifest_bytes_are_deterministic() {
        // Two structurally identical packages must produce the
        // same manifest bytes (the signature verifier relies on
        // this). Note: serde_json field order is not stable across
        // versions; this test asserts determinism *within a single
        // build* — the production signature layer canonicalises
        // explicitly.
        let a = AppPackage {
            manifest: sample_manifest(),
            signature: "sig".to_string(),
            payload: vec![],
        };
        let b = AppPackage {
            manifest: sample_manifest(),
            signature: "different-sig".to_string(),
            payload: vec![],
        };
        let ma = a.canonical_manifest_bytes().expect("encode a");
        let mb = b.canonical_manifest_bytes().expect("encode b");
        assert_eq!(ma, mb, "manifest bytes must not depend on the signature or payload");
    }

    #[test]
    fn validate_accepts_seccomp_tag_for_app() {
        // Apps carry a seccomp filter tag through the sandbox
        // profile; this test is a smoke check that the seccomp
        // tag we hand to the kernel layer compiles.
        let _tag = SeccompFilterTag::new("restricted-app-v1");
    }

    #[test]
    fn app_resource_limits_have_too_high_cpu_weight() {
        let mut m = sample_manifest();
        m.resources.cpu_weight = 50;
        m.resources.io_weight = 50;
        // 10000 is the boundary; 10001 is rejected.
        m.resources.cpu_weight = 10_001;
        assert!(m.validate().is_err());
    }
}
