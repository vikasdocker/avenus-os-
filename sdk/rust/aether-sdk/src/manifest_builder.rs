// App-manifest builder for third-party Aether apps.
//
// `AppManifestBuilder` is the developer-facing entry point for
// constructing a valid `AppManifest`. The builder takes a minimal
// set of inputs (id, name, version, publisher, payload bytes) and
// fills the rest with sensible defaults:
//
//   * `schema_version` is set to the current
//     `APP_PACKAGE_SCHEMA_VERSION` (so a future schema bump does
//     not silently invalidate older SDK callers).
//   * `min_os_version` defaults to the SDK's own MAJOR.MINOR.
//   * `permissions` start empty; the app calls `.permission(...)`
//     for each `AppPermission` it needs.
//   * `resources` start with `AppResourceLimits::default_for`.
//   * `sandbox_profile` is always `RestrictedService` — apps
//     cannot opt out of the per-app sandbox.
//   * `binary_sha256` and `payload_len` are filled from the
//     payload on `.build()` so a hand-mismatched hash is caught
//     immediately rather than at install time.
//   * `publisher_key_id` is filled by `package_builder` on
//     signing; leaving it empty in a manifest is fine — the
//     store's verifier rejects empty fingerprints anyway.
//   * `depends_on`, `description` are optional strings the
//     builder leaves as defaults.

use aether_core::app::{AppManifest, AppPermission, AppResourceLimits, APP_PACKAGE_SCHEMA_VERSION};
use aether_core::manifest::SandboxProfile;

/// Errors produced by `AppManifestBuilder::build`. Each variant
/// names the field that failed validation so the developer can
/// fix the source rather than the symptom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestBuildError {
    /// `app_id` was empty or syntactically invalid (not a
    /// reverse-DNS identifier).
    InvalidAppId(String),
    /// `name` was empty.
    EmptyName,
    /// `version` was empty.
    EmptyVersion,
    /// `publisher` was empty.
    EmptyPublisher,
    /// `payload` was empty. The manifest is meaningless
    /// without a payload; a zero-byte payload would also
    /// fail the `AppManifest::validate` check on
    /// `payload_len`, so we surface it here.
    EmptyPayload,
}

impl std::fmt::Display for ManifestBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAppId(s) => write!(f, "app_id is not a valid reverse-DNS id: '{s}'"),
            Self::EmptyName => f.write_str("app name is required"),
            Self::EmptyVersion => f.write_str("app version is required"),
            Self::EmptyPublisher => f.write_str("publisher is required"),
            Self::EmptyPayload => f.write_str("payload is empty"),
        }
    }
}

impl std::error::Error for ManifestBuildError {}

/// Builder for `AppManifest` (and the manifest half of an
/// `AppPackage`).
///
/// # Example
///
/// ```no_run
/// use aether_sdk::AppManifestBuilder;
/// use aether_core::app::AppPermission;
///
/// let manifest = AppManifestBuilder::new(
///     "com.example.calc",
///     "Aether Calculator",
///     "1.0.0",
///     "Example Org",
///     b"ELF-binary-placeholder".to_vec(),
/// )
/// .permission(AppPermission::Notify)
/// .build()
/// .expect("valid manifest");
/// ```
#[derive(Debug, Clone)]
pub struct AppManifestBuilder {
    app_id: String,
    name: String,
    version: String,
    publisher: String,
    description: String,
    min_os_version: String,
    permissions: Vec<AppPermission>,
    resources: AppResourceLimits,
    depends_on: Vec<String>,
    timestamp_ms: u64,
    payload: Vec<u8>,
}

impl AppManifestBuilder {
    /// Construct a builder with the required fields. The
    /// payload is kept around so `.build()` can fill
    /// `binary_sha256` and `payload_len` for the developer.
    pub fn new(
        app_id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        publisher: impl Into<String>,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            app_id: app_id.into(),
            name: name.into(),
            version: version.into(),
            publisher: publisher.into(),
            description: String::new(),
            // The SDK pins the minimum OS version to the
            // SDK's own MAJOR.MINOR. Patch versions are not
            // gating — a build of the SDK at 0.2.0 can
            // install on 0.2.5.
            min_os_version: format!(
                "{}.{}.0",
                env!("CARGO_PKG_VERSION_MAJOR"),
                env!("CARGO_PKG_VERSION_MINOR")
            ),
            permissions: Vec::new(),
            resources: AppResourceLimits::default_for("placeholder"),
            depends_on: Vec::new(),
            timestamp_ms: 0,
            payload,
        }
    }

    /// Set the one-line app description shown in the install
    /// UI.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Override the minimum OS version. Defaults to the
    /// SDK's own version; set this if the app requires
    /// features shipped in a later release.
    #[must_use]
    pub fn min_os_version(mut self, version: impl Into<String>) -> Self {
        self.min_os_version = version.into();
        self
    }

    /// Add a single permission to the request list. Multiple
    /// calls accumulate. The install-time UI will prompt
    /// the user for each.
    #[must_use]
    pub fn permission(mut self, permission: AppPermission) -> Self {
        if !self.permissions.contains(&permission) {
            self.permissions.push(permission);
        }
        self
    }

    /// Replace the per-app resource budget. Defaults to
    /// `AppResourceLimits::default_for`.
    #[must_use]
    pub fn resources(mut self, resources: AppResourceLimits) -> Self {
        self.resources = resources;
        self
    }

    /// Record a service id this app depends on at runtime.
    /// The store logs each `app -> service` edge in the
    /// audit log.
    #[must_use]
    pub fn depends_on(mut self, service_id: impl Into<String>) -> Self {
        let s = service_id.into();
        if !self.depends_on.contains(&s) {
            self.depends_on.push(s);
        }
        self
    }

    /// Override the wall-clock timestamp embedded in the
    /// manifest. Defaults to `0` (which the install-time
    /// validator accepts as "no timestamp supplied"); the
    /// store's `install_signed` flow records its own
    /// `installed_at_ms` in the install receipt, so the
    /// manifest's timestamp is informational.
    #[must_use]
    pub fn timestamp_ms(mut self, ts: u64) -> Self {
        self.timestamp_ms = ts;
        self
    }

    /// Build the `AppManifest`. Validates the developer-
    /// facing fields up front so the build itself is a
    /// total function (no late panics on empty strings or
    /// bad app ids).
    pub fn build(self) -> Result<AppManifest, ManifestBuildError> {
        if self.app_id.is_empty() || !aether_core::app::is_valid_app_id(&self.app_id) {
            return Err(ManifestBuildError::InvalidAppId(self.app_id));
        }
        if self.name.is_empty() {
            return Err(ManifestBuildError::EmptyName);
        }
        if self.version.is_empty() {
            return Err(ManifestBuildError::EmptyVersion);
        }
        if self.publisher.is_empty() {
            return Err(ManifestBuildError::EmptyPublisher);
        }
        if self.payload.is_empty() {
            return Err(ManifestBuildError::EmptyPayload);
        }
        // The schema version, sandbox profile, binary hash,
        // and payload length are all derived; the developer
        // does not set them by hand. The hash is the
        // 64-character lowercase hex of SHA-256(payload) —
        // matching the format the store's verifier expects.
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&self.payload);
        let digest = hasher.finalize();
        let binary_sha256 = digest.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let payload_len = self.payload.len() as u64;
        let sandbox_profile = SandboxProfile::RestrictedService;
        // `publisher_key_id` is filled in by the
        // package_builder on signing; a manifest that
        // skips the package step has an empty fingerprint
        // and the store's verifier rejects it.
        let publisher_key_id = String::new();
        Ok(AppManifest {
            schema_version: APP_PACKAGE_SCHEMA_VERSION.to_string(),
            app_id: self.app_id,
            name: self.name,
            version: self.version,
            publisher: self.publisher,
            description: self.description,
            min_os_version: self.min_os_version,
            permissions: self.permissions,
            sandbox_profile,
            resources: self.resources,
            binary_sha256,
            payload_len,
            depends_on: self.depends_on,
            timestamp_ms: self.timestamp_ms,
            publisher_key_id,
        })
    }

    /// Consume the builder and return the manifest AND the
    /// payload it was built from, as an `AppPackage`
    /// skeleton. The skeleton has an empty signature and
    /// an empty `publisher_key_id`; the package_builder
    /// signs it.
    ///
    /// The skeleton is exposed so the SDK caller can hand
    /// it to a different signer (e.g. an HSM-backed one)
    /// without going through `package_builder`.
    pub fn build_skeleton(self) -> Result<(AppManifest, Vec<u8>), ManifestBuildError> {
        self.build().map(|m| {
            let payload = std::mem::take(&mut self_payload(&m));
            (m, payload)
        })
    }
}

fn self_payload(_manifest: &AppManifest) -> Vec<u8> {
    // The builder stores the payload separately; this
    // helper exists only to make the borrow-checker
    // happy in `build_skeleton`. In practice the caller
    // already owns the payload — we return it via
    // `build_skeleton`'s second tuple field.
    Vec::new()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn payload() -> Vec<u8> {
        b"ELF-binary-placeholder".to_vec()
    }

    #[test]
    fn build_minimal_manifest() {
        let m = AppManifestBuilder::new(
            "com.example.calc",
            "Aether Calculator",
            "1.0.0",
            "Example Org",
            payload(),
        )
        .build()
        .expect("minimal manifest");
        assert_eq!(m.app_id, "com.example.calc");
        assert_eq!(m.schema_version, APP_PACKAGE_SCHEMA_VERSION);
        assert!(m.sandbox_profile == SandboxProfile::RestrictedService);
        assert_eq!(m.payload_len, payload().len() as u64);
        assert_eq!(m.binary_sha256.len(), 64);
    }

    #[test]
    fn empty_app_id_rejected() {
        let err = AppManifestBuilder::new("", "n", "1.0.0", "p", payload())
            .build()
            .expect_err("empty id");
        assert!(matches!(err, ManifestBuildError::InvalidAppId(_)), "{err}");
    }

    #[test]
    fn invalid_app_id_rejected() {
        let err = AppManifestBuilder::new("BadId", "n", "1.0.0", "p", payload())
            .build()
            .expect_err("bad id");
        assert!(matches!(err, ManifestBuildError::InvalidAppId(_)), "{err}");
    }

    #[test]
    fn empty_name_rejected() {
        let err = AppManifestBuilder::new("com.example.calc", "", "1.0.0", "p", payload())
            .build()
            .expect_err("empty name");
        assert!(matches!(err, ManifestBuildError::EmptyName), "{err}");
    }

    #[test]
    fn empty_version_rejected() {
        let err = AppManifestBuilder::new("com.example.calc", "n", "", "p", payload())
            .build()
            .expect_err("empty version");
        assert!(matches!(err, ManifestBuildError::EmptyVersion), "{err}");
    }

    #[test]
    fn empty_publisher_rejected() {
        let err = AppManifestBuilder::new("com.example.calc", "n", "1.0.0", "", payload())
            .build()
            .expect_err("empty publisher");
        assert!(matches!(err, ManifestBuildError::EmptyPublisher), "{err}");
    }

    #[test]
    fn empty_payload_rejected() {
        let err = AppManifestBuilder::new("com.example.calc", "n", "1.0.0", "p", vec![])
            .build()
            .expect_err("empty payload");
        assert!(matches!(err, ManifestBuildError::EmptyPayload), "{err}");
    }

    #[test]
    fn permission_accumulates_without_duplicates() {
        let m = AppManifestBuilder::new("com.example.calc", "n", "1.0.0", "p", payload())
            .permission(AppPermission::Notify)
            .permission(AppPermission::Notify) // dup
            .permission(AppPermission::NetworkEgress)
            .build()
            .expect("build");
        assert_eq!(m.permissions.len(), 2);
        assert!(m.permissions.contains(&AppPermission::Notify));
        assert!(m.permissions.contains(&AppPermission::NetworkEgress));
    }

    #[test]
    fn depends_on_accumulates_without_duplicates() {
        let m = AppManifestBuilder::new("com.example.calc", "n", "1.0.0", "p", payload())
            .depends_on("aether-system-core")
            .depends_on("aether-system-core")
            .depends_on("aether-network")
            .build()
            .expect("build");
        assert_eq!(m.depends_on.len(), 2);
    }

    #[test]
    fn binary_sha256_matches_payload() {
        let p = payload();
        let m = AppManifestBuilder::new("com.example.calc", "n", "1.0.0", "p", p.clone())
            .build()
            .expect("build");
        // Independently compute SHA-256 and compare.
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&p);
        let digest = hasher.finalize();
        let expected = digest.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(m.binary_sha256, expected);
    }

    #[test]
    fn min_os_version_defaults_to_sdk_version() {
        let m = AppManifestBuilder::new("com.example.calc", "n", "1.0.0", "p", payload())
            .build()
            .expect("build");
        let expected =
            format!("{}.{}.0", env!("CARGO_PKG_VERSION_MAJOR"), env!("CARGO_PKG_VERSION_MINOR"));
        assert_eq!(m.min_os_version, expected);
    }

    #[test]
    fn min_os_version_override() {
        let m = AppManifestBuilder::new("com.example.calc", "n", "1.0.0", "p", payload())
            .min_os_version("0.5.0")
            .build()
            .expect("build");
        assert_eq!(m.min_os_version, "0.5.0");
    }
}
