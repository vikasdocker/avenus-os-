// Package builder — assembles and signs an `AppPackage`.
//
// `AppPackageBuilder` is the developer-facing entry point
// for the second half of the packaging flow: turning a
// validated `AppManifest` plus a payload into a signed
// `AppPackage` that the store can verify and install.
//
// The builder hides three implementation details that the
// manifest-only builder leaves alone:
//
//   1. `publisher_key_id` is derived from the signer's
//      Ed25519 public key bytes (the lowercase hex of
//      SHA-256(pubkey)). Manifests leave this empty
//      because the key isn't known until signing.
//   2. The Ed25519 signature is computed over the
//      manifest canonical bytes — not over the
//      manifest + payload. The store verifier
//      recomputes this on install and rejects any
//      manifest whose bytes don't match the signed
//      hash.
//   3. The base64 signature is appended as the
//      `signature` field of the package, alongside
//      the manifest and payload.
//
// Most apps call the convenience constructor
// `AppPackageBuilder::sign` with a freshly generated
// key, persist the key (and the matching
// `publisher_key_id`) out-of-band, and ship the package
// to the store. HSM-backed signers can instead call
// `.manifest(...).payload(...)` and invoke
// `AppPackageSigner::sign_package` directly with their
// own key.

use aether_core::app::AppPackage;
use aether_security::app_signing::AppPackageSigner;

/// Errors produced by `AppPackageBuilder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageBuildError {
    /// The manifest failed validation (empty app id,
    /// mismatched hash, etc.). The builder surfaces
    /// `AppManifest::validate` errors as-is.
    InvalidManifest(String),
    /// The payload was empty. The store would reject
    /// this anyway, so we surface it up front.
    EmptyPayload,
    /// The signer rejected the package for some other
    /// reason (caller is responsible for interpreting
    /// the inner string).
    Signer(String),
}

impl std::fmt::Display for PackageBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidManifest(s) => write!(f, "invalid manifest: {s}"),
            Self::EmptyPayload => f.write_str("payload is empty"),
            Self::Signer(s) => write!(f, "signer rejected package: {s}"),
        }
    }
}

impl std::error::Error for PackageBuildError {}

/// Pre-sign manifest check. Mirrors `AppManifest::validate`
/// but allows `publisher_key_id` to be empty — the signer
/// fills that in. Catches all the other shape problems
/// (missing name, bad SHA-256, weight out of range, etc.)
/// up front so the developer gets a clean error.
fn validate_manifest_pre_sign(
    manifest: &aether_core::app::AppManifest,
) -> Result<(), PackageBuildError> {
    use aether_core::app::APP_PACKAGE_SCHEMA_VERSION;
    if manifest.schema_version != APP_PACKAGE_SCHEMA_VERSION {
        return Err(PackageBuildError::InvalidManifest(format!(
            "unsupported app manifest schema_version '{}' (this build expects '{APP_PACKAGE_SCHEMA_VERSION}')",
            manifest.schema_version
        )));
    }
    if manifest.app_id.is_empty() {
        return Err(PackageBuildError::InvalidManifest("app_id is required".to_string()));
    }
    if !aether_core::app::is_valid_app_id(&manifest.app_id) {
        return Err(PackageBuildError::InvalidManifest(format!(
            "app_id '{}' is not a valid reverse-DNS identifier",
            manifest.app_id
        )));
    }
    if manifest.name.is_empty() {
        return Err(PackageBuildError::InvalidManifest("name is required".to_string()));
    }
    if manifest.version.is_empty() {
        return Err(PackageBuildError::InvalidManifest("version is required".to_string()));
    }
    if manifest.publisher.is_empty() {
        return Err(PackageBuildError::InvalidManifest("publisher is required".to_string()));
    }
    if manifest.min_os_version.is_empty() {
        return Err(PackageBuildError::InvalidManifest("min_os_version is required".to_string()));
    }
    if manifest.binary_sha256.len() != 64
        || !manifest.binary_sha256.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(PackageBuildError::InvalidManifest(format!(
            "binary_sha256 '{}' is not a 64-character hex string",
            manifest.binary_sha256
        )));
    }
    if manifest.payload_len == 0 {
        return Err(PackageBuildError::InvalidManifest("payload_len must be > 0".to_string()));
    }
    if manifest.resources.cpu_weight < 1 || manifest.resources.cpu_weight > 10_000 {
        return Err(PackageBuildError::InvalidManifest(format!(
            "cpu_weight {} is outside the cgroup v2 range 1..=10000",
            manifest.resources.cpu_weight
        )));
    }
    if manifest.resources.io_weight < 1 || manifest.resources.io_weight > 10_000 {
        return Err(PackageBuildError::InvalidManifest(format!(
            "io_weight {} is outside the cgroup v2 range 1..=10000",
            manifest.resources.io_weight
        )));
    }
    Ok(())
}

/// Builder for signed `AppPackage`.
///
/// # Example
///
/// ```no_run
/// use aether_sdk::{AppManifestBuilder, AppPackageBuilder};
/// use aether_core::app::AppPermission;
/// use aether_security::app_signing::AppPackageSigner;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let payload = b"ELF-binary-placeholder".to_vec();
/// let manifest = AppManifestBuilder::new(
///     "com.example.calc",
///     "Aether Calculator",
///     "1.0.0",
///     "Example Org",
///     payload.clone(),
/// )
/// .permission(AppPermission::Notify)
/// .build()
/// .expect("manifest");
///
/// // The signing key is normally generated once and
/// // persisted alongside the publisher identity.
/// let signer = AppPackageSigner::generate();
///
/// let package = AppPackageBuilder::build_signed(manifest, payload, &signer)?;
/// assert!(!package.signature.is_empty());
/// assert!(!package.manifest.publisher_key_id.is_empty());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct AppPackageBuilder {
    manifest: aether_core::app::AppManifest,
    payload: Vec<u8>,
}

impl AppPackageBuilder {
    /// One-shot: build, sign, and return the package.
    /// Equivalent to `AppPackageBuilder::new(manifest, payload).sign(signer)`.
    pub fn build_signed(
        manifest: aether_core::app::AppManifest,
        payload: Vec<u8>,
        signer: &AppPackageSigner,
    ) -> Result<AppPackage, PackageBuildError> {
        Self::new(manifest, payload)?.sign(signer)
    }

    /// Construct a builder for `(manifest, payload)`. Validates the
    /// pair up front so `.sign(...)` cannot fail for reasons other
    /// than signing.
    ///
    /// Note: the full `AppManifest::validate` check (which
    /// requires `publisher_key_id` to be set) runs inside
    /// `.sign(...)`. Before signing we run a relaxed check
    /// that lets an empty `publisher_key_id` through — it
    /// is the signer's job to fill that in.
    pub fn new(
        manifest: aether_core::app::AppManifest,
        payload: Vec<u8>,
    ) -> Result<Self, PackageBuildError> {
        if payload.is_empty() {
            return Err(PackageBuildError::EmptyPayload);
        }
        validate_manifest_pre_sign(&manifest)?;
        Ok(Self { manifest, payload })
    }

    /// Replace the manifest (e.g. after re-validating with a
    /// different `min_os_version`). Keeps the payload.
    #[must_use]
    pub fn manifest(mut self, manifest: aether_core::app::AppManifest) -> Self {
        self.manifest = manifest;
        self
    }

    /// Replace the payload.
    #[must_use]
    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    /// Borrow the current manifest, primarily for tests
    /// and for HSM flows that want to inspect what
    /// would be signed before handing it off.
    #[must_use]
    pub fn manifest_ref(&self) -> &aether_core::app::AppManifest {
        &self.manifest
    }

    /// Borrow the current payload.
    #[must_use]
    pub fn payload_ref(&self) -> &[u8] {
        &self.payload
    }

    /// Sign the manifest, fill the `publisher_key_id` on
    /// the manifest, and assemble the `AppPackage`. The
    /// signature covers the manifest's canonical bytes;
    /// the store recomputes this on install.
    pub fn sign(self, signer: &AppPackageSigner) -> Result<AppPackage, PackageBuildError> {
        // Re-validate the manifest with the strict
        // validator, now that we'll have a non-empty
        // publisher_key_id by the time the package is
        // assembled. First: relaxed check.
        validate_manifest_pre_sign(&self.manifest)?;
        if self.payload.is_empty() {
            return Err(PackageBuildError::EmptyPayload);
        }

        // Fill the publisher_key_id from the signer's
        // public key fingerprint. SHA-256 hex of the
        // 32-byte compressed public key is the canonical
        // fingerprint the trust registry stores.
        let mut manifest = self.manifest;
        manifest.publisher_key_id = signer.public_key_fingerprint_hex();

        // Run the strict validate now that the
        // publisher_key_id is populated. If the manifest
        // is malformed in any other respect, fail here
        // rather than inside the signer.
        manifest.validate().map_err(|e| PackageBuildError::InvalidManifest(e.to_string()))?;

        // Hand off to the security crate's signer, which
        // produces the canonical manifest bytes and
        // computes the Ed25519 signature. The signer
        // also re-hashes the payload into the manifest
        // so the verifier can spot a payload swap.
        let pkg = AppPackage { manifest, payload: self.payload, signature: String::new() };
        signer.sign_package(pkg).map_err(PackageBuildError::Signer)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_core::app::AppResourceLimits;
    use ed25519_dalek::VerifyingKey;

    fn payload() -> Vec<u8> {
        b"ELF-binary-placeholder".to_vec()
    }

    fn manifest() -> aether_core::app::AppManifest {
        super::super::AppManifestBuilder::new(
            "com.example.calc",
            "Aether Calculator",
            "1.0.0",
            "Example Org",
            payload(),
        )
        .build()
        .expect("manifest")
    }

    #[test]
    fn sign_produces_non_empty_signature_and_fingerprint() {
        let m = manifest();
        let signer = AppPackageSigner::generate();
        let pkg = AppPackageBuilder::build_signed(m, payload(), &signer).expect("sign");
        assert!(!pkg.signature.is_empty(), "signature is empty");
        assert!(!pkg.manifest.publisher_key_id.is_empty(), "fingerprint empty");
        // 64 bytes of Ed25519 -> 88 base64 chars.
        assert_eq!(pkg.signature.len(), 88);
        // 32 bytes SHA-256 -> 64 hex chars.
        assert_eq!(pkg.manifest.publisher_key_id.len(), 64);
    }

    #[test]
    fn sign_empty_payload_rejected() {
        let m = manifest();
        let signer = AppPackageSigner::generate();
        let err = AppPackageBuilder::build_signed(m, vec![], &signer).expect_err("empty payload");
        assert_eq!(err, PackageBuildError::EmptyPayload);
    }

    #[test]
    fn new_rejects_empty_payload() {
        let m = manifest();
        let err = AppPackageBuilder::new(m, vec![]).expect_err("empty payload");
        assert_eq!(err, PackageBuildError::EmptyPayload);
    }

    #[test]
    fn sign_then_verify_round_trips() {
        // Sanity: a package signed here should be
        // accepted by the store's verifier.
        let m = manifest();
        let signer = AppPackageSigner::generate();
        let pkg = AppPackageBuilder::build_signed(m, payload(), &signer).expect("sign");
        let verifier = aether_security::app_signing::AppPackageVerifier::new(vec![
            signer.public_key_fingerprint_hex()
        ]);
        let pk_bytes = signer.public_key_bytes();
        let vk = VerifyingKey::from_bytes(&pk_bytes).expect("vk");
        verifier
            .verify_with_key(&pkg, &vk)
            .expect("package should verify with the signer's public key");
    }

    #[test]
    fn manifest_swap_changes_signature_but_keeps_fingerprint() {
        // After `.manifest(...)` + `.sign(...)`, the
        // fingerprint on the package should reflect
        // the SAME signer, but the signature itself
        // must change because the canonical bytes
        // changed.
        let m1 = manifest();
        let signer = AppPackageSigner::generate();
        let pkg1 = AppPackageBuilder::build_signed(m1, payload(), &signer).expect("sign");
        let fp1 = pkg1.manifest.publisher_key_id.clone();

        // Build a second manifest, swap it in, sign again.
        let m2 = super::super::AppManifestBuilder::new(
            "com.example.calc",
            "Aether Calculator",
            "1.0.1", // bumped version
            "Example Org",
            payload(),
        )
        .build()
        .expect("m2");
        let pkg2 =
            AppPackageBuilder::new(m2, payload()).expect("builder").sign(&signer).expect("sign");
        // Same signer -> same fingerprint, but the
        // canonical bytes signed differ (the version
        // changed), so the signature must differ.
        assert_eq!(fp1, pkg2.manifest.publisher_key_id);
        assert_ne!(pkg1.signature, pkg2.signature);
    }

    #[test]
    fn builder_clones_share_payload() {
        let m = manifest();
        let b = AppPackageBuilder::new(m, payload()).expect("builder");
        let b2 = b.clone();
        assert_eq!(b.payload_ref(), b2.payload_ref());
        assert_eq!(b.manifest_ref().app_id, b2.manifest_ref().app_id);
    }

    #[test]
    fn manifest_with_permission_signs() {
        // Sanity: a manifest with permissions should
        // still sign successfully. The signer doesn't
        // care about permissions; the store checks
        // them at install time.
        let m =
            super::super::AppManifestBuilder::new("com.example.calc", "n", "1.0.0", "p", payload())
                .permission(aether_core::app::AppPermission::NetworkEgress)
                .resources(AppResourceLimits::default_for("calc"))
                .build()
                .expect("m");
        let signer = AppPackageSigner::generate();
        let pkg = AppPackageBuilder::build_signed(m, payload(), &signer).expect("sign");
        assert!(pkg.manifest.permissions.contains(&aether_core::app::AppPermission::NetworkEgress));
    }
}
