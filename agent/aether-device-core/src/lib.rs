// Aether Device Core - identity, pairing, and
// cross-device coordination contracts.
//
// This crate is the **out-of-scope shell** for
// Phase 14 (Multi-Device Aether). It defines the
// types a future `aether-device-runtime` daemon
// will use to discover, pair with, and exchange
// planning state with peer Aether devices, but
// it does not open any sockets, perform any
// network I/O, or talk to the LLM.
//
// Responsibilities:
//   * `DeviceId` — a unique identifier for a
//     single Aether device (phone, tablet,
//     laptop, desktop, IoT, headless).
//   * `DeviceClass` — the device taxonomy
//     (`Phone`, `Tablet`, `Laptop`, `Desktop`,
//     `IoT`, `Headless`, `Server`, `External`).
//   * `DeviceFingerprint` — the SHA-256 fingerprint
//     of the device's public key. Used to verify
//     that a remote `DeviceId` is who it claims to
//     be during pairing.
//   * `PairingState` — the typed lifecycle of a
//     pairing relationship
//     (Available → Pairing → Paired / Revoked /
//     Expired).
//   * `Pairing` — one pairing record: the remote
//     device, the local-side shared secret, the
//     pairing state, the granted capabilities, the
//     audit log.
//   * `DeviceRegistry` — the in-memory map of
//     known peers. A peer is either paired
//     (trusted to send us observations and
//     proposals) or unpaired (we know they exist
//     but they cannot reach us). The registry is
//     the single source of truth for the future
//     `aether-device-runtime`.
//   * `PairingRequest` / `PairingAcceptance` — the
//     typed handshake. Both sides produce a
//     `PairingCode`; the future runtime
//     out-of-band-confirms the codes match before
//     flipping the relationship to `Paired`.
//   * `RemoteObservation` / `RemoteProposal` — the
//     typed envelope a paired peer sends. Every
//     incoming observation / proposal is bound to
//     the peer it came from; the local agentd
//     gates the cross-device flow on the peer
//     being in `Paired` state.
//
// Out of scope (lives in the future device
// runtime):
//   * Actual network transport (QUIC, BLE, Wi-Fi
//     Direct, etc).
//   * Out-of-band pairing UX (QR, NFC, spoken
//     code, etc).
//   * Persisting the registry to disk.
//   * The mTLS-style handshake itself — we only
//     define its inputs / outputs.

pub mod fingerprint;
pub mod identity;
pub mod pairing;
pub mod registry;
pub mod remote;

pub use fingerprint::DeviceFingerprint;
pub use identity::{DeviceClass, DeviceId, DEVICE_ID_MAX_LEN};
pub use pairing::{
    Pairing, PairingAcceptance, PairingCode, PairingError, PairingGrant, PairingRequest,
    PairingState,
};
pub use registry::{DeviceRegistry, DeviceRegistryError, RegisteredDevice};
pub use remote::{accept_remote_delivery, RemoteDeliveryError, RemoteDeliveryKind,
    RemoteObservation, RemoteProposal, RemoteSource};
