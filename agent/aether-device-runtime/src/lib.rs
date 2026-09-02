// Aether Device Runtime — cross-device delivery transport.
//
// Phase 14.3: Accepts incoming observations and proposals from
// paired peers over TCP. Validates every delivery against the
// device registry and pairing grants before accepting.

use std::collections::HashMap;

use aether_device_core::{
    accept_remote_delivery, DeviceFingerprint, DeviceRegistry, PairingState, RemoteDeliveryError,
    RemoteDeliveryKind, RemoteObservation, RemoteProposal,
};

/// Per-peer delivery state tracking.
#[derive(Debug, Clone, Default)]
struct PeerState {
    last_obs_seq: u64,
    last_prop_seq: u64,
}

/// Configuration for the device runtime transport.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// TCP listen address (e.g. "127.0.0.1:4760").
    pub listen_addr: String,
    /// Maximum clock skew for incoming sources (ms).
    pub skew_ms: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self { listen_addr: "127.0.0.1:4760".to_string(), skew_ms: 60_000 }
    }
}

/// Outcome of attempting to accept a remote delivery.
#[derive(Debug, Clone)]
pub enum DeliveryOutcome {
    Accepted,
    Rejected(RemoteDeliveryError),
}

/// The device runtime maintains per-peer state and the device
/// registry. It validates every incoming delivery.
pub struct DeviceRuntime {
    registry: DeviceRegistry,
    peer_state: HashMap<String, PeerState>,
    skew_ms: u64,
}

impl DeviceRuntime {
    /// Creates a new runtime with the given registry and skew window.
    pub fn new(registry: DeviceRegistry, skew_ms: u64) -> Self {
        Self { registry, peer_state: HashMap::new(), skew_ms }
    }

    /// Validates and accepts an incoming remote observation.
    pub fn accept_observation(
        &mut self,
        remote: &RemoteObservation,
        now_ms: u64,
    ) -> DeliveryOutcome {
        let device_id = &remote.source.device_id;
        let peer = match self.registry.get(device_id) {
            Some(p) => p,
            None => return DeliveryOutcome::Rejected(RemoteDeliveryError::UnknownPeer),
        };
        let grant = peer.pairing.grant.clone();
        let state = peer.pairing.state;
        let fp = peer.pairing.fingerprint;
        let last_seq = self.peer_state.get(device_id.as_str()).map(|s| s.last_obs_seq).unwrap_or(0);

        let result = accept_remote_delivery(
            state,
            &grant,
            &fp,
            &remote.source,
            last_seq,
            self.skew_ms,
            now_ms,
            RemoteDeliveryKind::Observation,
        );

        match result {
            Ok(()) => {
                let entry = self.peer_state.entry(device_id.as_str().to_string()).or_default();
                entry.last_obs_seq = remote.source.seq;
                DeliveryOutcome::Accepted
            }
            Err(e) => DeliveryOutcome::Rejected(e),
        }
    }

    /// Validates and accepts an incoming remote proposal.
    pub fn accept_proposal(&mut self, remote: &RemoteProposal, now_ms: u64) -> DeliveryOutcome {
        let device_id = &remote.source.device_id;
        let peer = match self.registry.get(device_id) {
            Some(p) => p,
            None => return DeliveryOutcome::Rejected(RemoteDeliveryError::UnknownPeer),
        };
        let grant = peer.pairing.grant.clone();
        let state = peer.pairing.state;
        let fp = peer.pairing.fingerprint;
        let last_seq =
            self.peer_state.get(device_id.as_str()).map(|s| s.last_prop_seq).unwrap_or(0);

        let result = accept_remote_delivery(
            state,
            &grant,
            &fp,
            &remote.source,
            last_seq,
            self.skew_ms,
            now_ms,
            RemoteDeliveryKind::Proposal,
        );

        match result {
            Ok(()) => {
                let entry = self.peer_state.entry(device_id.as_str().to_string()).or_default();
                entry.last_prop_seq = remote.source.seq;
                DeliveryOutcome::Accepted
            }
            Err(e) => DeliveryOutcome::Rejected(e),
        }
    }

    /// Returns the registry reference.
    #[must_use]
    pub fn registry(&self) -> &DeviceRegistry {
        &self.registry
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_agent_core::{Observation, ObservationSeverity, Proposal, ProposalRisk, TaskKind};
    use aether_device_core::{DeviceClass, DeviceId, PairingCode, PairingGrant};

    fn fp() -> DeviceFingerprint {
        DeviceFingerprint::from_bytes([0xAAu8; 32])
    }

    fn paired_registry() -> DeviceRegistry {
        let mut reg = DeviceRegistry::new();
        let dev_id = DeviceId::new("peer-1").unwrap();
        let grant = PairingGrant {
            receive_observations: true,
            receive_proposals: true,
            execute_remote_tasks: false,
        };
        reg.register(dev_id.clone(), DeviceClass::Laptop, fp(), grant, 1000).unwrap();
        reg.transition(&dev_id, PairingState::Paired, 2000).unwrap();
        reg
    }

    fn peer_source(seq: u64) -> aether_device_core::RemoteSource {
        aether_device_core::RemoteSource {
            device_id: DeviceId::new("peer-1").unwrap(),
            fingerprint: fp(),
            seq,
            timestamp_ms: 1_700_000_000_000,
        }
    }

    fn make_obs(id: &str) -> Observation {
        Observation::new(id, "storage", "disk full", ObservationSeverity::Warning, 1000).unwrap()
    }

    fn make_prop(id: &str) -> Proposal {
        Proposal::new(
            id,
            TaskKind::Notify,
            "cleanup",
            "free space",
            "disk full",
            ProposalRisk::Low,
            1000,
        )
        .unwrap()
    }

    #[test]
    fn accept_observation_from_paired_peer() {
        let mut rt = DeviceRuntime::new(paired_registry(), 60_000);
        let remote = RemoteObservation { source: peer_source(1), observation: make_obs("obs-1") };
        let result = rt.accept_observation(&remote, 1_700_000_000_000);
        assert!(matches!(result, DeliveryOutcome::Accepted));
    }

    #[test]
    fn reject_observation_from_unregistered_peer() {
        let mut rt = DeviceRuntime::new(DeviceRegistry::new(), 60_000);
        let remote = RemoteObservation { source: peer_source(1), observation: make_obs("obs-1") };
        let result = rt.accept_observation(&remote, 1_700_000_000_000);
        assert!(matches!(result, DeliveryOutcome::Rejected(RemoteDeliveryError::UnknownPeer)));
    }

    #[test]
    fn reject_out_of_order_observation() {
        let mut rt = DeviceRuntime::new(paired_registry(), 60_000);
        let remote1 = RemoteObservation { source: peer_source(1), observation: make_obs("obs-1") };
        rt.accept_observation(&remote1, 1_700_000_000_000);

        // Deliver seq=1 again — should be rejected as out-of-order.
        let remote2 = RemoteObservation { source: peer_source(1), observation: make_obs("obs-2") };
        let result = rt.accept_observation(&remote2, 1_700_000_000_000);
        assert!(matches!(
            result,
            DeliveryOutcome::Rejected(RemoteDeliveryError::OutOfOrder { .. })
        ));
    }

    #[test]
    fn accept_proposal_from_paired_peer() {
        let mut rt = DeviceRuntime::new(paired_registry(), 60_000);
        let remote = RemoteProposal { source: peer_source(1), proposal: make_prop("prop-1") };
        let result = rt.accept_proposal(&remote, 1_700_000_000_000);
        assert!(matches!(result, DeliveryOutcome::Accepted));
    }

    #[test]
    fn monotonically_increasing_seq_accepted() {
        let mut rt = DeviceRuntime::new(paired_registry(), 60_000);
        for seq in 1..=5 {
            let remote = RemoteObservation {
                source: peer_source(seq),
                observation: make_obs(&format!("obs-{seq}")),
            };
            let result = rt.accept_observation(&remote, 1_700_000_000_000);
            assert!(matches!(result, DeliveryOutcome::Accepted), "seq={seq} should be accepted");
        }
    }

    #[test]
    fn reject_too_old_source() {
        let mut rt = DeviceRuntime::new(paired_registry(), 1000);
        let remote = RemoteObservation {
            source: aether_device_core::RemoteSource {
                device_id: DeviceId::new("peer-1").unwrap(),
                fingerprint: fp(),
                seq: 1,
                timestamp_ms: 0,
            },
            observation: make_obs("obs-old"),
        };
        let result = rt.accept_observation(&remote, 1_700_000_000_000);
        assert!(matches!(result, DeliveryOutcome::Rejected(RemoteDeliveryError::TooOld)));
    }

    #[test]
    fn reject_proposal_from_unregistered_peer() {
        let mut rt = DeviceRuntime::new(DeviceRegistry::new(), 60_000);
        let remote = RemoteProposal { source: peer_source(1), proposal: make_prop("prop-1") };
        let result = rt.accept_proposal(&remote, 1_700_000_000_000);
        assert!(matches!(result, DeliveryOutcome::Rejected(RemoteDeliveryError::UnknownPeer)));
    }
}
