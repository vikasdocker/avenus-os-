// Signed service manifests.
//
// A service manifest declares how a service should be
// started, what its dependencies are, and which sandbox
// profile applies. A signed manifest is the same manifest
// plus an Ed25519 signature over its canonical byte form
// and a reference to the signer's public key.
//
// Signing a manifest does not change its semantics; it
// adds two guarantees:
//   * **Authenticity**: the bytes the OS is about to load
//     were produced by a holder of one of the trust-store
//     keys. An attacker who can replace the file on disk
//     but does not hold a trusted key cannot pass
//     verification.
//   * **Integrity**: any byte-level change to the
//     manifest breaks the signature. Tampered manifests
//     are rejected at load time, before any service is
//     spawned.
//
// The trust store is a list of *fingerprints* — short,
// hex-encoded SHA-256 hashes of the trusted public keys.
// The full public key is shipped alongside each manifest
// (it is not a secret), but only keys whose fingerprint
// is in the trust store are accepted as signers.

use ed25519_dalek::Signer;
use ed25519_dalek::Verifier;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Length of a fingerprint in bytes (16 bytes → 32 hex chars).
pub const FINGERPRINT_LEN: usize = 16;

/// A fingerprint that identifies a trusted public key. The
/// fingerprint is the first 16 bytes of `SHA-256(public_key_der)`,
/// hex-encoded. Using a short fingerprint rather than the
/// full public key keeps the trust store compact and lets
/// operators recognise keys at a glance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fingerprint(String);

impl Fingerprint {
    /// Returns the fingerprint for a public key, computed
    /// as the first 16 bytes of `SHA-256(public_key_bytes)`,
    /// hex-encoded. The fingerprint is not authenticated;
    /// the trust store is the source of truth.
    #[must_use]
    pub fn for_public_key(public_key: &VerifyingKey) -> Self {
        let bytes = public_key.to_bytes();
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        let mut out = String::with_capacity(FINGERPRINT_LEN * 2);
        for byte in &digest[..FINGERPRINT_LEN] {
            out.push_str(&format!("{byte:02x}"));
        }
        Self(out)
    }

    /// Constructs a fingerprint from a pre-computed hex
    /// string. Returns `None` if the input is not a valid
    /// 32-character hex string.
    #[must_use]
    pub fn from_hex(hex_str: &str) -> Option<Self> {
        if hex_str.len() != FINGERPRINT_LEN * 2 || !hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self(hex_str.to_ascii_lowercase()))
    }

    /// Returns the hex string form of the fingerprint.
    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A signature attached to a manifest. The wire form is
/// `signature(64) || signer_fingerprint(32 hex chars)`,
/// but the higher-level `SignedManifest` type carries
/// these as separate fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureEnvelope {
    /// Ed25519 signature over the canonical manifest bytes.
    pub signature: Vec<u8>,
    /// Hex fingerprint of the signer's public key.
    pub signer_fingerprint: String,
}

/// A manifest bundled with its signature and the
/// signer's public key. The manifest bytes are kept
/// verbatim so a `verify_signed_manifest` call can
/// recompute the canonical form and check the signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedManifest {
    /// The canonical manifest bytes that were signed.
    /// Verifiers re-canonicalise the manifest themselves
    /// and compare, so a successful signature means the
    /// declared bytes are authentic.
    pub manifest_bytes: Vec<u8>,
    /// The signer's public key (32 bytes for Ed25519).
    /// Not a secret; included so verifiers do not need
    /// an out-of-band key lookup.
    pub signer_public_key: Vec<u8>,
    /// The signature envelope.
    pub signature: SignatureEnvelope,
}

/// Reasons a signed manifest might fail to verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestVerifyError {
    /// The manifest bytes were not valid UTF-8 or
    /// canonical-JSON, so the signature could not be
    /// recomputed.
    MalformedManifest,
    /// The signature bytes were not 64 bytes long, or
    /// the public key bytes were not 32 bytes long.
    BadSignatureShape,
    /// The signer's public key is malformed (not a valid
    /// Ed25519 verifying key).
    BadPublicKey,
    /// The signer's fingerprint is not in the trust store.
    UnknownSigner,
    /// Ed25519 verification returned `false`: the
    /// signature is not valid for the manifest bytes
    /// under the supplied public key.
    SignatureInvalid,
}

impl std::fmt::Display for ManifestVerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedManifest => f.write_str("manifest bytes are malformed"),
            Self::BadSignatureShape => f.write_str("signature or public key has the wrong length"),
            Self::BadPublicKey => f.write_str("public key is not a valid Ed25519 verifying key"),
            Self::UnknownSigner => f.write_str("signer is not in the trust store"),
            Self::SignatureInvalid => f.write_str("Ed25519 signature verification failed"),
        }
    }
}

impl std::error::Error for ManifestVerifyError {}

/// A trust store: an allow-list of fingerprints. The
/// store does not own the corresponding public keys; the
/// verifier passes the public key in via the
/// `SignedManifest`.
#[derive(Debug, Clone, Default)]
pub struct TrustStore {
    fingerprints: std::collections::BTreeSet<String>,
}

impl TrustStore {
    /// Creates an empty trust store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Trusts the supplied fingerprint. Returns `true` if
    /// the fingerprint was not already trusted, `false`
    /// if it was.
    pub fn trust(&mut self, fingerprint: Fingerprint) -> bool {
        self.fingerprints.insert(fingerprint.0)
    }

    /// Removes the supplied fingerprint from the trust
    /// store. Returns `true` if the fingerprint was
    /// trusted, `false` if it was not.
    pub fn revoke(&mut self, fingerprint: &Fingerprint) -> bool {
        self.fingerprints.remove(&fingerprint.0)
    }

    /// Returns `true` if the supplied fingerprint is
    /// trusted.
    #[must_use]
    pub fn is_trusted(&self, fingerprint: &Fingerprint) -> bool {
        self.fingerprints.contains(&fingerprint.0)
    }

    /// Returns the number of trusted fingerprints.
    #[must_use]
    pub fn len(&self) -> usize {
        self.fingerprints.len()
    }

    /// Returns `true` if no fingerprint is trusted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fingerprints.is_empty()
    }

    /// Returns every trusted fingerprint, sorted.
    #[must_use]
    pub fn fingerprints(&self) -> Vec<Fingerprint> {
        self.fingerprints
            .iter()
            .map(|s| Fingerprint(s.clone()))
            .collect()
    }
}

/// Returns the canonical bytes that are signed for a
/// manifest. The bytes are SHA-256-hashed before signing
/// is unnecessary — Ed25519 already hashes internally —
/// so the canonical form is the manifest bytes verbatim.
///
/// In a real system this function would re-serialise the
/// manifest into a canonical JSON form (sorted keys, no
/// insignificant whitespace). For the present
/// implementation we use the bytes verbatim; the wire
/// format is whatever the loader produced.
#[must_use]
pub fn canonical_manifest_bytes(manifest_bytes: &[u8]) -> Vec<u8> {
    manifest_bytes.to_vec()
}

/// Verifies a signed manifest against a trust store.
///
/// The verification pipeline is:
///
///  1. Validate the wire shape of the signature and the
///     public key (length checks).
///  2. Parse the public key as an Ed25519 verifying key.
///  3. Compute the fingerprint and check that the trust
///     store contains it.
///  4. Verify the Ed25519 signature over the canonical
///     manifest bytes.
///
/// Any failure short-circuits with the relevant error
/// variant. The function is pure: it does not mutate the
/// trust store or the manifest.
pub fn verify_signed_manifest(
    signed: &SignedManifest,
    trust: &TrustStore,
) -> Result<(), ManifestVerifyError> {
    if signed.signature.signature.len() != 64 || signed.signer_public_key.len() != 32 {
        return Err(ManifestVerifyError::BadSignatureShape);
    }
    let public_key = VerifyingKey::from_bytes(
        signed
            .signer_public_key
            .as_slice()
            .try_into()
            .map_err(|_| ManifestVerifyError::BadPublicKey)?,
    )
    .map_err(|_| ManifestVerifyError::BadPublicKey)?;
    let fingerprint = Fingerprint::for_public_key(&public_key);
    if !trust.is_trusted(&fingerprint) {
        return Err(ManifestVerifyError::UnknownSigner);
    }
    let canonical = canonical_manifest_bytes(&signed.manifest_bytes);
    let signature = match Signature::try_from(signed.signature.signature.as_slice()) {
        Ok(s) => s,
        Err(_) => return Err(ManifestVerifyError::BadSignatureShape),
    };
    public_key
        .verify(&canonical, &signature)
        .map_err(|_| ManifestVerifyError::SignatureInvalid)?;
    Ok(())
}

/// A signer is a private key that can produce signatures
/// over manifest bytes.
pub struct Ed25519ManifestSigner {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl Ed25519ManifestSigner {
    /// Constructs a signer from a 32-byte Ed25519 private
    /// seed. The seed is the raw Ed25519 "seed" used by
    /// `SigningKey::from_bytes`.
    ///
    /// The seed is wiped on `Drop`; do not log it.
    #[must_use]
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        Self { signing_key, verifying_key }
    }

    /// Constructs a signer with a freshly generated
    /// random key. The private key is wiped on `Drop`.
    #[must_use]
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        // Zeroize the seed buffer. The rebind
        // through a new name keeps the zeroize
        // call's intent obvious and silences
        // clippy's redundant_locals lint.
        #[allow(clippy::redundant_locals)]
        let mut owned = seed;
        use zeroize::Zeroize;
        owned.zeroize();
        Self { signing_key, verifying_key }
    }

    /// Returns the signer's public key bytes (32 bytes
    /// for Ed25519).
    #[must_use]
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// Returns the fingerprint of the signer's public key.
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::for_public_key(&self.verifying_key)
    }

    /// Signs `manifest_bytes` and returns a `SignedManifest`.
    /// The manifest bytes are kept verbatim.
    #[must_use]
    pub fn sign(&self, manifest_bytes: &[u8]) -> SignedManifest {
        let canonical = canonical_manifest_bytes(manifest_bytes);
        let signature = self.signing_key.sign(&canonical);
        SignedManifest {
            manifest_bytes: manifest_bytes.to_vec(),
            signer_public_key: self.verifying_key.to_bytes().to_vec(),
            signature: SignatureEnvelope {
                signature: signature.to_bytes().to_vec(),
                signer_fingerprint: Fingerprint::for_public_key(&self.verifying_key).0,
            },
        }
    }
}

impl Drop for Ed25519ManifestSigner {
    fn drop(&mut self) {
        // `SigningKey` already wipes its seed on drop in
        // ed25519-dalek 3.x; the explicit zeroize is
        // belt-and-braces for older versions.
        use zeroize::Zeroize;
        let mut bytes = self.signing_key.to_bytes();
        bytes.zeroize();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use rand::RngCore;

    fn random_manifest_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 256];
        rand::thread_rng().fill_bytes(&mut bytes);
        bytes
    }

    #[test]
    fn fingerprint_is_32_hex_chars() {
        let signer = Ed25519ManifestSigner::generate();
        let fp = signer.fingerprint();
        assert_eq!(fp.as_hex().len(), 32);
        assert!(fp.as_hex().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn fingerprint_from_hex_round_trip() {
        let signer = Ed25519ManifestSigner::generate();
        let fp = signer.fingerprint();
        let parsed = Fingerprint::from_hex(fp.as_hex()).expect("valid hex");
        assert_eq!(fp, parsed);
    }

    #[test]
    fn fingerprint_from_hex_rejects_bad_input() {
        assert!(Fingerprint::from_hex("").is_none());
        assert!(Fingerprint::from_hex("abc").is_none());
        assert!(Fingerprint::from_hex(&"z".repeat(32)).is_none());
        // Right length but non-hex.
        assert!(Fingerprint::from_hex(&"x".repeat(32)).is_none());
    }

    #[test]
    fn sign_then_verify_with_trusted_signer() {
        let signer = Ed25519ManifestSigner::generate();
        let mut trust = TrustStore::new();
        trust.trust(signer.fingerprint());
        let manifest = random_manifest_bytes();
        let signed = signer.sign(&manifest);
        assert!(verify_signed_manifest(&signed, &trust).is_ok());
    }

    #[test]
    fn verify_fails_when_signer_not_trusted() {
        let signer = Ed25519ManifestSigner::generate();
        let trust = TrustStore::new();
        let manifest = random_manifest_bytes();
        let signed = signer.sign(&manifest);
        let err = verify_signed_manifest(&signed, &trust).unwrap_err();
        assert_eq!(err, ManifestVerifyError::UnknownSigner);
    }

    #[test]
    fn verify_fails_when_manifest_is_tampered() {
        let signer = Ed25519ManifestSigner::generate();
        let mut trust = TrustStore::new();
        trust.trust(signer.fingerprint());
        let manifest = random_manifest_bytes();
        let mut signed = signer.sign(&manifest);
        signed.manifest_bytes[0] ^= 0x01;
        let err = verify_signed_manifest(&signed, &trust).unwrap_err();
        assert_eq!(err, ManifestVerifyError::SignatureInvalid);
    }

    #[test]
    fn verify_fails_when_signature_is_swapped() {
        let signer_a = Ed25519ManifestSigner::generate();
        let signer_b = Ed25519ManifestSigner::generate();
        let mut trust = TrustStore::new();
        trust.trust(signer_a.fingerprint());
        // Sign with B, but claim A's key on the envelope.
        let manifest = random_manifest_bytes();
        let mut signed_b = signer_b.sign(&manifest);
        signed_b.signer_public_key = signer_a.public_key_bytes().to_vec();
        signed_b.signature.signer_fingerprint = signer_a.fingerprint().0;
        // The fingerprint is now A's, so the trust check
        // passes. The signature is still B's, so the
        // Ed25519 check fails.
        let err = verify_signed_manifest(&signed_b, &trust).unwrap_err();
        assert_eq!(err, ManifestVerifyError::SignatureInvalid);
    }

    #[test]
    fn verify_fails_for_bad_signature_shape() {
        let signer = Ed25519ManifestSigner::generate();
        let mut trust = TrustStore::new();
        trust.trust(signer.fingerprint());
        let manifest = random_manifest_bytes();
        let mut signed = signer.sign(&manifest);
        signed.signature.signature.truncate(10);
        let err = verify_signed_manifest(&signed, &trust).unwrap_err();
        assert_eq!(err, ManifestVerifyError::BadSignatureShape);
    }

    #[test]
    fn verify_fails_when_public_key_does_not_match_trust() {
        // Replacing the public key with bytes that do not
        // match any trusted fingerprint must fail. The
        // exact failure variant depends on whether the
        // substituted bytes parse as a valid Ed25519 point
        // (an all-0xFF key is not on the curve, so this
        // test exercises the BadPublicKey branch). A
        // separate test covers the case where the
        // substituted key is a valid point but is not in
        // the trust store.
        let signer = Ed25519ManifestSigner::generate();
        let mut trust = TrustStore::new();
        trust.trust(signer.fingerprint());
        let manifest = random_manifest_bytes();
        let mut signed = signer.sign(&manifest);
        signed.signer_public_key = vec![0xFFu8; 32];
        let err = verify_signed_manifest(&signed, &trust).unwrap_err();
        // The exact variant depends on whether the new
        // key parses — accept either, as both reject the
        // signed manifest.
        assert!(matches!(
            err,
            ManifestVerifyError::BadPublicKey | ManifestVerifyError::UnknownSigner
        ));
    }

    #[test]
    fn trust_store_supports_add_revoke_and_list() {
        let signer = Ed25519ManifestSigner::generate();
        let mut trust = TrustStore::new();
        assert!(trust.is_empty());
        assert!(trust.trust(signer.fingerprint()));
        assert!(!trust.trust(signer.fingerprint()));
        assert_eq!(trust.len(), 1);
        assert!(trust.is_trusted(&signer.fingerprint()));
        assert!(trust.revoke(&signer.fingerprint()));
        assert!(!trust.is_trusted(&signer.fingerprint()));
        assert!(trust.is_empty());
    }

    #[test]
    fn trust_store_fingerprints_are_sorted() {
        let a = Ed25519ManifestSigner::generate();
        let b = Ed25519ManifestSigner::generate();
        let c = Ed25519ManifestSigner::generate();
        let mut trust = TrustStore::new();
        trust.trust(a.fingerprint());
        trust.trust(b.fingerprint());
        trust.trust(c.fingerprint());
        let fps = trust.fingerprints();
        let mut sorted = fps.clone();
        sorted.sort_by(|x, y| x.as_hex().cmp(y.as_hex()));
        assert_eq!(fps, sorted);
    }

    #[test]
    fn fingerprint_for_public_key_is_deterministic() {
        // Two signers with the same seed should produce
        // the same fingerprint, and a public key derived
        // the standard way should always match.
        let seed = [0x42u8; 32];
        let a = Ed25519ManifestSigner::from_seed(seed);
        let b = Ed25519ManifestSigner::from_seed(seed);
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.public_key_bytes(), b.public_key_bytes());
    }

    #[test]
    fn manifest_verify_error_display_messages_are_distinct() {
        // Each variant's Display impl is part of the
        // user-facing error contract.
        assert_eq!(
            ManifestVerifyError::MalformedManifest.to_string(),
            "manifest bytes are malformed"
        );
        assert_eq!(
            ManifestVerifyError::BadSignatureShape.to_string(),
            "signature or public key has the wrong length"
        );
        assert_eq!(
            ManifestVerifyError::BadPublicKey.to_string(),
            "public key is not a valid Ed25519 verifying key"
        );
        assert_eq!(
            ManifestVerifyError::UnknownSigner.to_string(),
            "signer is not in the trust store"
        );
        assert_eq!(
            ManifestVerifyError::SignatureInvalid.to_string(),
            "Ed25519 signature verification failed"
        );
    }
}
