//! Aether Hardware Service — the typed model of
//! every piece of hardware Aether can see.
//!
//! Phase 8 of the ROADMAP. The contract has three
//! pieces:
//!
//! 1. **`DeviceKind`** — a closed enum of the
//!    hardware classes the service knows about
//!    (Cpu, Gpu, Display, Keyboard, Touchpad,
//!    Mouse, AudioOutput, Microphone, Camera,
//!    Wifi, Bluetooth, Ethernet, Usb, Storage,
//!    Battery, ThermalSensor, ExternalDisplay,
//!    Printer, FutureSensor). The taxonomy is
//!    the same across every device so the agent
//!    can pattern-match.
//!
//! 2. **`Device`** — a single piece of
//!    hardware. It carries an id, kind, vendor
//!    / product strings, a human-readable name,
//!    a `DeviceState` (Present / Disconnected /
//!    Enabled / Disabled / Errored), a
//!    `PowerState` (SelfPowered / Battery /
//!    PoweredOff), and a list of typed
//!    `Capability`s the agent can exercise.
//!
//! 3. **`HardwareService`** — the registry of
//!    all known devices. The service can
//!    upsert, remove, find, list, and look up
//!    devices by kind. It also supports
//!    `find_capable` (return the devices that
//!    claim a specific capability, e.g. "the
//!    audio output the user can route to") and
//!    `toggle` / `set_state` (the typed writes
//!    the agent / shell can make).
//!
//! The crate is *pure* — it does not touch the
//! HAL, the kernel, or any hardware bus. The
//! future hardware service daemon (Phase 10's
//! real bring-up) is what calls
//! `upsert_device` when a device appears or
//! disappears; this crate defines the model
//! the rest of Aether consumes.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// The kind of hardware a `Device` represents.
/// The taxonomy is closed so the agent can
/// pattern-match exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DeviceKind {
    /// The system CPU (one entry per socket /
    /// cluster).
    Cpu,
    /// The system GPU.
    Gpu,
    /// A built-in or attached display panel.
    Display,
    /// An external display (HDMI / DP / USB-C
    /// alt-mode / AirPlay / Miracast sink).
    ExternalDisplay,
    /// A keyboard (built-in, USB, Bluetooth).
    Keyboard,
    /// A touchpad / trackpad.
    Touchpad,
    /// A mouse / pointing device.
    Mouse,
    /// An audio output device (speakers,
    /// headphones, headset sink, HDMI audio).
    AudioOutput,
    /// A microphone / audio input.
    Microphone,
    /// A camera (built-in, USB, IP).
    Camera,
    /// A Wi-Fi adapter.
    Wifi,
    /// A Bluetooth adapter.
    Bluetooth,
    /// A wired Ethernet adapter.
    Ethernet,
    /// A USB host controller or device.
    Usb,
    /// A storage device (SSD, HDD, NVMe, SD
    /// card, USB stick).
    Storage,
    /// A battery (the system battery, or a
    /// device-class battery on a paired peer).
    Battery,
    /// A thermal sensor (CPU package,
    /// per-zone, ambient).
    ThermalSensor,
    /// A printer (or "print to PDF" sink).
    Printer,
    /// A future sensor kind (the OS can
    /// surface arbitrary new sensors through
    /// the same model).
    FutureSensor,
}

impl DeviceKind {
    /// The kebab-case name (stable for the
    /// renderer / IPC).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Display => "display",
            Self::ExternalDisplay => "external-display",
            Self::Keyboard => "keyboard",
            Self::Touchpad => "touchpad",
            Self::Mouse => "mouse",
            Self::AudioOutput => "audio-output",
            Self::Microphone => "microphone",
            Self::Camera => "camera",
            Self::Wifi => "wifi",
            Self::Bluetooth => "bluetooth",
            Self::Ethernet => "ethernet",
            Self::Usb => "usb",
            Self::Storage => "storage",
            Self::Battery => "battery",
            Self::ThermalSensor => "thermal-sensor",
            Self::Printer => "printer",
            Self::FutureSensor => "future-sensor",
        }
    }

    /// A human-readable label (the shell uses
    /// this as the icon tooltip).
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Gpu => "GPU",
            Self::Display => "Display",
            Self::ExternalDisplay => "External display",
            Self::Keyboard => "Keyboard",
            Self::Touchpad => "Touchpad",
            Self::Mouse => "Mouse",
            Self::AudioOutput => "Audio output",
            Self::Microphone => "Microphone",
            Self::Camera => "Camera",
            Self::Wifi => "Wi-Fi",
            Self::Bluetooth => "Bluetooth",
            Self::Ethernet => "Ethernet",
            Self::Usb => "USB",
            Self::Storage => "Storage",
            Self::Battery => "Battery",
            Self::ThermalSensor => "Thermal sensor",
            Self::Printer => "Printer",
            Self::FutureSensor => "Future sensor",
        }
    }
}

/// A device's working state. The hardware
/// service daemon updates this on plug /
/// unplug / enable / disable events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DeviceState {
    /// The device is present and operating
    /// normally.
    Present,
    /// The device is present but the user /
    /// OS has disabled it (e.g. Airplane mode
    /// on Wi-Fi, mute on the mic).
    Disabled,
    /// The device is reporting an error (the
    /// driver can read it but the OS can't
    /// fully control it).
    Errored,
    /// The device has been removed since the
    /// last upsert (e.g. unplugged USB stick).
    /// The service keeps the entry briefly so
    /// the UI can animate the removal, then
    /// drops it.
    Disconnected,
}

impl DeviceState {
    /// The kebab-case name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Disabled => "disabled",
            Self::Errored => "errored",
            Self::Disconnected => "disconnected",
        }
    }
}

/// A device's power source. Most devices are
/// self-powered (they take power from the
/// system). Some are battery-powered (a paired
/// Bluetooth mouse) and report their own
/// battery level. Some are powered off (e.g.
/// a USB port in software-off state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PowerState {
    /// The device draws power from the system.
    SelfPowered,
    /// The device has its own battery. The
    /// level is `level_per_mille` (0..=1000).
    Battery {
        /// The battery level in per-mille
        /// (0..=1000). `Some(750)` means 75%.
        level_per_mille: Option<u16>,
    },
    /// The device is in a powered-off state
    /// (e.g. a USB port with `usb_control`.
    PowerOff,
}

/// A typed capability a device exposes. The
/// agent / shell uses these to discover what
/// it can ask a device to do without parsing
/// free-form strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Capability {
    /// Route audio output to this device
    /// (e.g. "switch the music to my
    /// headphones").
    RouteAudio,
    /// Use this device as a microphone input
    /// (e.g. "use the USB mic for the meeting").
    CaptureAudio,
    /// Capture stills / video from this
    /// camera.
    CaptureVideo,
    /// Connect to a Wi-Fi network through
    /// this adapter. The `ssid` argument is
    /// the network name.
    ConnectWifi {
        /// The network SSID.
        ssid: String,
    },
    /// Pair / connect to a Bluetooth peer.
    /// The `peer_id` argument is the peer's
    /// hardware address.
    ConnectBluetooth {
        /// The peer's MAC / address.
        peer_id: String,
    },
    /// Mount this storage device (e.g. when
    /// the user plugs in a USB stick).
    MountStorage,
    /// Unmount this storage device.
    UnmountStorage,
    /// Enable the device (turn on Wi-Fi, mute
    /// a mic, etc).
    Enable,
    /// Disable the device.
    Disable,
    /// Set the display's brightness (0..=100).
    SetBrightness {
        /// The target brightness percentage.
        percent: u8,
    },
    /// Print to this printer.
    Print {
        /// The path of the document to print.
        path: String,
    },
}

impl Capability {
    /// The kebab-case name of the capability
    /// (the IPC layer uses this as the verb).
    #[must_use]
    pub fn verb(&self) -> &'static str {
        match self {
            Self::RouteAudio => "route-audio",
            Self::CaptureAudio => "capture-audio",
            Self::CaptureVideo => "capture-video",
            Self::ConnectWifi { .. } => "connect-wifi",
            Self::ConnectBluetooth { .. } => "connect-bluetooth",
            Self::MountStorage => "mount-storage",
            Self::UnmountStorage => "unmount-storage",
            Self::Enable => "enable",
            Self::Disable => "disable",
            Self::SetBrightness { .. } => "set-brightness",
            Self::Print { .. } => "print",
        }
    }

    /// Whether exercising this capability
    /// requires user consent. Destructive /
    /// privacy-sensitive capabilities
    /// (mounting storage, capturing video /
    /// audio, connecting to a peer, printing)
    /// are tagged `true`; the read-only /
    /// display-state ones are `false`.
    #[must_use]
    pub const fn requires_consent(&self) -> bool {
        match self {
            Self::RouteAudio
            | Self::SetBrightness { .. }
            | Self::Enable
            | Self::Disable => false,
            Self::CaptureAudio
            | Self::CaptureVideo
            | Self::ConnectWifi { .. }
            | Self::ConnectBluetooth { .. }
            | Self::MountStorage
            | Self::UnmountStorage
            | Self::Print { .. } => true,
        }
    }
}

/// A single piece of hardware. The hardware
/// service daemon (the future Phase 10 bring-up)
/// produces these; the agent and the shell
/// consume them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Device {
    /// A unique, stable id (the HAL's bus path
    /// or the service's UUID for synthesized
    /// devices).
    pub id: String,
    /// The device's kind.
    pub kind: DeviceKind,
    /// A human-readable name (e.g. "Built-in
    /// Audio", "Logitech MX Master 3",
    /// "PLP2 SSD").
    pub name: String,
    /// The vendor string (e.g. "Logitech",
    /// "Intel", "Realtek"). May be empty if
    /// the device is built-in.
    pub vendor: String,
    /// The product string (e.g. "MX Master 3",
    /// "AX211"). May be empty.
    pub product: String,
    /// The current working state.
    pub state: DeviceState,
    /// The power state.
    pub power: PowerState,
    /// The capabilities the device exposes.
    /// The renderer lists them; the agent
    /// pattern-matches on them.
    pub capabilities: Vec<Capability>,
}

impl Device {
    /// A new device. All fields except
    /// `capabilities` are required; the
    /// `capabilities` list starts empty and
    /// the caller pushes the device's
    /// capabilities on.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        kind: DeviceKind,
        name: impl Into<String>,
        vendor: impl Into<String>,
        product: impl Into<String>,
        state: DeviceState,
        power: PowerState,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            name: name.into(),
            vendor: vendor.into(),
            product: product.into(),
            state,
            power,
            capabilities: Vec::new(),
        }
    }

    /// Append a capability.
    #[must_use]
    pub fn with_capability(mut self, cap: Capability) -> Self {
        self.capabilities.push(cap);
        self
    }

    /// Whether the device is currently usable
    /// (i.e. `Present` and not errored).
    #[must_use]
    pub const fn is_usable(&self) -> bool {
        matches!(self.state, DeviceState::Present)
    }

    /// Whether the device claims a specific
    /// capability. The agent uses this to
    /// answer "can I route audio to this
    /// device?".
    #[must_use]
    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }
}

/// The hardware service: the in-memory registry
/// of every device the OS can see. The service
/// is the single source of truth for the
/// renderer (the system tray polls it) and the
/// agent (the proposal pipeline queries it).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HardwareService {
    /// The registered devices.
    pub devices: Vec<Device>,
}

impl HardwareService {
    /// A new, empty service.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Upsert a device. If a device with the
    /// same id is already registered, it is
    /// replaced (the hardware service daemon
    /// calls this on every state change).
    /// Returns `true` if a new device was
    /// added, `false` if an existing one was
    /// updated.
    #[must_use]
    pub fn upsert(&mut self, device: Device) -> bool {
        if let Some(existing) = self.devices.iter_mut().find(|d| d.id == device.id) {
            *existing = device;
            return false;
        }
        self.devices.push(device);
        true
    }

    /// Remove a device by id. Returns the
    /// removed device (the renderer can use
    /// the return to animate the removal).
    #[must_use]
    pub fn remove(&mut self, id: &str) -> Option<Device> {
        let pos = self.devices.iter().position(|d| d.id == id)?;
        Some(self.devices.remove(pos))
    }

    /// Look up a device by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    /// List all devices of a given kind.
    #[must_use]
    pub fn with_kind(&self, kind: DeviceKind) -> Vec<&Device> {
        self.devices.iter().filter(|d| d.kind == kind).collect()
    }

    /// Find the first usable device that
    /// claims a given capability. Useful for
    /// "route audio to the next-available
    /// speaker".
    #[must_use]
    pub fn find_capable(&self, cap: &Capability) -> Option<&Device> {
        self.devices
            .iter()
            .find(|d| d.is_usable() && d.has_capability(cap))
    }

    /// Find all usable devices that claim a
    /// given capability. Useful for "list all
    /// the audio outputs the user can switch
    /// to".
    #[must_use]
    pub fn all_capable(&self, cap: &Capability) -> Vec<&Device> {
        self.devices
            .iter()
            .filter(|d| d.is_usable() && d.has_capability(cap))
            .collect()
    }

    /// Set a device's state. The hardware
    /// service daemon's enabler uses this when
    /// the user toggles a switch. Returns the
    /// previous state for audit logging.
    #[must_use]
    pub fn set_state(&mut self, id: &str, state: DeviceState) -> Option<DeviceState> {
        let device = self.devices.iter_mut().find(|d| d.id == id)?;
        let prev = device.state;
        device.state = state;
        Some(prev)
    }

    /// Toggle a device's state between
    /// `Present` and `Disabled`. Returns the
    /// new state.
    #[must_use]
    pub fn toggle(&mut self, id: &str) -> Option<DeviceState> {
        // Read the current state first to avoid
        // holding a borrow across the set_state
        // call.
        let current = self.devices.iter().find(|d| d.id == id)?.state;
        let new_state = match current {
            DeviceState::Disabled => DeviceState::Present,
            // Toggle from any other state is
            // disabled; the user can re-enable.
            _ => DeviceState::Disabled,
        };
        self.set_state(id, new_state);
        Some(new_state)
    }

    /// The total number of devices.
    #[must_use]
    pub fn len(&self) -> usize {
        self.devices.len()
    }

    /// Whether the service has no devices.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }
}

/// The result of exercising a capability. The
/// hardware service daemon returns this from
/// its IO; the IPC layer marshals it across
/// the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CapabilityResult {
    /// The capability was exercised
    /// successfully. The optional `detail`
    /// carries human-readable context (e.g.
    /// "connected to MyWifi at 866 Mbps").
    Ok {
        /// Optional detail string.
        detail: String,
    },
    /// The device refused the capability
    /// (e.g. the battery is too low to
    /// `MountStorage`).
    Refused {
        /// Why the device refused.
        reason: String,
    },
    /// The device is in a state that doesn't
    /// allow the capability (e.g. trying to
    /// `ConnectWifi` while the adapter is
    /// `Disabled`).
    InvalidState {
        /// The device's current state.
        state: DeviceState,
    },
    /// The capability timed out.
    Timeout,
}

impl CapabilityResult {
    /// Whether the capability was exercised
    /// successfully.
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        matches!(self, Self::Ok { .. })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn audio_out(id: &str) -> Device {
        Device::new(
            id,
            DeviceKind::AudioOutput,
            "Built-in Audio",
            "ACME",
            "AC-100",
            DeviceState::Present,
            PowerState::SelfPowered,
        )
        .with_capability(Capability::RouteAudio)
    }

    fn mic(id: &str) -> Device {
        Device::new(
            id,
            DeviceKind::Microphone,
            "USB Mic",
            "Blue",
            "Yeti",
            DeviceState::Present,
            PowerState::SelfPowered,
        )
        .with_capability(Capability::CaptureAudio)
    }

    fn bt_mouse(id: &str, level: u16) -> Device {
        Device::new(
            id,
            DeviceKind::Mouse,
            "MX Master 3",
            "Logitech",
            "MX Master 3",
            DeviceState::Present,
            PowerState::Battery { level_per_mille: Some(level) },
        )
        .with_capability(Capability::Disable)
        .with_capability(Capability::Enable)
    }

    #[test]
    fn device_kind_as_str() {
        assert_eq!(DeviceKind::Cpu.as_str(), "cpu");
        assert_eq!(DeviceKind::AudioOutput.as_str(), "audio-output");
        assert_eq!(DeviceKind::FutureSensor.as_str(), "future-sensor");
    }

    #[test]
    fn device_kind_label() {
        assert_eq!(DeviceKind::Wifi.label(), "Wi-Fi");
        assert_eq!(DeviceKind::AudioOutput.label(), "Audio output");
    }

    #[test]
    fn device_state_as_str() {
        assert_eq!(DeviceState::Present.as_str(), "present");
        assert_eq!(DeviceState::Disabled.as_str(), "disabled");
        assert_eq!(DeviceState::Errored.as_str(), "errored");
        assert_eq!(DeviceState::Disconnected.as_str(), "disconnected");
    }

    #[test]
    fn device_new_starts_empty_capabilities() {
        let d = Device::new(
            "x",
            DeviceKind::Cpu,
            "CPU",
            "",
            "",
            DeviceState::Present,
            PowerState::SelfPowered,
        );
        assert!(d.capabilities.is_empty());
    }

    #[test]
    fn device_with_capability_appends() {
        let d = audio_out("a");
        assert_eq!(d.capabilities.len(), 1);
        assert!(d.has_capability(&Capability::RouteAudio));
    }

    #[test]
    fn device_is_usable_only_when_present() {
        let d = audio_out("a");
        assert!(d.is_usable());
        let d2 = Device::new(
            "b",
            DeviceKind::AudioOutput,
            "x",
            "",
            "",
            DeviceState::Disabled,
            PowerState::SelfPowered,
        );
        assert!(!d2.is_usable());
    }

    #[test]
    fn has_capability_finds_match() {
        let d = audio_out("a");
        assert!(d.has_capability(&Capability::RouteAudio));
        assert!(!d.has_capability(&Capability::CaptureAudio));
    }

    #[test]
    fn capability_verb() {
        assert_eq!(Capability::RouteAudio.verb(), "route-audio");
        assert_eq!(Capability::Enable.verb(), "enable");
        assert_eq!(
            Capability::ConnectWifi { ssid: "x".into() }.verb(),
            "connect-wifi"
        );
    }

    #[test]
    fn capability_requires_consent() {
        assert!(!Capability::RouteAudio.requires_consent());
        assert!(!Capability::Enable.requires_consent());
        assert!(Capability::CaptureAudio.requires_consent());
        assert!(Capability::CaptureVideo.requires_consent());
        assert!(Capability::ConnectWifi { ssid: "x".into() }.requires_consent());
        assert!(Capability::MountStorage.requires_consent());
        assert!(Capability::Print { path: "x".into() }.requires_consent());
    }

    #[test]
    fn service_starts_empty() {
        let s = HardwareService::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn service_upsert_adds_new() {
        let mut s = HardwareService::new();
        assert!(s.upsert(audio_out("a")));
        assert!(!s.is_empty());
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn service_upsert_replaces_existing() {
        let mut s = HardwareService::new();
        assert!(s.upsert(audio_out("a")));
        // Same id, different state -> not added.
        let mut updated = audio_out("a");
        updated.state = DeviceState::Disabled;
        assert!(!s.upsert(updated));
        assert_eq!(s.len(), 1);
        assert_eq!(s.get("a").unwrap().state, DeviceState::Disabled);
    }

    #[test]
    fn service_remove_returns_device() {
        let mut s = HardwareService::new();
        let _ = s.upsert(audio_out("a"));
        let removed = s.remove("a");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "a");
        assert!(s.is_empty());
    }

    #[test]
    fn service_remove_missing_returns_none() {
        let mut s = HardwareService::new();
        assert!(s.remove("nope").is_none());
    }

    #[test]
    fn service_get() {
        let mut s = HardwareService::new();
        let _ = s.upsert(audio_out("a"));
        assert!(s.get("a").is_some());
        assert!(s.get("nope").is_none());
    }

    #[test]
    fn service_with_kind() {
        let mut s = HardwareService::new();
        let _ = s.upsert(audio_out("a"));
        let _ = s.upsert(mic("b"));
        let audios = s.with_kind(DeviceKind::AudioOutput);
        assert_eq!(audios.len(), 1);
        let mics = s.with_kind(DeviceKind::Microphone);
        assert_eq!(mics.len(), 1);
    }

    #[test]
    fn service_find_capable_returns_first_match() {
        let mut s = HardwareService::new();
        let _ = s.upsert(audio_out("a"));
        let _ = s.upsert(mic("b"));
        let dev = s.find_capable(&Capability::RouteAudio).unwrap();
        assert_eq!(dev.id, "a");
    }

    #[test]
    fn service_find_capable_skips_unusable() {
        let mut s = HardwareService::new();
        let mut a = audio_out("a");
        a.state = DeviceState::Disabled;
        let _ = s.upsert(a);
        let _ = s.upsert(audio_out("b"));
        let dev = s.find_capable(&Capability::RouteAudio).unwrap();
        assert_eq!(dev.id, "b");
    }

    #[test]
    fn service_find_capable_returns_none_when_missing() {
        let s = HardwareService::new();
        assert!(s.find_capable(&Capability::RouteAudio).is_none());
    }

    #[test]
    fn service_all_capable() {
        let mut s = HardwareService::new();
        let _ = s.upsert(audio_out("a"));
        let _ = s.upsert(audio_out("b"));
        let _ = s.upsert(mic("c"));
        let audios = s.all_capable(&Capability::RouteAudio);
        assert_eq!(audios.len(), 2);
        let mics = s.all_capable(&Capability::CaptureAudio);
        assert_eq!(mics.len(), 1);
    }

    #[test]
    fn service_set_state_records_previous() {
        let mut s = HardwareService::new();
        let _ = s.upsert(audio_out("a"));
        let prev = s.set_state("a", DeviceState::Disabled);
        assert_eq!(prev, Some(DeviceState::Present));
        assert_eq!(s.get("a").unwrap().state, DeviceState::Disabled);
    }

    #[test]
    fn service_set_state_unknown_device() {
        let mut s = HardwareService::new();
        assert_eq!(s.set_state("nope", DeviceState::Disabled), None);
    }

    #[test]
    fn service_toggle_present_to_disabled() {
        let mut s = HardwareService::new();
        let _ = s.upsert(audio_out("a"));
        let new = s.toggle("a");
        assert_eq!(new, Some(DeviceState::Disabled));
    }

    #[test]
    fn service_toggle_disabled_to_present() {
        let mut s = HardwareService::new();
        let mut a = audio_out("a");
        a.state = DeviceState::Disabled;
        let _ = s.upsert(a);
        let new = s.toggle("a");
        assert_eq!(new, Some(DeviceState::Present));
    }

    #[test]
    fn service_toggle_error_to_disabled() {
        let mut s = HardwareService::new();
        let mut a = audio_out("a");
        a.state = DeviceState::Errored;
        let _ = s.upsert(a);
        let new = s.toggle("a");
        assert_eq!(new, Some(DeviceState::Disabled));
    }

    #[test]
    fn power_state_battery_carries_level() {
        let p = PowerState::Battery { level_per_mille: Some(750) };
        match p {
            PowerState::Battery { level_per_mille } => assert_eq!(level_per_mille, Some(750)),
            _ => panic!(),
        }
    }

    #[test]
    fn capability_result_is_ok() {
        assert!(CapabilityResult::Ok { detail: "".into() }.is_ok());
        assert!(!CapabilityResult::Timeout.is_ok());
        assert!(!CapabilityResult::Refused { reason: "x".into() }.is_ok());
    }

    #[test]
    fn bluetooth_mouse_has_battery() {
        let m = bt_mouse("a", 850);
        assert_eq!(
            m.power,
            PowerState::Battery { level_per_mille: Some(850) }
        );
        assert!(m.has_capability(&Capability::Enable));
    }

    #[test]
    fn device_serde_round_trip() {
        let d = audio_out("a");
        let s = serde_json::to_string(&d).unwrap();
        let back: Device = serde_json::from_str(&s).unwrap();
        assert_eq!(d, back);
    }

    #[test]
    fn service_serde_round_trip() {
        let mut svc = HardwareService::new();
        let _ = svc.upsert(audio_out("a"));
        let _ = svc.upsert(mic("b"));
        let s = serde_json::to_string(&svc).unwrap();
        let back: HardwareService = serde_json::from_str(&s).unwrap();
        assert_eq!(svc, back);
    }

    #[test]
    fn device_kind_serde_round_trip() {
        for k in [
            DeviceKind::Cpu,
            DeviceKind::Gpu,
            DeviceKind::Display,
            DeviceKind::ExternalDisplay,
            DeviceKind::Keyboard,
            DeviceKind::Touchpad,
            DeviceKind::Mouse,
            DeviceKind::AudioOutput,
            DeviceKind::Microphone,
            DeviceKind::Camera,
            DeviceKind::Wifi,
            DeviceKind::Bluetooth,
            DeviceKind::Ethernet,
            DeviceKind::Usb,
            DeviceKind::Storage,
            DeviceKind::Battery,
            DeviceKind::ThermalSensor,
            DeviceKind::Printer,
            DeviceKind::FutureSensor,
        ] {
            let s = serde_json::to_string(&k).unwrap();
            let back: DeviceKind = serde_json::from_str(&s).unwrap();
            assert_eq!(k, back);
        }
    }

    #[test]
    fn capability_serde_round_trip() {
        let c = Capability::ConnectWifi { ssid: "MyWifi".into() };
        let s = serde_json::to_string(&c).unwrap();
        let back: Capability = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
    }
}
