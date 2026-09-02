//! Hardware compatibility profiles for Aether OS.
//!
//! A profile describes a specific hardware configuration:
//! which devices are present, which drivers are needed, and
//! what capabilities are available. Profiles let Aether adapt
//! to different hardware without hard-coding.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use aether_hal::DeviceKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A hardware compatibility profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    /// Profile identifier (e.g. "qemu-standard", "framework-13").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Profile version.
    pub version: String,
    /// Platform class.
    pub platform: Platform,
    /// CPU configuration.
    pub cpu: CpuProfile,
    /// GPU configuration.
    pub gpu: GpuProfile,
    /// Display configuration.
    pub display: DisplayProfile,
    /// Input devices.
    pub input: InputProfile,
    /// Audio configuration.
    pub audio: AudioProfile,
    /// Network interfaces.
    pub network: NetworkProfile,
    /// Storage configuration.
    pub storage: StorageProfile,
    /// Power management.
    pub power: PowerProfile,
    /// Device-specific quirks.
    pub quirks: Vec<String>,
    /// Driver requirements.
    pub drivers: Vec<DriverRequirement>,
}

/// Platform class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    /// QEMU virtual machine.
    Qemu,
    /// Physical desktop.
    Desktop,
    /// Physical laptop.
    Laptop,
    /// IoT / embedded device.
    Iot,
    /// Server.
    Server,
    /// ARM single-board computer.
    ArmSbc,
}

/// CPU profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuProfile {
    /// Expected model substring.
    pub model_contains: String,
    /// Minimum cores.
    pub min_cores: u32,
    /// Minimum frequency in MHz.
    pub min_freq_mhz: u32,
    /// Whether thermal monitoring is available.
    pub has_thermal: bool,
}

/// GPU profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuProfile {
    /// GPU kind.
    pub kind: GpuKind,
    /// Driver name.
    pub driver: String,
    /// Minimum VRAM in MiB.
    pub min_vram_mib: u32,
    /// Whether hardware acceleration is expected.
    pub hardware_accel: bool,
}

/// GPU kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuKind {
    /// Virtio GPU (QEMU).
    Virtio,
    /// Intel integrated GPU.
    IntelIntegrated,
    /// AMD discrete GPU.
    AmdDiscrete,
    /// NVIDIA discrete GPU.
    NvidiaDiscrete,
    /// Software rendering (no GPU).
    Software,
}

/// Display profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayProfile {
    /// Minimum resolution (width, height).
    pub min_resolution: (u32, u32),
    /// Preferred refresh rate.
    pub preferred_refresh_hz: u32,
    /// Whether brightness control is available.
    pub has_brightness: bool,
    /// Whether multiple displays are expected.
    pub multi_monitor: bool,
}

/// Input profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputProfile {
    /// Whether a keyboard is expected.
    pub has_keyboard: bool,
    /// Whether a touchpad is expected.
    pub has_touchpad: bool,
    /// Whether a mouse is expected.
    pub has_mouse: bool,
    /// Whether a touchscreen is expected.
    pub has_touchscreen: bool,
    /// Whether trackpoint is expected.
    pub has_trackpoint: bool,
}

/// Audio profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioProfile {
    /// Audio driver.
    pub driver: String,
    /// Whether speakers are built-in.
    pub has_speakers: bool,
    /// Whether a microphone is built-in.
    pub has_microphone: bool,
    /// Whether headphone jack exists.
    pub has_headphone_jack: bool,
    /// Whether HDMI audio is available.
    pub has_hdmi_audio: bool,
}

/// Network profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProfile {
    /// Whether Wi-Fi is available.
    pub has_wifi: bool,
    /// Wi-Fi driver.
    pub wifi_driver: Option<String>,
    /// Whether Bluetooth is available.
    pub has_bluetooth: bool,
    /// Bluetooth driver.
    pub bluetooth_driver: Option<String>,
    /// Whether Ethernet is available.
    pub has_ethernet: bool,
    /// Ethernet driver.
    pub ethernet_driver: Option<String>,
}

/// Storage profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageProfile {
    /// Storage type.
    pub kind: StorageKind,
    /// Minimum size in GiB.
    pub min_size_gib: u32,
    /// Whether NVMe is expected.
    pub nvme: bool,
}

/// Storage kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageKind {
    /// Virtio block device (QEMU).
    VirtioBlock,
    /// SATA.
    Sata,
    /// NVMe.
    Nvme,
    /// eMMC.
    Emmc,
}

/// Power profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerProfile {
    /// Whether battery is present.
    pub has_battery: bool,
    /// Whether suspend/resume is supported.
    pub supports_suspend: bool,
    /// Whether Wake-on-LAN is supported.
    pub has_wol: bool,
}

/// A driver requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverRequirement {
    /// Driver module name.
    pub module: String,
    /// Device kind this driver serves.
    pub kind: DeviceKind,
    /// Whether the driver is required (vs optional).
    pub required: bool,
    /// Fallback driver if primary is unavailable.
    pub fallback: Option<String>,
}

/// A collection of hardware profiles.
pub struct ProfileRegistry {
    profiles: HashMap<String, HardwareProfile>,
}

impl Default for ProfileRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self { profiles: HashMap::new() }
    }

    /// Register a profile.
    pub fn register(&mut self, profile: HardwareProfile) {
        self.profiles.insert(profile.id.clone(), profile);
    }

    /// Look up a profile by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&HardwareProfile> {
        self.profiles.get(id)
    }

    /// List all registered profile IDs.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.profiles.keys().map(String::as_str).collect()
    }

    /// Find the best profile for a detected device set.
    #[must_use]
    pub fn best_match(&self, detected: &[DeviceKind]) -> Option<&HardwareProfile> {
        self.profiles.values().max_by_key(|p| {
            let matching = detected.iter().filter(|k| p.has_device_kind(k)).count();
            matching
        })
    }
}

impl HardwareProfile {
    /// Check if this profile expects a given device kind.
    #[must_use]
    pub fn has_device_kind(&self, kind: &DeviceKind) -> bool {
        matches!(
            (kind, &self.platform),
            (DeviceKind::Cpu, _)
                | (DeviceKind::Gpu, _)
                | (DeviceKind::Display, _)
                | (DeviceKind::Keyboard, Platform::Laptop | Platform::Desktop)
                | (DeviceKind::Touchpad, Platform::Laptop)
                | (DeviceKind::Mouse, Platform::Desktop | Platform::Qemu)
                | (DeviceKind::AudioOutput, _)
                | (DeviceKind::Microphone, Platform::Laptop | Platform::Desktop)
                | (DeviceKind::Wifi, Platform::Laptop)
                | (DeviceKind::Bluetooth, Platform::Laptop)
                | (DeviceKind::Ethernet, _)
                | (DeviceKind::Storage, _)
                | (DeviceKind::Battery, Platform::Laptop)
        )
    }

    /// Get all driver requirements.
    #[must_use]
    pub fn required_drivers(&self) -> Vec<&DriverRequirement> {
        self.drivers.iter().filter(|d| d.required).collect()
    }
}

/// Create the QEMU standard profile.
#[must_use]
pub fn qemu_standard_profile() -> HardwareProfile {
    HardwareProfile {
        id: "qemu-standard".into(),
        name: "QEMU Standard Virtual Machine".into(),
        version: "1.0".into(),
        platform: Platform::Qemu,
        cpu: CpuProfile {
            model_contains: "QEMU".into(),
            min_cores: 2,
            min_freq_mhz: 2000,
            has_thermal: false,
        },
        gpu: GpuProfile {
            kind: GpuKind::Virtio,
            driver: "virtio-gpu".into(),
            min_vram_mib: 128,
            hardware_accel: false,
        },
        display: DisplayProfile {
            min_resolution: (1024, 768),
            preferred_refresh_hz: 60,
            has_brightness: false,
            multi_monitor: false,
        },
        input: InputProfile {
            has_keyboard: true,
            has_touchpad: false,
            has_mouse: true,
            has_touchscreen: false,
            has_trackpoint: false,
        },
        audio: AudioProfile {
            driver: "hda".into(),
            has_speakers: true,
            has_microphone: true,
            has_headphone_jack: false,
            has_hdmi_audio: false,
        },
        network: NetworkProfile {
            has_wifi: false,
            wifi_driver: None,
            has_bluetooth: false,
            bluetooth_driver: None,
            has_ethernet: true,
            ethernet_driver: Some("e1000".into()),
        },
        storage: StorageProfile { kind: StorageKind::VirtioBlock, min_size_gib: 20, nvme: false },
        power: PowerProfile { has_battery: false, supports_suspend: false, has_wol: false },
        quirks: vec!["no-acpi-battery".into(), "virtio-input".into()],
        drivers: vec![
            DriverRequirement {
                module: "virtio-pci".into(),
                kind: DeviceKind::Storage,
                required: true,
                fallback: None,
            },
            DriverRequirement {
                module: "virtio-gpu".into(),
                kind: DeviceKind::Gpu,
                required: true,
                fallback: Some("bochs".into()),
            },
            DriverRequirement {
                module: "e1000".into(),
                kind: DeviceKind::Ethernet,
                required: true,
                fallback: Some("rtl8139".into()),
            },
            DriverRequirement {
                module: "hda-intel".into(),
                kind: DeviceKind::AudioOutput,
                required: false,
                fallback: None,
            },
        ],
    }
}

/// Create a generic laptop profile.
#[must_use]
pub fn laptop_profile() -> HardwareProfile {
    HardwareProfile {
        id: "generic-laptop".into(),
        name: "Generic Laptop".into(),
        version: "1.0".into(),
        platform: Platform::Laptop,
        cpu: CpuProfile {
            model_contains: "".into(),
            min_cores: 2,
            min_freq_mhz: 1500,
            has_thermal: true,
        },
        gpu: GpuProfile {
            kind: GpuKind::IntelIntegrated,
            driver: "i915".into(),
            min_vram_mib: 256,
            hardware_accel: true,
        },
        display: DisplayProfile {
            min_resolution: (1366, 768),
            preferred_refresh_hz: 60,
            has_brightness: true,
            multi_monitor: false,
        },
        input: InputProfile {
            has_keyboard: true,
            has_touchpad: true,
            has_mouse: false,
            has_touchscreen: false,
            has_trackpoint: false,
        },
        audio: AudioProfile {
            driver: "hda-intel".into(),
            has_speakers: true,
            has_microphone: true,
            has_headphone_jack: true,
            has_hdmi_audio: true,
        },
        network: NetworkProfile {
            has_wifi: true,
            wifi_driver: Some("iwlwifi".into()),
            has_bluetooth: true,
            bluetooth_driver: Some("btintel".into()),
            has_ethernet: true,
            ethernet_driver: Some("e1000e".into()),
        },
        storage: StorageProfile { kind: StorageKind::Nvme, min_size_gib: 128, nvme: true },
        power: PowerProfile { has_battery: true, supports_suspend: true, has_wol: true },
        quirks: Vec::new(),
        drivers: vec![
            DriverRequirement {
                module: "i915".into(),
                kind: DeviceKind::Gpu,
                required: true,
                fallback: Some("modesetting".into()),
            },
            DriverRequirement {
                module: "iwlwifi".into(),
                kind: DeviceKind::Wifi,
                required: true,
                fallback: None,
            },
            DriverRequirement {
                module: "btintel".into(),
                kind: DeviceKind::Bluetooth,
                required: false,
                fallback: None,
            },
            DriverRequirement {
                module: "snd-hda-intel".into(),
                kind: DeviceKind::AudioOutput,
                required: true,
                fallback: None,
            },
        ],
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn qemu_profile_has_required_fields() {
        let p = qemu_standard_profile();
        assert!(!p.id.is_empty());
        assert!(!p.name.is_empty());
        assert_eq!(p.platform, Platform::Qemu);
    }

    #[test]
    fn laptop_profile_has_battery() {
        let p = laptop_profile();
        assert!(p.power.has_battery);
        assert!(p.input.has_touchpad);
    }

    #[test]
    fn profile_registry_lookup() {
        let mut reg = ProfileRegistry::new();
        reg.register(qemu_standard_profile());
        reg.register(laptop_profile());
        assert!(reg.get("qemu-standard").is_some());
        assert!(reg.get("generic-laptop").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn profile_registry_best_match() {
        let mut reg = ProfileRegistry::new();
        reg.register(qemu_standard_profile());
        reg.register(laptop_profile());
        let detected =
            vec![DeviceKind::Cpu, DeviceKind::Gpu, DeviceKind::Keyboard, DeviceKind::Touchpad];
        let best = reg.best_match(&detected);
        assert!(best.is_some());
        assert_eq!(best.unwrap().platform, Platform::Laptop);
    }

    #[test]
    fn required_drivers_filters_correctly() {
        let p = qemu_standard_profile();
        let required = p.required_drivers();
        assert!(required.iter().all(|d| d.required));
        assert!(required.len() >= 2);
    }

    #[test]
    fn has_device_kind_matches_platform() {
        let qemu = qemu_standard_profile();
        assert!(qemu.has_device_kind(&DeviceKind::Cpu));
        assert!(qemu.has_device_kind(&DeviceKind::Mouse));
        assert!(!qemu.has_device_kind(&DeviceKind::Touchpad));

        let laptop = laptop_profile();
        assert!(laptop.has_device_kind(&DeviceKind::Touchpad));
        assert!(laptop.has_device_kind(&DeviceKind::Battery));
    }

    #[test]
    fn profile_serialization_round_trip() {
        let p = qemu_standard_profile();
        let json = serde_json::to_string(&p).unwrap();
        let decoded: HardwareProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, p.id);
        assert_eq!(decoded.platform, p.platform);
    }
}
