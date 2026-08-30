// Signed Aether updates.
//
// A signed update is the wire format the Aether update
// system uses to ship new OS images, service bundles, or
// agent models to a running system. The format is:
//
//   header  : JSON object, 1 KiB or less
//             {
//               "magic":        "AETHER-UPDATE-V1",
//               "kind":         "os-image" | "service-bundle" | "agent-model",
//               "target":       "<component-id>",
//               "version":      "<semver string>",
//               "timestamp_ms": <u64>,
//               "payload_len":  <usize>,
//               "signer_key_id": "<hex fingerprint>"
//             }
//   payload : <payload_len> bytes, opaque
//   trailer : <signature_len> bytes
//             The signature is over the concatenation
//             of the header bytes and the payload bytes.
//             For Ed25519 the signature is 64 bytes.
//
// The trailer length is implicit (always 64 for Ed25519).
// A future revision can negotiate the algorithm in the
// header.
//
// This module is *out of scope* of the shell — it
// exposes the format, the builder, and the verifier, but
// does not implement delivery, journaling, or atomic
// apply. Those are the responsibility of
// `aether-update-agent` (a future daemon).
//
// Threat model:
//   * An attacker who can write files into the update
//     staging area but does not hold the signing key
//     cannot produce a valid SignedUpdate. The verifier
//     rejects anything whose signature is not produced
//     by a key whose fingerprint is in the supplied
//     trust list.
//   * A replay of an older update is detectable by the
//     `timestamp_ms` and `target` fields. The verifier
//     does not enforce monotonic timestamps; the caller
//     can apply a "newer-than" check on top.

use ed25519_dalek::Signer;
use ed25519_dalek::Verifier;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::manifest_signing::Fingerprint;

/// The magic string at the start of every update header.
/// Hard-coded so a partially-written or corrupt update is
/// rejected before the rest of the header is parsed.
pub const MAGIC: &str = "AETHER-UPDATE-V1";

/// Fixed signature length for Ed25519. Kept as a constant
/// so the trailer slice is unambiguous on the wire.
pub const SIGNATURE_LEN: usize = 64;

/// The kind of payload inside an update. New variants
/// can be added without changing the wire format; the
/// header carries a string, not an enum, so an older
/// verifier can still parse a newer header (it just
/// rejects the unknown kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateKind {
    /// A full OS image update. Replaces the root
    /// filesystem on next boot.
    OsImage,
    /// A signed bundle of service manifests and
    /// binaries, applied service-by-service.
    ServiceBundle,
    /// A new model artifact for the agent runtime.
    AgentModel,
}

impl UpdateKind {
    /// Returns the canonical kebab-case name. The
    /// `Display` impl produces the same string.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OsImage => "os-image",
            Self::ServiceBundle => "service-bundle",
            Self::AgentModel => "agent-model",
        }
    }
}

impl std::fmt::Display for UpdateKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The header of a signed update. The header is
/// serialised to JSON and forms the first part of the
/// signed byte range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateHeader {
    /// Always `\`AETHER-UPDATE-V1\``.
    pub magic: String,
    /// The kind of payload.
    pub kind: UpdateKind,
    /// The component the update targets (e.g.
    /// "aether-system-core" or "agent-model-base").
    pub target: String,
    /// The version this update rolls the target to.
    pub version: String,
    /// Wall-clock timestamp when the update was signed.
    pub timestamp_ms: u64,
    /// Length of the payload in bytes.
    pub payload_len: usize,
    /// Hex fingerprint of the signer's public key. The
    /// header is not signed by itself; the signature
    /// covers header+payload, so the fingerprint is
    /// part of the signed bytes.
    pub signer_key_id: String,
}

impl UpdateHeader {
    /// Returns the canonical byte representation that
    /// gets fed into the signature. Sorted keys would be
    /// more portable across languages, but `serde_json`'s
    /// default output is good enough for the in-process
    /// verifier we ship today. A future revision can
    /// switch to a canonical JSON serializer.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// The complete signed update, as it sits on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedUpdate {
    pub header: UpdateHeader,
    pub payload: Vec<u8>,
    /// Ed25519 signature over `header_bytes || payload`.
    pub signature: Vec<u8>,
}

impl SignedUpdate {
    /// Returns the bytes that the signature covers:
    /// `header_bytes || payload`.
    #[must_use]
    pub fn signed_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.header.canonical_bytes().len() + self.payload.len());
        buf.extend_from_slice(&self.header.canonical_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }
}

/// Reasons a signed update might fail to verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateVerifyError {
    /// The header's `magic` field is missing or wrong.
    BadMagic,
    /// The header's `payload_len` does not match the
    /// length of the actual payload bytes.
    PayloadLengthMismatch,
    /// The signature is the wrong length for Ed25519.
    BadSignatureLength,
    /// The signer's public key bytes (looked up via
    /// `signer_key_id`) do not parse as an Ed25519 key.
    BadPublicKey,
    /// The signer's fingerprint is not in the trust list.
    UnknownSigner,
    /// The supplied `target` is empty or otherwise
    /// malformed.
    BadTarget,
    /// The Ed25519 signature did not verify.
    SignatureInvalid,
}

impl std::fmt::Display for UpdateVerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadMagic => f.write_str("update header magic mismatch"),
            Self::PayloadLengthMismatch => f.write_str("payload length does not match header"),
            Self::BadSignatureLength => f.write_str("signature is not 64 bytes"),
            Self::BadPublicKey => f.write_str("signer public key is not a valid Ed25519 key"),
            Self::UnknownSigner => f.write_str("signer is not in the trust list"),
            Self::BadTarget => f.write_str("update target is empty or malformed"),
            Self::SignatureInvalid => f.write_str("Ed25519 signature verification failed"),
        }
    }
}

impl std::error::Error for UpdateVerifyError {}

/// A trust list for signed updates. A `SignedUpdate` is
/// accepted only if its `signer_key_id` is in this list
/// AND the corresponding public key produces a valid
/// signature. The list is loaded once at daemon startup
/// from a fingerprint file; runtime mutation is not
/// supported in the shell.
#[derive(Debug, Clone, Default)]
pub struct UpdateTrustList {
    fingerprints: std::collections::BTreeSet<String>,
}

impl UpdateTrustList {
    /// Creates an empty trust list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Trusts the supplied fingerprint. Returns `true`
    /// if the fingerprint was newly added.
    pub fn trust(&mut self, fingerprint: Fingerprint) -> bool {
        self.fingerprints.insert(fingerprint.as_hex().to_string())
    }

    /// Returns `true` if the fingerprint is trusted.
    #[must_use]
    pub fn is_trusted(&self, fingerprint: &Fingerprint) -> bool {
        self.fingerprints.contains(fingerprint.as_hex())
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
}

/// Verifies a signed update. The verifier is pure: it
/// does not mutate the trust list or the update. The
/// caller is responsible for atomic-apply semantics.
#[deprecated(note = "use verify_signed_update_trusted instead; this helper is no longer useful")]
pub fn verify_signed_update(
    _update: &SignedUpdate,
    _trust: &UpdateTrustList,
) -> Result<(), UpdateVerifyError> {
    // Kept for source-level compatibility with prior
    // call sites; the trust list alone is not enough to
    // verify an update — the public key bytes are also
    // required. Callers should use
    // `verify_signed_update_trusted`.
    Err(UpdateVerifyError::UnknownSigner)
}

/// Verifies a signed update given an explicit public
/// key. The public key must correspond to the
/// fingerprint listed in the update's header; the
/// trust-list check is the caller's responsibility.
pub fn verify_signed_update_with_key(
    update: &SignedUpdate,
    public_key_bytes: &[u8; 32],
) -> Result<(), UpdateVerifyError> {
    if update.header.magic != MAGIC {
        return Err(UpdateVerifyError::BadMagic);
    }
    if update.header.target.is_empty() {
        return Err(UpdateVerifyError::BadTarget);
    }
    if update.header.payload_len != update.payload.len() {
        return Err(UpdateVerifyError::PayloadLengthMismatch);
    }
    if update.signature.len() != SIGNATURE_LEN {
        return Err(UpdateVerifyError::BadSignatureLength);
    }
    let verifying_key =
        VerifyingKey::from_bytes(public_key_bytes).map_err(|_| UpdateVerifyError::BadPublicKey)?;
    let signed = update.signed_bytes();
    let signature = match Signature::try_from(update.signature.as_slice()) {
        Ok(s) => s,
        Err(_) => return Err(UpdateVerifyError::BadSignatureLength),
    };
    verifying_key.verify(&signed, &signature).map_err(|_| UpdateVerifyError::SignatureInvalid)?;
    Ok(())
}

/// Convenience helper: verifies a signed update with
/// the caller-supplied public key AND confirms the
/// header's `signer_key_id` matches the public key's
/// fingerprint AND the fingerprint is in the trust list.
pub fn verify_signed_update_trusted(
    update: &SignedUpdate,
    public_key_bytes: &[u8; 32],
    trust: &UpdateTrustList,
) -> Result<(), UpdateVerifyError> {
    if update.header.magic != MAGIC {
        return Err(UpdateVerifyError::BadMagic);
    }
    if update.header.target.is_empty() {
        return Err(UpdateVerifyError::BadTarget);
    }
    if update.header.payload_len != update.payload.len() {
        return Err(UpdateVerifyError::PayloadLengthMismatch);
    }
    if update.signature.len() != SIGNATURE_LEN {
        return Err(UpdateVerifyError::BadSignatureLength);
    }
    let verifying_key =
        VerifyingKey::from_bytes(public_key_bytes).map_err(|_| UpdateVerifyError::BadPublicKey)?;
    let fingerprint = Fingerprint::for_public_key(&verifying_key);
    if fingerprint.as_hex() != update.header.signer_key_id {
        return Err(UpdateVerifyError::UnknownSigner);
    }
    if !trust.is_trusted(&fingerprint) {
        return Err(UpdateVerifyError::UnknownSigner);
    }
    let signed = update.signed_bytes();
    let signature = match Signature::try_from(update.signature.as_slice()) {
        Ok(s) => s,
        Err(_) => return Err(UpdateVerifyError::BadSignatureLength),
    };
    verifying_key.verify(&signed, &signature).map_err(|_| UpdateVerifyError::SignatureInvalid)?;
    Ok(())
}

/// The signing side of a signed update. Constructed
/// from a `SigningKey`; signs a payload and produces a
/// `SignedUpdate` envelope.
pub struct UpdateSigner {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl UpdateSigner {
    /// Constructs an `UpdateSigner` from a 32-byte Ed25519
    /// seed. The seed is wiped on `Drop`.
    #[must_use]
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        Self { signing_key, verifying_key }
    }

    /// Constructs an `UpdateSigner` with a freshly
    /// generated random key.
    #[must_use]
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        // Zeroize the seed buffer. We must clone
        // first because `signing_key` is built
        // from it; the redundant rebind silences
        // clippy and keeps the zeroize call.
        #[allow(clippy::redundant_locals)]
        let mut owned = seed;
        use zeroize::Zeroize;
        owned.zeroize();
        Self { signing_key, verifying_key }
    }

    /// Returns the signer's public key bytes.
    #[must_use]
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// Returns the signer's fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::for_public_key(&self.verifying_key)
    }

    /// Signs `payload` and returns a `SignedUpdate`. The
    /// `target` and `version` are written into the
    /// header verbatim.
    #[must_use]
    pub fn sign(
        &self,
        kind: UpdateKind,
        target: &str,
        version: &str,
        timestamp_ms: u64,
        payload: &[u8],
    ) -> SignedUpdate {
        let header = UpdateHeader {
            magic: MAGIC.to_string(),
            kind,
            target: target.to_string(),
            version: version.to_string(),
            timestamp_ms,
            payload_len: payload.len(),
            signer_key_id: self.fingerprint().as_hex().to_string(),
        };
        let signed_bytes = {
            let mut buf = Vec::with_capacity(header.canonical_bytes().len() + payload.len());
            buf.extend_from_slice(&header.canonical_bytes());
            buf.extend_from_slice(payload);
            buf
        };
        let signature = self.signing_key.sign(&signed_bytes);
        SignedUpdate { header, payload: payload.to_vec(), signature: signature.to_bytes().to_vec() }
    }
}

impl Drop for UpdateSigner {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        let mut bytes = self.signing_key.to_bytes();
        bytes.zeroize();
    }
}

/// A builder that lets callers compose a `SignedUpdate`
/// header by header. Useful for tests and for callers
/// that need to construct a header with a caller-chosen
/// fingerprint (e.g. a pre-existing key server).
pub struct SignedUpdateBuilder {
    header: UpdateHeader,
    payload: Vec<u8>,
    signature: Vec<u8>,
}

impl SignedUpdateBuilder {
    /// Starts a builder with the magic and zero-length
    /// payload. Callers fill in `kind`, `target`,
    /// `version`, `timestamp_ms`, `payload_len`, and
    /// `signer_key_id` before calling `build`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            header: UpdateHeader {
                magic: MAGIC.to_string(),
                kind: UpdateKind::ServiceBundle,
                target: String::new(),
                version: String::new(),
                timestamp_ms: 0,
                payload_len: 0,
                signer_key_id: String::new(),
            },
            payload: Vec::new(),
            signature: Vec::new(),
        }
    }

    /// Sets the kind.
    #[must_use]
    pub fn kind(mut self, kind: UpdateKind) -> Self {
        self.header.kind = kind;
        self
    }

    /// Sets the target.
    #[must_use]
    pub fn target(mut self, target: impl Into<String>) -> Self {
        self.header.target = target.into();
        self
    }

    /// Sets the version.
    #[must_use]
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.header.version = version.into();
        self
    }

    /// Sets the timestamp.
    #[must_use]
    pub fn timestamp_ms(mut self, ts: u64) -> Self {
        self.header.timestamp_ms = ts;
        self
    }

    /// Sets the payload and updates `payload_len` to
    /// match.
    #[must_use]
    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.header.payload_len = payload.len();
        self.payload = payload;
        self
    }

    /// Sets the signer fingerprint.
    #[must_use]
    pub fn signer_key_id(mut self, id: impl Into<String>) -> Self {
        self.header.signer_key_id = id.into();
        self
    }

    /// Sets the signature bytes directly.
    #[must_use]
    pub fn signature(mut self, sig: Vec<u8>) -> Self {
        self.signature = sig;
        self
    }

    /// Returns the finished `SignedUpdate`.
    #[must_use]
    pub fn build(self) -> SignedUpdate {
        SignedUpdate { header: self.header, payload: self.payload, signature: self.signature }
    }
}

impl Default for SignedUpdateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Computes the SHA-256 hash of an update's payload.
/// Useful for content addressing and for the agent
/// runtime to confirm the payload it received matches
/// the one the user approved.
#[must_use]
pub fn payload_sha256(payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn random_payload() -> Vec<u8> {
        let mut payload = vec![0u8; 256];
        rand::thread_rng().fill_bytes(&mut payload);
        payload
    }

    #[test]
    fn sign_then_verify_with_trust_list() {
        let signer = UpdateSigner::generate();
        let mut trust = UpdateTrustList::new();
        trust.trust(signer.fingerprint());
        let payload = random_payload();
        let update =
            signer.sign(UpdateKind::ServiceBundle, "aether-system-core", "0.2.0", 1_000, &payload);

        let pk = signer.public_key_bytes();
        assert!(verify_signed_update_trusted(&update, &pk, &trust).is_ok());
    }

    #[test]
    fn verify_fails_when_signer_not_trusted() {
        let signer = UpdateSigner::generate();
        let trust = UpdateTrustList::new();
        let payload = random_payload();
        let update = signer.sign(UpdateKind::OsImage, "os-root", "0.3.0", 1_000, &payload);
        let pk = signer.public_key_bytes();
        let err = verify_signed_update_trusted(&update, &pk, &trust).unwrap_err();
        assert_eq!(err, UpdateVerifyError::UnknownSigner);
    }

    #[test]
    fn verify_fails_when_payload_is_tampered() {
        let signer = UpdateSigner::generate();
        let mut trust = UpdateTrustList::new();
        trust.trust(signer.fingerprint());
        let payload = random_payload();
        let mut update = signer.sign(UpdateKind::ServiceBundle, "svc", "0.1.0", 1_000, &payload);
        update.payload[0] ^= 0x01;
        let pk = signer.public_key_bytes();
        let err = verify_signed_update_trusted(&update, &pk, &trust).unwrap_err();
        assert_eq!(err, UpdateVerifyError::SignatureInvalid);
    }

    #[test]
    fn verify_fails_when_header_is_tampered() {
        let signer = UpdateSigner::generate();
        let mut trust = UpdateTrustList::new();
        trust.trust(signer.fingerprint());
        let payload = random_payload();
        let mut update = signer.sign(UpdateKind::AgentModel, "model", "0.1.0", 1_000, &payload);
        // Tamper with a non-signed field of the header.
        // The signature is over the canonical header
        // bytes, so any change to a signed field fails
        // the signature. Changing a non-signed field
        // (e.g. `version`) still changes the canonical
        // bytes, so it also fails.
        update.header.version = "9.9.9".to_string();
        let pk = signer.public_key_bytes();
        let err = verify_signed_update_trusted(&update, &pk, &trust).unwrap_err();
        assert_eq!(err, UpdateVerifyError::SignatureInvalid);
    }

    #[test]
    fn verify_fails_with_wrong_public_key() {
        let signer_a = UpdateSigner::generate();
        let signer_b = UpdateSigner::generate();
        let mut trust = UpdateTrustList::new();
        trust.trust(signer_a.fingerprint());
        let payload = random_payload();
        let update = signer_a.sign(UpdateKind::OsImage, "os", "0.1.0", 1_000, &payload);
        // Try to verify with B's key, claiming A's trust.
        let pk_b = signer_b.public_key_bytes();
        let err = verify_signed_update_trusted(&update, &pk_b, &trust).unwrap_err();
        assert_eq!(err, UpdateVerifyError::UnknownSigner);
    }

    #[test]
    fn verify_fails_on_bad_magic() {
        let signer = UpdateSigner::generate();
        let mut trust = UpdateTrustList::new();
        trust.trust(signer.fingerprint());
        let payload = random_payload();
        let mut update = signer.sign(UpdateKind::ServiceBundle, "svc", "0.1.0", 1_000, &payload);
        update.header.magic = "WRONG".to_string();
        let pk = signer.public_key_bytes();
        let err = verify_signed_update_trusted(&update, &pk, &trust).unwrap_err();
        assert_eq!(err, UpdateVerifyError::BadMagic);
    }

    #[test]
    fn verify_fails_on_empty_target() {
        let signer = UpdateSigner::generate();
        let mut trust = UpdateTrustList::new();
        trust.trust(signer.fingerprint());
        let payload = random_payload();
        let update = signer.sign(UpdateKind::ServiceBundle, "", "0.1.0", 1_000, &payload);
        let pk = signer.public_key_bytes();
        let err = verify_signed_update_trusted(&update, &pk, &trust).unwrap_err();
        assert_eq!(err, UpdateVerifyError::BadTarget);
    }

    #[test]
    fn verify_fails_on_payload_length_mismatch() {
        let signer = UpdateSigner::generate();
        let mut trust = UpdateTrustList::new();
        trust.trust(signer.fingerprint());
        let payload = random_payload();
        let mut update = signer.sign(UpdateKind::ServiceBundle, "svc", "0.1.0", 1_000, &payload);
        // Change the header to claim a different length
        // without changing the payload.
        update.header.payload_len = payload.len() + 7;
        let pk = signer.public_key_bytes();
        let err = verify_signed_update_trusted(&update, &pk, &trust).unwrap_err();
        assert_eq!(err, UpdateVerifyError::PayloadLengthMismatch);
    }

    #[test]
    fn verify_fails_on_bad_signature_length() {
        let signer = UpdateSigner::generate();
        let mut trust = UpdateTrustList::new();
        trust.trust(signer.fingerprint());
        let payload = random_payload();
        let mut update = signer.sign(UpdateKind::ServiceBundle, "svc", "0.1.0", 1_000, &payload);
        update.signature.truncate(10);
        let pk = signer.public_key_bytes();
        let err = verify_signed_update_trusted(&update, &pk, &trust).unwrap_err();
        assert_eq!(err, UpdateVerifyError::BadSignatureLength);
    }

    #[test]
    fn verify_fails_on_bad_public_key_bytes() {
        let signer = UpdateSigner::generate();
        let mut trust = UpdateTrustList::new();
        trust.trust(signer.fingerprint());
        let payload = random_payload();
        let update = signer.sign(UpdateKind::ServiceBundle, "svc", "0.1.0", 1_000, &payload);
        // All-0xFF is not on the Ed25519 curve.
        let bad_pk = [0xFFu8; 32];
        let err = verify_signed_update_trusted(&update, &bad_pk, &trust).unwrap_err();
        assert!(matches!(err, UpdateVerifyError::BadPublicKey | UpdateVerifyError::UnknownSigner));
    }

    #[test]
    fn update_kind_round_trip_via_string() {
        // The kind is serialised as a kebab-case string;
        // round-tripping it through the public API must
        // preserve the value.
        let k = UpdateKind::ServiceBundle;
        let s = serde_json::to_string(&k).unwrap();
        assert_eq!(s, "\"service-bundle\"");
        let parsed: UpdateKind = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, UpdateKind::ServiceBundle);
        assert_eq!(parsed.as_str(), "service-bundle");
    }

    #[test]
    fn builder_produces_well_formed_envelope() {
        let signer = UpdateSigner::generate();
        let payload = random_payload();
        let update = signer.sign(UpdateKind::AgentModel, "model", "0.2.0", 1_000, &payload);
        // Round-trip the envelope through the builder.
        let rebuilt = SignedUpdateBuilder::new()
            .kind(update.header.kind)
            .target(update.header.target.clone())
            .version(update.header.version.clone())
            .timestamp_ms(update.header.timestamp_ms)
            .payload(update.payload.clone())
            .signer_key_id(update.header.signer_key_id.clone())
            .signature(update.signature.clone())
            .build();
        assert_eq!(rebuilt, update);
    }

    #[test]
    fn payload_sha256_is_deterministic() {
        let payload = b"hello world".to_vec();
        let h1 = payload_sha256(&payload);
        let h2 = payload_sha256(&payload);
        assert_eq!(h1, h2);
    }

    #[test]
    fn update_trust_list_supports_add_and_query() {
        let signer = UpdateSigner::generate();
        let mut trust = UpdateTrustList::new();
        assert!(trust.is_empty());
        assert!(trust.trust(signer.fingerprint()));
        assert!(!trust.trust(signer.fingerprint()));
        assert_eq!(trust.len(), 1);
        assert!(trust.is_trusted(&signer.fingerprint()));
    }

    #[test]
    fn update_verify_error_display_messages_are_distinct() {
        // Each variant's Display impl is part of the
        // public contract.
        assert_eq!(UpdateVerifyError::BadMagic.to_string(), "update header magic mismatch");
        assert_eq!(
            UpdateVerifyError::PayloadLengthMismatch.to_string(),
            "payload length does not match header"
        );
        assert_eq!(UpdateVerifyError::BadSignatureLength.to_string(), "signature is not 64 bytes");
        assert_eq!(
            UpdateVerifyError::BadPublicKey.to_string(),
            "signer public key is not a valid Ed25519 key"
        );
        assert_eq!(UpdateVerifyError::UnknownSigner.to_string(), "signer is not in the trust list");
        assert_eq!(UpdateVerifyError::BadTarget.to_string(), "update target is empty or malformed");
        assert_eq!(
            UpdateVerifyError::SignatureInvalid.to_string(),
            "Ed25519 signature verification failed"
        );
    }
}
