// Cross-device observation / proposal flow.
//
// A paired peer may push `Observation`s and
// `Proposal`s into our local agent. The
// `RemoteSource` carries the device id, the
// fingerprint, and a `seq` counter so the
// local agent can deduplicate and audit
// cross-device traffic.
//
// The cross-device flow is gated on the
// peer being in `Paired` state AND the
// grant covering the operation:
//   * `RemoteObservation` requires
//     `PairingGrant::receive_observations`.
//   * `RemoteProposal` requires
//     `PairingGrant::receive_proposals`.
//   * The remote `Observation` / `Proposal`
//     must carry the same id space as the
//     local one (a peer cannot inject an
//     observation id we already used).
//
// This file only defines the *contract*.
// The future device runtime is the only
// thing allowed to actually deliver a
// `RemoteObservation` or `RemoteProposal`
// into the local agent; today the shell
// stores them and surfaces them via IPC.

use serde::{Deserialize, Serialize};

use aether_agent_core::{Observation, Proposal};

use crate::fingerprint::DeviceFingerprint;
use crate::identity::DeviceId;
use crate::pairing::{PairingGrant, PairingState};

/// Where a cross-device piece of state came
/// from. The future runtime produces a
/// `RemoteSource` for every observation /
/// proposal it delivers; the local agent
/// records the source alongside the data so
/// the audit log can attribute it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteSource {
    /// The peer's device id.
    pub device_id: DeviceId,
    /// The peer's public-key fingerprint.
    /// The local agent re-verifies the
    /// fingerprint against the registry on
    /// every delivery so a revoked key is
    /// caught before it is accepted.
    pub fingerprint: DeviceFingerprint,
    /// A monotonic per-peer sequence
    /// counter. The local agent uses this
    /// to deduplicate and to reject
    /// out-of-order deliveries.
    pub seq: u64,
    /// Wall-clock timestamp the peer
    /// produced the source. The local agent
    /// rejects sources older than the
    /// configured skew window.
    pub timestamp_ms: u64,
}

/// An observation delivered by a paired
/// peer. The local agent appends the
/// observation to its bounded log with the
/// `source` field recorded alongside the
/// observation id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteObservation {
    pub source: RemoteSource,
    pub observation: Observation,
}

/// A proposal delivered by a paired peer.
/// The local agent validates the proposal
/// against the local observation log
/// (note: a remote proposal can only cite
/// *remote* observations; the local agent
/// does not yet have a way to map a remote
/// observation id into the local log).
/// Today the validator accepts remote
/// evidence as opaque ids; the future
/// runtime is responsible for resolving
/// them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteProposal {
    pub source: RemoteSource,
    pub proposal: Proposal,
}

/// Reasons a cross-device delivery is
/// rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteDeliveryError {
    /// The peer is not registered.
    UnknownPeer,
    /// The peer is not in `Paired` state.
    NotPaired,
    /// The peer's grant does not cover the
    /// operation (e.g. trying to deliver a
    /// proposal when only
    /// `receive_observations` is granted).
    GrantMissing,
    /// The peer's fingerprint does not
    /// match the registry's entry. The
    /// peer's key may have been rotated
    /// without re-pairing; the local agent
    /// refuses the delivery.
    FingerprintMismatch,
    /// The peer's source `seq` is older than
    /// the highest `seq` the local agent has
    /// seen from this peer. Out-of-order
    /// delivery is rejected so the audit
    /// log stays monotonic.
    OutOfOrder { last_seq: u64, new_seq: u64 },
    /// The source is older than the
    /// configured skew window.
    TooOld,
}

impl std::fmt::Display for RemoteDeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPeer => f.write_str("peer is not registered"),
            Self::NotPaired => f.write_str("peer is not in Paired state"),
            Self::GrantMissing => f.write_str("peer grant does not cover the operation"),
            Self::FingerprintMismatch => {
                f.write_str("peer fingerprint does not match the registry entry")
            }
            Self::OutOfOrder { last_seq, new_seq } => write!(
                f,
                "peer seq {new_seq} is older than the last seen seq {last_seq}"
            ),
            Self::TooOld => f.write_str("peer source is older than the skew window"),
        }
    }
}

impl std::error::Error for RemoteDeliveryError {}

/// What kind of cross-device delivery is
/// being attempted. Used by
/// `accept_remote_delivery` to pick the
/// right grant check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteDeliveryKind {
    Observation,
    Proposal,
}

/// Checks whether a `RemoteSource` may be
/// accepted given the peer's current pairing
/// state, grant, fingerprint, and the last
/// sequence number the local agent has seen
/// from this peer. The future runtime calls
/// this before delivering the payload to
/// the local agent.
#[allow(clippy::too_many_arguments)]
pub fn accept_remote_delivery(
    state: PairingState,
    grant: &PairingGrant,
    registered_fingerprint: &DeviceFingerprint,
    source: &RemoteSource,
    last_seq: u64,
    skew_ms: u64,
    now_ms: u64,
    kind: RemoteDeliveryKind,
) -> Result<(), RemoteDeliveryError> {
    if !state.is_trusted() {
        return Err(RemoteDeliveryError::NotPaired);
    }
    if source.fingerprint != *registered_fingerprint {
        return Err(RemoteDeliveryError::FingerprintMismatch);
    }
    match kind {
        RemoteDeliveryKind::Observation => {
            if !grant.receive_observations {
                return Err(RemoteDeliveryError::GrantMissing);
            }
        }
        RemoteDeliveryKind::Proposal => {
            if !grant.receive_proposals {
                return Err(RemoteDeliveryError::GrantMissing);
            }
        }
    }
    if source.seq <= last_seq {
        return Err(RemoteDeliveryError::OutOfOrder {
            last_seq,
            new_seq: source.seq,
        });
    }
    if now_ms.saturating_sub(source.timestamp_ms) > skew_ms {
        return Err(RemoteDeliveryError::TooOld);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    use aether_agent_core::{Observation, ObservationSeverity, Proposal, ProposalRisk, TaskKind};
    use crate::pairing::PairingGrant;

    fn source(seq: u64) -> RemoteSource {
        RemoteSource {
            device_id: crate::identity::DeviceId::new("dev-peer").unwrap(),
            fingerprint: DeviceFingerprint::from_bytes([0x33u8; 32]),
            seq,
            timestamp_ms: 1_700_000_000_000,
        }
    }

    fn obs() -> Observation {
        Observation::new(
            "obs-1",
            "storage",
            "disk is full",
            ObservationSeverity::Warning,
            1_700_000_000_000,
        )
        .unwrap()
    }

    fn prop() -> Proposal {
        Proposal::new(
            "prop-1",
            TaskKind::ProposeCleanup,
            "free up space",
            "delete cached files",
            "disk is full",
            ProposalRisk::Medium,
            1_700_000_000_000,
        )
        .unwrap()
    }

    #[test]
    fn accept_when_paired_granted_fresh_in_order() {
        let s = source(1);
        let grant = PairingGrant::default();
        let fp = s.fingerprint;
        let r = accept_remote_delivery(
            PairingState::Paired,
            &grant,
            &fp,
            &s,
            0,
            60_000,
            1_700_000_030_000,
            RemoteDeliveryKind::Observation,
        );
        assert!(r.is_ok());
    }

    #[test]
    fn reject_when_not_paired() {
        let s = source(1);
        let grant = PairingGrant::default();
        let fp = s.fingerprint;
        let err = accept_remote_delivery(
            PairingState::Available,
            &grant,
            &fp,
            &s,
            0,
            60_000,
            1_700_000_030_000,
            RemoteDeliveryKind::Observation,
        )
        .unwrap_err();
        assert!(matches!(err, RemoteDeliveryError::NotPaired));
    }

    #[test]
    fn reject_when_fingerprint_mismatch() {
        let s = source(1);
        let grant = PairingGrant::default();
        let other_fp = DeviceFingerprint::from_bytes([0x44u8; 32]);
        let err = accept_remote_delivery(
            PairingState::Paired,
            &grant,
            &other_fp,
            &s,
            0,
            60_000,
            1_700_000_030_000,
            RemoteDeliveryKind::Observation,
        )
        .unwrap_err();
        assert!(matches!(err, RemoteDeliveryError::FingerprintMismatch));
    }

    #[test]
    fn reject_when_grant_missing_for_observation() {
        let s = source(1);
        let grant = PairingGrant {
            receive_observations: false,
            receive_proposals: true,
            execute_remote_tasks: false,
        };
        let fp = s.fingerprint;
        let err = accept_remote_delivery(
            PairingState::Paired,
            &grant,
            &fp,
            &s,
            0,
            60_000,
            1_700_000_030_000,
            RemoteDeliveryKind::Observation,
        )
        .unwrap_err();
        assert!(matches!(err, RemoteDeliveryError::GrantMissing));
    }

    #[test]
    fn reject_when_grant_missing_for_proposal() {
        let s = source(1);
        let grant = PairingGrant {
            receive_observations: true,
            receive_proposals: false,
            execute_remote_tasks: false,
        };
        let fp = s.fingerprint;
        let err = accept_remote_delivery(
            PairingState::Paired,
            &grant,
            &fp,
            &s,
            0,
            60_000,
            1_700_000_030_000,
            RemoteDeliveryKind::Proposal,
        )
        .unwrap_err();
        assert!(matches!(err, RemoteDeliveryError::GrantMissing));
    }

    #[test]
    fn reject_out_of_order() {
        let s = source(5);
        let grant = PairingGrant::default();
        let fp = s.fingerprint;
        let err = accept_remote_delivery(
            PairingState::Paired,
            &grant,
            &fp,
            &s,
            10,
            60_000,
            1_700_000_030_000,
            RemoteDeliveryKind::Observation,
        )
        .unwrap_err();
        assert!(matches!(err, RemoteDeliveryError::OutOfOrder { .. }));
    }

    #[test]
    fn reject_too_old() {
        let mut s = source(1);
        s.timestamp_ms = 1_700_000_000_000;
        let grant = PairingGrant::default();
        let fp = s.fingerprint;
        let err = accept_remote_delivery(
            PairingState::Paired,
            &grant,
            &fp,
            &s,
            0,
            60_000,
            // 10 minutes later, 1 minute skew
            // -> too old.
            1_700_000_660_000,
            RemoteDeliveryKind::Observation,
        )
        .unwrap_err();
        assert!(matches!(err, RemoteDeliveryError::TooOld));
    }

    #[test]
    fn remote_observation_carries_observation_and_source() {
        let r = RemoteObservation {
            source: source(1),
            observation: obs(),
        };
        assert_eq!(r.observation.id, "obs-1");
        assert_eq!(r.source.seq, 1);
    }

    #[test]
    fn remote_proposal_carries_proposal_and_source() {
        let r = RemoteProposal {
            source: source(1),
            proposal: prop(),
        };
        assert_eq!(r.proposal.id.as_str(), "prop-1");
        assert_eq!(r.source.seq, 1);
    }
}
