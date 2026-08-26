// Aether System Core - control daemon binary.
//
// Loads service manifests, starts all services in dependency order, and
// serves the local control protocol used by `aetherctl`:
// newline-delimited JSON requests/responses over TCP loopback.

use aether_core::ipc::{IpcError, IpcRequest, IpcResponse};
use aether_core::types::ServiceStatus;
use aether_system_core::{
    build_manager, load_manifests_from_dir, ServiceExecutor, ServiceHandle,
};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// In-process executor: internal services run inside this daemon; process
/// services are represented with a deterministic pseudo-pid for now.
#[derive(Debug, Default)]
struct LocalExecutor {
    next_pid: AtomicU64,
}

impl ServiceExecutor for LocalExecutor {
    fn start(&mut self, service_id: &str) -> Result<ServiceHandle, aether_core::error::AetherError> {
        let pid = self.next_pid.fetch_add(1, Ordering::SeqCst) + 1000;
        eprintln!("[system-core] started '{service_id}' (pid {pid})");
        Ok(ServiceHandle {
            service_id: service_id.to_string(),
            pid: u32::try_from(pid).unwrap_or(u32::MAX),
        })
    }

    fn stop(&mut self, handle: &ServiceHandle) -> Result<(), aether_core::error::AetherError> {
        eprintln!("[system-core] stopped '{}' (pid {})", handle.service_id, handle.pid);
        Ok(())
    }

    fn health(
        &mut self,
        _handle: &ServiceHandle,
    ) -> Result<aether_core::types::HealthStatus, aether_core::error::AetherError> {
        Ok(aether_core::types::HealthStatus::Healthy)
    }
}

fn dispatch(manager: &mut aether_system_core::manager::ServiceManager, req: &IpcRequest) -> IpcResponse {
    let mut executor = LocalExecutor::default();
    match req.command.as_str() {
        "status" => match serde_json::to_value(manager.system_status()) {
            Ok(result) => IpcResponse::ok("status", result),
            Err(e) => IpcResponse::err(
                "status",
                IpcError {
                    code: "INTERNAL".to_string(),
                    message: e.to_string(),
                },
            ),
        },
        "start" | "stop" | "restart" => {
            let id = req.parameters.get("service").and_then(|v| v.as_str());
            let Some(service_id) = id else {
                return IpcResponse::err(
                    &req.command,
                    IpcError {
                        code: "INVALID_INPUT".to_string(),
                        message: "parameter 'service' is required".to_string(),
                    },
                );
            };
            // Keep dependency order sane for start/restart of one unit.
            let deps_ok = manager
                .graph()
                .manifest(service_id)
                .map(|m| {
                    m.dependencies.iter().all(|d| {
                        manager
                            .graph()
                            .manifest(d)
                            .map(|_| true)
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            if !deps_ok && manager.graph().manifest(service_id).is_none() {
                return IpcResponse::err(
                    &req.command,
                    IpcError {
                        code: "NOT_FOUND".to_string(),
                        message: format!("unknown service '{service_id}'"),
                    },
                );
            }
            if req.command == "start" || req.command == "restart" {
                // Dependencies must be running first.
                if let Some(manifest) = manager.graph().manifest(service_id).cloned() {
                    for d in &manifest.dependencies {
                        let running = manager
                            .system_status()
                            .services
                            .iter()
                            .any(|s| &s.service_id == d && s.status == ServiceStatus::Running);
                        if !running {
                            let _ = manager.start_one(&mut executor, d);
                        }
                    }
                }
                match manager.restart_one(&mut executor, service_id) {
                    Ok(()) => IpcResponse::ok(
                        &req.command,
                        serde_json::json!({ "service": service_id, "state": "RUNNING" }),
                    ),
                    Err(e) => IpcResponse::err(&req.command, e.into()),
                }
            } else {
                match manager.stop_one(&mut executor, service_id) {
                    Ok(()) => IpcResponse::ok(
                        &req.command,
                        serde_json::json!({ "service": service_id, "state": "STOPPED" }),
                    ),
                    Err(e) => IpcResponse::err(&req.command, e.into()),
                }
            }
        }
        "shutdown" => IpcResponse::ok("shutdown", serde_json::json!({ "state": "SHUTTING_DOWN" })),
        other => IpcResponse::err(
            other,
            IpcError {
                code: "INVALID_INPUT".to_string(),
                message: format!("unknown command '{other}'"),
            },
        ),
    }
}

fn main() {
    let manifests_dir = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("AETHER_MANIFEST_DIR").ok())
        .unwrap_or_else(|| "/etc/aether/services.d".to_string());

    let port: u16 = std::env::var("AETHER_CONTROL_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4747);

    eprintln!("[system-core] loading manifests from {manifests_dir}");
    let manifests = match load_manifests_from_dir(&PathBuf::from(&manifests_dir)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[system-core] fatal: {e}");
            std::process::exit(1);
        }
    };

    let mut manager = match build_manager(&manifests) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[system-core] fatal: {e}");
            std::process::exit(1);
        }
    };

    let mut executor = LocalExecutor::default();
    if let Err(e) = manager.start_all(&mut executor) {
        eprintln!("[system-core] fatal: {e}");
        std::process::exit(1);
    }
    eprintln!(
        "[system-core] {} services running; control plane on 127.0.0.1:{port}",
        manager.graph().len()
    );

    let listener = match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[system-core] fatal: cannot bind control port: {e}");
            std::process::exit(1);
        }
    };

    let shutting_down = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    for stream in listener.incoming() {
        if shutting_down.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        let peer = stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        eprintln!("[system-core] control connection from {peer}");
        let mut writer = match stream.try_clone() {
            Ok(w) => w,
            Err(_) => continue,
        };
        let reader = BufReader::new(stream);

        let mut stop_requested = false;
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let response = match serde_json::from_str::<IpcRequest>(&line) {
                Ok(req) => {
                    if req.command == "shutdown" {
                        stop_requested = true;
                    }
                    dispatch(&mut manager, &req)
                }
                Err(e) => IpcResponse::err(
                    "?",
                    IpcError {
                        code: "INVALID_INPUT".to_string(),
                        message: format!("bad request: {e}"),
                    },
                ),
            };
            let mut payload = serde_json::to_string(&response).unwrap_or_default();
            payload.push('\n');
            if writer.write_all(payload.as_bytes()).is_err() {
                break;
            }
        }

        if stop_requested {
            eprintln!("[system-core] shutdown requested");
            shutting_down.store(true, Ordering::SeqCst);
            let _ = manager.stop_all(&mut LocalExecutor::default());
            break;
        }
    }

    eprintln!("[system-core] exited cleanly");
}
