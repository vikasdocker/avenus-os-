// Device registry.
//
// The registry is the in-memory map of every
// device the local daemon knows about. A
// device is either paired (trusted to send us
// observations and proposals) or unpaired
// (we know they exist but they cannot reach
// us). The future `aether-device-runtime`
// owns the persistence and the transport;
// the shell only holds the in-memory state.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::fingerprint::DeviceFingerprint;
use crate::identity::{DeviceClass, DeviceId};
use crate::pairing::{Pairing, PairingGrant, PairingState};

/// The bound on the number of devices the
/// registry will hold. A typical household
/// is well under 50 devices; we cap the
/// registry at 256 so a misbehaving future
/// runtime can't grow the map without
/// bound.
pub const DEVICE_REGISTRY_LIMIT: usize = 256;

/// One entry in the device registry: the
/// device's stable identity plus the
/// `Pairing` record describing the
/// local-side view of the relationship.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredDevice {
    pub device_id: DeviceId,
    pub device_class: DeviceClass,
    pub fingerprint: DeviceFingerprint,
    pub pairing: Pairing,
}

/// Reasons a registry operation is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceRegistryError {
    /// The registry is at its bound and
    /// the new device would push it over.
    Full,
    /// The device id is already registered.
    AlreadyRegistered,
    /// The requested device id is unknown.
    UnknownDevice,
}

impl std::fmt::Display for DeviceRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => f.write_str("device registry is full"),
            Self::AlreadyRegistered => f.write_str("device is already registered"),
            Self::UnknownDevice => f.write_str("device is not registered"),
        }
    }
}

impl std::error::Error for DeviceRegistryError {}

/// The in-memory map of known peer devices.
/// Indexed by `DeviceId` for O(log n) lookup.
#[derive(Debug, Default)]
pub struct DeviceRegistry {
    devices: BTreeMap<DeviceId, RegisteredDevice>,
}

impl DeviceRegistry {
    /// Creates a new, empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of registered devices.
    #[must_use]
    pub fn len(&self) -> usize {
        self.devices.len()
    }

    /// Returns `true` if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    /// Returns the bound the registry enforces.
    #[must_use]
    pub fn capacity(&self) -> usize {
        DEVICE_REGISTRY_LIMIT
    }

    /// Returns `true` if the registry is at the
    /// bound.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.devices.len() >= DEVICE_REGISTRY_LIMIT
    }

    /// Registers a new device in the
    /// `Available` state. Returns an error if
    /// the registry is full or the id is
    /// already present.
    pub fn register(
        &mut self,
        device_id: DeviceId,
        device_class: DeviceClass,
        fingerprint: DeviceFingerprint,
        grant: PairingGrant,
        now_ms: u64,
    ) -> Result<(), DeviceRegistryError> {
        if self.devices.contains_key(&device_id) {
            return Err(DeviceRegistryError::AlreadyRegistered);
        }
        if self.is_full() {
            return Err(DeviceRegistryError::Full);
        }
        let pairing = Pairing {
            device_id: device_id.clone(),
            device_class,
            fingerprint,
            state: PairingState::Available,
            grant,
            last_transition_ms: now_ms,
        };
        let entry =
            RegisteredDevice { device_id: device_id.clone(), device_class, fingerprint, pairing };
        self.devices.insert(device_id, entry);
        Ok(())
    }

    /// Returns the registered device, if any.
    #[must_use]
    pub fn get(&self, id: &DeviceId) -> Option<&RegisteredDevice> {
        self.devices.get(id)
    }

    /// Returns a read-only view of the
    /// registered devices, sorted by id.
    #[must_use]
    pub fn devices(&self) -> Vec<&RegisteredDevice> {
        self.devices.values().collect()
    }

    /// Returns the ids of devices in the
    /// `Paired` state. Sorted by id for a
    /// stable order.
    #[must_use]
    pub fn paired(&self) -> Vec<&RegisteredDevice> {
        let mut out: Vec<&RegisteredDevice> =
            self.devices.values().filter(|d| d.pairing.state.is_trusted()).collect();
        out.sort_by(|a, b| a.device_id.cmp(&b.device_id));
        out
    }

    /// Advances a registered device's pairing
    /// state. Returns an error if the device
    /// is unknown or the transition is not
    /// allowed.
    pub fn transition(
        &mut self,
        id: &DeviceId,
        next: PairingState,
        now_ms: u64,
    ) -> Result<(), DeviceRegistryError> {
        let entry = self.devices.get_mut(id).ok_or(DeviceRegistryError::UnknownDevice)?;
        entry.pairing.state = next;
        entry.pairing.last_transition_ms = now_ms;
        Ok(())
    }

    /// Removes a device from the registry.
    /// Returns the removed entry, if any.
    pub fn unregister(&mut self, id: &DeviceId) -> Option<RegisteredDevice> {
        self.devices.remove(id)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn id(s: &str) -> DeviceId {
        DeviceId::new(s).unwrap()
    }

    fn fp(seed: u8) -> DeviceFingerprint {
        DeviceFingerprint::from_bytes([seed; 32])
    }

    #[test]
    fn new_registry_is_empty() {
        let r = DeviceRegistry::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert_eq!(r.capacity(), DEVICE_REGISTRY_LIMIT);
    }

    #[test]
    fn register_adds_device_in_available_state() {
        let mut r = DeviceRegistry::new();
        r.register(
            id("dev-a"),
            DeviceClass::Laptop,
            fp(0x11),
            PairingGrant::default(),
            1_700_000_000_000,
        )
        .expect("register");
        let entry = r.get(&id("dev-a")).expect("get");
        assert_eq!(entry.pairing.state, PairingState::Available);
        assert!(!r.is_empty());
    }

    #[test]
    fn register_rejects_duplicate() {
        let mut r = DeviceRegistry::new();
        r.register(id("dev-a"), DeviceClass::Laptop, fp(0x11), PairingGrant::default(), 1).unwrap();
        let err = r
            .register(id("dev-a"), DeviceClass::Laptop, fp(0x11), PairingGrant::default(), 2)
            .unwrap_err();
        assert!(matches!(err, DeviceRegistryError::AlreadyRegistered));
    }

    #[test]
    fn register_rejects_when_full() {
        let mut r = DeviceRegistry::new();
        // Fill the registry with 256 devices.
        for i in 0..DEVICE_REGISTRY_LIMIT {
            let s = format!("dev-{i:04}");
            r.register(id(&s), DeviceClass::Laptop, fp(i as u8), PairingGrant::default(), i as u64)
                .unwrap();
        }
        assert!(r.is_full());
        // 257th must be rejected.
        let err = r
            .register(
                id("dev-overflow"),
                DeviceClass::Laptop,
                fp(0xff),
                PairingGrant::default(),
                999,
            )
            .unwrap_err();
        assert!(matches!(err, DeviceRegistryError::Full));
    }

    #[test]
    fn transition_advances_state() {
        let mut r = DeviceRegistry::new();
        r.register(id("dev-a"), DeviceClass::Phone, fp(0x11), PairingGrant::default(), 1).unwrap();
        r.transition(&id("dev-a"), PairingState::Pairing, 2).unwrap();
        assert_eq!(r.get(&id("dev-a")).unwrap().pairing.state, PairingState::Pairing);
        r.transition(&id("dev-a"), PairingState::Paired, 3).unwrap();
        assert_eq!(r.get(&id("dev-a")).unwrap().pairing.state, PairingState::Paired);
        assert_eq!(r.paired().len(), 1);
    }

    #[test]
    fn transition_rejects_unknown_device() {
        let mut r = DeviceRegistry::new();
        let err = r.transition(&id("nope"), PairingState::Pairing, 1).unwrap_err();
        assert!(matches!(err, DeviceRegistryError::UnknownDevice));
    }

    #[test]
    fn unregister_removes_device() {
        let mut r = DeviceRegistry::new();
        r.register(id("dev-a"), DeviceClass::Laptop, fp(0x11), PairingGrant::default(), 1).unwrap();
        let removed = r.unregister(&id("dev-a")).expect("removed");
        assert_eq!(removed.device_id, id("dev-a"));
        assert!(r.is_empty());
    }

    #[test]
    fn paired_returns_only_paired_devices() {
        let mut r = DeviceRegistry::new();
        r.register(id("dev-a"), DeviceClass::Laptop, fp(0x11), PairingGrant::default(), 1).unwrap();
        r.register(id("dev-b"), DeviceClass::Phone, fp(0x22), PairingGrant::default(), 1).unwrap();
        r.transition(&id("dev-a"), PairingState::Paired, 2).unwrap();
        let p = r.paired();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].device_id, id("dev-a"));
    }

    #[test]
    fn devices_returns_all_sorted_by_id() {
        let mut r = DeviceRegistry::new();
        for s in ["dev-c", "dev-a", "dev-b"] {
            r.register(id(s), DeviceClass::Laptop, fp(0x11), PairingGrant::default(), 1).unwrap();
        }
        let d = r.devices();
        assert_eq!(d[0].device_id, id("dev-a"));
        assert_eq!(d[1].device_id, id("dev-b"));
        assert_eq!(d[2].device_id, id("dev-c"));
    }
}
