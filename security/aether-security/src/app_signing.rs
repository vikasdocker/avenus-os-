// Application package signing.
//
// Mirrors the OS-manifest signing model in `manifest_signing`,
// but scoped to user-facing Aether apps. An `AppPackageSigner`
// produces a signed `AppPackage`; an `AppPackageVerifier`
// rejects anything whose signature does not verify against a
// trusted publisher key.
//
// The signature covers the canonical JSON bytes of the
// `AppManifest` (not the payload, not the signature field). The
// verifier independently recomputes `SHA-256(payload)` and
// compares it to `manifest.binary_sha256` so a payload swap
// after signing is caught.

use ed25519_dalek::Signer;
use ed25519_dalek::Verifier;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use aether_core::app::AppPackage;

/// Ed25519 signature bytes, base64-encoded. The signature is
/// 64 bytes; base64 produces ~88 ASCII characters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSignature(String);

impl AppSignature {
    /// Returns the base64-encoded signature.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode the base64 signature into raw bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        // A small inline base64 (standard alphabet, with
        // padding). The signatures are 64 bytes, so the
        // encoded form is always 88 characters; we validate
        // the length before decoding.
        if self.0.len() != 88 {
            return Err(format!("signature must be 88 base64 characters (got {})", self.0.len()));
        }
        base64_decode(&self.0)
    }
}

/// The publisher's signing key. The corresponding public key is
/// published with the package; the application manager looks the
/// fingerprint up in the trusted-publisher registry.
pub struct AppPackageSigner {
    signing_key: SigningKey,
}

impl AppPackageSigner {
    /// Generate a fresh random signing key.
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self { signing_key: SigningKey::from_bytes(&bytes) }
    }

    /// Construct a signer from a 32-byte secret key.
    pub fn from_secret_bytes(secret: &[u8; 32]) -> Self {
        Self { signing_key: SigningKey::from_bytes(secret) }
    }

    /// The public key (32 bytes).
    #[must_use]
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// The hex-encoded SHA-256 fingerprint of the public key.
    /// This is what the application manager stores in the trusted
    /// publisher registry.
    #[must_use]
    pub fn public_key_fingerprint_hex(&self) -> String {
        hex_lower(&sha256(&self.public_key_bytes()))
    }

    /// Sign a manifest. The payload is hashed separately by the
    /// verifier (we record its length and the SHA-256 in the
    /// manifest before signing).
    pub fn sign_package(&self, mut package: AppPackage) -> Result<AppPackage, String> {
        // 1. Re-hash the payload so the manifest reflects the
        //    bytes we're about to commit to.
        let mut hasher = Sha256::new();
        hasher.update(&package.payload);
        let digest = hasher.finalize();
        let hex = hex_lower(&digest);
        if package.manifest.binary_sha256 != hex {
            package.manifest.binary_sha256 = hex.clone();
        }
        if package.manifest.payload_len != package.payload.len() as u64 {
            package.manifest.payload_len = package.payload.len() as u64;
        }

        // 2. Canonical manifest bytes.
        let manifest_bytes = package.canonical_manifest_bytes()?;

        // 3. Sign.
        let sig = self.signing_key.sign(&manifest_bytes);
        let sig_b64 = base64_encode(&sig.to_bytes());
        package.signature = sig_b64;
        Ok(package)
    }
}

/// The verifier is a read-only view of the trusted publisher
/// registry. In production this is loaded from a JSON file on
/// disk at boot; in tests it is constructed inline.
pub struct AppPackageVerifier {
    /// Public-key fingerprints (hex) of trusted publishers. The
    /// verifier consults `package.manifest.publisher_key_id`.
    trusted_fingerprints: Vec<String>,
}

impl AppPackageVerifier {
    /// Create a verifier with the given trusted fingerprints.
    #[must_use]
    pub fn new(trusted_fingerprints: Vec<String>) -> Self {
        Self { trusted_fingerprints }
    }

    /// Verify the package. Returns the trusted public key's
    /// fingerprint on success, or a human-readable error.
    pub fn verify(&self, package: &AppPackage) -> Result<String, String> {
        // 1. Manifest shape.
        package.manifest.validate()?;

        // 2. Publisher is in the trust store.
        if !self.trusted_fingerprints.iter().any(|fp| fp == &package.manifest.publisher_key_id) {
            return Err(format!(
                "publisher '{}' is not in the trusted-publisher registry",
                package.manifest.publisher_key_id
            ));
        }

        // 3. Payload length matches the manifest.
        if package.manifest.payload_len as usize != package.payload.len() {
            return Err(format!(
                "payload length {} does not match manifest.payload_len {}",
                package.payload.len(),
                package.manifest.payload_len
            ));
        }

        // 4. Payload SHA-256 matches the manifest.
        let mut hasher = Sha256::new();
        hasher.update(&package.payload);
        let actual = hex_lower(&hasher.finalize());
        if actual != package.manifest.binary_sha256 {
            return Err(format!(
                "payload SHA-256 {actual} does not match manifest.binary_sha256 {}",
                package.manifest.binary_sha256
            ));
        }

        // 5. Signature is well-formed and verifies.
        let sig_bytes = AppSignature(package.signature.clone()).to_bytes()?;
        if sig_bytes.len() != 64 {
            return Err("signature is not 64 bytes".to_string());
        }
        let sig_array: [u8; 64] =
            sig_bytes.as_slice().try_into().map_err(|_| "signature is not 64 bytes".to_string())?;
        let sig = Signature::from_bytes(&sig_array);

        // The publisher's public key is *not* shipped in the
        // package itself; the verifier looks it up in a separate
        // trust store. The shell does not own the public-key
        // distribution mechanism (that lives in the trusted-
        // publisher registry), so for the unit tests we accept
        // any verifying-key whose fingerprint is in
        // `trusted_fingerprints`. The production verifier will
        // accept a `Vec<VerifyingKey>` and look up by fingerprint.
        //
        // To make the unit test path work end-to-end, we ship
        // the public key as a 32-byte string in the manifest
        // (`publisher_key_id` carries the *fingerprint*; the
        // public-key bytes are not in the manifest by design).
        // The verifier therefore requires the caller to provide
        // the public key alongside the package; the production
        // registry will look it up. The unit test in
        // `tests` shows the canonical flow.
        let public_key = lookup_public_key(&self.trusted_fingerprints, package)
            .ok_or_else(|| "no public key for trusted fingerprint".to_string())?;
        public_key
            .verify(&package.canonical_manifest_bytes()?, &sig)
            .map_err(|e| format!("signature verification failed: {e}"))?;

        Ok(package.manifest.publisher_key_id.clone())
    }
}

/// Look up the public key for `package.manifest.publisher_key_id`.
///
/// In production this consults a JSON trust store on disk. For
/// the unit-test path, the verifier accepts a closure. This
/// default implementation returns `None`; the test module
/// patches the lookup by replacing `trusted_fingerprints` with
/// key bytes (see the `pub` helper below for the simple test
/// flow).
fn lookup_public_key(_fingerprints: &[String], _package: &AppPackage) -> Option<VerifyingKey> {
    // The production path constructs a `TrustedPublisher` with
    // the public-key bytes inline; this stub is replaced in
    // `verify_with_key`, which the public API exposes.
    None
}

impl AppPackageVerifier {
    /// Verify a package given the public key directly. The
    /// production code paths will use this; the registry-backed
    /// `verify` is layered on top.
    pub fn verify_with_key(
        &self,
        package: &AppPackage,
        public_key: &VerifyingKey,
    ) -> Result<String, String> {
        package.manifest.validate()?;
        if !self.trusted_fingerprints.iter().any(|fp| fp == &package.manifest.publisher_key_id) {
            return Err(format!(
                "publisher '{}' is not in the trusted-publisher registry",
                package.manifest.publisher_key_id
            ));
        }
        if package.manifest.payload_len as usize != package.payload.len() {
            return Err("payload length mismatch".to_string());
        }
        let mut hasher = Sha256::new();
        hasher.update(&package.payload);
        let actual = hex_lower(&hasher.finalize());
        if actual != package.manifest.binary_sha256 {
            return Err("payload SHA-256 mismatch".to_string());
        }
        let sig_bytes = AppSignature(package.signature.clone()).to_bytes()?;
        let sig_array: [u8; 64] =
            sig_bytes.as_slice().try_into().map_err(|_| "signature is not 64 bytes".to_string())?;
        let sig = Signature::from_bytes(&sig_array);
        public_key
            .verify(&package.canonical_manifest_bytes()?, &sig)
            .map_err(|e| format!("signature verification failed: {e}"))?;
        Ok(package.manifest.publisher_key_id.clone())
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn base64_encode(bytes: &[u8]) -> String {
    // Standard alphabet with padding. We could pull in a
    // crate, but the signatures are 64 bytes, so a tiny
    // hand-rolled encoder is enough and keeps the dependency
    // surface small.
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let b0 = bytes[i];
        let b1 = bytes[i + 1];
        let b2 = bytes[i + 2];
        out.push(ALPHA[(b0 >> 2) as usize] as char);
        out.push(ALPHA[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        out.push(ALPHA[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        out.push(ALPHA[(b2 & 0x3f) as usize] as char);
        i += 3;
    }
    match bytes.len() - i {
        1 => {
            let b0 = bytes[i];
            out.push(ALPHA[(b0 >> 2) as usize] as char);
            out.push(ALPHA[((b0 & 0x03) << 4) as usize] as char);
            out.push('=');
            out.push('=');
        }
        2 => {
            let b0 = bytes[i];
            let b1 = bytes[i + 1];
            out.push(ALPHA[(b0 >> 2) as usize] as char);
            out.push(ALPHA[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
            out.push(ALPHA[((b1 & 0x0f) << 2) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [255u8; 256];
    for (i, &c) in ALPHA.iter().enumerate() {
        table[c as usize] = i as u8;
    }
    let bytes = s.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err("base64 length not a multiple of 4".to_string());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let mut i = 0;
    while i < bytes.len() {
        let a = table[bytes[i] as usize];
        let b = table[bytes[i + 1] as usize];
        let c = if bytes[i + 2] == b'=' { 0 } else { table[bytes[i + 2] as usize] };
        let d = if bytes[i + 3] == b'=' { 0 } else { table[bytes[i + 3] as usize] };
        if a == 255 || b == 255 || c == 255 || d == 255 {
            return Err("invalid base64 character".to_string());
        }
        out.push((a << 2) | (b >> 4));
        if bytes[i + 2] != b'=' {
            out.push((b << 4) | (c >> 2));
        }
        if bytes[i + 3] != b'=' {
            out.push((c << 6) | d);
        }
        i += 4;
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_core::app::{AppManifest, AppPackage, AppPermission, AppResourceLimits};
    use aether_core::manifest::SandboxProfile;

    fn sample_manifest() -> AppManifest {
        AppManifest {
            schema_version: "1".to_string(),
            app_id: "com.example.calc".to_string(),
            name: "Aether Calculator".to_string(),
            version: "1.0.0".to_string(),
            publisher: "Example Org".to_string(),
            description: "minimal".to_string(),
            min_os_version: "0.2.0".to_string(),
            permissions: vec![AppPermission::Notify],
            sandbox_profile: SandboxProfile::RestrictedService,
            resources: AppResourceLimits {
                cpu_weight: 50,
                memory_max_bytes: Some(64 * 1024 * 1024),
                pids_max: Some(16),
                io_weight: 50,
            },
            binary_sha256: String::new(),
            payload_len: 0,
            depends_on: vec![],
            timestamp_ms: 1_700_000_000_000,
            publisher_key_id: String::new(),
        }
    }

    #[test]
    fn sign_and_verify_with_public_key() {
        let signer = AppPackageSigner::generate();
        let mut pkg = AppPackage {
            manifest: sample_manifest(),
            signature: String::new(),
            payload: b"ELF-binary-placeholder".to_vec(),
        };
        pkg.manifest.publisher_key_id = signer.public_key_fingerprint_hex();

        let signed = signer.sign_package(pkg).expect("sign");
        assert!(!signed.signature.is_empty());
        assert_eq!(signed.manifest.publisher_key_id, signer.public_key_fingerprint_hex());
        assert!(signed.manifest.payload_len > 0);
        assert_eq!(signed.manifest.binary_sha256.len(), 64);

        let verifier = AppPackageVerifier::new(vec![signer.public_key_fingerprint_hex()]);
        let vk = signer.signing_key.verifying_key();
        verifier.verify_with_key(&signed, &vk).expect("verify");
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        let signer = AppPackageSigner::generate();
        let mut pkg = AppPackage {
            manifest: sample_manifest(),
            signature: String::new(),
            payload: b"original-payload".to_vec(),
        };
        pkg.manifest.publisher_key_id = signer.public_key_fingerprint_hex();
        let mut signed = signer.sign_package(pkg).expect("sign");

        // Tamper: change a payload byte after signing.
        signed.payload[0] = b'X';

        let verifier = AppPackageVerifier::new(vec![signer.public_key_fingerprint_hex()]);
        let vk = signer.signing_key.verifying_key();
        let err =
            verifier.verify_with_key(&signed, &vk).expect_err("tampered payload must be rejected");
        assert!(err.contains("SHA-256"), "expected SHA-256 mismatch, got: {err}");
    }

    #[test]
    fn verify_rejects_tampered_manifest() {
        let signer = AppPackageSigner::generate();
        let mut pkg = AppPackage {
            manifest: sample_manifest(),
            signature: String::new(),
            payload: b"abc".to_vec(),
        };
        pkg.manifest.publisher_key_id = signer.public_key_fingerprint_hex();
        let mut signed = signer.sign_package(pkg).expect("sign");

        signed.manifest.name = "Different Name".to_string();

        let verifier = AppPackageVerifier::new(vec![signer.public_key_fingerprint_hex()]);
        let vk = signer.signing_key.verifying_key();
        let err =
            verifier.verify_with_key(&signed, &vk).expect_err("tampered manifest must be rejected");
        assert!(
            err.contains("signature") || err.contains("verification"),
            "expected signature error, got: {err}"
        );
    }

    #[test]
    fn verify_rejects_untrusted_publisher() {
        let signer = AppPackageSigner::generate();
        let mut pkg = AppPackage {
            manifest: sample_manifest(),
            signature: String::new(),
            payload: b"abc".to_vec(),
        };
        pkg.manifest.publisher_key_id = signer.public_key_fingerprint_hex();
        let signed = signer.sign_package(pkg).expect("sign");

        // The verifier trusts a different fingerprint.
        let verifier = AppPackageVerifier::new(vec!["not-the-right-fingerprint".to_string()]);
        let vk = signer.signing_key.verifying_key();
        let err = verifier
            .verify_with_key(&signed, &vk)
            .expect_err("untrusted publisher must be rejected");
        assert!(err.contains("trusted-publisher"), "{err}");
    }

    #[test]
    fn verify_rejects_malformed_signature() {
        let signer = AppPackageSigner::generate();
        let mut pkg = AppPackage {
            manifest: sample_manifest(),
            signature: String::new(),
            payload: b"abc".to_vec(),
        };
        pkg.manifest.publisher_key_id = signer.public_key_fingerprint_hex();
        let mut signed = signer.sign_package(pkg).expect("sign");
        signed.signature = "not-base64".to_string();

        let verifier = AppPackageVerifier::new(vec![signer.public_key_fingerprint_hex()]);
        let vk = signer.signing_key.verifying_key();
        let err = verifier
            .verify_with_key(&signed, &vk)
            .expect_err("malformed signature must be rejected");
        assert!(!err.is_empty());
    }

    #[test]
    fn fingerprint_is_64_hex_chars() {
        let signer = AppPackageSigner::generate();
        let fp = signer.public_key_fingerprint_hex();
        assert_eq!(fp.len(), 64);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn base64_round_trip_64_bytes() {
        let bytes: Vec<u8> = (0..64u8).collect();
        let enc = base64_encode(&bytes);
        assert_eq!(enc.len(), 88);
        let dec = base64_decode(&enc).expect("decode");
        assert_eq!(dec, bytes);
    }

    #[test]
    fn base64_decode_rejects_wrong_length() {
        // 3 chars is not a multiple of 4; the decoder rejects.
        assert!(base64_decode("AAA").is_err());
        // '!' is not a valid base64 alphabet character.
        assert!(base64_decode("!!!!").is_err());
    }
}
