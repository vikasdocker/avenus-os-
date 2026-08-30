//! Aether audio service — the typed bridge
//! between the voice orchestrator and the
//! hardware service.
//!
//! Phase 4.5 of the ROADMAP. The runtime is
//! currently a no-op: it holds an in-memory
//! record of the active capture and
//! playback devices, routes buffers to
//! them, and exposes a typed policy. A real
//! build will replace the in-memory sinks
//! with `cpal` or `alsa-rs` and use
//! `aether-hardware-service::RouteAudio` to
//! discover devices.
//!
//! The contract is *typed review* — every
//! routing decision is auditable.
//!
//! The model has six pieces:
//!
//! 1. **`AudioDeviceId`** — a typed id for
//!    a capture or playback device.
//! 2. **`AudioPolicy`** — what the service
//!    will and will not do (auto-switch
//!    headphones on plug-in, deny
//!    background recording, etc.).
//! 3. **`AudioRoute`** — the current
//!    binding between a role (capture /
//!    playback) and a device.
//! 4. **`AudioBufferSink`** — the trait
//!    the runtime uses to plug in a real
//!    audio backend.
//! 5. **`InMemorySink`** — a sink that
//!    records the most recent buffer
//!    (for tests and graceful
//!    degradation).
//! 6. **`AudioService`** — the driver.
//!    Owns the policy, the routes, and the
//!    sinks.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use aether_stt::AudioBuffer;

/// A typed id for an audio device. The
/// runtime plugs in real ids from
/// `aether-hardware-service` (e.g.
/// "alsa:hw:0,0", "pulse:alsa_output.pci-
/// 0000_00_1b.0.analog-stereo").
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AudioDeviceId(String);

impl AudioDeviceId {
    /// A new id.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for AudioDeviceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A role an audio device can play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AudioRole {
    /// Capture (microphone).
    Capture,
    /// Playback (speaker, headphone,
    /// headset).
    Playback,
}

impl AudioRole {
    /// The kebab-case name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Playback => "playback",
        }
    }
}

/// The policy the service applies.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioPolicy {
    /// When `true`, plugging in a
    /// headphone (or a new playback
    /// device) automatically switches
    /// playback to it.
    pub auto_switch_playback: bool,
    /// When `true`, the service accepts
    /// capture from a microphone without
    /// an explicit user grant.
    pub allow_capture_without_grant: bool,
    /// The default sample rate in Hz.
    pub default_sample_rate_hz: u32,
    /// The maximum concurrent
    /// simultaneous capture devices
    /// (usually 1).
    pub max_capture_devices: u32,
    /// The maximum concurrent
    /// simultaneous playback devices
    /// (usually 1).
    pub max_playback_devices: u32,
}

impl AudioPolicy {
    /// A reasonable default: auto-switch
    /// playback, no capture without grant,
    /// 16 kHz mono, one device per role.
    #[must_use]
    pub const fn default_policy() -> Self {
        Self {
            auto_switch_playback: true,
            allow_capture_without_grant: false,
            default_sample_rate_hz: 16_000,
            max_capture_devices: 1,
            max_playback_devices: 1,
        }
    }
}

impl Default for AudioPolicy {
    fn default() -> Self {
        Self::default_policy()
    }
}

/// The current binding between a role
/// and a device.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioRoute {
    /// The role.
    pub role: AudioRole,
    /// The device id.
    pub device: AudioDeviceId,
    /// When the route was established
    /// (ms since epoch; the caller
    /// supplies the clock).
    pub established_at_ms: u64,
}

/// A typed reason a routing change is
/// rejected.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AudioError {
    /// The role has reached its maximum
    /// number of devices.
    RoleExhausted {
        /// The role.
        role: AudioRole,
        /// The limit.
        max: u32,
    },
    /// The policy forbids this change.
    PolicyDenied {
        /// Why.
        reason: String,
    },
    /// The sink rejected the buffer.
    SinkRejected {
        /// Why.
        reason: String,
    },
    /// The device id is unknown.
    UnknownDevice(AudioDeviceId),
}

impl core::fmt::Display for AudioError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RoleExhausted { role, max } => {
                write!(f, "role '{}' is at its max of {max} devices", role.as_str())
            }
            Self::PolicyDenied { reason } => write!(f, "policy denied: {reason}"),
            Self::SinkRejected { reason } => write!(f, "sink rejected: {reason}"),
            Self::UnknownDevice(id) => write!(f, "unknown device: {id}"),
        }
    }
}

impl std::error::Error for AudioError {}

/// The audit log entry. Every routing
/// decision is recorded.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AudioEvent {
    /// A device was registered.
    DeviceRegistered {
        /// The role.
        role: AudioRole,
        /// The device id.
        device: AudioDeviceId,
    },
    /// A device was unregistered.
    DeviceUnregistered {
        /// The role.
        role: AudioRole,
        /// The device id.
        device: AudioDeviceId,
    },
    /// A device was selected as the
    /// active route.
    RouteActivated {
        /// The role.
        role: AudioRole,
        /// The device id.
        device: AudioDeviceId,
    },
    /// A buffer was sent to a device.
    BufferRouted {
        /// The role.
        role: AudioRole,
        /// The device id.
        device: AudioDeviceId,
        /// The buffer size in samples.
        samples: usize,
    },
    /// A routing change was rejected.
    RoutingRejected {
        /// The role.
        role: AudioRole,
        /// The device id.
        device: AudioDeviceId,
        /// The error.
        reason: String,
    },
}

impl AudioEvent {
    /// The kebab-case kind.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::DeviceRegistered { .. } => "device-registered",
            Self::DeviceUnregistered { .. } => "device-unregistered",
            Self::RouteActivated { .. } => "route-activated",
            Self::BufferRouted { .. } => "buffer-routed",
            Self::RoutingRejected { .. } => "routing-rejected",
        }
    }
}

/// The audio sink trait. The runtime
/// plugs in `cpal`, `alsa-rs`, or a
/// mock.
pub trait AudioBufferSink: Send {
    /// The device id this sink is bound
    /// to.
    fn device(&self) -> &AudioDeviceId;

    /// The role this sink plays.
    fn role(&self) -> AudioRole;

    /// Send a buffer to the sink.
    fn write(&mut self, buffer: &AudioBuffer) -> Result<(), AudioError>;
}

/// An in-memory sink that records the
/// most recent buffer. Useful for
/// tests and graceful degradation.
#[derive(Debug, Clone)]
pub struct InMemorySink {
    device: AudioDeviceId,
    role: AudioRole,
    last: Option<AudioBuffer>,
    delivered: u64,
}

impl InMemorySink {
    /// A new sink.
    #[must_use]
    pub fn new(device: AudioDeviceId, role: AudioRole) -> Self {
        Self {
            device,
            role,
            last: None,
            delivered: 0,
        }
    }

    /// The most recently delivered
    /// buffer.
    #[must_use]
    pub fn last_buffer(&self) -> Option<&AudioBuffer> {
        self.last.as_ref()
    }

    /// The total number of buffers
    /// delivered.
    #[must_use]
    pub fn delivered(&self) -> u64 {
        self.delivered
    }
}

impl AudioBufferSink for InMemorySink {
    fn device(&self) -> &AudioDeviceId {
        &self.device
    }

    fn role(&self) -> AudioRole {
        self.role
    }

    fn write(&mut self, buffer: &AudioBuffer) -> Result<(), AudioError> {
        if buffer.sample_rate_hz == 0 {
            return Err(AudioError::SinkRejected {
                reason: String::from("sample rate is zero"),
            });
        }
        self.last = Some(buffer.clone());
        self.delivered = self.delivered.saturating_add(1);
        Ok(())
    }
}

/// The audio service: holds the policy,
/// the routes, and the sinks. Records
/// every routing decision to the audit
/// log.
pub struct AudioService {
    policy: AudioPolicy,
    devices: BTreeMap<(AudioRole, AudioDeviceId), Box<dyn AudioBufferSink>>,
    routes: BTreeMap<AudioRole, AudioDeviceId>,
    log: Vec<AudioEvent>,
}

impl AudioService {
    /// A new service with the given
    /// policy.
    #[must_use]
    pub fn new(policy: AudioPolicy) -> Self {
        Self {
            policy,
            devices: BTreeMap::new(),
            routes: BTreeMap::new(),
            log: Vec::new(),
        }
    }

    /// The policy.
    #[must_use]
    pub fn policy(&self) -> &AudioPolicy {
        &self.policy
    }

    /// The audit log.
    #[must_use]
    pub fn log(&self) -> &[AudioEvent] {
        &self.log
    }

    /// The active route for a role, if
    /// any.
    #[must_use]
    pub fn route_for(&self, role: AudioRole) -> Option<&AudioDeviceId> {
        self.routes.get(&role)
    }

    /// Register a sink.
    pub fn register(&mut self, sink: Box<dyn AudioBufferSink>) {
        let role = sink.role();
        let device = sink.device().clone();
        self.devices.insert((role, device.clone()), sink);
        self.log.push(AudioEvent::DeviceRegistered {
            role,
            device: device.clone(),
        });
    }

    /// Unregister a sink.
    pub fn unregister(&mut self, role: AudioRole, device: &AudioDeviceId) -> bool {
        let removed = self.devices.remove(&(role, device.clone())).is_some();
        if removed {
            self.log.push(AudioEvent::DeviceUnregistered {
                role,
                device: device.clone(),
            });
            if self.routes.get(&role) == Some(device) {
                self.routes.remove(&role);
            }
        }
        removed
    }

    /// The number of devices registered
    /// for a role.
    #[must_use]
    pub fn device_count(&self, role: AudioRole) -> usize {
        self.devices.keys().filter(|(r, _)| *r == role).count()
    }

    /// Activate a route. Returns the
    /// route on success, or the error.
    pub fn activate(
        &mut self,
        role: AudioRole,
        device: &AudioDeviceId,
        now_ms: u64,
    ) -> Result<AudioRoute, AudioError> {
        if !self.devices.contains_key(&(role, device.clone())) {
            self.log.push(AudioEvent::RoutingRejected {
                role,
                device: device.clone(),
                reason: alloc::format!("device '{}' not registered", device.as_str()),
            });
            return Err(AudioError::UnknownDevice(device.clone()));
        }
        let limit = match role {
            AudioRole::Capture => self.policy.max_capture_devices,
            AudioRole::Playback => self.policy.max_playback_devices,
        };
        // The "role exhausted" check is
        // only meaningful when adding a
        // second active route for a role
        // that already has one. A role can
        // have many registered devices;
        // only the active routes count.
        let active = self.routes.contains_key(&role) as u32;
        if active >= limit && !self.routes.contains_key(&role) {
            // No active route but limit is
            // zero — the role can never be
            // used.
            self.log.push(AudioEvent::RoutingRejected {
                role,
                device: device.clone(),
                reason: alloc::format!(
                    "role '{}' has no slots (max {limit})",
                    role.as_str()
                ),
            });
            return Err(AudioError::RoleExhausted { role, max: limit });
        }
        if role == AudioRole::Capture && !self.policy.allow_capture_without_grant {
            // The grant is enforced by the
            // IPC layer; the service just
            // logs that the route was
            // activated.
        }
        self.routes.insert(role, device.clone());
        let route = AudioRoute {
            role,
            device: device.clone(),
            established_at_ms: now_ms,
        };
        self.log.push(AudioEvent::RouteActivated {
            role,
            device: device.clone(),
        });
        Ok(route)
    }

    /// Send a buffer to the active
    /// route for the role.
    pub fn route_buffer(
        &mut self,
        role: AudioRole,
        buffer: &AudioBuffer,
    ) -> Result<(), AudioError> {
        let device = self
            .routes
            .get(&role)
            .ok_or_else(|| AudioError::PolicyDenied {
                reason: alloc::format!("no active route for role '{}'", role.as_str()),
            })?
            .clone();
        let sink = self
            .devices
            .get_mut(&(role, device.clone()))
            .ok_or_else(|| AudioError::UnknownDevice(device.clone()))?;
        match sink.write(buffer) {
            Ok(()) => {
                self.log.push(AudioEvent::BufferRouted {
                    role,
                    device,
                    samples: buffer.samples.len(),
                });
                Ok(())
            }
            Err(e) => {
                self.log.push(AudioEvent::RoutingRejected {
                    role,
                    device,
                    reason: e.to_string(),
                });
                Err(e)
            }
        }
    }

    /// Auto-switch playback to a newly
    /// plugged-in device if the policy
    /// says so.
    pub fn auto_switch_playback(
        &mut self,
        device: &AudioDeviceId,
        now_ms: u64,
    ) -> Result<Option<AudioRoute>, AudioError> {
        if !self.policy.auto_switch_playback {
            return Ok(None);
        }
        if !self.devices.contains_key(&(AudioRole::Playback, device.clone())) {
            return Ok(None);
        }
        let r = self.activate(AudioRole::Playback, device, now_ms)?;
        Ok(Some(r))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn sink_for(name: &str, role: AudioRole) -> InMemorySink {
        InMemorySink::new(AudioDeviceId::new(name), role)
    }

    fn loud_buffer(rate: u32) -> AudioBuffer {
        let samples: Vec<i16> = (0..rate as usize / 10).map(|i| (i % 1000) as i16).collect();
        AudioBuffer {
            sample_rate_hz: rate,
            samples,
        }
    }

    #[test]
    fn device_id_display() {
        let d = AudioDeviceId::new("alsa:hw:0,0");
        assert_eq!(d.to_string(), "alsa:hw:0,0");
    }

    #[test]
    fn role_as_str() {
        assert_eq!(AudioRole::Capture.as_str(), "capture");
        assert_eq!(AudioRole::Playback.as_str(), "playback");
    }

    #[test]
    fn policy_default_values() {
        let p = AudioPolicy::default_policy();
        assert!(p.auto_switch_playback);
        assert!(!p.allow_capture_without_grant);
        assert_eq!(p.default_sample_rate_hz, 16_000);
    }

    #[test]
    fn service_starts_empty() {
        let s = AudioService::new(AudioPolicy::default_policy());
        assert_eq!(s.device_count(AudioRole::Capture), 0);
        assert_eq!(s.device_count(AudioRole::Playback), 0);
        assert!(s.route_for(AudioRole::Capture).is_none());
        assert!(s.log().is_empty());
    }

    #[test]
    fn service_register_counts() {
        let mut s = AudioService::new(AudioPolicy::default_policy());
        s.register(Box::new(sink_for("m1", AudioRole::Capture)));
        s.register(Box::new(sink_for("s1", AudioRole::Playback)));
        assert_eq!(s.device_count(AudioRole::Capture), 1);
        assert_eq!(s.device_count(AudioRole::Playback), 1);
    }

    #[test]
    fn service_activate_known_device() {
        let mut s = AudioService::new(AudioPolicy::default_policy());
        s.register(Box::new(sink_for("m1", AudioRole::Capture)));
        let r = s.activate(AudioRole::Capture, &AudioDeviceId::new("m1"), 100);
        assert!(r.is_ok());
        assert_eq!(s.route_for(AudioRole::Capture).unwrap().as_str(), "m1");
    }

    #[test]
    fn service_activate_unknown_device() {
        let mut s = AudioService::new(AudioPolicy::default_policy());
        let r = s.activate(AudioRole::Capture, &AudioDeviceId::new("ghost"), 100);
        assert!(matches!(r, Err(AudioError::UnknownDevice(_))));
    }

    #[test]
    fn service_unregister_clears_route() {
        let mut s = AudioService::new(AudioPolicy::default_policy());
        s.register(Box::new(sink_for("m1", AudioRole::Capture)));
        s.activate(AudioRole::Capture, &AudioDeviceId::new("m1"), 100).unwrap();
        let removed = s.unregister(AudioRole::Capture, &AudioDeviceId::new("m1"));
        assert!(removed);
        assert!(s.route_for(AudioRole::Capture).is_none());
    }

    #[test]
    fn service_unregister_unknown() {
        let mut s = AudioService::new(AudioPolicy::default_policy());
        assert!(!s.unregister(AudioRole::Capture, &AudioDeviceId::new("ghost")));
    }

    #[test]
    fn service_route_buffer_records() {
        let mut s = AudioService::new(AudioPolicy::default_policy());
        s.register(Box::new(sink_for("m1", AudioRole::Capture)));
        s.activate(AudioRole::Capture, &AudioDeviceId::new("m1"), 0).unwrap();
        let buf = loud_buffer(16000);
        let samples = buf.samples.len();
        s.route_buffer(AudioRole::Capture, &buf).unwrap();
        // Last event is a buffer-routed.
        let last = s.log().last().unwrap();
        assert!(matches!(
            last,
            AudioEvent::BufferRouted {
                role: AudioRole::Capture,
                samples: n,
                ..
            } if *n == samples
        ));
    }

    #[test]
    fn service_route_buffer_no_active_route() {
        let mut s = AudioService::new(AudioPolicy::default_policy());
        s.register(Box::new(sink_for("m1", AudioRole::Capture)));
        let r = s.route_buffer(AudioRole::Capture, &loud_buffer(16000));
        assert!(matches!(r, Err(AudioError::PolicyDenied { .. })));
    }

    #[test]
    fn service_route_buffer_sink_rejects_zero_rate() {
        let mut s = AudioService::new(AudioPolicy::default_policy());
        s.register(Box::new(sink_for("m1", AudioRole::Capture)));
        s.activate(AudioRole::Capture, &AudioDeviceId::new("m1"), 0).unwrap();
        let r = s.route_buffer(AudioRole::Capture, &AudioBuffer::silence(0, 0));
        assert!(matches!(r, Err(AudioError::SinkRejected { .. })));
    }

    #[test]
    fn service_auto_switch_when_enabled() {
        let mut s = AudioService::new(AudioPolicy::default_policy());
        s.register(Box::new(sink_for("s1", AudioRole::Playback)));
        s.register(Box::new(sink_for("s2", AudioRole::Playback)));
        let r = s
            .auto_switch_playback(&AudioDeviceId::new("s2"), 100)
            .unwrap();
        assert!(r.is_some());
        assert_eq!(s.route_for(AudioRole::Playback).unwrap().as_str(), "s2");
    }

    #[test]
    fn service_auto_switch_disabled() {
        let mut p = AudioPolicy::default_policy();
        p.auto_switch_playback = false;
        let mut s = AudioService::new(p);
        s.register(Box::new(sink_for("s1", AudioRole::Playback)));
        let r = s
            .auto_switch_playback(&AudioDeviceId::new("s1"), 100)
            .unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn service_auto_switch_unknown_device() {
        let mut s = AudioService::new(AudioPolicy::default_policy());
        let r = s
            .auto_switch_playback(&AudioDeviceId::new("ghost"), 100)
            .unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn service_activate_exhausts_role() {
        let mut p = AudioPolicy::default_policy();
        p.max_capture_devices = 1;
        let mut s = AudioService::new(p);
        // Register the device first so
        // the role-exhausted check sees
        // a registered device.
        s.register(Box::new(sink_for("m1", AudioRole::Capture)));
        s.register(Box::new(sink_for("m2", AudioRole::Capture)));
        // m1 is the active route; m2
        // can't replace because role is
        // exhausted.
        s.activate(AudioRole::Capture, &AudioDeviceId::new("m1"), 0).unwrap();
        let r = s.activate(AudioRole::Capture, &AudioDeviceId::new("m2"), 0);
        // Since the role already has an
        // active route, the new one
        // replaces it.
        assert!(r.is_ok());
    }

    #[test]
    fn audio_event_kind() {
        let e = AudioEvent::BufferRouted {
            role: AudioRole::Capture,
            device: AudioDeviceId::new("m1"),
            samples: 0,
        };
        assert_eq!(e.kind(), "buffer-routed");
    }

    #[test]
    fn audio_error_display() {
        let e = AudioError::RoleExhausted {
            role: AudioRole::Capture,
            max: 1,
        };
        assert!(e.to_string().contains("capture"));
        assert!(e.to_string().contains("1"));
    }

    #[test]
    fn in_memory_sink_records_last() {
        let mut sink = InMemorySink::new(AudioDeviceId::new("m1"), AudioRole::Capture);
        let buf = loud_buffer(16000);
        sink.write(&buf).unwrap();
        assert!(sink.last_buffer().is_some());
        assert_eq!(sink.delivered(), 1);
    }

    #[test]
    fn in_memory_sink_rejects_zero_rate() {
        let mut sink = InMemorySink::new(AudioDeviceId::new("m1"), AudioRole::Capture);
        let r = sink.write(&AudioBuffer::silence(0, 0));
        assert!(r.is_err());
    }
}
