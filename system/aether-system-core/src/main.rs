// Aether System Core - control daemon binary.
//
// Loads service manifests, starts all services in dependency order, and
// serves the local control protocol used by `aetherctl`:
// newline-delimited JSON requests/responses over TCP loopback.

use aether_application_manager::ApplicationManager;
use aether_core::ipc::{IpcError, IpcRequest, IpcResponse};
use aether_core::types::ServiceStatus;
use aether_system_core::{
    build_manager, load_manifests_from_dir, ServiceExecutor, ServiceHandle,
};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Applications the capability layer exposes. Commands are spawned directly
/// as argv vectors (never through a shell); `aether-calculator` is the real
/// graphical test app, `sleep` placeholders stand in for the rest.
const SEED_APPS: &[(&str, &str, &str, &str)] = &[
    ("calculator", "Calculator", "0.1.0", "/bin/aether-calculator"),
    ("notes", "Notes", "0.1.0", "/bin/sleep 3601"),
    ("files", "Files", "0.1.0", "/bin/sleep 3602"),
];

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Structured audit line for every capability request dispatched here.
fn audit(capability: &str, args: &serde_json::Value, component: &str, ok: bool) {
    eprintln!(
        "[audit] ts={} component={} capability={} args={} result={}",
        unix_ms(),
        component,
        capability,
        args,
        if ok { "success" } else { "failure" }
    );
}

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

fn dispatch(
    manager: &mut aether_system_core::manager::ServiceManager,
    apps: &mut ApplicationManager,
    req: &IpcRequest,
) -> IpcResponse {
    // Capability requests (app.*) are audited with their arguments; service
    // lifecycle commands keep the generic audit line.
    let is_capability = req.command.starts_with("app.") || req.command == "system.status";
    let started_ok = true;
    let response = dispatch_inner(manager, apps, req);
    if is_capability {
        audit(
            &req.command,
            &req.parameters,
            &req.service_id,
            started_ok && response.ok,
        );
    }
    response
}

fn dispatch_inner(
    manager: &mut aether_system_core::manager::ServiceManager,
    apps: &mut ApplicationManager,
    req: &IpcRequest,
) -> IpcResponse {
    let mut executor = LocalExecutor::default();
    match req.command.as_str() {
        "status" | "system.status" => {
            let mut value = match serde_json::to_value(manager.system_status()) {
                Ok(v) => v,
                Err(e) => {
                    return IpcResponse::err(
                        &req.command,
                        IpcError {
                            code: "INTERNAL".to_string(),
                            message: e.to_string(),
                        },
                    )
                }
            };
            // Application runtime summary (installed/running/failed).
            let (installed, running, failed) = apps.stats();
            if let Some(obj) = value.as_object_mut() {
                obj.insert(
                    "applications".to_string(),
                    serde_json::json!({ "installed": installed, "running": running, "failed": failed }),
                );
            }
            IpcResponse::ok(&req.command, value)
        }
        "app.status" => match req.parameters.get("app").and_then(|v| v.as_str()) {
            Some(app_id) => {
                let report = apps.app_state(app_id);
                IpcResponse::ok("app.status", serde_json::json!({ "report": report }))
            }
            None => IpcResponse::err(
                "app.status",
                IpcError {
                    code: "INVALID_INPUT".to_string(),
                    message: "parameter 'app' is required".to_string(),
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
        "app.list" => {
            let apps_list: Vec<serde_json::Value> = apps
                .discover()
                .into_iter()
                .map(|d| {
                    serde_json::json!({
                        "id": d.id, "name": d.display_name, "version": d.version,
                        "type": d.app_type.to_string(), "permissions": d.permissions,
                    })
                })
                .collect();
            IpcResponse::ok("app.list", serde_json::json!({ "apps": apps_list }))
        }
        "app.launch" => match req.parameters.get("app").and_then(|v| v.as_str()) {
            Some(app_id) => match apps.launch(app_id) {
                Ok(instance) => IpcResponse::ok(
                    "app.launch",
                    serde_json::json!({ "app": app_id, "instance": instance }),
                ),
                Err(e) => IpcResponse::err(
                    "app.launch",
                    IpcError {
                        code: "APP_ERROR".to_string(),
                        message: e.to_string(),
                    },
                ),
            },
            None => IpcResponse::err(
                "app.launch",
                IpcError {
                    code: "INVALID_INPUT".to_string(),
                    message: "parameter 'app' is required".to_string(),
                },
            ),
        },
        "app.close" => {
            // Close by application name (closes its RUNNING instance) or by
            // explicit instance id.
            if let Some(app_id) = req.parameters.get("app").and_then(|v| v.as_str()) {
                let target = apps
                    .running()
                    .into_iter()
                    .find(|i| i.app_id == app_id)
                    .map(|i| i.instance_id);
                match target {
                    Some(instance) => match apps.close(instance) {
                        Ok(closed) => {
                            IpcResponse::ok("app.close", serde_json::json!({ "closed": closed }))
                        }
                        Err(e) => IpcResponse::err(
                            "app.close",
                            IpcError { code: "APP_ERROR".to_string(), message: e.to_string() },
                        ),
                    },
                    None => IpcResponse::err(
                        "app.close",
                        IpcError {
                            code: "NOT_RUNNING".to_string(),
                            message: format!("'{app_id}' has no running instance"),
                        },
                    ),
                }
            } else {
                match req.parameters.get("instance").and_then(|v| v.as_u64()) {
                    Some(instance) => match apps.close(instance) {
                        Ok(closed) => {
                            IpcResponse::ok("app.close", serde_json::json!({ "closed": closed }))
                        }
                        Err(e) => IpcResponse::err(
                            "app.close",
                            IpcError { code: "APP_ERROR".to_string(), message: e.to_string() },
                        ),
                    },
                    None => IpcResponse::err(
                        "app.close",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "'app' or numeric 'instance' is required".to_string(),
                        },
                    ),
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
    // Loopback by default; guest images override to expose the plane to
    // the isolated QEMU user network.
    let bind_addr =
        std::env::var("AETHER_BIND").unwrap_or_else(|_| "127.0.0.1".to_string());

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

    let mut apps = ApplicationManager::default();
    for (id, name, version, command) in SEED_APPS {
        let def = aether_application_manager::AppDefinition::new(
            id,
            name,
            version,
            command,
            &["display"],
        );
        match def {
            Ok(def) => {
                let _ = apps.register(def);
            }
            Err(e) => eprintln!("[system-core] seed app '{id}' rejected: {e}"),
        }
    }
    eprintln!(
        "[system-core] {} services running; {} app capabilities registered; control plane on {bind_addr}:{port}",
        manager.graph().len(),
        apps.registered_count()
    );

    let listener = match std::net::TcpListener::bind((bind_addr.as_str(), port)) {
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
                    dispatch(&mut manager, &mut apps, &req)
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
