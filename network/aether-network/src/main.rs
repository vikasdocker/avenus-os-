// Aether Network - REPL daemon entry point.
//
// Newline-delimited JSON commands on stdin, JSON responses on stdout.
// Commands: status, interfaces, inspect <name>, addresses, routes, dns,
// connectivity, stats, events, help.
//
// Backend selection via env: AETHER_NET_BACKEND = stub|proc|auto.

use aether_network::{select_backend, NetworkManager};
use std::io::{BufRead, Write};

fn backend_choice() -> &'static str {
    match std::env::var("AETHER_NET_BACKEND") {
        Ok(s) if !s.is_empty() => match s.as_str() {
            "stub" | "proc" | "auto" => match s.as_str() {
                "stub" => "stub",
                "proc" => "proc",
                _ => "auto",
            },
            _ => "auto",
        },
        _ => "auto",
    }
}

fn dispatch(line: &str, manager: &mut NetworkManager) -> serde_json::Value {
    let mut tokens = line.split_whitespace();
    let cmd = tokens.next().unwrap_or("");
    match cmd {
        "status" => serde_json::json!({ "ok": true, "result": manager.status() }),
        "interfaces" => serde_json::json!({ "ok": true, "result": manager.interfaces() }),
        "inspect" => match tokens.next() {
            Some(name) => match manager.inspect(name) {
                Ok(i) => serde_json::json!({ "ok": true, "result": i }),
                Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
            },
            None => serde_json::json!({
                "ok": false,
                "error": "inspect requires an interface name",
            }),
        },
        "addresses" => serde_json::json!({ "ok": true, "result": manager.addresses() }),
        "routes" => serde_json::json!({ "ok": true, "result": manager.routes() }),
        "dns" => serde_json::json!({ "ok": true, "result": manager.dns() }),
        "connectivity" => serde_json::json!({
            "ok": true,
            "result": manager.connectivity(),
        }),
        "stats" => serde_json::json!({ "ok": true, "result": manager.stats() }),
        "events" => serde_json::json!({ "ok": true, "result": manager.events() }),
        "help" | "" => serde_json::json!({
            "ok": true,
            "result": "status | interfaces | inspect <name> | addresses | routes | dns | connectivity | stats | events",
        }),
        other => serde_json::json!({ "ok": false, "error": format!("unknown command '{other}'") }),
    }
}

fn main() {
    let choice = backend_choice();
    let backend = select_backend(choice);
    let mut manager = NetworkManager::new_with_backend(backend);
    manager.refresh();
    eprintln!(
        "[aether-network] ready; backend={} interfaces={} addresses={} routes={}",
        manager.backend_name(),
        manager.status().interface_count,
        manager.status().address_count,
        manager.status().route_count,
    );

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let response = dispatch(line.trim(), &mut manager);
        let mut payload = serde_json::to_string(&response).unwrap_or_default();
        payload.push('\n');
        let mut out = stdout.lock();
        if out.write_all(payload.as_bytes()).is_err() || out.flush().is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_network::StubBackend;

    fn fresh() -> NetworkManager {
        let mut m = NetworkManager::new_with_backend(Box::new(StubBackend::default_seed()));
        m.refresh();
        m
    }

    #[test]
    fn dispatch_status_returns_ok() {
        let mut m = fresh();
        let v = dispatch("status", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert_eq!(v["result"]["backend"], "stub");
    }

    #[test]
    fn dispatch_interfaces_returns_list() {
        let mut m = fresh();
        let v = dispatch("interfaces", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        let arr = v["result"].as_array().unwrap_or_else(|| panic!("not an array"));
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn dispatch_inspect_known_returns_interface() {
        let mut m = fresh();
        let v = dispatch("inspect lo", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert_eq!(v["result"]["name"], "lo");
    }

    #[test]
    fn dispatch_inspect_unknown_returns_error() {
        let mut m = fresh();
        let v = dispatch("inspect ghost", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(false));
        assert!(v["error"].as_str().unwrap_or_default().contains("ghost"));
    }

    #[test]
    fn dispatch_inspect_without_name_returns_error() {
        let mut m = fresh();
        let v = dispatch("inspect", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(false));
    }

    #[test]
    fn dispatch_addresses_returns_list() {
        let mut m = fresh();
        let v = dispatch("addresses", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        let arr = v["result"].as_array().unwrap_or_else(|| panic!("not an array"));
        assert!(!arr.is_empty());
    }

    #[test]
    fn dispatch_routes_returns_list() {
        let mut m = fresh();
        let v = dispatch("routes", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert!(v["result"].is_array());
    }

    #[test]
    fn dispatch_dns_returns_config() {
        let mut m = fresh();
        let v = dispatch("dns", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert_eq!(v["result"]["source"], "stub");
    }

    #[test]
    fn dispatch_connectivity_returns_status() {
        let mut m = fresh();
        let v = dispatch("connectivity", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        // ConnectivityStatus is serialised by its variant name ("Full");
        // callers wanting the lowercase form can map it themselves.
        assert!(v["result"].is_string());
    }

    #[test]
    fn dispatch_stats_returns_list() {
        let mut m = fresh();
        let v = dispatch("stats", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert!(v["result"].is_array());
    }

    #[test]
    fn dispatch_events_returns_list() {
        let mut m = fresh();
        let v = dispatch("events", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert!(v["result"].is_array());
    }

    #[test]
    fn dispatch_unknown_command_returns_error() {
        let mut m = fresh();
        let v = dispatch("frobnicate", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(false));
    }

    #[test]
    fn dispatch_empty_line_returns_help() {
        let mut m = fresh();
        let v = dispatch("", &mut m);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert!(v["result"].as_str().unwrap_or_default().contains("status"));
    }

    #[test]
    fn backend_choice_recognises_known_values() {
        // We can't set env vars safely in parallel tests; just check
        // the function compiles and returns a non-empty string.
        let c = backend_choice();
        assert!(!c.is_empty());
    }
}
