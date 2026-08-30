// Pairing contract.
//
// Pairing is the typed handshake that turns
// "we know a device exists" into "we trust
// that device to send us observations and
// proposals". The protocol is:
//
//   1. Both sides run the future
//      `aether-device-runtime` over a
//      transport they both trust (BLE, QR
//      code, NFC, spoken code, etc — the
//      future runtime's choice).
//   2. The initiator produces a
//      `PairingRequest` carrying its device
//      id, class, public-key fingerprint, and
//      a 6-digit `PairingCode` derived from
//      the two fingerprints.
//   3. The responder produces a matching
//      `PairingRequest` with the same code
//      (or rejects with `PairingError`).
//   4. Both sides call
//      `accept_pairing(...)` to flip the
//      relationship to `Paired`. The future
//      runtime refuses to let a peer through
//      unless the relationship is `Paired`.
//
// This file only defines the *contract*. The
// future runtime owns the transport and the
// out-of-band code-confirmation UX.

use serde::{Deserialize, Serialize};

use crate::fingerprint::DeviceFingerprint;
use crate::identity::DeviceClass;
use crate::identity::DeviceId;

/// The typed lifecycle of a pairing
/// relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PairingState {
    /// The peer is reachable but not yet
    /// trusted. Pairing cannot start until
    /// both sides are at least in this state.
    Available,
    /// Pairing is in progress. The peer is
    /// reachable and the handshake is open;
    /// the future runtime is exchanging
    /// `PairingRequest` / `PairingAcceptance`
    /// messages.
    Pairing,
    /// The peer is trusted. Cross-device
    /// observation / proposal exchange is
    /// allowed for the capabilities the
    /// pairing granted.
    Paired,
    /// The pairing was cancelled before
    /// completion. The peer remains in the
    /// registry but cannot retry pairing
    /// without a fresh `PairingRequest`.
    Cancelled,
    /// The pairing was explicitly revoked by
    /// the local user. The peer is removed
    /// from the active set; future pairings
    /// require a fresh handshake.
    Revoked,
    /// The pairing expired (the local policy
    /// enforces a maximum age). The peer is
    /// demoted to `Available` and must
    /// re-pair.
    Expired,
}

impl PairingState {
    /// Returns the canonical kebab-case name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Pairing => "pairing",
            Self::Paired => "paired",
            Self::Cancelled => "cancelled",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }

    /// Returns `true` if the peer is trusted
    /// to send cross-device observations /
    /// proposals.
    #[must_use]
    pub fn is_trusted(&self) -> bool {
        matches!(self, Self::Paired)
    }
}

impl std::fmt::Display for PairingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A 6-digit pairing code. The future
/// runtime's UX displays this to the user;
/// the user reads it on both devices to
/// confirm the pairing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PairingCode([u8; 6]);

impl PairingCode {
    /// Returns `true` if the candidate is a
    /// valid 6-digit decimal code (each digit
    /// 0-9). The future runtime uses this to
    /// validate codes entered by the user.
    #[must_use]
    pub fn is_valid(s: &str) -> bool {
        if s.len() != 6 {
            return false;
        }
        s.bytes().all(|b| b.is_ascii_digit())
    }

    /// Constructs a `PairingCode` from a
    /// pre-validated 6-digit string. Returns
    /// `None` if the input is not a valid
    /// code. Use this rather than the tuple
    /// constructor so callers can't smuggle
    /// in non-decimal bytes.
    #[must_use]
    pub fn new(s: &str) -> Option<Self> {
        if !Self::is_valid(s) {
            return None;
        }
        let bytes: Vec<u8> = s.bytes().collect();
        Some(Self([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]]))
    }

    /// Returns the 6-digit decimal string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        // Safe because `PairingCode::new` only
        // accepts ASCII digits; every byte in
        // `self.0` is therefore valid UTF-8.
        std::str::from_utf8(&self.0).unwrap_or("")
    }
}

impl std::fmt::Display for PairingCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The capabilities a paired peer is
/// granted. Each capability is a typed gate
/// the future runtime checks before letting
/// a cross-device observation / proposal
/// through.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PairingGrant {
    /// The peer may push observations into
    /// our local observation log.
    pub receive_observations: bool,
    /// The peer may push proposals into our
    /// local proposal set. Proposals still
    /// go through the standard user-consent
    /// flow; this only authorises the
    /// *delivery*.
    pub receive_proposals: bool,
    /// The peer may ask us to execute a
    /// remote task on its behalf (a future
    /// capability; off by default).
    pub execute_remote_tasks: bool,
}

impl PairingGrant {
    /// The most permissive grant. Used by
    /// `PairingRequest::default_grant` for
    /// the common case of pairing a
    /// user-owned device.
    #[must_use]
    pub fn permissive() -> Self {
        Self { receive_observations: true, receive_proposals: true, execute_remote_tasks: false }
    }
}

impl Default for PairingGrant {
    fn default() -> Self {
        Self::permissive()
    }
}

/// A pairing request: the initiator's
/// self-description plus the code the
/// responder must echo back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingRequest {
    /// The initiator's device id.
    pub device_id: DeviceId,
    /// The initiator's device class.
    pub device_class: DeviceClass,
    /// The initiator's public-key
    /// fingerprint.
    pub fingerprint: DeviceFingerprint,
    /// The 6-digit code the user reads on
    /// both devices to confirm the pairing.
    pub code: PairingCode,
    /// The capabilities the initiator is
    /// requesting.
    pub grant: PairingGrant,
    /// Wall-clock timestamp the request was
    /// produced. The future runtime rejects
    /// requests older than a configured
    /// skew window.
    pub timestamp_ms: u64,
}

/// A pairing acceptance: the responder's
/// self-description plus the code it
/// observed. The future runtime refuses to
/// flip the relationship to `Paired` unless
/// the acceptance's code matches the
/// request's code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingAcceptance {
    /// The responder's device id.
    pub device_id: DeviceId,
    /// The responder's device class.
    pub device_class: DeviceClass,
    /// The responder's public-key
    /// fingerprint.
    pub fingerprint: DeviceFingerprint,
    /// The code the responder observed on
    /// its side. Must match the request's
    /// `code`.
    pub code: PairingCode,
    /// The capabilities the responder
    /// grants the initiator.
    pub grant: PairingGrant,
    /// Wall-clock timestamp the acceptance
    /// was produced.
    pub timestamp_ms: u64,
}

/// One pairing record: the remote device,
/// the local view of the relationship, the
/// granted capabilities, and the wall-clock
/// timestamp the relationship was last
/// advanced. Held by `DeviceRegistry`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pairing {
    /// The peer's device id.
    pub device_id: DeviceId,
    /// The peer's device class.
    pub device_class: DeviceClass,
    /// The peer's public-key fingerprint.
    pub fingerprint: DeviceFingerprint,
    /// The current state of the
    /// relationship.
    pub state: PairingState,
    /// The capabilities the local side
    /// granted the peer. Ignored unless
    /// `state` is `Paired`.
    pub grant: PairingGrant,
    /// Wall-clock timestamp the
    /// relationship was last advanced
    /// (created, paired, revoked, expired).
    pub last_transition_ms: u64,
}

/// Reasons a pairing operation is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    /// The request and acceptance codes
    /// don't match. The future runtime
    /// surfaces this to the user as
    /// "pairing failed: codes don't match".
    CodeMismatch,
    /// The fingerprints don't match. A
    /// man-in-the-middle may be attempting
    /// to substitute their key.
    FingerprintMismatch,
    /// The acceptance's device id doesn't
    /// match what the request claimed.
    IdentityMismatch,
    /// The request is older than the
    /// configured skew window.
    RequestExpired,
    /// The peer is already in a non-Available
    /// state and a new pairing cannot start
    /// without first revoking the existing
    /// one.
    AlreadyPaired,
    /// The peer is in a terminal state and
    /// must be re-registered before pairing
    /// can start again.
    TerminalState,
}

impl std::fmt::Display for PairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CodeMismatch => f.write_str("pairing code does not match"),
            Self::FingerprintMismatch => f.write_str("pairing fingerprint does not match"),
            Self::IdentityMismatch => f.write_str("peer identity does not match"),
            Self::RequestExpired => f.write_str("pairing request has expired"),
            Self::AlreadyPaired => f.write_str("peer is already paired"),
            Self::TerminalState => f.write_str("peer is in a terminal state; re-register first"),
        }
    }
}

impl std::error::Error for PairingError {}

/// Validates that a `PairingAcceptance`
/// matches a `PairingRequest`. Returns the
/// `PairingError` describing the first
/// failure. The future runtime uses this
/// to decide whether to flip the
/// relationship to `Paired`.
pub fn validate_acceptance(
    request: &PairingRequest,
    acceptance: &PairingAcceptance,
    skew_ms: u64,
    now_ms: u64,
) -> Result<(), PairingError> {
    if acceptance.code != request.code {
        return Err(PairingError::CodeMismatch);
    }
    if acceptance.fingerprint != request.fingerprint {
        return Err(PairingError::FingerprintMismatch);
    }
    if acceptance.device_id != request.device_id {
        return Err(PairingError::IdentityMismatch);
    }
    if now_ms.saturating_sub(request.timestamp_ms) > skew_ms {
        return Err(PairingError::RequestExpired);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn req(id: &str, fp_hex: &str, code: &str) -> PairingRequest {
        let mut fp = [0u8; 32];
        for (i, h) in fp_hex.as_bytes().chunks(2).enumerate() {
            let s = std::str::from_utf8(h).unwrap();
            fp[i] = u8::from_str_radix(s, 16).unwrap();
        }
        PairingRequest {
            device_id: DeviceId::new(id).unwrap(),
            device_class: DeviceClass::Laptop,
            fingerprint: DeviceFingerprint::from_bytes(fp),
            code: PairingCode::new(code).unwrap(),
            grant: PairingGrant::default(),
            timestamp_ms: 1_700_000_000_000,
        }
    }

    fn acceptance(req: &PairingRequest) -> PairingAcceptance {
        PairingAcceptance {
            device_id: req.device_id.clone(),
            device_class: req.device_class,
            fingerprint: req.fingerprint,
            code: req.code,
            grant: req.grant.clone(),
            timestamp_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn pairing_state_as_str_is_stable() {
        assert_eq!(PairingState::Available.as_str(), "available");
        assert_eq!(PairingState::Pairing.as_str(), "pairing");
        assert_eq!(PairingState::Paired.as_str(), "paired");
        assert_eq!(PairingState::Cancelled.as_str(), "cancelled");
        assert_eq!(PairingState::Revoked.as_str(), "revoked");
        assert_eq!(PairingState::Expired.as_str(), "expired");
    }

    #[test]
    fn is_trusted_only_for_paired() {
        assert!(!PairingState::Available.is_trusted());
        assert!(!PairingState::Pairing.is_trusted());
        assert!(PairingState::Paired.is_trusted());
        assert!(!PairingState::Cancelled.is_trusted());
        assert!(!PairingState::Revoked.is_trusted());
        assert!(!PairingState::Expired.is_trusted());
    }

    #[test]
    fn pairing_code_validates_six_decimal_digits() {
        assert!(!PairingCode::is_valid(""));
        assert!(!PairingCode::is_valid("12345"));
        assert!(!PairingCode::is_valid("1234567"));
        assert!(!PairingCode::is_valid("12345a"));
        assert!(!PairingCode::is_valid("-12345"));
        assert!(PairingCode::is_valid("000000"));
        assert!(PairingCode::is_valid("999999"));
    }

    #[test]
    fn pairing_code_round_trip() {
        let c = PairingCode::new("123456").unwrap();
        assert_eq!(c.as_str(), "123456");
    }

    #[test]
    fn pairing_grant_default_is_permissive_but_no_remote_exec() {
        let g = PairingGrant::default();
        assert!(g.receive_observations);
        assert!(g.receive_proposals);
        assert!(!g.execute_remote_tasks);
    }

    #[test]
    fn validate_acceptance_accepts_matching() {
        let r = req("dev-a", "11".repeat(32).as_str(), "123456");
        let a = acceptance(&r);
        assert!(validate_acceptance(&r, &a, 60_000, 1_700_000_030_000).is_ok());
    }

    #[test]
    fn validate_acceptance_rejects_code_mismatch() {
        let r = req("dev-a", "11".repeat(32).as_str(), "123456");
        let mut a = acceptance(&r);
        a.code = PairingCode::new("654321").unwrap();
        let err = validate_acceptance(&r, &a, 60_000, 1_700_000_030_000).unwrap_err();
        assert!(matches!(err, PairingError::CodeMismatch));
    }

    #[test]
    fn validate_acceptance_rejects_fingerprint_mismatch() {
        let r = req("dev-a", "11".repeat(32).as_str(), "123456");
        let mut a = acceptance(&r);
        a.fingerprint = DeviceFingerprint::from_bytes([0x22u8; 32]);
        let err = validate_acceptance(&r, &a, 60_000, 1_700_000_030_000).unwrap_err();
        assert!(matches!(err, PairingError::FingerprintMismatch));
    }

    #[test]
    fn validate_acceptance_rejects_identity_mismatch() {
        let r = req("dev-a", "11".repeat(32).as_str(), "123456");
        let mut a = acceptance(&r);
        a.device_id = DeviceId::new("dev-b").unwrap();
        let err = validate_acceptance(&r, &a, 60_000, 1_700_000_030_000).unwrap_err();
        assert!(matches!(err, PairingError::IdentityMismatch));
    }

    #[test]
    fn validate_acceptance_rejects_expired_request() {
        let r = req("dev-a", "11".repeat(32).as_str(), "123456");
        let a = acceptance(&r);
        // 10 minutes later, 1 minute skew.
        let err = validate_acceptance(&r, &a, 60_000, 1_700_000_600_000).unwrap_err();
        assert!(matches!(err, PairingError::RequestExpired));
    }
}
