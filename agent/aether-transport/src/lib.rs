//! Device pairing transport abstractions for Aether OS.
//!
//! Provides trait-based abstractions over BLE, QR code, and
//! NFC pairing transports. Each transport discovers nearby
//! devices and exchanges pairing messages. Real implementations
//! use kernel/kernel-module interfaces; mock implementations
//! simulate discovery for QEMU testing.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use aether_device_core::DeviceId;
use serde::{Deserialize, Serialize};
use std::fmt;

// ------------------------------------------------------------------- errors

/// Transport error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportError {
    /// Transport not available on this hardware.
    NotAvailable,
    /// Transport is powered off.
    PoweredOff,
    /// No devices found.
    NoDevicesFound,
    /// Connection failed.
    ConnectionFailed(String),
    /// Data transfer failed.
    TransferFailed(String),
    /// Timeout waiting for device.
    Timeout,
    /// Pairing was rejected by the remote device.
    Rejected,
    /// Invalid data received.
    InvalidData(String),
    /// I/O error.
    IoError(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "transport not available"),
            Self::PoweredOff => write!(f, "transport powered off"),
            Self::NoDevicesFound => write!(f, "no devices found"),
            Self::ConnectionFailed(s) => write!(f, "connection failed: {s}"),
            Self::TransferFailed(s) => write!(f, "transfer failed: {s}"),
            Self::Timeout => write!(f, "timeout"),
            Self::Rejected => write!(f, "pairing rejected"),
            Self::InvalidData(s) => write!(f, "invalid data: {s}"),
            Self::IoError(s) => write!(f, "I/O error: {s}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// Convenience result type.
pub type TransportResult<T> = Result<T, TransportError>;

// ------------------------------------------------------------------ types

/// The transport type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportKind {
    /// Bluetooth Low Energy.
    Ble,
    /// QR code scanning.
    Qr,
    /// Near-field communication (NFC).
    NFC,
}

impl fmt::Display for TransportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ble => write!(f, "BLE"),
            Self::Qr => write!(f, "QR"),
            Self::NFC => write!(f, "NFC"),
        }
    }
}

/// A discovered device before pairing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    /// Device identifier.
    pub device_id: DeviceId,
    /// Device name.
    pub name: String,
    /// Signal strength (0-100), if applicable.
    pub signal_strength: Option<u8>,
    /// Transport-specific metadata.
    pub metadata: TransportMetadata,
    /// Whether this device has been seen before.
    pub previously_paired: bool,
}

/// Transport-specific metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportMetadata {
    /// BLE metadata.
    Ble {
        /// RSSI value.
        rssi: i16,
        /// Advertised service UUIDs.
        service_uuids: Vec<String>,
    },
    /// QR code metadata.
    Qr {
        /// QR code payload (base64-encoded).
        payload: String,
        /// Whether the QR has been scanned.
        scanned: bool,
    },
    /// NFC metadata.
    Nfc {
        /// NFC tag ID.
        tag_id: String,
        /// Tag technology.
        technology: String,
    },
}

/// A pairing channel opened over a transport.
pub trait PairingChannel: Send {
    /// Get the channel's transport kind.
    fn kind(&self) -> TransportKind;

    /// Get the remote device ID.
    fn remote_device_id(&self) -> &DeviceId;

    /// Send bytes over the channel.
    fn send(&mut self, data: &[u8]) -> TransportResult<()>;

    /// Receive bytes from the channel (blocking).
    fn receive(&mut self) -> TransportResult<Vec<u8>>;

    /// Close the channel.
    fn close(&mut self) -> TransportResult<()>;

    /// Whether the channel is still open.
    fn is_open(&self) -> bool;
}

// ========================================================== transport trait

/// The transport trait. Implementations provide device
/// discovery and pairing channel establishment.
pub trait Transport: Send + Sync {
    /// Get the transport kind.
    fn kind(&self) -> TransportKind;

    /// Get the transport name.
    fn name(&self) -> &str;

    /// Check if this transport is available.
    fn is_available(&self) -> bool;

    /// Power on the transport.
    fn power_on(&mut self) -> TransportResult<()>;

    /// Power off the transport.
    fn power_off(&mut self) -> TransportResult<()>;

    /// Whether the transport is currently powered on.
    fn is_powered_on(&self) -> bool;

    /// Start scanning for nearby devices.
    fn start_scan(&mut self) -> TransportResult<()>;

    /// Stop scanning.
    fn stop_scan(&mut self) -> TransportResult<()>;

    /// Get the list of currently discovered devices.
    fn discovered_devices(&self) -> TransportResult<Vec<DiscoveredDevice>>;

    /// Open a pairing channel to a specific device.
    fn open_channel(&self, device_id: &DeviceId) -> TransportResult<Box<dyn PairingChannel>>;

    /// Get the maximum transfer size in bytes for this transport.
    fn max_transfer_size(&self) -> usize;

    /// Get the typical discovery range in meters (0 = unlimited).
    fn range_meters(&self) -> Option<u32>;
}

// ========================================================== mock transport

/// Mock BLE transport for QEMU testing.
pub struct MockBleTransport {
    devices: Vec<DiscoveredDevice>,
    powered: bool,
    scanning: bool,
}

impl MockBleTransport {
    /// Create a new mock BLE transport.
    #[must_use]
    pub fn new() -> Self {
        Self { devices: Vec::new(), powered: false, scanning: false }
    }

    /// Add a mock device for discovery.
    pub fn add_device(&mut self, device: DiscoveredDevice) {
        self.devices.push(device);
    }
}

impl Default for MockBleTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for MockBleTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::Ble
    }

    fn name(&self) -> &str {
        "mock-ble"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn power_on(&mut self) -> TransportResult<()> {
        self.powered = true;
        Ok(())
    }

    fn power_off(&mut self) -> TransportResult<()> {
        self.powered = false;
        self.scanning = false;
        Ok(())
    }

    fn is_powered_on(&self) -> bool {
        self.powered
    }

    fn start_scan(&mut self) -> TransportResult<()> {
        if !self.powered {
            return Err(TransportError::PoweredOff);
        }
        self.scanning = true;
        Ok(())
    }

    fn stop_scan(&mut self) -> TransportResult<()> {
        self.scanning = false;
        Ok(())
    }

    fn discovered_devices(&self) -> TransportResult<Vec<DiscoveredDevice>> {
        if !self.scanning {
            return Ok(Vec::new());
        }
        Ok(self.devices.clone())
    }

    fn open_channel(&self, device_id: &DeviceId) -> TransportResult<Box<dyn PairingChannel>> {
        let exists = self.devices.iter().any(|d| &d.device_id == device_id);
        if !exists {
            return Err(TransportError::NoDevicesFound);
        }
        Ok(Box::new(MockPairingChannel {
            kind: TransportKind::Ble,
            remote: device_id.clone(),
            open: true,
            buffer: Vec::new(),
        }))
    }

    fn max_transfer_size(&self) -> usize {
        512
    }

    fn range_meters(&self) -> Option<u32> {
        Some(10)
    }
}

/// Mock QR transport for QEMU testing.
pub struct MockQrTransport {
    pending_scans: Vec<DiscoveredDevice>,
    powered: bool,
}

impl MockQrTransport {
    /// Create a new mock QR transport.
    #[must_use]
    pub fn new() -> Self {
        Self { pending_scans: Vec::new(), powered: false }
    }

    /// Queue a QR code for scanning.
    pub fn queue_scan(&mut self, device: DiscoveredDevice) {
        self.pending_scans.push(device);
    }
}

impl Default for MockQrTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for MockQrTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::Qr
    }

    fn name(&self) -> &str {
        "mock-qr"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn power_on(&mut self) -> TransportResult<()> {
        self.powered = true;
        Ok(())
    }

    fn power_off(&mut self) -> TransportResult<()> {
        self.powered = false;
        Ok(())
    }

    fn is_powered_on(&self) -> bool {
        self.powered
    }

    fn start_scan(&mut self) -> TransportResult<()> {
        if !self.powered {
            return Err(TransportError::PoweredOff);
        }
        Ok(())
    }

    fn stop_scan(&mut self) -> TransportResult<()> {
        Ok(())
    }

    fn discovered_devices(&self) -> TransportResult<Vec<DiscoveredDevice>> {
        Ok(self.pending_scans.clone())
    }

    fn open_channel(&self, device_id: &DeviceId) -> TransportResult<Box<dyn PairingChannel>> {
        let exists = self.pending_scans.iter().any(|d| &d.device_id == device_id);
        if !exists {
            return Err(TransportError::NoDevicesFound);
        }
        Ok(Box::new(MockPairingChannel {
            kind: TransportKind::Qr,
            remote: device_id.clone(),
            open: true,
            buffer: Vec::new(),
        }))
    }

    fn max_transfer_size(&self) -> usize {
        1024
    }

    fn range_meters(&self) -> Option<u32> {
        None
    }
}

/// Mock NFC transport for QEMU testing.
pub struct MockNfcTransport {
    devices: Vec<DiscoveredDevice>,
    powered: bool,
}

impl MockNfcTransport {
    /// Create a new mock NFC transport.
    #[must_use]
    pub fn new() -> Self {
        Self { devices: Vec::new(), powered: false }
    }

    /// Add a mock NFC tag.
    pub fn add_tag(&mut self, device: DiscoveredDevice) {
        self.devices.push(device);
    }
}

impl Default for MockNfcTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for MockNfcTransport {
    fn kind(&self) -> TransportKind {
        TransportKind::NFC
    }

    fn name(&self) -> &str {
        "mock-nfc"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn power_on(&mut self) -> TransportResult<()> {
        self.powered = true;
        Ok(())
    }

    fn power_off(&mut self) -> TransportResult<()> {
        self.powered = false;
        Ok(())
    }

    fn is_powered_on(&self) -> bool {
        self.powered
    }

    fn start_scan(&mut self) -> TransportResult<()> {
        if !self.powered {
            return Err(TransportError::PoweredOff);
        }
        Ok(())
    }

    fn stop_scan(&mut self) -> TransportResult<()> {
        Ok(())
    }

    fn discovered_devices(&self) -> TransportResult<Vec<DiscoveredDevice>> {
        Ok(self.devices.clone())
    }

    fn open_channel(&self, device_id: &DeviceId) -> TransportResult<Box<dyn PairingChannel>> {
        let exists = self.devices.iter().any(|d| &d.device_id == device_id);
        if !exists {
            return Err(TransportError::NoDevicesFound);
        }
        Ok(Box::new(MockPairingChannel {
            kind: TransportKind::NFC,
            remote: device_id.clone(),
            open: true,
            buffer: Vec::new(),
        }))
    }

    fn max_transfer_size(&self) -> usize {
        256
    }

    fn range_meters(&self) -> Option<u32> {
        Some(1)
    }
}

// ---------------------------------------------------------- mock channel

struct MockPairingChannel {
    kind: TransportKind,
    remote: DeviceId,
    open: bool,
    buffer: Vec<u8>,
}

impl PairingChannel for MockPairingChannel {
    fn kind(&self) -> TransportKind {
        self.kind
    }

    fn remote_device_id(&self) -> &DeviceId {
        &self.remote
    }

    fn send(&mut self, data: &[u8]) -> TransportResult<()> {
        if !self.open {
            return Err(TransportError::ConnectionFailed("channel closed".into()));
        }
        self.buffer.extend_from_slice(data);
        Ok(())
    }

    fn receive(&mut self) -> TransportResult<Vec<u8>> {
        if !self.open {
            return Err(TransportError::ConnectionFailed("channel closed".into()));
        }
        let data = self.buffer.clone();
        self.buffer.clear();
        Ok(data)
    }

    fn close(&mut self) -> TransportResult<()> {
        self.open = false;
        Ok(())
    }

    fn is_open(&self) -> bool {
        self.open
    }
}

/// Transport manager — holds all available transports.
pub struct TransportManager {
    transports: Vec<Box<dyn Transport>>,
}

impl TransportManager {
    /// Create an empty transport manager.
    #[must_use]
    pub fn new() -> Self {
        Self { transports: Vec::new() }
    }

    /// Register a transport.
    pub fn register(&mut self, transport: Box<dyn Transport>) {
        self.transports.push(transport);
    }

    /// Get a transport by kind.
    pub fn get(&self, kind: TransportKind) -> Option<&dyn Transport> {
        self.transports.iter().find(|t| t.kind() == kind).map(|t| t.as_ref())
    }

    /// Get all available transports.
    #[must_use]
    pub fn available(&self) -> Vec<&dyn Transport> {
        self.transports.iter().filter(|t| t.is_available()).map(|t| t.as_ref()).collect()
    }

    /// Discover devices across all active transports.
    pub fn discover_all(&self) -> TransportResult<Vec<DiscoveredDevice>> {
        let mut all = Vec::new();
        for t in &self.transports {
            if t.is_powered_on() {
                all.extend(t.discovered_devices()?);
            }
        }
        Ok(all)
    }
}

impl Default for TransportManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn test_device(id: &str) -> DiscoveredDevice {
        DiscoveredDevice {
            device_id: DeviceId::new(id).expect("valid device id"),
            name: format!("Device {id}"),
            signal_strength: Some(80),
            metadata: TransportMetadata::Ble {
                rssi: -50,
                service_uuids: vec!["0000180d-0000-1000-8000-00805f9b34fb".into()],
            },
            previously_paired: false,
        }
    }

    #[test]
    fn ble_transport_lifecycle() {
        let mut t = MockBleTransport::new();
        assert!(!t.is_powered_on());
        t.power_on().unwrap();
        assert!(t.is_powered_on());
        t.power_off().unwrap();
        assert!(!t.is_powered_on());
    }

    #[test]
    fn ble_scan_requires_power() {
        let mut t = MockBleTransport::new();
        let result = t.start_scan();
        assert_eq!(result.unwrap_err(), TransportError::PoweredOff);
    }

    #[test]
    fn ble_discover_devices() {
        let mut t = MockBleTransport::new();
        t.power_on().unwrap();
        t.add_device(test_device("a1"));
        t.add_device(test_device("a2"));
        t.start_scan().unwrap();
        let devices = t.discovered_devices().unwrap();
        assert_eq!(devices.len(), 2);
    }

    #[test]
    fn ble_open_channel() {
        let mut t = MockBleTransport::new();
        t.power_on().unwrap();
        t.add_device(test_device("a1"));
        t.start_scan().unwrap();
        let dev = t.discovered_devices().unwrap().remove(0);
        let mut ch = t.open_channel(&dev.device_id).unwrap();
        assert!(ch.is_open());
        ch.send(b"hello").unwrap();
        let data = ch.receive().unwrap();
        assert_eq!(data, b"hello");
        ch.close().unwrap();
        assert!(!ch.is_open());
    }

    #[test]
    fn ble_channel_rejects_unknown_device() {
        let t = MockBleTransport::new();
        let result = t.open_channel(&DeviceId::new("unknown").expect("valid id"));
        assert!(result.is_err());
    }

    #[test]
    fn qr_transport_lifecycle() {
        let mut t = MockQrTransport::new();
        t.power_on().unwrap();
        assert!(t.is_powered_on());
        t.power_off().unwrap();
        assert!(!t.is_powered_on());
    }

    #[test]
    fn qr_discover_devices() {
        let mut t = MockQrTransport::new();
        t.power_on().unwrap();
        t.queue_scan(test_device("q1"));
        let devices = t.discovered_devices().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, DeviceId::new("q1").expect("valid id"));
    }

    #[test]
    fn nfc_transport_lifecycle() {
        let mut t = MockNfcTransport::new();
        t.power_on().unwrap();
        assert!(t.is_powered_on());
    }

    #[test]
    fn nfc_discover_devices() {
        let mut t = MockNfcTransport::new();
        t.power_on().unwrap();
        t.add_tag(test_device("n1"));
        let devices = t.discovered_devices().unwrap();
        assert_eq!(devices.len(), 1);
    }

    #[test]
    fn transport_manager_discover_all() {
        let mut mgr = TransportManager::new();
        let mut ble = MockBleTransport::new();
        ble.power_on().unwrap();
        ble.start_scan().unwrap();
        ble.add_device(test_device("b1"));
        mgr.register(Box::new(ble));

        let mut nfc = MockNfcTransport::new();
        nfc.power_on().unwrap();
        nfc.start_scan().unwrap();
        nfc.add_tag(test_device("n1"));
        mgr.register(Box::new(nfc));

        let all = mgr.discover_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn transport_manager_get_by_kind() {
        let mut mgr = TransportManager::new();
        mgr.register(Box::new(MockBleTransport::new()));
        mgr.register(Box::new(MockQrTransport::new()));
        assert!(mgr.get(TransportKind::Ble).is_some());
        assert!(mgr.get(TransportKind::Qr).is_some());
        assert!(mgr.get(TransportKind::NFC).is_none());
    }

    #[test]
    fn transport_kind_display() {
        assert_eq!(TransportKind::Ble.to_string(), "BLE");
        assert_eq!(TransportKind::Qr.to_string(), "QR");
        assert_eq!(TransportKind::NFC.to_string(), "NFC");
    }

    #[test]
    fn transport_error_display() {
        let e = TransportError::ConnectionFailed("test".into());
        assert!(e.to_string().contains("test"));
    }

    #[test]
    fn max_transfer_sizes() {
        let ble = MockBleTransport::new();
        let qr = MockQrTransport::new();
        let nfc = MockNfcTransport::new();
        assert_eq!(ble.max_transfer_size(), 512);
        assert_eq!(qr.max_transfer_size(), 1024);
        assert_eq!(nfc.max_transfer_size(), 256);
    }

    #[test]
    fn range_meters() {
        let ble = MockBleTransport::new();
        let qr = MockQrTransport::new();
        let nfc = MockNfcTransport::new();
        assert_eq!(ble.range_meters(), Some(10));
        assert_eq!(qr.range_meters(), None);
        assert_eq!(nfc.range_meters(), Some(1));
    }
}
