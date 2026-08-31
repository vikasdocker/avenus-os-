//! Aether proactive daemon — the long-running
//! supervisor that observes the system and surfaces
//! action items the user can review.
//!
//! Phase 13.2 closure. The daemon connects to
//! `aether-system-core` over the loopback TCP control
//! plane (port 4747), polls the system status
//! endpoints on every tick, classifies the result
//! into typed observations, and feeds them through
//! the `aether-background-agent` state machine.
//!
//! The daemon is a *supervisor*, not an executor:
//! it never runs anything itself. The action items
//! it surfaces go to the IPC sink, where a human
//! (or the agent runtime) can review and approve
//! them. This is the "review-then-execute" model
//! mandated by the project.
//!
//! Usage:
//!   aether-proactived \
//!     --control-plane 127.0.0.1:4747 \
//!     --tick-ms 5000
//!
//! Flags:
//!   --control-plane <HOST:PORT>   the system-core TCP endpoint.
//!                                  Default: 127.0.0.1:4747.
//!   --tick-ms <MILLIS>            how often to poll. Default: 5000.
//!   --once                        run a single tick and exit. Used by
//!                                  tests and the boot smoke test.
//!   -h, --help                    print this help and exit.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::ExitCode;
use std::time::Duration;

use aether_proactive::{DaemonLoop, ObservationSink, SystemProbe, TickResult};
use aether_agent_core::Observation;

/// The default loopback control plane endpoint.
const DEFAULT_CONTROL_PLANE: &str = "127.0.0.1:4747";
/// The default tick interval (5 s).
const DEFAULT_TICK_MS: u64 = 5_000;
/// The IPC read timeout.
const IPC_READ_TIMEOUT: Duration = Duration::from_secs(5);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let parsed = match parse_args(&args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("usage error: {e}");
            return ExitCode::from(2);
        }
    };

    let mut loop_ = DaemonLoop::new();
    let mut sink = IpcSink::new(parsed.control_plane.clone());

    if parsed.once {
        let probe = match probe_system(&mut sink) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("proactived: probe failed: {e}");
                return ExitCode::from(3);
            }
        };
        let result: TickResult = loop_.tick(&probe, parsed.now_ms, &mut sink);
        print_tick_result(&result);
        return ExitCode::SUCCESS;
    }

    println!(
        "aether-proactived: control_plane={} tick_ms={} now_ms={}",
        parsed.control_plane, parsed.tick_ms, parsed.now_ms
    );
    loop {
        let probe = match probe_system(&mut sink) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("proactived: probe failed: {e}; sleeping one tick");
                std::thread::sleep(Duration::from_millis(parsed.tick_ms));
                continue;
            }
        };
        let result = loop_.tick(&probe, parsed.now_ms, &mut sink);
        print_tick_result(&result);
        std::thread::sleep(Duration::from_millis(parsed.tick_ms));
    }
}

struct Cli {
    control_plane: String,
    tick_ms: u64,
    now_ms: u64,
    once: bool,
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut control_plane = DEFAULT_CONTROL_PLANE.to_string();
    let mut tick_ms = DEFAULT_TICK_MS;
    let mut now_ms: u64 = system_time_now_ms();
    let mut once = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--control-plane" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| "--control-plane requires a value".to_string())?;
                control_plane = v.clone();
            }
            "--tick-ms" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| "--tick-ms requires a value".to_string())?;
                tick_ms = v
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --tick-ms: {e}"))?;
            }
            "--now-ms" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| "--now-ms requires a value".to_string())?;
                now_ms = v
                    .parse::<u64>()
                    .map_err(|e| format!("invalid --now-ms: {e}"))?;
            }
            "--once" => {
                once = true;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }
    Ok(Cli { control_plane, tick_ms, now_ms, once })
}

fn print_help() {
    println!("aether-proactived");
    println!();
    println!("USAGE:");
    println!("    aether-proactived [--control-plane HOST:PORT] [--tick-ms MILLIS]");
    println!("    aether-proactived --once [--control-plane HOST:PORT] [--now-ms MILLIS]");
    println!();
    println!("OPTIONS:");
    println!("    --control-plane <HOST:PORT>   system-core TCP endpoint. Default: 127.0.0.1:4747.");
    println!("    --tick-ms <MILLIS>            poll interval. Default: 5000.");
    println!("    --once                        run a single tick and exit.");
    println!("    --now-ms <MILLIS>             wall-clock timestamp for the tick.");
    println!("    -h, --help                    print this help.");
}

fn print_tick_result(result: &TickResult) {
    println!(
        "aether-proactived: tick: observations={} actions={}",
        result.observations, result.actions
    );
}

// ------------------------------------------------------------- probe

/// Probe the system by asking `aether-system-core` for
/// its current status. Every field of the returned
/// `SystemProbe` is best-effort: the probe does not
/// fail if a particular field is unavailable; it
/// simply leaves it at `None` / empty.
fn probe_system(sink: &mut IpcSink) -> Result<SystemProbe, String> {
    let mut probe = SystemProbe::default();
    if let Ok(value) = sink.rpc("system.info", serde_json::json!({})) {
        if let Some(mem) = value.get("memory_percent").and_then(|v| v.as_u64()) {
            probe.memory_percent = Some(mem.min(100) as u8);
        }
    }
    if let Ok(value) = sink.rpc("storage.status", serde_json::json!({})) {
        if let Some(mounts) = value.get("mounts").and_then(|v| v.as_array()) {
            for mount in mounts {
                if let (Some(name), Some(percent)) = (
                    mount.get("mount").and_then(|v| v.as_str()),
                    mount.get("percent").and_then(|v| v.as_u64()),
                ) {
                    probe.storage_percent.insert(name.to_string(), percent.min(100) as u8);
                }
            }
        }
    }
    if let Ok(value) = sink.rpc("network.status", serde_json::json!({})) {
        if let Some(reachable) = value.get("reachable").and_then(|v| v.as_bool()) {
            probe.network_reachable = Some(reachable);
        }
    }
    if let Ok(value) = sink.rpc("process.list", serde_json::json!({})) {
        if let Some(procs) = value.get("processes").and_then(|v| v.as_array()) {
            for p in procs {
                if let Some(pid) = p.get("pid").and_then(|v| v.as_u64()) {
                    if let Some(cpu) = p.get("cpu_percent").and_then(|v| v.as_u64()) {
                        probe.process_cpu.insert(pid as u32, cpu.min(100) as u8);
                    }
                    if let Some(mib) = p.get("rss_mib").and_then(|v| v.as_u64()) {
                        probe.process_memory_mib.insert(pid as u32, mib);
                    }
                }
            }
        }
    }
    Ok(probe)
}

// ------------------------------------------------------------- sink

/// The IPC sink. Talks to `aether-system-core` over
/// the loopback TCP control plane to push observations
/// and proposals.
pub struct IpcSink {
    endpoint: String,
}

impl IpcSink {
    /// Construct a new sink pointing at the given
    /// `HOST:PORT` endpoint.
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }

    /// Send an Aether JSON-RPC request and return the
    /// decoded `result` value.
    pub fn rpc(
        &mut self,
        command: &str,
        parameters: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let (host, port) = parse_endpoint(&self.endpoint)?;
        let mut stream = TcpStream::connect((host.as_str(), port))
            .map_err(|e| format!("connect {}: {e}", self.endpoint))?;
        stream
            .set_read_timeout(Some(IPC_READ_TIMEOUT))
            .map_err(|e| format!("read timeout: {e}"))?;
        let req = serde_json::json!({
            "service_id": "aether-proactive",
            "command": command,
            "parameters": parameters,
        });
        stream
            .write_all(format!("{req}\n").as_bytes())
            .map_err(|e| format!("send: {e}"))?;
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .map_err(|e| format!("recv: {e}"))?;
        if line.is_empty() {
            return Err("empty response".to_string());
        }
        let value: serde_json::Value =
            serde_json::from_str(line.trim()).map_err(|e| format!("decode: {e}"))?;
        if let Some(err) = value.get("error") {
            return Err(format!("rpc error: {err}"));
        }
        Ok(value.get("result").cloned().unwrap_or(serde_json::json!({})))
    }
}

impl ObservationSink for IpcSink {
    fn submit_observation(&mut self, obs: Observation) {
        let value = serde_json::to_value(&obs).unwrap_or_else(|_| serde_json::json!({}));
        if let Err(e) = self.rpc("agent.observe", serde_json::json!({ "observation": value })) {
            eprintln!("proactived: submit_observation failed: {e}");
        }
    }

    fn submit_actions(&mut self, items: Vec<aether_background_agent::ActionItem>) {
        for item in items {
            if let Err(e) = self.rpc(
                "agent.propose",
                serde_json::json!({
                    "title": item.title,
                    "description": item.description,
                    "payload": item.payload,
                    "risk": item.task_risk,
                    "requires_consent": item.requires_consent,
                }),
            ) {
                eprintln!("proactived: submit_actions failed: {e}");
            }
        }
    }

    fn tick_started(&mut self, now_ms: u64) {
        println!("aether-proactived: tick_started now_ms={now_ms}");
    }

    fn tick_finished(&mut self, now_ms: u64, observations: usize, actions: usize) {
        println!(
            "aether-proactived: tick_finished now_ms={now_ms} observations={observations} actions={actions}"
        );
    }
}

fn parse_endpoint(endpoint: &str) -> Result<(String, u16), String> {
    let (host, port) = endpoint
        .rsplit_once(':')
        .ok_or_else(|| format!("invalid endpoint: {endpoint}"))?;
    let port = port.parse::<u16>().map_err(|e| format!("invalid port: {e}"))?;
    Ok((host.to_string(), port))
}

fn system_time_now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_defaults() {
        let cli = parse_args(&["aether-proactived".to_string()]).unwrap();
        assert_eq!(cli.control_plane, "127.0.0.1:4747");
        assert_eq!(cli.tick_ms, 5_000);
        assert!(!cli.once);
    }

    #[test]
    fn parse_args_with_overrides() {
        let cli = parse_args(&[
            "aether-proactived".to_string(),
            "--control-plane".to_string(),
            "10.0.0.1:4747".to_string(),
            "--tick-ms".to_string(),
            "1000".to_string(),
            "--once".to_string(),
        ])
        .unwrap();
        assert_eq!(cli.control_plane, "10.0.0.1:4747");
        assert_eq!(cli.tick_ms, 1000);
        assert!(cli.once);
    }

    #[test]
    fn parse_args_unknown_flag_errors() {
        let cli = parse_args(&["aether-proactived".to_string(), "--bogus".to_string()]);
        assert!(cli.is_err());
    }

    #[test]
    fn parse_endpoint_splits_host_port() {
        let (h, p) = parse_endpoint("127.0.0.1:4747").unwrap();
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 4747);
    }

    #[test]
    fn parse_endpoint_rejects_missing_port() {
        assert!(parse_endpoint("127.0.0.1").is_err());
    }

    #[test]
    fn parse_endpoint_rejects_non_numeric_port() {
        assert!(parse_endpoint("127.0.0.1:abc").is_err());
    }
}
