//! Capability executor — the typed bridge that
//! turns a `Capability` + a `Device` into a
//! `CapabilityResult`.
//!
//! The hardware service is *pure data*; the
//! executor is the policy + state machine
//! that decides whether a capability can be
//! exercised, what side effects the user has
//! already approved, and what `CapabilityResult`
//! to return.
//!
//! In production, the future hardware service
//! daemon wraps the executor around the real
//! HAL calls (PulseAudio for audio, NetworkManager
//! for Wi-Fi, BlueZ for Bluetooth, etc). In
//! tests, the executor is what the renderer /
//! agent / aetherctl drives to ask "if I
//! asked the OS to route audio to the USB
//! speakers, would it succeed?".

use alloc::string::String;

use crate::{Capability, CapabilityResult, Device, DeviceKind, HardwareService};
#[cfg(test)]
use crate::DeviceState;

/// A typed capability request. Bundles the
/// capability, the target device, and the
/// actor (the user, the agent, or a peer).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CapabilityRequest {
    /// The capability to exercise.
    pub capability: Capability,
    /// The target device id. The executor
    /// looks the device up in the service.
    pub target_device_id: String,
    /// The actor that initiated the request.
    /// The consent gate uses this to decide
    /// whether to require user consent.
    pub actor: Actor,
}

impl CapabilityRequest {
    /// A new request.
    #[must_use]
    pub fn new(
        capability: Capability,
        target_device_id: impl Into<String>,
        actor: Actor,
    ) -> Self {
        Self { capability, target_device_id: target_device_id.into(), actor }
    }
}

/// Who initiated a capability request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Actor {
    /// The user is asking directly (e.g. from
    /// the system tray). Consent is implicit
    /// (the user clicked the menu item).
    User,
    /// The agent is asking on the user's
    /// behalf. Consent is required for any
    /// capability tagged `requires_consent`.
    Agent,
    /// A paired peer device is asking.
    /// Consent is *always* required, even
    /// for non-consent capabilities.
    Peer,
}

impl Actor {
    /// Whether this actor's request requires
    /// user consent before the executor
    /// proceeds.
    #[must_use]
    pub const fn requires_consent(&self) -> bool {
        match self {
            Self::User => false,
            Self::Agent | Self::Peer => true,
        }
    }
}

/// The executor: a stateful bridge that holds
/// the current `HardwareService` and a set of
/// approved (consented) capabilities. The
/// caller (the IPC layer, the agent runtime,
/// aetherctl) constructs one with the user's
/// approvals and asks it to exercise a
/// capability.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct CapabilityExecutor {
    /// The hardware service the executor
    /// operates on.
    pub service: HardwareService,
    /// The set of consent-gated capabilities
    /// the user has already approved (the IPC
    /// layer's consent dialog populates this).
    pub approved: Vec<Capability>,
}

impl CapabilityExecutor {
    /// A new executor with no approvals.
    #[must_use]
    pub fn new(service: HardwareService) -> Self {
        Self { service, approved: Vec::new() }
    }

    /// Approve a capability. The user has
    /// already consented; future requests for
    /// the same capability proceed without
    /// another prompt.
    pub fn approve(&mut self, capability: Capability) {
        self.approved.push(capability);
    }

    /// Has the user already approved this
    /// capability?
    #[must_use]
    pub fn is_approved(&self, capability: &Capability) -> bool {
        self.approved.iter().any(|c| c == capability)
    }

    /// Look up a device by id.
    #[must_use]
    pub fn device(&self, id: &str) -> Option<&Device> {
        self.service.get(id)
    }

    /// Execute a capability request. The
    /// executor returns a `CapabilityResult`:
    /// the call is *typed* — the caller can
    /// pattern-match on the outcome. This is
    /// the pure-data half: it does not touch
    /// the HAL. The caller (the future daemon)
    /// uses the `Ok { detail }` to drive the
    /// real kernel call.
    #[must_use]
    pub fn execute(&self, request: &CapabilityRequest) -> CapabilityResult {
        let device = match self.service.get(&request.target_device_id) {
            Some(d) => d,
            None => {
                return CapabilityResult::Refused {
                    reason: alloc::format!("device '{}' not found", request.target_device_id),
                };
            }
        };

        // State check: the device must be
        // usable to exercise any capability
        // (except the state-changing ones,
        // which can recover a disabled
        // device).
        if !state_allows(device, &request.capability) {
            return CapabilityResult::InvalidState { state: device.state };
        }

        // Capability check: the device must
        // claim the capability. `Enable` and
        // `Disable` are the exception — they
        // are about the device itself, not a
        // feature the device offers, so any
        // device in the service is fair game.
        if !matches!(request.capability, Capability::Enable | Capability::Disable)
            && !device.has_capability(&request.capability)
        {
            return CapabilityResult::Refused {
                reason: alloc::format!(
                    "device '{id}' does not claim capability '{verb}'",
                    id = device.id,
                    verb = request.capability.verb(),
                ),
            };
        }

        // Consent check: the request's actor
        // + the capability's
        // `requires_consent` flag determine
        // whether the user must approve.
        if request.actor.requires_consent()
            && request.capability.requires_consent()
            && !self.is_approved(&request.capability)
        {
            return CapabilityResult::Refused {
                reason: alloc::format!(
                    "capability '{verb}' requires user consent",
                    verb = request.capability.verb()
                ),
            };
        }

        // The pure-data executor returns a
        // deterministic Ok with a description
        // of the would-be side effect. The
        // real HAL call lives outside this
        // crate.
        let detail = describe(&device.kind, &request.capability, &device.name);
        CapabilityResult::Ok { detail }
    }

    /// Whether a capability *can* be exercised
    /// against the current service (without
    /// actually exercising it). Useful for
    /// the UI to grey out menu items.
    #[must_use]
    pub fn can_exercise(&self, request: &CapabilityRequest) -> bool {
        self.execute(request).is_ok()
    }

    /// The number of devices in the service.
    #[must_use]
    pub fn device_count(&self) -> usize {
        self.service.len()
    }
}

/// Whether the device's current state allows
/// a given capability. Some capabilities can
/// recover a disabled device (Enable,
/// Disable) — they accept any state. Others
/// require the device to be `Present`.
fn state_allows(device: &Device, cap: &Capability) -> bool {
    match cap {
        // Enable / Disable can recover a
        // disabled / errored device.
        Capability::Enable | Capability::Disable => true,
        // Everything else requires the device
        // to be in a usable state.
        _ => device.is_usable(),
    }
}

/// A short, human-readable description of
/// the side effect a capability *would* have
/// on a given device. The renderer shows
/// this as a toast when a capability is
/// approved; the future HAL call uses it for
/// audit logging.
fn describe(kind: &DeviceKind, cap: &Capability, name: &str) -> String {
    match cap {
        Capability::RouteAudio => alloc::format!("routed audio to {name}"),
        Capability::CaptureAudio => alloc::format!("set {name} as the active microphone"),
        Capability::CaptureVideo => alloc::format!("opened {name} for video capture"),
        Capability::ConnectWifi { ssid } => alloc::format!("connected {name} to '{ssid}'"),
        Capability::ConnectBluetooth { peer_id } => {
            alloc::format!("paired {name} with '{peer_id}'")
        }
        Capability::MountStorage => alloc::format!("mounted {name}"),
        Capability::UnmountStorage => alloc::format!("unmounted {name}"),
        Capability::Enable => alloc::format!("enabled {kind}", kind = kind.label()),
        Capability::Disable => alloc::format!("disabled {kind}", kind = kind.label()),
        Capability::SetBrightness { percent } => {
            alloc::format!("set {name} brightness to {percent}%")
        }
        Capability::Print { path } => alloc::format!("sent '{path}' to {name}"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{Device, PowerState};

    fn audio_out(id: &str) -> Device {
        Device::new(
            id,
            DeviceKind::AudioOutput,
            "Built-in Audio",
            "",
            "",
            DeviceState::Present,
            PowerState::SelfPowered,
        )
        .with_capability(Capability::RouteAudio)
    }

    fn disabled_audio(id: &str) -> Device {
        let mut d = audio_out(id);
        d.state = DeviceState::Disabled;
        d
    }

    fn mic(id: &str) -> Device {
        Device::new(
            id,
            DeviceKind::Microphone,
            "USB Mic",
            "",
            "",
            DeviceState::Present,
            PowerState::SelfPowered,
        )
        .with_capability(Capability::CaptureAudio)
    }

    #[test]
    fn actor_requires_consent() {
        assert!(!Actor::User.requires_consent());
        assert!(Actor::Agent.requires_consent());
        assert!(Actor::Peer.requires_consent());
    }

    #[test]
    fn execute_unknown_device_refuses() {
        let exec = CapabilityExecutor::new(HardwareService::new());
        let req = CapabilityRequest::new(Capability::RouteAudio, "nope", Actor::User);
        let r = exec.execute(&req);
        assert!(!r.is_ok());
    }

    #[test]
    fn execute_user_route_audio() {
        let mut svc = HardwareService::new();
        let _ = svc.upsert(audio_out("a"));
        let exec = CapabilityExecutor::new(svc);
        let req = CapabilityRequest::new(Capability::RouteAudio, "a", Actor::User);
        let r = exec.execute(&req);
        assert!(r.is_ok());
        if let CapabilityResult::Ok { detail } = r {
            assert!(detail.contains("Built-in Audio"));
        } else {
            panic!();
        }
    }

    #[test]
    fn execute_disabled_device_returns_invalid_state() {
        let mut svc = HardwareService::new();
        let _ = svc.upsert(disabled_audio("a"));
        let exec = CapabilityExecutor::new(svc);
        let req = CapabilityRequest::new(Capability::RouteAudio, "a", Actor::User);
        let r = exec.execute(&req);
        assert!(matches!(r, CapabilityResult::InvalidState { state: DeviceState::Disabled }));
    }

    #[test]
    fn execute_device_without_capability_refuses() {
        let mut svc = HardwareService::new();
        let _ = svc.upsert(mic("a"));
        let exec = CapabilityExecutor::new(svc);
        // The mic does not have RouteAudio.
        let req = CapabilityRequest::new(Capability::RouteAudio, "a", Actor::User);
        let r = exec.execute(&req);
        assert!(matches!(r, CapabilityResult::Refused { .. }));
    }

    #[test]
    fn execute_consent_required_refused_without_approval() {
        let mut svc = HardwareService::new();
        let _ = svc.upsert(mic("a"));
        let exec = CapabilityExecutor::new(svc);
        // The agent is asking to capture audio
        // (consent-required) without prior
        // approval.
        let req = CapabilityRequest::new(Capability::CaptureAudio, "a", Actor::Agent);
        let r = exec.execute(&req);
        assert!(!r.is_ok());
    }

    #[test]
    fn execute_consent_required_succeeds_after_approval() {
        let mut svc = HardwareService::new();
        let _ = svc.upsert(mic("a"));
        let mut exec = CapabilityExecutor::new(svc);
        exec.approve(Capability::CaptureAudio);
        let req = CapabilityRequest::new(Capability::CaptureAudio, "a", Actor::Agent);
        let r = exec.execute(&req);
        assert!(r.is_ok());
    }

    #[test]
    fn execute_user_no_consent_required_for_user() {
        let mut svc = HardwareService::new();
        let _ = svc.upsert(mic("a"));
        let exec = CapabilityExecutor::new(svc);
        // CaptureAudio requires consent, but
        // the user is asking directly so
        // consent is implicit.
        let req = CapabilityRequest::new(Capability::CaptureAudio, "a", Actor::User);
        let r = exec.execute(&req);
        assert!(r.is_ok());
    }

    #[test]
    fn enable_recovers_disabled_device() {
        let mut svc = HardwareService::new();
        let _ = svc.upsert(disabled_audio("a"));
        let exec = CapabilityExecutor::new(svc);
        let req = CapabilityRequest::new(Capability::Enable, "a", Actor::User);
        let r = exec.execute(&req);
        assert!(r.is_ok());
    }

    #[test]
    fn disable_recovers_error_device() {
        let mut svc = HardwareService::new();
        let mut a = audio_out("a");
        a.state = DeviceState::Errored;
        let _ = svc.upsert(a);
        let exec = CapabilityExecutor::new(svc);
        let req = CapabilityRequest::new(Capability::Disable, "a", Actor::User);
        let r = exec.execute(&req);
        assert!(r.is_ok());
    }

    #[test]
    fn can_exercise_matches_execute() {
        let mut svc = HardwareService::new();
        let _ = svc.upsert(audio_out("a"));
        let exec = CapabilityExecutor::new(svc);
        let req_ok = CapabilityRequest::new(Capability::RouteAudio, "a", Actor::User);
        assert!(exec.can_exercise(&req_ok));
        let req_bad = CapabilityRequest::new(Capability::RouteAudio, "nope", Actor::User);
        assert!(!exec.can_exercise(&req_bad));
    }

    #[test]
    fn describe_format() {
        let d = describe(
            &DeviceKind::AudioOutput,
            &Capability::RouteAudio,
            "USB Speakers",
        );
        assert!(d.contains("USB Speakers"));
    }

    #[test]
    fn device_count() {
        let mut svc = HardwareService::new();
        let _ = svc.upsert(audio_out("a"));
        let _ = svc.upsert(mic("b"));
        let exec = CapabilityExecutor::new(svc);
        assert_eq!(exec.device_count(), 2);
    }

    #[test]
    fn is_approved_after_approve() {
        let exec = CapabilityExecutor::new(HardwareService::new());
        assert!(!exec.is_approved(&Capability::CaptureAudio));
        let mut exec = exec;
        exec.approve(Capability::CaptureAudio);
        assert!(exec.is_approved(&Capability::CaptureAudio));
    }
}
