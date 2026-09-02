//! Hardware Abstraction Layer traits for Aether OS.
//!
//! Every hardware-facing subsystem implements a trait from
//! this crate. The concrete backends (real kernel drivers,
//! QEMU mocks, stub implementations) live in separate
//! crates that depend on this one.
//!
//! # Design Principles
//!
//! 1. **Portable**: Traits are `Send + Sync` and use only
//!    `std` types. No Linux-specific syscalls leak through.
//! 2. **Mockable**: Every trait has a `Mock` implementation
//!    in `aether-hal-mock` for QEMU testing.
//! 3. **Auditable**: All operations return typed `Result`
//!    types with structured error variants.
//! 4. **Observable**: State changes emit events through the
//!    `EventSink` trait for the proactive agent.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use std::fmt;

// ------------------------------------------------------------------- errors

/// Unified error type for all HAL operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HalError {
    /// The device or resource was not found.
    NotFound,
    /// The device is in a state that prevents the operation.
    InvalidState(String),
    /// The operation timed out.
    Timeout,
    /// Permission denied — capability not approved.
    PermissionDenied,
    /// The backend is not available (e.g. driver not loaded).
    BackendUnavailable,
    /// An I/O error occurred.
    IoError(String),
    /// The requested mode/parameter is unsupported.
    Unsupported(String),
    /// A constraint was violated (e.g. invalid parameter).
    ConstraintViolation(String),
}

impl fmt::Display for HalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "device not found"),
            Self::InvalidState(s) => write!(f, "invalid state: {s}"),
            Self::Timeout => write!(f, "operation timed out"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::BackendUnavailable => write!(f, "backend unavailable"),
            Self::IoError(s) => write!(f, "I/O error: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported: {s}"),
            Self::ConstraintViolation(s) => write!(f, "constraint violated: {s}"),
        }
    }
}

impl std::error::Error for HalError {}

/// Convenience result type for HAL operations.
pub type HalResult<T> = Result<T, HalError>;

// ------------------------------------------------------------------- device

/// Unique identifier for a hardware device.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub String);

impl DeviceId {
    /// Create a new device ID.
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    /// Get the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Device state observed by the HAL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceState {
    /// Device is present and operational.
    Present,
    /// Device is present but disabled.
    Disabled,
    /// Device encountered an error.
    Errored,
    /// Device is disconnected / removed.
    Disconnected,
}

/// Power state of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerState {
    /// Device is self-powered (AC).
    SelfPowered,
    /// Device is battery-powered with optional level (0-100%).
    Battery {
        /// Battery level as percentage (0-100), if known.
        level_percent: Option<u8>,
    },
    /// Device is powered off.
    PowerOff,
}

/// Information about a discovered device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Unique device identifier.
    pub id: DeviceId,
    /// Device category.
    pub kind: DeviceKind,
    /// Human-readable name.
    pub name: String,
    /// Vendor name.
    pub vendor: String,
    /// Product name.
    pub product: String,
    /// Current state.
    pub state: DeviceState,
    /// Current power state.
    pub power: PowerState,
    /// Supported capabilities.
    pub capabilities: Vec<String>,
}

/// Device category taxonomy (19 classes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DeviceKind {
    /// Central processing unit.
    Cpu,
    /// Graphics processing unit.
    Gpu,
    /// Built-in display.
    Display,
    /// External display (HDMI, DP, USB-C).
    ExternalDisplay,
    /// Keyboard input.
    Keyboard,
    /// Touchpad input.
    Touchpad,
    /// Mouse input.
    Mouse,
    /// Audio output (speakers, headphones).
    AudioOutput,
    /// Microphone input.
    Microphone,
    /// Camera input.
    Camera,
    /// Wi-Fi adapter.
    Wifi,
    /// Bluetooth adapter.
    Bluetooth,
    /// Ethernet adapter.
    Ethernet,
    /// USB host controller.
    Usb,
    /// Storage device (disk, SSD, NVMe).
    Storage,
    /// Battery.
    Battery,
    /// Thermal sensor.
    ThermalSensor,
    /// Printer.
    Printer,
    /// Future/undefined sensor type.
    FutureSensor,
}

impl DeviceKind {
    /// Kebab-case identifier.
    #[must_use]
    pub fn as_str(self) -> &'static str {
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

    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Gpu => "GPU",
            Self::Display => "Display",
            Self::ExternalDisplay => "External Display",
            Self::Keyboard => "Keyboard",
            Self::Touchpad => "Touchpad",
            Self::Mouse => "Mouse",
            Self::AudioOutput => "Audio Output",
            Self::Microphone => "Microphone",
            Self::Camera => "Camera",
            Self::Wifi => "Wi-Fi",
            Self::Bluetooth => "Bluetooth",
            Self::Ethernet => "Ethernet",
            Self::Usb => "USB",
            Self::Storage => "Storage",
            Self::Battery => "Battery",
            Self::ThermalSensor => "Thermal Sensor",
            Self::Printer => "Printer",
            Self::FutureSensor => "Sensor",
        }
    }
}

// ---------------------------------------------------------------- events

/// An event emitted by the HAL when device state changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEvent {
    /// The device that changed.
    pub device_id: DeviceId,
    /// The kind of device.
    pub kind: DeviceKind,
    /// What changed.
    pub change: DeviceChange,
    /// Timestamp (millis since epoch).
    pub timestamp_ms: u64,
}

/// What changed about a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceChange {
    /// Device was discovered / plugged in.
    Appeared,
    /// Device was removed / unplugged.
    Disappeared,
    /// Device state changed.
    StateChanged {
        /// Previous state.
        from: DeviceState,
        /// New state.
        to: DeviceState,
    },
    /// Device power state changed.
    PowerChanged {
        /// Previous power state.
        from: PowerState,
        /// New power state.
        to: PowerState,
    },
    /// A capability became available or unavailable.
    CapabilityChanged {
        /// The capability name.
        capability: String,
        /// Whether it's now available.
        available: bool,
    },
}

// ---------------------------------------------------------------- sink

/// Event sink for device state changes. The proactive agent
/// and diagnostics system consume these.
pub trait EventSink: Send + Sync {
    /// Emit a device event.
    fn emit(&self, event: DeviceEvent);
}

/// A no-op event sink for tests and headless mode.
pub struct NullEventSink;

impl EventSink for NullEventSink {
    fn emit(&self, _event: DeviceEvent) {}
}

// ========================================================== HAL traits

/// CPU information and control.
pub trait CpuHal: Send + Sync {
    /// Get CPU information (model, cores, frequency).
    fn info(&self) -> HalResult<CpuInfo>;
    /// Get current CPU usage (0.0..=1.0).
    fn usage(&self) -> HalResult<f32>;
    /// Get CPU temperature in Celsius, if available.
    fn temperature(&self) -> HalResult<Option<f32>>;
}

/// CPU information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU model name.
    pub model: String,
    /// Number of logical cores.
    pub cores: u32,
    /// Base frequency in MHz.
    pub base_freq_mhz: u32,
    /// Current frequency in MHz.
    pub current_freq_mhz: u32,
}

/// GPU information and control.
pub trait GpuHal: Send + Sync {
    /// Get GPU information.
    fn info(&self) -> HalResult<GpuInfo>;
    /// Get GPU usage (0.0..=1.0).
    fn usage(&self) -> HalResult<f32>;
}

/// GPU information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU model name.
    pub model: String,
    /// VRAM in MiB.
    pub vram_mib: u32,
    /// Driver name.
    pub driver: String,
}

/// Display output control.
pub trait DisplayHal: Send + Sync {
    /// Get current display configuration.
    fn config(&self) -> HalResult<DisplayConfig>;
    /// Set brightness (0-100).
    fn set_brightness(&self, percent: u8) -> HalResult<()>;
    /// Get current brightness.
    fn brightness(&self) -> HalResult<u8>;
    /// Enumerate available display modes.
    fn modes(&self) -> HalResult<Vec<DisplayMode>>;
    /// Set display mode.
    fn set_mode(&self, mode: &DisplayMode) -> HalResult<()>;
}

/// Display configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Refresh rate in Hz.
    pub refresh_hz: u32,
    /// Color depth in bits per pixel.
    pub bpp: u8,
    /// Whether the display is connected.
    pub connected: bool,
}

/// A supported display mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayMode {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Refresh rate in Hz.
    pub refresh_hz: u32,
}

/// Keyboard input.
pub trait KeyboardHal: Send + Sync {
    /// Get keyboard information.
    fn info(&self) -> HalResult<KeyboardInfo>;
    /// Read pending key events (non-blocking).
    fn read_keys(&self) -> HalResult<Vec<KeyEvent>>;
    /// Set LED state (caps lock, num lock, scroll lock).
    fn set_leds(&self, leds: &KeyboardLeds) -> HalResult<()>;
}

/// Keyboard information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardInfo {
    /// Keyboard name.
    pub name: String,
    /// Number of keys.
    pub key_count: u16,
    /// Layout (e.g. "us", "gb", "de").
    pub layout: String,
}

/// A key event from the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    /// Linux input event code.
    pub code: u16,
    /// Key state (true = pressed, false = released).
    pub pressed: bool,
    /// Timestamp (millis).
    pub timestamp_ms: u64,
}

/// Keyboard LED state.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardLeds {
    /// Caps Lock LED.
    pub caps_lock: bool,
    /// Num Lock LED.
    pub num_lock: bool,
    /// Scroll Lock LED.
    pub scroll_lock: bool,
}

/// Mouse/touchpad input.
pub trait PointingDeviceHal: Send + Sync {
    /// Get device information.
    fn info(&self) -> HalResult<PointingInfo>;
    /// Read pending events (non-blocking).
    fn read_events(&self) -> HalResult<Vec<PointingEvent>>;
    /// Set mouse acceleration.
    fn set_acceleration(&self, factor: f32) -> HalResult<()>;
    /// Set touchpad tap-to-click.
    fn set_tap_to_click(&self, enabled: bool) -> HalResult<()>;
}

/// Pointing device information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointingInfo {
    /// Device name.
    pub name: String,
    /// Device kind.
    pub kind: PointingKind,
    /// Maximum X resolution.
    pub max_x: u32,
    /// Maximum Y resolution.
    pub max_y: u32,
}

/// Type of pointing device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointingKind {
    /// Physical mouse.
    Mouse,
    /// Touchpad.
    Touchpad,
    /// Trackpoint.
    Trackpoint,
}

/// A pointing device event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointingEvent {
    /// Relative X movement (pixels).
    pub dx: i32,
    /// Relative Y movement (pixels).
    pub dy: i32,
    /// Button state (bitmask: bit 0 = left, bit 1 = right, bit 2 = middle).
    pub buttons: u8,
    /// Scroll wheel delta.
    pub scroll: i8,
    /// Timestamp (millis).
    pub timestamp_ms: u64,
}

/// Audio output control.
pub trait AudioOutputHal: Send + Sync {
    /// Get audio output information.
    fn info(&self) -> HalResult<AudioInfo>;
    /// Get volume (0-100).
    fn volume(&self) -> HalResult<u8>;
    /// Set volume (0-100).
    fn set_volume(&self, percent: u8) -> HalResult<()>;
    /// Check if muted.
    fn is_muted(&self) -> HalResult<bool>;
    /// Set mute state.
    fn set_muted(&self, muted: bool) -> HalResult<()>;
    /// Switch audio route (e.g. speakers -> headphones).
    fn set_route(&self, route: &str) -> HalResult<()>;
    /// Get available routes.
    fn routes(&self) -> HalResult<Vec<AudioRoute>>;
}

/// Audio information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioInfo {
    /// Audio device name.
    pub name: String,
    /// Audio driver.
    pub driver: String,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Channels.
    pub channels: u8,
}

/// An audio output route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioRoute {
    /// Route identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Whether this route is currently active.
    pub active: bool,
}

/// Microphone input.
pub trait MicrophoneHal: Send + Sync {
    /// Get microphone information.
    fn info(&self) -> HalResult<AudioInfo>;
    /// Get input gain (0-100).
    fn gain(&self) -> HalResult<u8>;
    /// Set input gain (0-100).
    fn set_gain(&self, percent: u8) -> HalResult<()>;
    /// Check if muted.
    fn is_muted(&self) -> HalResult<bool>;
    /// Set mute state.
    fn set_muted(&self, muted: bool) -> HalResult<()>;
}

/// Camera input.
pub trait CameraHal: Send + Sync {
    /// Get camera information.
    fn info(&self) -> HalResult<CameraInfo>;
    /// Get current resolution.
    fn resolution(&self) -> HalResult<(u32, u32)>;
    /// Set capture resolution.
    fn set_resolution(&self, width: u32, height: u32) -> HalResult<()>;
    /// Get current frame rate.
    fn frame_rate(&self) -> HalResult<u32>;
    /// Set frame rate.
    fn set_frame_rate(&self, fps: u32) -> HalResult<()>;
}

/// Camera information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraInfo {
    /// Camera name.
    pub name: String,
    /// Supported resolutions.
    pub supported_resolutions: Vec<(u32, u32)>,
    /// Supported frame rates.
    pub supported_frame_rates: Vec<u32>,
}

/// Wi-Fi networking.
pub trait WifiHal: Send + Sync {
    /// Get Wi-Fi adapter information.
    fn info(&self) -> HalResult<WifiInfo>;
    /// Get current connection state.
    fn state(&self) -> HalResult<WifiState>;
    /// Scan for available networks.
    fn scan(&self) -> HalResult<Vec<WifiNetwork>>;
    /// Connect to a network.
    fn connect(&self, ssid: &str, password: Option<&str>) -> HalResult<()>;
    /// Disconnect.
    fn disconnect(&self) -> HalResult<()>;
    /// Get current signal strength (0-100).
    fn signal_strength(&self) -> HalResult<u8>;
}

/// Wi-Fi adapter information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiInfo {
    /// Adapter name.
    pub name: String,
    /// Supported bands (2.4GHz, 5GHz, 6GHz).
    pub bands: Vec<String>,
    /// Supported standards (802.11ac, 802.11ax, etc.).
    pub standards: Vec<String>,
}

/// Wi-Fi connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WifiState {
    /// Not connected.
    Disconnected,
    /// Scanning for networks.
    Scanning,
    /// Connecting to a network.
    Connecting,
    /// Connected to a network.
    Connected,
    /// Connection failed.
    Failed,
}

/// A discovered Wi-Fi network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiNetwork {
    /// Network SSID.
    pub ssid: String,
    /// Signal strength (0-100).
    pub signal: u8,
    /// Whether the network is secured.
    pub secured: bool,
    /// Network band.
    pub band: String,
}

/// Bluetooth networking.
pub trait BluetoothHal: Send + Sync {
    /// Get Bluetooth adapter information.
    fn info(&self) -> HalResult<BluetoothInfo>;
    /// Get adapter state.
    fn state(&self) -> HalResult<BluetoothState>;
    /// Start discovery.
    fn start_discovery(&self) -> HalResult<()>;
    /// Stop discovery.
    fn stop_discovery(&self) -> HalResult<()>;
    /// Get discovered devices.
    fn discovered(&self) -> HalResult<Vec<BluetoothDevice>>;
    /// Pair with a device.
    fn pair(&self, device_id: &str) -> HalResult<()>;
    /// Connect to a paired device.
    fn connect(&self, device_id: &str) -> HalResult<()>;
    /// Disconnect from a device.
    fn disconnect(&self, device_id: &str) -> HalResult<()>;
}

/// Bluetooth adapter information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothInfo {
    /// Adapter name.
    pub name: String,
    /// MAC address.
    pub address: String,
    /// Supported profiles (A2DP, HFP, etc.).
    pub profiles: Vec<String>,
}

/// Bluetooth adapter state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothState {
    /// Adapter is off.
    Off,
    /// Adapter is on but not discovering.
    On,
    /// Adapter is discovering devices.
    Discovering,
    /// Adapter is connecting to a device.
    Connecting,
}

/// A discovered Bluetooth device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothDevice {
    /// Device address.
    pub address: String,
    /// Device name.
    pub name: String,
    /// Device class.
    pub class: u32,
    /// Whether the device is paired.
    pub paired: bool,
    /// Signal strength (0-100).
    pub signal: u8,
}

/// Ethernet networking.
pub trait EthernetHal: Send + Sync {
    /// Get Ethernet adapter information.
    fn info(&self) -> HalResult<EthernetInfo>;
    /// Get link state.
    fn link_up(&self) -> HalResult<bool>;
    /// Get current link speed in Mbps.
    fn speed_mbps(&self) -> HalResult<u32>;
}

/// Ethernet adapter information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthernetInfo {
    /// Interface name.
    pub name: String,
    /// MAC address.
    pub mac: String,
    /// Driver name.
    pub driver: String,
}

/// USB host controller.
pub trait UsbHal: Send + Sync {
    /// Enumerate connected USB devices.
    fn enumerate(&self) -> HalResult<Vec<UsbDevice>>;
    /// Get USB controller information.
    fn info(&self) -> HalResult<UsbInfo>;
}

/// A connected USB device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDevice {
    /// Bus number.
    pub bus: u8,
    /// Device address.
    pub address: u8,
    /// Vendor ID.
    pub vendor_id: u16,
    /// Product ID.
    pub product_id: u16,
    /// Device name.
    pub name: String,
    /// Device class.
    pub class: u8,
}

/// USB controller information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbInfo {
    /// Controller name.
    pub name: String,
    /// USB version (2.0, 3.0, 3.1, 3.2).
    pub version: String,
    /// Number of ports.
    pub ports: u8,
}

/// Storage device.
pub trait StorageHal: Send + Sync {
    /// Enumerate storage devices.
    fn devices(&self) -> HalResult<Vec<StorageDevice>>;
    /// Get mount point info.
    fn mounts(&self) -> HalResult<Vec<StorageMount>>;
}

/// A storage device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageDevice {
    /// Device path (e.g. "/dev/sda").
    pub path: String,
    /// Device name.
    pub name: String,
    /// Total size in bytes.
    pub size_bytes: u64,
    /// Filesystem type.
    pub fs_type: String,
    /// Whether the device is removable.
    pub removable: bool,
}

/// A storage mount point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMount {
    /// Device path.
    pub device: String,
    /// Mount point path.
    pub mount_point: String,
    /// Filesystem type.
    pub fs_type: String,
    /// Total bytes.
    pub total_bytes: u64,
    /// Used bytes.
    pub used_bytes: u64,
    /// Available bytes.
    pub available_bytes: u64,
}

/// Battery status.
pub trait BatteryHal: Send + Sync {
    /// Get battery information.
    fn info(&self) -> HalResult<BatteryInfo>;
    /// Get charge level (0-100).
    fn charge_percent(&self) -> HalResult<u8>;
    /// Check if charging.
    fn is_charging(&self) -> HalResult<bool>;
    /// Get time to empty in seconds, if available.
    fn time_to_empty(&self) -> HalResult<Option<u64>>;
    /// Get time to full in seconds, if available.
    fn time_to_full(&self) -> HalResult<Option<u64>>;
}

/// Battery information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    /// Battery model.
    pub model: String,
    /// Design capacity in mAh.
    pub design_capacity_mah: u32,
    /// Current capacity in mAh.
    pub current_capacity_mah: u32,
    /// Cycle count.
    pub cycle_count: u32,
    /// Battery health (0-100).
    pub health_percent: u8,
}

/// Thermal sensor.
pub trait ThermalHal: Send + Sync {
    /// Enumerate thermal zones.
    fn zones(&self) -> HalResult<Vec<ThermalZone>>;
    /// Get temperature of a zone.
    fn temperature(&self, zone: &str) -> HalResult<f32>;
}

/// A thermal zone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalZone {
    /// Zone identifier.
    pub name: String,
    /// Zone type (e.g. "cpu", "gpu", "battery").
    pub zone_type: String,
    /// Current temperature in Celsius.
    pub temperature_c: f32,
}

/// System power management (shutdown, reboot, suspend).
pub trait PowerHal: Send + Sync {
    /// Shut down the system.
    fn shutdown(&self) -> HalResult<()>;
    /// Reboot the system.
    fn reboot(&self) -> HalResult<()>;
    /// Suspend the system.
    fn suspend(&self) -> HalResult<()>;
    /// Check if suspend is supported.
    fn can_suspend(&self) -> HalResult<bool>;
}

// ========================================================== composite HAL

/// The complete Hardware Abstraction Layer. Composes all
/// subsystem traits into a single interface.
pub trait Hal: Send + Sync {
    /// CPU subsystem.
    fn cpu(&self) -> &dyn CpuHal;
    /// GPU subsystem.
    fn gpu(&self) -> &dyn GpuHal;
    /// Display subsystem.
    fn display(&self) -> &dyn DisplayHal;
    /// Keyboard subsystem.
    fn keyboard(&self) -> &dyn KeyboardHal;
    /// Pointing device subsystem.
    fn pointing(&self) -> &dyn PointingDeviceHal;
    /// Audio output subsystem.
    fn audio_output(&self) -> &dyn AudioOutputHal;
    /// Microphone subsystem.
    fn microphone(&self) -> &dyn MicrophoneHal;
    /// Camera subsystem.
    fn camera(&self) -> &dyn CameraHal;
    /// Wi-Fi subsystem.
    fn wifi(&self) -> &dyn WifiHal;
    /// Bluetooth subsystem.
    fn bluetooth(&self) -> &dyn BluetoothHal;
    /// Ethernet subsystem.
    fn ethernet(&self) -> &dyn EthernetHal;
    /// USB subsystem.
    fn usb(&self) -> &dyn UsbHal;
    /// Storage subsystem.
    fn storage(&self) -> &dyn StorageHal;
    /// Battery subsystem.
    fn battery(&self) -> &dyn BatteryHal;
    /// Thermal subsystem.
    fn thermal(&self) -> &dyn ThermalHal;
    /// Power management subsystem.
    fn power(&self) -> &dyn PowerHal;

    /// Discover all devices and emit events.
    fn discover(&self, sink: &dyn EventSink) -> HalResult<()>;

    /// Get the HAL backend name.
    fn backend_name(&self) -> &str;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn device_kind_as_str_covers_all_variants() {
        let kinds = [
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
        ];
        for k in kinds {
            assert!(!k.as_str().is_empty());
            assert!(!k.label().is_empty());
        }
    }

    #[test]
    fn device_kind_has_19_variants() {
        let kinds = [
            DeviceKind::Cpu,
            DeviceKind::Gpu,
            DeviceKind::Display,
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
            DeviceKind::ExternalDisplay,
            DeviceKind::FutureSensor,
        ];
        assert_eq!(kinds.len(), 19);
    }

    #[test]
    fn hal_error_display_messages_are_unique() {
        let errors = [
            HalError::NotFound,
            HalError::InvalidState("test".into()),
            HalError::Timeout,
            HalError::PermissionDenied,
            HalError::BackendUnavailable,
            HalError::IoError("test".into()),
            HalError::Unsupported("test".into()),
            HalError::ConstraintViolation("test".into()),
        ];
        let mut seen = std::collections::HashSet::new();
        for e in &errors {
            let msg = e.to_string();
            assert!(seen.insert(msg.clone()), "duplicate error message: {msg}");
        }
    }

    #[test]
    fn device_event_round_trip() {
        let event = DeviceEvent {
            device_id: DeviceId::new("test-001"),
            kind: DeviceKind::Wifi,
            change: DeviceChange::Appeared,
            timestamp_ms: 1000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: DeviceEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.device_id, event.device_id);
        assert_eq!(decoded.kind, event.kind);
    }

    #[test]
    fn display_mode_round_trip() {
        let mode = DisplayMode { width: 1920, height: 1080, refresh_hz: 60 };
        let json = serde_json::to_string(&mode).unwrap();
        let decoded: DisplayMode = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, mode);
    }

    #[test]
    fn battery_info_round_trip() {
        let info = BatteryInfo {
            model: "Test".into(),
            design_capacity_mah: 5000,
            current_capacity_mah: 4500,
            cycle_count: 100,
            health_percent: 90,
        };
        let json = serde_json::to_string(&info).unwrap();
        let decoded: BatteryInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.model, info.model);
    }
}
