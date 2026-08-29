// Aether Network - typed network surface, manager, and backend abstraction.
//
// This crate is the canonical Aether model for everything network-related.
// It does NOT reconfigure the network. It only describes it. The agent
// runtime, the shell, and the network surface all read from this crate.
//
// Layering:
//
//   backend (stub | /proc)  ──►  NetworkManager  ──►  typed models  ──►  callers
//
// The manager caches a single snapshot. All public queries operate on
// that snapshot so test output is deterministic and so callers never
// touch the filesystem directly.

pub mod manager;
pub mod proc;

use serde::{Deserialize, Serialize};
use std::fmt;

// ----------------------------------------------------------------- enums

/// Physical/logical kind of a network interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterfaceKind {
    Loopback,
    Ethernet,
    Wifi,
    Bridge,
    Tunnel,
    Virtual,
    Unknown,
}

impl InterfaceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Loopback => "loopback",
            Self::Ethernet => "ethernet",
            Self::Wifi => "wifi",
            Self::Bridge => "bridge",
            Self::Tunnel => "tunnel",
            Self::Virtual => "virtual",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for InterfaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Operational state of a network interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterfaceState {
    Up,
    Down,
    Unknown,
}

impl InterfaceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for InterfaceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// IP address family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AddressFamily {
    V4,
    V6,
}

impl AddressFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V4 => "ipv4",
            Self::V6 => "ipv6",
        }
    }
}

impl fmt::Display for AddressFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// High-level connectivity verdict (derived from local state only —
/// no active probing in the first cut).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectivityStatus {
    /// At least one non-loopback interface is up and a default route
    /// is present.
    Full,
    /// An interface is up but no default route.
    Limited,
    /// No usable interface.
    None,
    /// The backend could not determine connectivity.
    Unknown,
}

impl ConnectivityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Limited => "limited",
            Self::None => "none",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for ConnectivityStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ------------------------------------------------------------------ models

/// A network interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interface {
    pub name: String,
    pub kind: InterfaceKind,
    pub state: InterfaceState,
    pub mac_address: String,
    pub mtu: u32,
    pub index: u32,
}

impl Interface {
    pub fn is_up(&self) -> bool {
        matches!(self.state, InterfaceState::Up)
    }
}

/// A bound address on an interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub interface: String,
    pub family: AddressFamily,
    pub address: String,
    pub prefix_len: u8,
    pub scope: String,
}

/// A routing-table entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub family: AddressFamily,
    pub destination: String,
    pub prefix_len: u8,
    pub gateway: String,
    pub interface: String,
    pub metric: u32,
}

/// DNS resolver configuration.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsConfig {
    pub nameservers: Vec<String>,
    pub search_domains: Vec<String>,
    /// Free-form source label: "resolv.conf", "systemd-resolved",
    /// "stub", etc.
    pub source: String,
}

impl DnsConfig {
    pub fn empty() -> Self {
        Self {
            nameservers: Vec::new(),
            search_domains: Vec::new(),
            source: "empty".to_string(),
        }
    }
}

/// Per-interface traffic counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceStats {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}

// --------------------------------------------------------------- events

/// Discrete change observed by a backend. The manager keeps the last
/// `MAX_EVENTS` events so callers can introspect recent history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    LinkUp(String),
    LinkDown(String),
    AddressAdded(Address),
    AddressRemoved(Address),
    DnsChanged,
}

impl Event {
    pub fn label(&self) -> &'static str {
        match self {
            Self::LinkUp(_) => "link.up",
            Self::LinkDown(_) => "link.down",
            Self::AddressAdded(_) => "address.added",
            Self::AddressRemoved(_) => "address.removed",
            Self::DnsChanged => "dns.changed",
        }
    }
}

/// Maximum number of events kept in the manager's rolling log.
pub const MAX_EVENTS: usize = 64;

// ------------------------------------------------------------------ errors

/// All failure modes surfaced by the network crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkError {
    /// The backend refused to load data.
    Backend(String),
    /// A lookup for an interface by name failed.
    NotFound(String),
    /// A line/file could not be parsed.
    Parse(String),
    /// An I/O error (missing file, permission denied, etc).
    Io(String),
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(s) => write!(f, "backend error: {s}"),
            Self::NotFound(s) => write!(f, "not found: {s}"),
            Self::Parse(s) => write!(f, "parse error: {s}"),
            Self::Io(s) => write!(f, "i/o error: {s}"),
        }
    }
}

impl std::error::Error for NetworkError {}

impl From<std::io::Error> for NetworkError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

// ----------------------------------------------------------------- status

/// High-level summary suitable for system status panels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub backend: String,
    pub interface_count: usize,
    pub interfaces_up: usize,
    pub address_count: usize,
    pub route_count: usize,
    pub connectivity: ConnectivityStatus,
    pub dns_source: String,
}

// --------------------------------------------------------------- re-exports

pub use manager::{
    select_backend, NetworkBackend, NetworkManager, StubBackend, StubSeed,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_kind_display() {
        assert_eq!(InterfaceKind::Loopback.to_string(), "loopback");
        assert_eq!(InterfaceKind::Ethernet.as_str(), "ethernet");
        assert_eq!(InterfaceKind::Wifi.to_string(), "wifi");
    }

    #[test]
    fn interface_state_display() {
        assert_eq!(InterfaceState::Up.as_str(), "up");
        assert_eq!(InterfaceState::Down.to_string(), "down");
        assert_eq!(InterfaceState::Unknown.to_string(), "unknown");
    }

    #[test]
    fn address_family_display() {
        assert_eq!(AddressFamily::V4.to_string(), "ipv4");
        assert_eq!(AddressFamily::V6.as_str(), "ipv6");
    }

    #[test]
    fn connectivity_display() {
        assert_eq!(ConnectivityStatus::Full.as_str(), "full");
        assert_eq!(ConnectivityStatus::Limited.to_string(), "limited");
        assert_eq!(ConnectivityStatus::None.as_str(), "none");
        assert_eq!(ConnectivityStatus::Unknown.to_string(), "unknown");
    }

    #[test]
    fn interface_is_up_matches_state() {
        let mut i = Interface {
            name: "eth0".to_string(),
            kind: InterfaceKind::Ethernet,
            state: InterfaceState::Up,
            mac_address: "00:00:00:00:00:00".to_string(),
            mtu: 1500,
            index: 2,
        };
        assert!(i.is_up());
        i.state = InterfaceState::Down;
        assert!(!i.is_up());
    }

    #[test]
    fn interface_serializes_round_trip() {
        let i = Interface {
            name: "eth0".to_string(),
            kind: InterfaceKind::Ethernet,
            state: InterfaceState::Up,
            mac_address: "02:42:ac:11:00:02".to_string(),
            mtu: 1500,
            index: 2,
        };
        let json = serde_json::to_string(&i).unwrap_or_default();
        let back: Interface = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back, i);
    }

    #[test]
    fn address_serializes_round_trip() {
        let a = Address {
            interface: "eth0".to_string(),
            family: AddressFamily::V4,
            address: "10.0.2.15".to_string(),
            prefix_len: 24,
            scope: "global".to_string(),
        };
        let json = serde_json::to_string(&a).unwrap_or_default();
        let back: Address = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back, a);
    }

    #[test]
    fn route_serializes_round_trip() {
        let r = Route {
            family: AddressFamily::V4,
            destination: "0.0.0.0".to_string(),
            prefix_len: 0,
            gateway: "10.0.2.2".to_string(),
            interface: "eth0".to_string(),
            metric: 100,
        };
        let json = serde_json::to_string(&r).unwrap_or_default();
        let back: Route = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back, r);
    }

    #[test]
    fn dns_empty_has_empty_lists() {
        let d = DnsConfig::empty();
        assert!(d.nameservers.is_empty());
        assert!(d.search_domains.is_empty());
        assert_eq!(d.source, "empty");
    }

    #[test]
    fn stats_serializes_round_trip() {
        let s = InterfaceStats {
            interface: "eth0".to_string(),
            rx_bytes: 1024,
            tx_bytes: 2048,
            rx_packets: 10,
            tx_packets: 20,
            rx_errors: 0,
            tx_errors: 0,
            rx_dropped: 0,
            tx_dropped: 0,
        };
        let json = serde_json::to_string(&s).unwrap_or_default();
        let back: InterfaceStats = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back, s);
    }

    #[test]
    fn event_label_is_stable() {
        assert_eq!(
            Event::LinkUp("eth0".to_string()).label(),
            "link.up"
        );
        assert_eq!(
            Event::LinkDown("eth0".to_string()).label(),
            "link.down"
        );
        assert_eq!(Event::DnsChanged.label(), "dns.changed");
    }

    #[test]
    fn network_error_display_is_informative() {
        let e = NetworkError::NotFound("eth9".to_string());
        assert_eq!(e.to_string(), "not found: eth9");
        let e = NetworkError::Backend("proc unreachable".to_string());
        assert_eq!(e.to_string(), "backend error: proc unreachable");
    }

    #[test]
    fn io_error_converts_to_network_error() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let ne: NetworkError = io.into();
        assert!(matches!(ne, NetworkError::Io(_)));
    }
}
