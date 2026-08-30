// Device fingerprint.
//
// The fingerprint is the SHA-256 hash of the
// device's public key bytes. It is the only
// identity a remote peer can verify without
// holding the device's private key. Pairing
// codes are derived from the fingerprint, so a
// successful out-of-band code match implies
// the remote device holds the matching
// private key.

use serde::{Deserialize, Serialize};

/// A 32-byte SHA-256 fingerprint of a device's
/// public key. The fingerprint is the only
/// identity a remote peer can verify without
/// holding the device's private key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeviceFingerprint([u8; 32]);

impl DeviceFingerprint {
    /// Wraps a 32-byte hash. Callers should
    /// prefer `from_public_key`, which
    /// computes the hash, over this
    /// constructor.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Computes the fingerprint of a 32-byte
    /// public key. The future device runtime
    /// calls this with the signer's verifying
    /// key bytes; the shell stores the result.
    #[must_use]
    pub fn from_public_key(public_key: &[u8; 32]) -> Self {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(public_key);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        Self(out)
    }

    /// Returns the raw 32-byte fingerprint.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Returns the lowercase hex encoding of
    /// the fingerprint. This is the form
    /// pairing codes are derived from.
    #[must_use]
    pub fn as_hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in &self.0 {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }
}

impl std::fmt::Display for DeviceFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_hex())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_deterministic() {
        let key = [0x42u8; 32];
        let a = DeviceFingerprint::from_public_key(&key);
        let b = DeviceFingerprint::from_public_key(&key);
        assert_eq!(a, b);
    }

    #[test]
    fn different_keys_produce_different_fingerprints() {
        let a = DeviceFingerprint::from_public_key(&[0x42u8; 32]);
        let b = DeviceFingerprint::from_public_key(&[0x43u8; 32]);
        assert_ne!(a, b);
    }

    #[test]
    fn hex_round_trip() {
        let fp = DeviceFingerprint::from_public_key(&[0xabu8; 32]);
        let hex = fp.as_hex();
        assert_eq!(hex.len(), 64);
        // Lowercase only.
        assert_eq!(hex, hex.to_lowercase());
    }

    #[test]
    fn from_bytes_matches_from_public_key() {
        let key = [0x11u8; 32];
        let computed = DeviceFingerprint::from_public_key(&key);
        let direct = DeviceFingerprint::from_bytes(*computed.as_bytes());
        assert_eq!(computed, direct);
    }
}
