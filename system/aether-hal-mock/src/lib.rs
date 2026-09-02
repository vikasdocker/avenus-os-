//! Mock HAL backends for QEMU testing and validation.
//!
//! Every trait in `aether-hal` has a `Mock*` implementation
//! here that returns deterministic, configurable responses.
//! This lets the entire Aether stack be tested without
//! real hardware.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use aether_hal::*;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------- helpers

/// Build a default `DeviceInfo` for a given kind.
#[must_use]
pub fn default_device_info(kind: DeviceKind) -> DeviceInfo {
    let name = match kind {
        DeviceKind::Cpu => "Mock CPU",
        DeviceKind::Gpu => "Mock GPU",
        DeviceKind::Display => "Mock Display",
        DeviceKind::Keyboard => "Mock Keyboard",
        DeviceKind::Touchpad => "Mock Touchpad",
        DeviceKind::Mouse => "Mock Mouse",
        DeviceKind::AudioOutput => "Mock Speakers",
        DeviceKind::Microphone => "Mock Mic",
        DeviceKind::Camera => "Mock Camera",
        DeviceKind::Wifi => "Mock Wi-Fi",
        DeviceKind::Bluetooth => "Mock Bluetooth",
        DeviceKind::Ethernet => "Mock Ethernet",
        DeviceKind::Usb => "Mock USB",
        DeviceKind::Storage => "Mock Storage",
        DeviceKind::Battery => "Mock Battery",
        DeviceKind::ThermalSensor => "Mock Thermal",
        DeviceKind::Printer => "Mock Printer",
        DeviceKind::ExternalDisplay => "Mock External",
        DeviceKind::FutureSensor => "Mock Sensor",
        _ => "Mock Device",
    };
    DeviceInfo {
        id: DeviceId::new(&format!("mock-{}", kind.as_str())),
        kind,
        name: name.to_string(),
        vendor: "Aether Mock".to_string(),
        product: name.to_string(),
        state: DeviceState::Present,
        power: PowerState::SelfPowered,
        capabilities: Vec::new(),
    }
}

// ========================================================== mock state

/// Shared state across all mock HAL implementations.
#[derive(Debug, Clone)]
pub struct MockHalState {
    /// CPU usage (0.0..=1.0).
    pub cpu_usage: f32,
    /// CPU temperature.
    pub cpu_temp: Option<f32>,
    /// Display brightness.
    pub brightness: u8,
    /// Display mode.
    pub display_mode: DisplayConfig,
    /// Audio volume.
    pub volume: u8,
    /// Audio muted.
    pub audio_muted: bool,
    /// Audio routes.
    pub audio_routes: Vec<AudioRoute>,
    /// Microphone gain.
    pub mic_gain: u8,
    /// Microphone muted.
    pub mic_muted: bool,
    /// Wi-Fi state.
    pub wifi_state: WifiState,
    /// Wi-Fi networks.
    pub wifi_networks: Vec<WifiNetwork>,
    /// Wi-Fi signal strength.
    pub wifi_signal: u8,
    /// Bluetooth state.
    pub bt_state: BluetoothState,
    /// Bluetooth devices.
    pub bt_devices: Vec<BluetoothDevice>,
    /// Ethernet link up.
    pub eth_link_up: bool,
    /// Ethernet speed.
    pub eth_speed: u32,
    /// USB devices.
    pub usb_devices: Vec<UsbDevice>,
    /// Storage devices.
    pub storage_devices: Vec<StorageDevice>,
    /// Storage mounts.
    pub storage_mounts: Vec<StorageMount>,
    /// Battery charge percent.
    pub battery_charge: u8,
    /// Battery charging.
    pub battery_charging: bool,
    /// Thermal zones.
    pub thermal_zones: Vec<ThermalZone>,
    /// Keyboard LEDs.
    pub keyboard_leds: KeyboardLeds,
    /// Keyboard events queue.
    pub key_events: Vec<KeyEvent>,
    /// Pointing events queue.
    pub pointing_events: Vec<PointingEvent>,
    /// Can suspend.
    pub can_suspend: bool,
    /// Pending device events.
    pub events: Vec<DeviceEvent>,
}

impl Default for MockHalState {
    fn default() -> Self {
        Self {
            cpu_usage: 0.15,
            cpu_temp: Some(45.0),
            brightness: 80,
            display_mode: DisplayConfig {
                width: 1920,
                height: 1080,
                refresh_hz: 60,
                bpp: 32,
                connected: true,
            },
            volume: 75,
            audio_muted: false,
            audio_routes: vec![
                AudioRoute { id: "speakers".into(), name: "Speakers".into(), active: true },
                AudioRoute { id: "headphones".into(), name: "Headphones".into(), active: false },
            ],
            mic_gain: 50,
            mic_muted: false,
            wifi_state: WifiState::Connected,
            wifi_networks: vec![WifiNetwork {
                ssid: "Aether-Test".into(),
                signal: 85,
                secured: true,
                band: "5GHz".into(),
            }],
            wifi_signal: 85,
            bt_state: BluetoothState::On,
            bt_devices: Vec::new(),
            eth_link_up: true,
            eth_speed: 1000,
            usb_devices: Vec::new(),
            storage_devices: vec![StorageDevice {
                path: "/dev/sda".into(),
                name: "Mock SSD".into(),
                size_bytes: 256_000_000_000,
                fs_type: "ext4".into(),
                removable: false,
            }],
            storage_mounts: vec![StorageMount {
                device: "/dev/sda".into(),
                mount_point: "/".into(),
                fs_type: "ext4".into(),
                total_bytes: 256_000_000_000,
                used_bytes: 128_000_000_000,
                available_bytes: 128_000_000_000,
            }],
            battery_charge: 85,
            battery_charging: false,
            thermal_zones: vec![
                ThermalZone { name: "cpu".into(), zone_type: "cpu".into(), temperature_c: 45.0 },
                ThermalZone { name: "gpu".into(), zone_type: "gpu".into(), temperature_c: 40.0 },
            ],
            keyboard_leds: KeyboardLeds::default(),
            key_events: Vec::new(),
            pointing_events: Vec::new(),
            can_suspend: true,
            events: Vec::new(),
        }
    }
}

/// Shared reference to mock state.
pub type SharedMockState = Arc<Mutex<MockHalState>>;

/// Create a new shared mock state with defaults.
#[must_use]
pub fn new_mock_state() -> SharedMockState {
    Arc::new(Mutex::new(MockHalState::default()))
}

// ========================================================== mock HALs

/// Mock CPU HAL.
pub struct MockCpuHal {
    state: SharedMockState,
}

impl MockCpuHal {
    /// Create a new mock CPU HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl CpuHal for MockCpuHal {
    fn info(&self) -> HalResult<CpuInfo> {
        Ok(CpuInfo {
            model: "Mock CPU @ 3.0 GHz".into(),
            cores: 4,
            base_freq_mhz: 3000,
            current_freq_mhz: 3000,
        })
    }

    fn usage(&self) -> HalResult<f32> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.cpu_usage)
    }

    fn temperature(&self) -> HalResult<Option<f32>> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.cpu_temp)
    }
}

/// Mock GPU HAL.
pub struct MockGpuHal;

impl GpuHal for MockGpuHal {
    fn info(&self) -> HalResult<GpuInfo> {
        Ok(GpuInfo { model: "Mock GPU".into(), vram_mib: 256, driver: "mock-drm".into() })
    }

    fn usage(&self) -> HalResult<f32> {
        Ok(0.1)
    }
}

/// Mock Display HAL.
pub struct MockDisplayHal {
    state: SharedMockState,
}

impl MockDisplayHal {
    /// Create a new mock display HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl DisplayHal for MockDisplayHal {
    fn config(&self) -> HalResult<DisplayConfig> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.display_mode.clone())
    }

    fn set_brightness(&self, percent: u8) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.brightness = percent;
        Ok(())
    }

    fn brightness(&self) -> HalResult<u8> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.brightness)
    }

    fn modes(&self) -> HalResult<Vec<DisplayMode>> {
        Ok(vec![
            DisplayMode { width: 1920, height: 1080, refresh_hz: 60 },
            DisplayMode { width: 1920, height: 1080, refresh_hz: 144 },
            DisplayMode { width: 2560, height: 1440, refresh_hz: 60 },
        ])
    }

    fn set_mode(&self, mode: &DisplayMode) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.display_mode =
            DisplayConfig {
                width: mode.width,
                height: mode.height,
                refresh_hz: mode.refresh_hz,
                bpp: 32,
                connected: true,
            };
        Ok(())
    }
}

/// Mock Keyboard HAL.
pub struct MockKeyboardHal {
    state: SharedMockState,
}

impl MockKeyboardHal {
    /// Create a new mock keyboard HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl KeyboardHal for MockKeyboardHal {
    fn info(&self) -> HalResult<KeyboardInfo> {
        Ok(KeyboardInfo { name: "Mock Keyboard".into(), key_count: 104, layout: "us".into() })
    }

    fn read_keys(&self) -> HalResult<Vec<KeyEvent>> {
        let mut s = self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?;
        Ok(std::mem::take(&mut s.key_events))
    }

    fn set_leds(&self, leds: &KeyboardLeds) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.keyboard_leds = *leds;
        Ok(())
    }
}

/// Mock Pointing Device HAL.
pub struct MockPointingHal {
    state: SharedMockState,
}

impl MockPointingHal {
    /// Create a new mock pointing device HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl PointingDeviceHal for MockPointingHal {
    fn info(&self) -> HalResult<PointingInfo> {
        Ok(PointingInfo {
            name: "Mock Mouse".into(),
            kind: PointingKind::Mouse,
            max_x: 1920,
            max_y: 1080,
        })
    }

    fn read_events(&self) -> HalResult<Vec<PointingEvent>> {
        let mut s = self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?;
        Ok(std::mem::take(&mut s.pointing_events))
    }

    fn set_acceleration(&self, _factor: f32) -> HalResult<()> {
        Ok(())
    }

    fn set_tap_to_click(&self, _enabled: bool) -> HalResult<()> {
        Ok(())
    }
}

/// Mock Audio Output HAL.
pub struct MockAudioOutputHal {
    state: SharedMockState,
}

impl MockAudioOutputHal {
    /// Create a new mock audio output HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl AudioOutputHal for MockAudioOutputHal {
    fn info(&self) -> HalResult<AudioInfo> {
        Ok(AudioInfo {
            name: "Mock Audio".into(),
            driver: "mock-pulse".into(),
            sample_rate: 48000,
            channels: 2,
        })
    }

    fn volume(&self) -> HalResult<u8> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.volume)
    }

    fn set_volume(&self, percent: u8) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.volume = percent;
        Ok(())
    }

    fn is_muted(&self) -> HalResult<bool> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.audio_muted)
    }

    fn set_muted(&self, muted: bool) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.audio_muted = muted;
        Ok(())
    }

    fn set_route(&self, route: &str) -> HalResult<()> {
        let mut s = self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?;
        for r in &mut s.audio_routes {
            r.active = r.id == route;
        }
        Ok(())
    }

    fn routes(&self) -> HalResult<Vec<AudioRoute>> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.audio_routes.clone())
    }
}

/// Mock Microphone HAL.
pub struct MockMicrophoneHal {
    state: SharedMockState,
}

impl MockMicrophoneHal {
    /// Create a new mock microphone HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl MicrophoneHal for MockMicrophoneHal {
    fn info(&self) -> HalResult<AudioInfo> {
        Ok(AudioInfo {
            name: "Mock Mic".into(),
            driver: "mock-pulse".into(),
            sample_rate: 48000,
            channels: 1,
        })
    }

    fn gain(&self) -> HalResult<u8> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.mic_gain)
    }

    fn set_gain(&self, percent: u8) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.mic_gain = percent;
        Ok(())
    }

    fn is_muted(&self) -> HalResult<bool> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.mic_muted)
    }

    fn set_muted(&self, muted: bool) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.mic_muted = muted;
        Ok(())
    }
}

/// Mock Camera HAL.
pub struct MockCameraHal;

impl CameraHal for MockCameraHal {
    fn info(&self) -> HalResult<CameraInfo> {
        Ok(CameraInfo {
            name: "Mock Camera".into(),
            supported_resolutions: vec![(640, 480), (1280, 720), (1920, 1080)],
            supported_frame_rates: vec![15, 30, 60],
        })
    }

    fn resolution(&self) -> HalResult<(u32, u32)> {
        Ok((1280, 720))
    }

    fn set_resolution(&self, _w: u32, _h: u32) -> HalResult<()> {
        Ok(())
    }

    fn frame_rate(&self) -> HalResult<u32> {
        Ok(30)
    }

    fn set_frame_rate(&self, _fps: u32) -> HalResult<()> {
        Ok(())
    }
}

/// Mock Wi-Fi HAL.
pub struct MockWifiHal {
    state: SharedMockState,
}

impl MockWifiHal {
    /// Create a new mock Wi-Fi HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl WifiHal for MockWifiHal {
    fn info(&self) -> HalResult<WifiInfo> {
        Ok(WifiInfo {
            name: "Mock Wi-Fi".into(),
            bands: vec!["2.4GHz".into(), "5GHz".into()],
            standards: vec!["802.11ac".into(), "802.11ax".into()],
        })
    }

    fn state(&self) -> HalResult<WifiState> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.wifi_state)
    }

    fn scan(&self) -> HalResult<Vec<WifiNetwork>> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.wifi_networks.clone())
    }

    fn connect(&self, _ssid: &str, _password: Option<&str>) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.wifi_state =
            WifiState::Connected;
        Ok(())
    }

    fn disconnect(&self) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.wifi_state =
            WifiState::Disconnected;
        Ok(())
    }

    fn signal_strength(&self) -> HalResult<u8> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.wifi_signal)
    }
}

/// Mock Bluetooth HAL.
pub struct MockBluetoothHal {
    state: SharedMockState,
}

impl MockBluetoothHal {
    /// Create a new mock Bluetooth HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl BluetoothHal for MockBluetoothHal {
    fn info(&self) -> HalResult<BluetoothInfo> {
        Ok(BluetoothInfo {
            name: "Mock BT".into(),
            address: "AA:BB:CC:DD:EE:FF".into(),
            profiles: vec!["A2DP".into(), "HFP".into()],
        })
    }

    fn state(&self) -> HalResult<BluetoothState> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.bt_state)
    }

    fn start_discovery(&self) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.bt_state =
            BluetoothState::Discovering;
        Ok(())
    }

    fn stop_discovery(&self) -> HalResult<()> {
        self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.bt_state =
            BluetoothState::On;
        Ok(())
    }

    fn discovered(&self) -> HalResult<Vec<BluetoothDevice>> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.bt_devices.clone())
    }

    fn pair(&self, _device_id: &str) -> HalResult<()> {
        Ok(())
    }

    fn connect(&self, _device_id: &str) -> HalResult<()> {
        Ok(())
    }

    fn disconnect(&self, _device_id: &str) -> HalResult<()> {
        Ok(())
    }
}

/// Mock Ethernet HAL.
pub struct MockEthernetHal {
    state: SharedMockState,
}

impl MockEthernetHal {
    /// Create a new mock Ethernet HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl EthernetHal for MockEthernetHal {
    fn info(&self) -> HalResult<EthernetInfo> {
        Ok(EthernetInfo {
            name: "eth0".into(),
            mac: "00:11:22:33:44:55".into(),
            driver: "mock-e1000".into(),
        })
    }

    fn link_up(&self) -> HalResult<bool> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.eth_link_up)
    }

    fn speed_mbps(&self) -> HalResult<u32> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.eth_speed)
    }
}

/// Mock USB HAL.
pub struct MockUsbHal {
    state: SharedMockState,
}

impl MockUsbHal {
    /// Create a new mock USB HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl UsbHal for MockUsbHal {
    fn enumerate(&self) -> HalResult<Vec<UsbDevice>> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.usb_devices.clone())
    }

    fn info(&self) -> HalResult<UsbInfo> {
        Ok(UsbInfo { name: "Mock USB Controller".into(), version: "3.0".into(), ports: 4 })
    }
}

/// Mock Storage HAL.
pub struct MockStorageHal {
    state: SharedMockState,
}

impl MockStorageHal {
    /// Create a new mock storage HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl StorageHal for MockStorageHal {
    fn devices(&self) -> HalResult<Vec<StorageDevice>> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.storage_devices.clone())
    }

    fn mounts(&self) -> HalResult<Vec<StorageMount>> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.storage_mounts.clone())
    }
}

/// Mock Battery HAL.
pub struct MockBatteryHal {
    state: SharedMockState,
}

impl MockBatteryHal {
    /// Create a new mock battery HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl BatteryHal for MockBatteryHal {
    fn info(&self) -> HalResult<BatteryInfo> {
        Ok(BatteryInfo {
            model: "Mock Battery".into(),
            design_capacity_mah: 5000,
            current_capacity_mah: 4250,
            cycle_count: 50,
            health_percent: 95,
        })
    }

    fn charge_percent(&self) -> HalResult<u8> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.battery_charge)
    }

    fn is_charging(&self) -> HalResult<bool> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.battery_charging)
    }

    fn time_to_empty(&self) -> HalResult<Option<u64>> {
        Ok(Some(7200))
    }

    fn time_to_full(&self) -> HalResult<Option<u64>> {
        Ok(Some(3600))
    }
}

/// Mock Thermal HAL.
pub struct MockThermalHal {
    state: SharedMockState,
}

impl MockThermalHal {
    /// Create a new mock thermal HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl ThermalHal for MockThermalHal {
    fn zones(&self) -> HalResult<Vec<ThermalZone>> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.thermal_zones.clone())
    }

    fn temperature(&self, zone: &str) -> HalResult<f32> {
        let s = self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?;
        s.thermal_zones
            .iter()
            .find(|z| z.name == zone)
            .map(|z| z.temperature_c)
            .ok_or(HalError::NotFound)
    }
}

/// Mock Power HAL.
pub struct MockPowerHal {
    state: SharedMockState,
}

impl MockPowerHal {
    /// Create a new mock power HAL.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self { state }
    }
}

impl PowerHal for MockPowerHal {
    fn shutdown(&self) -> HalResult<()> {
        let mut s = self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?;
        s.events.push(DeviceEvent {
            device_id: DeviceId::new("system"),
            kind: DeviceKind::Cpu,
            change: DeviceChange::PowerChanged {
                from: PowerState::SelfPowered,
                to: PowerState::PowerOff,
            },
            timestamp_ms: 0,
        });
        Ok(())
    }

    fn reboot(&self) -> HalResult<()> {
        Ok(())
    }

    fn suspend(&self) -> HalResult<()> {
        Ok(())
    }

    fn can_suspend(&self) -> HalResult<bool> {
        Ok(self.state.lock().map_err(|e| HalError::IoError(e.to_string()))?.can_suspend)
    }
}

// ========================================================== composite mock HAL

/// Complete mock HAL with all subsystems.
pub struct MockHal {
    cpu: MockCpuHal,
    gpu: MockGpuHal,
    display: MockDisplayHal,
    keyboard: MockKeyboardHal,
    pointing: MockPointingHal,
    audio_output: MockAudioOutputHal,
    microphone: MockMicrophoneHal,
    camera: MockCameraHal,
    wifi: MockWifiHal,
    bluetooth: MockBluetoothHal,
    ethernet: MockEthernetHal,
    usb: MockUsbHal,
    storage: MockStorageHal,
    battery: MockBatteryHal,
    thermal: MockThermalHal,
    power: MockPowerHal,
}

impl MockHal {
    /// Create a new mock HAL with shared state.
    #[must_use]
    pub fn new(state: SharedMockState) -> Self {
        Self {
            cpu: MockCpuHal::new(state.clone()),
            gpu: MockGpuHal,
            display: MockDisplayHal::new(state.clone()),
            keyboard: MockKeyboardHal::new(state.clone()),
            pointing: MockPointingHal::new(state.clone()),
            audio_output: MockAudioOutputHal::new(state.clone()),
            microphone: MockMicrophoneHal::new(state.clone()),
            camera: MockCameraHal,
            wifi: MockWifiHal::new(state.clone()),
            bluetooth: MockBluetoothHal::new(state.clone()),
            ethernet: MockEthernetHal::new(state.clone()),
            usb: MockUsbHal::new(state.clone()),
            storage: MockStorageHal::new(state.clone()),
            battery: MockBatteryHal::new(state.clone()),
            thermal: MockThermalHal::new(state.clone()),
            power: MockPowerHal::new(state),
        }
    }
}

impl Hal for MockHal {
    fn cpu(&self) -> &dyn CpuHal {
        &self.cpu
    }
    fn gpu(&self) -> &dyn GpuHal {
        &self.gpu
    }
    fn display(&self) -> &dyn DisplayHal {
        &self.display
    }
    fn keyboard(&self) -> &dyn KeyboardHal {
        &self.keyboard
    }
    fn pointing(&self) -> &dyn PointingDeviceHal {
        &self.pointing
    }
    fn audio_output(&self) -> &dyn AudioOutputHal {
        &self.audio_output
    }
    fn microphone(&self) -> &dyn MicrophoneHal {
        &self.microphone
    }
    fn camera(&self) -> &dyn CameraHal {
        &self.camera
    }
    fn wifi(&self) -> &dyn WifiHal {
        &self.wifi
    }
    fn bluetooth(&self) -> &dyn BluetoothHal {
        &self.bluetooth
    }
    fn ethernet(&self) -> &dyn EthernetHal {
        &self.ethernet
    }
    fn usb(&self) -> &dyn UsbHal {
        &self.usb
    }
    fn storage(&self) -> &dyn StorageHal {
        &self.storage
    }
    fn battery(&self) -> &dyn BatteryHal {
        &self.battery
    }
    fn thermal(&self) -> &dyn ThermalHal {
        &self.thermal
    }
    fn power(&self) -> &dyn PowerHal {
        &self.power
    }

    fn discover(&self, sink: &dyn EventSink) -> HalResult<()> {
        let kinds = [
            DeviceKind::Cpu,
            DeviceKind::Gpu,
            DeviceKind::Display,
            DeviceKind::Keyboard,
            DeviceKind::Mouse,
            DeviceKind::AudioOutput,
            DeviceKind::Microphone,
            DeviceKind::Wifi,
            DeviceKind::Ethernet,
            DeviceKind::Usb,
            DeviceKind::Storage,
            DeviceKind::Battery,
            DeviceKind::ThermalSensor,
        ];
        for kind in kinds {
            sink.emit(DeviceEvent {
                device_id: DeviceId::new(&format!("mock-{}", kind.as_str())),
                kind,
                change: DeviceChange::Appeared,
                timestamp_ms: 0,
            });
        }
        Ok(())
    }

    fn backend_name(&self) -> &str {
        "mock"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn mock_hal_backend_name() {
        let state = new_mock_state();
        let hal = MockHal::new(state);
        assert_eq!(hal.backend_name(), "mock");
    }

    #[test]
    fn mock_cpu_usage_reflects_state() {
        let state = new_mock_state();
        let hal = MockHal::new(state.clone());
        assert_eq!(hal.cpu().usage().unwrap(), 0.15);
        state.lock().unwrap().cpu_usage = 0.95;
        assert_eq!(hal.cpu().usage().unwrap(), 0.95);
    }

    #[test]
    fn mock_display_brightness_is_read_write() {
        let state = new_mock_state();
        let hal = MockHal::new(state);
        assert_eq!(hal.display().brightness().unwrap(), 80);
        hal.display().set_brightness(50).unwrap();
        assert_eq!(hal.display().brightness().unwrap(), 50);
    }

    #[test]
    fn mock_audio_volume_is_read_write() {
        let state = new_mock_state();
        let hal = MockHal::new(state);
        assert_eq!(hal.audio_output().volume().unwrap(), 75);
        hal.audio_output().set_volume(30).unwrap();
        assert_eq!(hal.audio_output().volume().unwrap(), 30);
    }

    #[test]
    fn mock_wifi_connect_changes_state() {
        let state = new_mock_state();
        let hal = MockHal::new(state);
        hal.wifi().disconnect().unwrap();
        assert_eq!(hal.wifi().state().unwrap(), WifiState::Disconnected);
        hal.wifi().connect("test", None).unwrap();
        assert_eq!(hal.wifi().state().unwrap(), WifiState::Connected);
    }

    #[test]
    fn mock_discover_emits_events() {
        let state = new_mock_state();
        let hal = MockHal::new(state);
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = TestEventSink { events: events.clone() };
        hal.discover(&sink).unwrap();
        assert!(events.lock().unwrap().len() > 5);
    }

    #[test]
    fn mock_battery_charge_reflects_state() {
        let state = new_mock_state();
        let hal = MockHal::new(state.clone());
        assert_eq!(hal.battery().charge_percent().unwrap(), 85);
        state.lock().unwrap().battery_charge = 20;
        assert_eq!(hal.battery().charge_percent().unwrap(), 20);
    }

    #[test]
    fn mock_thermal_zones_are_accessible() {
        let state = new_mock_state();
        let hal = MockHal::new(state);
        let zones = hal.thermal().zones().unwrap();
        assert_eq!(zones.len(), 2);
        let temp = hal.thermal().temperature("cpu").unwrap();
        assert!((temp - 45.0).abs() < f32::EPSILON);
    }

    struct TestEventSink {
        events: Arc<Mutex<Vec<DeviceEvent>>>,
    }

    impl EventSink for TestEventSink {
        fn emit(&self, event: DeviceEvent) {
            self.events.lock().unwrap().push(event);
        }
    }
}
