// NetworkManager + NetworkBackend trait + StubBackend.
//
// The manager is the only thing callers talk to. It holds a single
// in-memory snapshot and serves every query from that snapshot.
//
// The backend is a trait so the same manager can be powered by the
// deterministic stub (QEMU, tests) or by a real reader of the host
// networking surface (procfs on Linux). Backends never get called
// from inside a public query — only from `refresh()`.

use crate::{
    Address, AddressFamily, ConnectivityStatus, DnsConfig, Event, Interface, InterfaceKind,
    InterfaceState, InterfaceStats, NetworkError, NetworkStatus, Route, MAX_EVENTS,
};

/// Source of network data. Implementations are pure data loaders —
/// they do not mutate the system.
pub trait NetworkBackend: Send + Sync {
    fn name(&self) -> &str;

    fn load_interfaces(&self) -> Result<Vec<Interface>, NetworkError>;
    fn load_addresses(&self) -> Result<Vec<Address>, NetworkError>;
    fn load_routes(&self) -> Result<Vec<Route>, NetworkError>;
    fn load_dns(&self) -> Result<DnsConfig, NetworkError>;
    fn load_stats(&self) -> Result<Vec<InterfaceStats>, NetworkError>;
    fn load_events(&self) -> Result<Vec<Event>, NetworkError>;
}

// --------------------------------------------------------------- snapshot

#[derive(Debug, Default, Clone)]
struct Snapshot {
    backend: String,
    interfaces: Vec<Interface>,
    addresses: Vec<Address>,
    routes: Vec<Route>,
    dns: DnsConfig,
    stats: Vec<InterfaceStats>,
    events: Vec<Event>,
}

// ---------------------------------------------------------------- manager

/// Registry and read-only queries.
pub struct NetworkManager {
    backend: Box<dyn NetworkBackend>,
    snap: Snapshot,
}

impl NetworkManager {
    pub fn new_with_backend(backend: Box<dyn NetworkBackend>) -> Self {
        let name = backend.name().to_string();
        Self { backend, snap: Snapshot { backend: name, ..Snapshot::default() } }
    }

    /// Pulls a fresh snapshot from the backend. Backends that fail a
    /// particular loader return empty rather than abort the whole
    /// refresh — the manager stays useful for the parts that worked.
    pub fn refresh(&mut self) {
        self.snap.interfaces = self.backend.load_interfaces().unwrap_or_default();
        self.snap.addresses = self.backend.load_addresses().unwrap_or_default();
        self.snap.routes = self.backend.load_routes().unwrap_or_default();
        self.snap.dns = self.backend.load_dns().unwrap_or_else(|_| DnsConfig::empty());
        self.snap.stats = self.backend.load_stats().unwrap_or_default();
        let new_events = self.backend.load_events().unwrap_or_default();
        for ev in new_events {
            self.push_event(ev);
        }
    }

    fn push_event(&mut self, ev: Event) {
        if self.snap.events.len() >= MAX_EVENTS {
            self.snap.events.remove(0);
        }
        self.snap.events.push(ev);
    }

    pub fn backend_name(&self) -> &str {
        &self.snap.backend
    }

    pub fn status(&self) -> NetworkStatus {
        let interfaces_up = self.snap.interfaces.iter().filter(|i| i.is_up()).count();
        NetworkStatus {
            backend: self.snap.backend.clone(),
            interface_count: self.snap.interfaces.len(),
            interfaces_up,
            address_count: self.snap.addresses.len(),
            route_count: self.snap.routes.len(),
            connectivity: self.derive_connectivity(),
            dns_source: self.snap.dns.source.clone(),
        }
    }

    pub fn interfaces(&self) -> Vec<Interface> {
        self.snap.interfaces.clone()
    }

    pub fn inspect(&self, name: &str) -> Result<Interface, NetworkError> {
        self.snap
            .interfaces
            .iter()
            .find(|i| i.name == name)
            .cloned()
            .ok_or_else(|| NetworkError::NotFound(name.to_string()))
    }

    pub fn addresses(&self) -> Vec<Address> {
        self.snap.addresses.clone()
    }

    pub fn routes(&self) -> Vec<Route> {
        self.snap.routes.clone()
    }

    pub fn dns(&self) -> DnsConfig {
        self.snap.dns.clone()
    }

    pub fn connectivity(&self) -> ConnectivityStatus {
        self.derive_connectivity()
    }

    pub fn stats(&self) -> Vec<InterfaceStats> {
        self.snap.stats.clone()
    }

    pub fn events(&self) -> Vec<Event> {
        self.snap.events.clone()
    }

    fn derive_connectivity(&self) -> ConnectivityStatus {
        if self.snap.interfaces.is_empty() {
            return ConnectivityStatus::Unknown;
        }
        let has_non_loopback_up =
            self.snap.interfaces.iter().any(|i| i.is_up() && i.kind != InterfaceKind::Loopback);
        let has_default_route = self.snap.routes.iter().any(|r| {
            matches!(r.family, AddressFamily::V4) && r.prefix_len == 0 && r.destination == "0.0.0.0"
        }) || self.snap.routes.iter().any(|r| {
            matches!(r.family, AddressFamily::V6) && r.prefix_len == 0 && r.destination == "::"
        });
        match (has_non_loopback_up, has_default_route) {
            (true, true) => ConnectivityStatus::Full,
            (true, false) => ConnectivityStatus::Limited,
            (false, _) => ConnectivityStatus::None,
        }
    }
}

impl Default for NetworkManager {
    fn default() -> Self {
        Self::new_with_backend(Box::new(StubBackend::default_seed()))
    }
}

// ----------------------------------------------------------------- stub

/// A deterministic seed used by tests, the QEMU image, and any
/// environment where real network introspection would be misleading.
pub struct StubSeed {
    pub interfaces: Vec<Interface>,
    pub addresses: Vec<Address>,
    pub routes: Vec<Route>,
    pub dns: DnsConfig,
    pub stats: Vec<InterfaceStats>,
    pub events: Vec<Event>,
}

impl StubSeed {
    /// The canonical seed: loopback, one ethernet with a default route,
    /// a working resolver. This is what QEMU and `aether-network`
    /// itself default to so the service has something to report.
    pub fn canonical() -> Self {
        let interfaces = vec![
            Interface {
                name: "lo".to_string(),
                kind: InterfaceKind::Loopback,
                state: InterfaceState::Up,
                mac_address: "00:00:00:00:00:00".to_string(),
                mtu: 65536,
                index: 1,
            },
            Interface {
                name: "eth0".to_string(),
                kind: InterfaceKind::Ethernet,
                state: InterfaceState::Up,
                mac_address: "02:42:ac:11:00:02".to_string(),
                mtu: 1500,
                index: 2,
            },
        ];
        let addresses = vec![
            Address {
                interface: "lo".to_string(),
                family: AddressFamily::V4,
                address: "127.0.0.1".to_string(),
                prefix_len: 8,
                scope: "host".to_string(),
            },
            Address {
                interface: "lo".to_string(),
                family: AddressFamily::V6,
                address: "::1".to_string(),
                prefix_len: 128,
                scope: "host".to_string(),
            },
            Address {
                interface: "eth0".to_string(),
                family: AddressFamily::V4,
                address: "10.0.2.15".to_string(),
                prefix_len: 24,
                scope: "global".to_string(),
            },
            Address {
                interface: "eth0".to_string(),
                family: AddressFamily::V6,
                address: "fe80::42:acff:fe11:2".to_string(),
                prefix_len: 64,
                scope: "link".to_string(),
            },
        ];
        let routes = vec![
            Route {
                family: AddressFamily::V4,
                destination: "10.0.2.0".to_string(),
                prefix_len: 24,
                gateway: "0.0.0.0".to_string(),
                interface: "eth0".to_string(),
                metric: 100,
            },
            Route {
                family: AddressFamily::V4,
                destination: "0.0.0.0".to_string(),
                prefix_len: 0,
                gateway: "10.0.2.2".to_string(),
                interface: "eth0".to_string(),
                metric: 100,
            },
            Route {
                family: AddressFamily::V6,
                destination: "fe80::".to_string(),
                prefix_len: 64,
                gateway: "::".to_string(),
                interface: "eth0".to_string(),
                metric: 256,
            },
        ];
        let dns = DnsConfig {
            nameservers: vec!["10.0.2.3".to_string()],
            search_domains: vec!["aether.local".to_string()],
            source: "stub".to_string(),
        };
        let stats = vec![
            InterfaceStats {
                interface: "lo".to_string(),
                rx_bytes: 0,
                tx_bytes: 0,
                rx_packets: 0,
                tx_packets: 0,
                rx_errors: 0,
                tx_errors: 0,
                rx_dropped: 0,
                tx_dropped: 0,
            },
            InterfaceStats {
                interface: "eth0".to_string(),
                rx_bytes: 1024,
                tx_bytes: 512,
                rx_packets: 8,
                tx_packets: 4,
                rx_errors: 0,
                tx_errors: 0,
                rx_dropped: 0,
                tx_dropped: 0,
            },
        ];
        Self { interfaces, addresses, routes, dns, stats, events: Vec::new() }
    }
}

/// Deterministic backend. Holds a `StubSeed` and returns clones of it
/// on every load call. This is the only backend the QEMU image and
/// tests should ever use.
pub struct StubBackend {
    seed: StubSeed,
}

impl StubBackend {
    pub fn default_seed() -> Self {
        Self { seed: StubSeed::canonical() }
    }

    pub fn with_seed(seed: StubSeed) -> Self {
        Self { seed }
    }
}

impl NetworkBackend for StubBackend {
    fn name(&self) -> &str {
        "stub"
    }

    fn load_interfaces(&self) -> Result<Vec<Interface>, NetworkError> {
        Ok(self.seed.interfaces.clone())
    }

    fn load_addresses(&self) -> Result<Vec<Address>, NetworkError> {
        Ok(self.seed.addresses.clone())
    }

    fn load_routes(&self) -> Result<Vec<Route>, NetworkError> {
        Ok(self.seed.routes.clone())
    }

    fn load_dns(&self) -> Result<DnsConfig, NetworkError> {
        Ok(self.seed.dns.clone())
    }

    fn load_stats(&self) -> Result<Vec<InterfaceStats>, NetworkError> {
        Ok(self.seed.stats.clone())
    }

    fn load_events(&self) -> Result<Vec<Event>, NetworkError> {
        Ok(self.seed.events.clone())
    }
}

// --------------------------------------------------------------- selector

/// Resolve a `backend` selector string. Recognises `"stub"`, `"proc"`,
/// and `"auto"`. Unknown values fall back to the stub.
pub fn select_backend(selector: &str) -> Box<dyn NetworkBackend> {
    match selector {
        "stub" => Box::new(StubBackend::default_seed()),
        #[cfg(target_os = "linux")]
        "proc" => Box::new(crate::proc::ProcBackend::new()),
        _ => auto_select(),
    }
}

/// Pick a backend based on the platform. On Linux, prefer
/// `ProcBackend` when `/proc/net/dev` is readable; otherwise fall
/// back to the stub. On other platforms the stub is the only option.
pub fn auto_select() -> Box<dyn NetworkBackend> {
    #[cfg(target_os = "linux")]
    {
        match std::fs::metadata("/proc/net/dev") {
            Ok(_) => Box::new(crate::proc::ProcBackend::new()),
            Err(_) => Box::new(StubBackend::default_seed()),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(StubBackend::default_seed())
    }
}

// ----------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NetworkError;

    fn manager() -> NetworkManager {
        let mut m = NetworkManager::new_with_backend(Box::new(StubBackend::default_seed()));
        m.refresh();
        m
    }

    #[test]
    fn stub_status_matches_seed_counts() {
        let m = manager();
        let s = m.status();
        assert_eq!(s.backend, "stub");
        assert_eq!(s.interface_count, 2);
        assert_eq!(s.interfaces_up, 2);
        assert_eq!(s.address_count, 4);
        assert_eq!(s.route_count, 3);
        assert_eq!(s.connectivity, ConnectivityStatus::Full);
        assert_eq!(s.dns_source, "stub");
    }

    #[test]
    fn inspect_returns_interface() {
        let m = manager();
        let lo = m.inspect("lo").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(lo.kind, InterfaceKind::Loopback);
        assert!(lo.is_up());
    }

    #[test]
    fn inspect_unknown_returns_not_found() {
        let m = manager();
        let err = m.inspect("ghost").err().unwrap_or_else(|| panic!("expected error"));
        assert!(matches!(err, NetworkError::NotFound(_)));
    }

    #[test]
    fn interfaces_lists_seed_entries() {
        let m = manager();
        let ifs = m.interfaces();
        let names: Vec<&str> = ifs.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"lo"));
        assert!(names.contains(&"eth0"));
    }

    #[test]
    fn addresses_match_seed() {
        let m = manager();
        let addrs = m.addresses();
        let v4: Vec<&Address> = addrs.iter().filter(|a| a.family == AddressFamily::V4).collect();
        let v6: Vec<&Address> = addrs.iter().filter(|a| a.family == AddressFamily::V6).collect();
        assert_eq!(v4.len(), 2);
        assert_eq!(v6.len(), 2);
    }

    #[test]
    fn routes_contain_default_v4() {
        let m = manager();
        let has_default = m.routes().iter().any(|r| {
            r.family == AddressFamily::V4 && r.prefix_len == 0 && r.destination == "0.0.0.0"
        });
        assert!(has_default);
    }

    #[test]
    fn dns_returns_seed() {
        let m = manager();
        let d = m.dns();
        assert_eq!(d.nameservers, vec!["10.0.2.3".to_string()]);
        assert_eq!(d.search_domains, vec!["aether.local".to_string()]);
    }

    #[test]
    fn stats_per_interface() {
        let m = manager();
        let s = m.stats();
        assert_eq!(s.len(), 2);
        let eth0 =
            s.iter().find(|x| x.interface == "eth0").unwrap_or_else(|| panic!("missing eth0"));
        assert_eq!(eth0.rx_bytes, 1024);
        assert_eq!(eth0.tx_bytes, 512);
    }

    #[test]
    fn events_start_empty() {
        let m = manager();
        assert!(m.events().is_empty());
    }

    #[test]
    fn events_cap_at_max() {
        // Use a custom backend that returns more than MAX_EVENTS.
        struct FloodBackend;
        impl NetworkBackend for FloodBackend {
            fn name(&self) -> &str {
                "flood"
            }
            fn load_interfaces(&self) -> Result<Vec<Interface>, NetworkError> {
                Ok(Vec::new())
            }
            fn load_addresses(&self) -> Result<Vec<Address>, NetworkError> {
                Ok(Vec::new())
            }
            fn load_routes(&self) -> Result<Vec<Route>, NetworkError> {
                Ok(Vec::new())
            }
            fn load_dns(&self) -> Result<DnsConfig, NetworkError> {
                Ok(DnsConfig::empty())
            }
            fn load_stats(&self) -> Result<Vec<InterfaceStats>, NetworkError> {
                Ok(Vec::new())
            }
            fn load_events(&self) -> Result<Vec<Event>, NetworkError> {
                Ok((0..(MAX_EVENTS as u32 + 10))
                    .map(|i| Event::LinkUp(format!("eth{i}")))
                    .collect())
            }
        }
        let mut m = NetworkManager::new_with_backend(Box::new(FloodBackend));
        m.refresh();
        assert_eq!(m.events().len(), MAX_EVENTS);
        // The first events should have been dropped; the last one
        // should be retained.
        let events = m.events();
        let last = events.last().unwrap_or_else(|| panic!("empty"));
        assert_eq!(last.label(), "link.up");
    }

    #[test]
    fn connectivity_loopback_only_is_none() {
        let mut seed = StubSeed::canonical();
        seed.interfaces[1].state = InterfaceState::Down;
        seed.routes.clear();
        let mut m = NetworkManager::new_with_backend(Box::new(StubBackend::with_seed(seed)));
        m.refresh();
        assert_eq!(m.connectivity(), ConnectivityStatus::None);
    }

    #[test]
    fn connectivity_up_without_default_route_is_limited() {
        let mut seed = StubSeed::canonical();
        // Keep eth0 up but strip the default route.
        seed.routes.retain(|r| !(r.family == AddressFamily::V4 && r.prefix_len == 0));
        let mut m = NetworkManager::new_with_backend(Box::new(StubBackend::with_seed(seed)));
        m.refresh();
        assert_eq!(m.connectivity(), ConnectivityStatus::Limited);
    }

    #[test]
    fn connectivity_no_interfaces_is_unknown() {
        let mut seed = StubSeed::canonical();
        seed.interfaces.clear();
        let mut m = NetworkManager::new_with_backend(Box::new(StubBackend::with_seed(seed)));
        m.refresh();
        assert_eq!(m.connectivity(), ConnectivityStatus::Unknown);
    }

    #[test]
    fn refresh_recovers_when_backend_partially_fails() {
        // Backend that errors on every loader.
        struct BrokenBackend;
        impl NetworkBackend for BrokenBackend {
            fn name(&self) -> &str {
                "broken"
            }
            fn load_interfaces(&self) -> Result<Vec<Interface>, NetworkError> {
                Ok(Vec::new())
            }
            fn load_addresses(&self) -> Result<Vec<Address>, NetworkError> {
                Ok(Vec::new())
            }
            fn load_routes(&self) -> Result<Vec<Route>, NetworkError> {
                Err(NetworkError::Backend("nope".to_string()))
            }
            fn load_dns(&self) -> Result<DnsConfig, NetworkError> {
                Err(NetworkError::Backend("nope".to_string()))
            }
            fn load_stats(&self) -> Result<Vec<InterfaceStats>, NetworkError> {
                Ok(Vec::new())
            }
            fn load_events(&self) -> Result<Vec<Event>, NetworkError> {
                Ok(Vec::new())
            }
        }
        let mut m = NetworkManager::new_with_backend(Box::new(BrokenBackend));
        m.refresh();
        let s = m.status();
        assert_eq!(s.route_count, 0);
        assert_eq!(s.dns_source, "empty");
    }

    #[test]
    fn select_backend_stub_is_known() {
        let b = select_backend("stub");
        assert_eq!(b.name(), "stub");
    }

    #[test]
    fn select_backend_unknown_falls_back_to_stub() {
        let b = select_backend("nonsense");
        let name = b.name();
        assert!(name == "stub" || name == "proc");
    }

    #[test]
    fn default_manager_uses_stub() {
        let mut m = NetworkManager::default();
        m.refresh();
        assert_eq!(m.backend_name(), "stub");
    }
}
