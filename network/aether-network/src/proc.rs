// Linux procfs backend for the network crate.
//
// This is intentionally minimal. It reads:
//   * /proc/net/dev      — interface names + counters
//   * /proc/net/route    — IPv4 routes
//   * /proc/net/if_inet6 — IPv6 addresses
//   * /etc/resolv.conf   — DNS nameservers and search domains
//
// Any file that is missing or unparseable is treated as "no data
// available" rather than an error. That way the manager can keep
// running in stripped-down QEMU images.

use crate::{
    Address, AddressFamily, DnsConfig, Event, Interface, InterfaceKind, InterfaceState,
    InterfaceStats, NetworkBackend, NetworkError, Route,
};

/// Real backend. Holds the root directory it should read from so
/// tests can point it at a tempdir.
pub struct ProcBackend {
    root: String,
}

impl ProcBackend {
    pub fn new() -> Self {
        Self { root: "/proc".to_string() }
    }

    /// Constructor for tests: a backend that reads from `root`
    /// instead of `/proc`. Resolv.conf is also rooted at `<root>/etc`.
    #[cfg(test)]
    pub fn with_root(root: &str) -> Self {
        Self { root: root.to_string() }
    }

    fn read(&self, rel: &str) -> Result<String, NetworkError> {
        let path = format!("{}/{}", self.root.trim_end_matches('/'), rel);
        std::fs::read_to_string(&path).map_err(NetworkError::from)
    }

    fn kind_for(name: &str) -> InterfaceKind {
        if name == "lo" {
            InterfaceKind::Loopback
        } else if name.starts_with("eth") || name.starts_with("en") {
            InterfaceKind::Ethernet
        } else if name.starts_with("wl") {
            InterfaceKind::Wifi
        } else if name.starts_with("br") {
            InterfaceKind::Bridge
        } else if name.starts_with("tun") || name.starts_with("wg") {
            InterfaceKind::Tunnel
        } else if name.starts_with("veth") || name.starts_with("docker") {
            InterfaceKind::Virtual
        } else {
            InterfaceKind::Unknown
        }
    }

    fn parse_proc_net_dev(content: &str) -> Vec<(String, InterfaceStats)> {
        // Format: header lines, then " iface: rx_bytes ... tx_bytes ..."
        // with leading whitespace and a colon. See man 5 proc.
        let mut out = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if let Some((name, rest)) = line.split_once(':') {
                let fields: Vec<&str> = rest.split_whitespace().collect();
                if fields.len() < 16 {
                    continue;
                }
                let parse = |i: usize| -> u64 { fields[i].parse::<u64>().unwrap_or(0) };
                let stats = InterfaceStats {
                    interface: name.trim().to_string(),
                    rx_bytes: parse(0),
                    rx_packets: parse(1),
                    rx_errors: parse(2),
                    rx_dropped: parse(3),
                    tx_bytes: parse(8),
                    tx_packets: parse(9),
                    tx_errors: parse(10),
                    tx_dropped: parse(11),
                };
                out.push((name.trim().to_string(), stats));
            }
        }
        out
    }

    fn parse_proc_net_route(content: &str) -> Result<Vec<Route>, NetworkError> {
        // /proc/net/route columns (after header):
        //   Iface Destination Gateway Flags RefCnt Use Metric Mask MTU Window IRTT
        let mut out = Vec::new();
        for (i, line) in content.lines().enumerate() {
            if i == 0 {
                continue;
            }
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 11 {
                continue;
            }
            let iface = cols[0].to_string();
            let destination = hex_to_ipv4(cols[1])
                .ok_or_else(|| NetworkError::Parse(format!("bad destination in route line {i}")))?;
            let gateway = hex_to_ipv4(cols[2])
                .ok_or_else(|| NetworkError::Parse(format!("bad gateway in route line {i}")))?;
            let mask = hex_to_ipv4(cols[7])
                .ok_or_else(|| NetworkError::Parse(format!("bad mask in route line {i}")))?;
            let metric: u32 = cols[6].parse().unwrap_or(0);
            let prefix_len = mask_to_prefix(mask.as_str());
            out.push(Route {
                family: AddressFamily::V4,
                destination,
                prefix_len,
                gateway,
                interface: iface,
                metric,
            });
        }
        Ok(out)
    }

    fn parse_proc_net_if_inet6(content: &str) -> Vec<Address> {
        // /proc/net/if_inet6 columns (net/ipv6/addrconf.c, hex):
        //   addr (32 hex) if_index (hex) prefix_len (hex) scope (hex)
        //   flags (hex) dev_name
        let mut out = Vec::new();
        for line in content.lines() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 6 {
                continue;
            }
            let raw = cols[0];
            let prefix_len: u8 = u8::from_str_radix(cols[2], 16).unwrap_or(64);
            let scope_code: u32 = u32::from_str_radix(cols[3], 16).unwrap_or(0);
            let iface = cols[5].to_string();
            if let Some(addr) = hex32_to_ipv6(raw) {
                let scope = match scope_code {
                    0x0 => "global",
                    0x10 => "host",
                    0x20 => "link",
                    0x40 => "site",
                    0x80 => "global",
                    _ => "unknown",
                };
                out.push(Address {
                    interface: iface,
                    family: AddressFamily::V6,
                    address: addr,
                    prefix_len,
                    scope: scope.to_string(),
                });
            }
        }
        out
    }

    fn parse_resolv_conf(content: &str) -> DnsConfig {
        let mut nameservers = Vec::new();
        let mut search_domains = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("nameserver") {
                let ns = rest.trim();
                if !ns.is_empty() {
                    nameservers.push(ns.to_string());
                }
            } else if let Some(rest) = line.strip_prefix("search") {
                search_domains.extend(rest.split_whitespace().map(|s| s.to_string()));
            } else if let Some(rest) = line.strip_prefix("domain") {
                if let Some(d) = rest.split_whitespace().next() {
                    search_domains.push(d.to_string());
                }
            }
        }
        DnsConfig { nameservers, search_domains, source: "resolv.conf".to_string() }
    }
}

impl Default for ProcBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn hex_to_ipv4(hex: &str) -> Option<String> {
    // /proc/net/route stores each IPv4 address as 8 hex characters
    // where the bytes of the address appear in network byte order
    // (big-endian). For example 10.0.2.2 -> "0a000202".
    if hex.len() != 8 {
        return None;
    }
    let bytes: [u8; 4] = [
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
        u8::from_str_radix(&hex[6..8], 16).ok()?,
    ];
    Some(format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]))
}

fn hex32_to_ipv6(hex: &str) -> Option<String> {
    // /proc/net/if_inet6 stores each IPv6 address as 32 hex chars,
    // 4 chars per 16-bit group, where each 16-bit group is laid out
    // in network byte order. For example ::1 -> "0000000000000000
    // 0000000000000001".
    if hex.len() != 32 {
        return None;
    }
    let mut groups: Vec<String> = Vec::with_capacity(8);
    for i in 0..8 {
        groups.push(hex[i * 4..i * 4 + 4].to_string());
    }
    Some(groups.join(":"))
}

fn mask_to_prefix(mask: &str) -> u8 {
    let mut count: u32 = 0;
    for part in mask.split('.') {
        if let Ok(n) = part.parse::<u8>() {
            count = count.saturating_add(n.count_ones());
        }
    }
    count.min(255) as u8
}

impl NetworkBackend for ProcBackend {
    fn name(&self) -> &str {
        "proc"
    }

    fn load_interfaces(&self) -> Result<Vec<Interface>, NetworkError> {
        let content = self.read("net/dev")?;
        let parsed = Self::parse_proc_net_dev(&content);
        let ifs = parsed
            .into_iter()
            .enumerate()
            .map(|(i, (name, stats))| Interface {
                name: name.clone(),
                kind: Self::kind_for(&name),
                // /proc/net/dev does not include link state directly; the
                // presence of counters is a reasonable proxy for "Up".
                // We refine this in a later phase with RTM_GETLINK.
                state: if stats.rx_packets + stats.tx_packets > 0 || name == "lo" {
                    InterfaceState::Up
                } else {
                    InterfaceState::Unknown
                },
                mac_address: String::new(),
                mtu: 1500,
                index: (i as u32) + 1,
            })
            .collect();
        Ok(ifs)
    }

    fn load_addresses(&self) -> Result<Vec<Address>, NetworkError> {
        let mut out: Vec<Address> = Vec::new();
        if let Ok(content) = self.read("net/if_inet6") {
            out.extend(Self::parse_proc_net_if_inet6(&content));
        }
        // /proc/net/route does not enumerate per-interface addresses, only
        // routes. We at least add a stub v4 address per interface via
        // net/dev so callers see something. Real address enumeration
        // requires netlink, which is out of scope for the first cut.
        if let Ok(content) = self.read("net/dev") {
            for (name, _) in Self::parse_proc_net_dev(&content) {
                if name == "lo" {
                    out.push(Address {
                        interface: name,
                        family: AddressFamily::V4,
                        address: "127.0.0.1".to_string(),
                        prefix_len: 8,
                        scope: "host".to_string(),
                    });
                }
            }
        }
        Ok(out)
    }

    fn load_routes(&self) -> Result<Vec<Route>, NetworkError> {
        let content = self.read("net/route")?;
        Self::parse_proc_net_route(&content)
    }

    fn load_dns(&self) -> Result<DnsConfig, NetworkError> {
        let path = format!("{}/etc/resolv.conf", self.root.trim_end_matches('/'));
        let content = std::fs::read_to_string(&path).map_err(NetworkError::from)?;
        Ok(Self::parse_resolv_conf(&content))
    }

    fn load_stats(&self) -> Result<Vec<InterfaceStats>, NetworkError> {
        let content = self.read("net/dev")?;
        Ok(Self::parse_proc_net_dev(&content).into_iter().map(|(_, s)| s).collect())
    }

    fn load_events(&self) -> Result<Vec<Event>, NetworkError> {
        // The first cut does not subscribe to netlink. The events API
        // is shaped for it, but this backend reports nothing.
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_proc_net_dev_extracts_counters() {
        let sample = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 100       1    0    0    0     0          0         0  100       1    0    0    0     0       0          0
  eth0: 200       2    0    0    0     0          0         0  300       3    0    0    0     0       0          0
";
        let parsed = ProcBackend::parse_proc_net_dev(sample);
        assert_eq!(parsed.len(), 2);
        let (name, stats) = &parsed[0];
        assert_eq!(name, "lo");
        assert_eq!(stats.rx_bytes, 100);
        assert_eq!(stats.tx_bytes, 100);
        let (name, stats) = &parsed[1];
        assert_eq!(name, "eth0");
        assert_eq!(stats.rx_bytes, 200);
        assert_eq!(stats.tx_packets, 3);
    }

    #[test]
    fn parse_proc_net_route_decodes_hex_columns() {
        // The hex columns are 32-bit values in network byte order.
        // 10.0.2.2 -> 0x0a000202
        let sample = "\
Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\tMTU\tWindow\tIRTT
eth0\t00000000\t0A000202\t0003\t0\t0\t100\t00000000\t0\t0\t0
eth0\t0A000000\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0
";
        let routes = ProcBackend::parse_proc_net_route(sample).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].destination, "0.0.0.0");
        assert_eq!(routes[0].gateway, "10.0.2.2");
        assert_eq!(routes[0].metric, 100);
        assert_eq!(routes[0].prefix_len, 0);
        assert_eq!(routes[1].destination, "10.0.0.0");
        assert_eq!(routes[1].gateway, "0.0.0.0");
        assert_eq!(routes[1].prefix_len, 24);
    }

    #[test]
    fn parse_proc_net_if_inet6_decodes_addresses() {
        // /proc/net/if_inet6 columns: addr if_index prefix_len scope flags dev_name
        // (prefix_len and scope are hex per the kernel proc handler).
        let sample = "\
00000000000000000000000000000001 01 80 10 00 lo
fe800000000000000000428a00002b1d 02 40 20 00 eth0
";
        let addrs = ProcBackend::parse_proc_net_if_inet6(sample);
        assert_eq!(addrs.len(), 2);
        assert_eq!(addrs[0].interface, "lo");
        assert_eq!(addrs[0].address, "0000:0000:0000:0000:0000:0000:0000:0001");
        assert_eq!(addrs[0].prefix_len, 128);
        assert_eq!(addrs[0].scope, "host");
        assert_eq!(addrs[1].interface, "eth0");
        assert_eq!(addrs[1].address, "fe80:0000:0000:0000:0000:428a:0000:2b1d");
        assert_eq!(addrs[1].prefix_len, 64);
        assert_eq!(addrs[1].scope, "link");
    }

    #[test]
    fn parse_resolv_conf_extracts_nameservers_and_search() {
        let sample = "\
# generated by aether
search aether.local lan
nameserver 10.0.2.3
nameserver 1.1.1.1
domain aether.local
";
        let cfg = ProcBackend::parse_resolv_conf(sample);
        assert_eq!(cfg.nameservers, vec!["10.0.2.3".to_string(), "1.1.1.1".to_string()]);
        assert!(cfg.search_domains.contains(&"aether.local".to_string()));
        assert!(cfg.search_domains.contains(&"lan".to_string()));
        assert_eq!(cfg.source, "resolv.conf");
    }

    #[test]
    fn kind_classifier_recognises_known_prefixes() {
        assert_eq!(ProcBackend::kind_for("lo"), InterfaceKind::Loopback);
        assert_eq!(ProcBackend::kind_for("eth0"), InterfaceKind::Ethernet);
        assert_eq!(ProcBackend::kind_for("enp0s3"), InterfaceKind::Ethernet);
        assert_eq!(ProcBackend::kind_for("wlan0"), InterfaceKind::Wifi);
        assert_eq!(ProcBackend::kind_for("br-1234"), InterfaceKind::Bridge);
        assert_eq!(ProcBackend::kind_for("tun0"), InterfaceKind::Tunnel);
        assert_eq!(ProcBackend::kind_for("vethabc"), InterfaceKind::Virtual);
        assert_eq!(ProcBackend::kind_for("zzz"), InterfaceKind::Unknown);
    }

    #[test]
    fn backend_loads_from_tempdir_root() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let root = dir.path().to_str().unwrap_or_default();
        fs::create_dir_all(format!("{root}/net")).unwrap_or_else(|e| panic!("{e}"));
        fs::create_dir_all(format!("{root}/etc")).unwrap_or_else(|e| panic!("{e}"));
        fs::write(
            format!("{root}/net/dev"),
            "Inter-|   Receive\n lo: 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n eth0: 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n",
        ).unwrap_or_else(|e| panic!("{e}"));
        fs::write(format!("{root}/etc/resolv.conf"), "nameserver 8.8.8.8\nsearch test\n")
            .unwrap_or_else(|e| panic!("{e}"));

        let be = ProcBackend::with_root(root);
        let ifs = be.load_interfaces().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(ifs.len(), 2);
        assert_eq!(ifs[0].name, "lo");
        let dns = be.load_dns().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(dns.nameservers, vec!["8.8.8.8".to_string()]);
        assert_eq!(dns.source, "resolv.conf");
    }

    #[test]
    fn backend_reports_error_for_missing_file() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let root = dir.path().to_str().unwrap_or_default();
        let be = ProcBackend::with_root(root);
        let err = be.load_interfaces().err().unwrap_or_else(|| panic!("expected error"));
        assert!(matches!(err, NetworkError::Io(_)));
    }

    #[test]
    fn backend_name_is_proc() {
        let be = ProcBackend::new();
        assert_eq!(be.name(), "proc");
    }

    #[test]
    fn hex_to_ipv4_decodes_be_hex() {
        assert_eq!(hex_to_ipv4("0a000002").as_deref(), Some("10.0.0.2"));
        assert_eq!(hex_to_ipv4("0a000202").as_deref(), Some("10.0.2.2"));
        assert_eq!(hex_to_ipv4("00000000").as_deref(), Some("0.0.0.0"));
        assert_eq!(hex_to_ipv4("ffffffff").as_deref(), Some("255.255.255.255"));
        assert_eq!(hex_to_ipv4("zzzz").as_deref(), None);
        assert_eq!(hex_to_ipv4("0a00020").as_deref(), None);
    }

    #[test]
    fn hex32_to_ipv6_decodes_groups() {
        assert_eq!(
            hex32_to_ipv6("00000000000000000000000000000001").as_deref(),
            Some("0000:0000:0000:0000:0000:0000:0000:0001")
        );
        assert_eq!(
            hex32_to_ipv6("fe800000000000000000428a00002b1d").as_deref(),
            Some("fe80:0000:0000:0000:0000:428a:0000:2b1d")
        );
        assert_eq!(hex32_to_ipv6("short").as_deref(), None);
    }

    #[test]
    fn mask_to_prefix_counts_bits() {
        assert_eq!(mask_to_prefix("255.255.255.0"), 24);
        assert_eq!(mask_to_prefix("255.255.255.252"), 30);
        assert_eq!(mask_to_prefix("0.0.0.0"), 0);
        assert_eq!(mask_to_prefix("255.255.255.255"), 32);
    }
}
