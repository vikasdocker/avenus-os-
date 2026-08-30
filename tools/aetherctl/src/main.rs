// Aetherctl - command-line control client for Aether OS.
//
// Usage:
//   aetherctl status
//   aetherctl start <service> | stop <service> | restart <service>
//   aetherctl shutdown
//
// Environment: AETHER_CONTROL (default "127.0.0.1:4747")

use aether_sdk::{AetherClient, ActorTrust, IpcRequest};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let endpoint = std::env::var("AETHER_CONTROL").unwrap_or_else(|_| "127.0.0.1:4747".to_string());

    let client = AetherClient::new(endpoint.clone(), Duration::from_secs(5));

    let request = match (args.first().map(String::as_str), args.get(1).map(String::as_str)) {
        (Some("status"), None) => Some(IpcRequest {
            service_id: "aether-system-core".to_string(),
            command: "status".to_string(),
            parameters: serde_json::json!({}),
            actor_trust: ActorTrust::Trusted,
        }),
        (Some(action @ ("start" | "stop" | "restart")), Some(service)) if args.len() == 2 => {
            Some(IpcRequest {
                service_id: service.to_string(),
                command: action.to_string(),
                parameters: serde_json::json!({ "service": service }),
                actor_trust: ActorTrust::Trusted,
            })
        }
        (Some("shutdown"), None) => Some(IpcRequest {
            service_id: "aether-system-core".to_string(),
            command: "shutdown".to_string(),
            parameters: serde_json::json!({}),
            actor_trust: ActorTrust::Trusted,
        }),
        _ => None,
    };

    let Some(request) = request else {
        eprintln!("aetherctl - Aether OS control client");
        eprintln!("usage:");
        eprintln!("  aetherctl status");
        eprintln!("  aetherctl start|stop|restart <service>");
        eprintln!("  aetherctl shutdown");
        eprintln!("environment: AETHER_CONTROL (default 127.0.0.1:4747)");
        std::process::exit(if args.is_empty() { 0 } else { 2 });
    };

    match client.request(&request) {
        Ok(res) => {
            let rendered =
                serde_json::to_string_pretty(&res).unwrap_or_else(|_| format!("{res:?}"));
            println!("{rendered}");
            if !res.ok {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("aetherctl: control plane at {endpoint} unreachable: {e}");
            std::process::exit(1);
        }
    }
}
