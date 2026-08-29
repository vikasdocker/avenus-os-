// Aether System Core - control daemon binary.
//
// Loads service manifests, starts all services in dependency order, and
// serves the local control protocol used by `aetherctl`:
// newline-delimited JSON requests/responses over TCP loopback.

use aether_application_manager::ApplicationManager;
use aether_core::error::ErrorKind;
use aether_core::ipc::{IpcError, IpcRequest, IpcResponse};
use aether_core::types::ServiceStatus;
use aether_storage::{FileManager, WorkspaceConfig};
use aether_storage::system_info;
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
    ("notes", "Notes", "0.1.0", "/bin/aether-notes"),
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

fn surface_port() -> u16 {
    std::env::var("AETHER_SURFACE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4750)
}

fn surface_call(req: serde_json::Value) -> Result<serde_json::Value, String> {
    use std::io::{BufRead, BufReader, Write};
    let port = surface_port();
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| format!("surface :{port} {e}"))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let payload = serde_json::to_string(&req).map_err(|e| format!("encode {e}"))?;
    stream
        .write_all(format!("{payload}\n").as_bytes())
        .map_err(|e| format!("send {e}"))?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| format!("recv {e}"))?;
    if line.trim().is_empty() {
        return Err("empty surface response".to_string());
    }
    let v: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|e| format!("decode {e}"))?;
    if v["ok"].as_bool().unwrap_or(false) {
        Ok(v)
    } else {
        Err(v["error"].as_str().unwrap_or("surface error").to_string())
    }
}

fn build_context_snapshot(
    manager: &mut aether_system_core::manager::ServiceManager,
    apps: &mut ApplicationManager,
) -> serde_json::Value {
    let status_val = serde_json::to_value(manager.system_status()).unwrap_or(serde_json::json!({}));
    let services = status_val["services"].clone();
    let overall_health = status_val["overall_health"].as_str().unwrap_or("UNKNOWN").to_string();

    // Apps
    let installed: Vec<String> = apps.discover().iter().map(|d| d.id.clone()).collect();
    let mut app_states = serde_json::Map::new();
    let mut running_apps: Vec<String> = Vec::new();
    for id in &installed {
        let report = apps.app_state(id);
        app_states.insert(id.clone(), serde_json::json!(report.state));
        if report.state == "RUNNING" {
            running_apps.push(id.clone());
        }
    }

    // Windows via surface
    let windows_val = surface_call(serde_json::json!({ "op": "window.list" }))
        .ok()
        .and_then(|v| v["windows"].as_array().cloned())
        .unwrap_or_default();
    let mut windows = Vec::new();
    let mut minimized = Vec::new();
    let mut active_window: Option<String> = None;
    let mut focused_id: Option<u64> = None;
    for w in &windows_val {
        let state = w["state"].as_str().unwrap_or("normal").to_string();
        let title = w["title"].as_str().unwrap_or("?").to_string();
        let focused = w["focused"].as_bool().unwrap_or(false);
        if focused {
            active_window = Some(title.clone());
            focused_id = w["id"].as_u64();
        }
        if state == "minimized" {
            minimized.push(title.clone());
        }
        windows.push(serde_json::json!({
            "id": w["id"],
            "app": w["app"],
            "title": title,
            "state": state,
            "focused": focused,
        }));
    }
    // If running_apps empty from app states but windows show apps, union
    for w in &windows {
        if let Some(app) = w["app"].as_str() {
            if !running_apps.contains(&app.to_string()) {
                running_apps.push(app.to_string());
            }
        }
    }

    serde_json::json!({
        "active_window": active_window,
        "focused_window_id": focused_id,
        "windows": windows,
        "minimized_windows": minimized,
        "running_apps": running_apps,
        "installed_apps": installed,
        "app_states": app_states,
        "overall_health": overall_health,
        "services": services,
    })
}

fn dispatch(
    manager: &mut aether_system_core::manager::ServiceManager,
    apps: &mut ApplicationManager,
    files: &mut FileManager,
    started_at: SystemTime,
    req: &IpcRequest,
) -> IpcResponse {
    // Capability requests are audited with their arguments (never log file content)
    let is_capability = req.command.starts_with("app.")
        || req.command.starts_with("window.")
        || req.command.starts_with("file.")
        || req.command.starts_with("system.")
        || req.command == "context.get";
    let started_ok = true;
    let response = dispatch_inner(manager, apps, files, started_at, req);
    if is_capability {
        // For file capabilities, sanitize args to avoid logging content
        let mut sanitized = req.parameters.clone();
        if let Some(obj) = sanitized.as_object_mut() {
            if obj.contains_key("content") {
                obj.insert("content".to_string(), serde_json::json!("[REDACTED]"));
            }
        }
        audit(
            &req.command,
            &sanitized,
            &req.service_id,
            started_ok && response.ok,
        );
    }
    response
}

fn dispatch_inner(
    manager: &mut aether_system_core::manager::ServiceManager,
    apps: &mut ApplicationManager,
    files: &mut FileManager,
    started_at: SystemTime,
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
        "context.get" => {
            let ctx = build_context_snapshot(manager, apps);
            IpcResponse::ok("context.get", ctx)
        }
        "window.list" => {
            match surface_call(serde_json::json!({ "op": "window.list" })) {
                Ok(v) => IpcResponse::ok("window.list", v),
                Err(e) => IpcResponse::err(
                    "window.list",
                    IpcError {
                        code: "SURFACE_UNAVAILABLE".to_string(),
                        message: e,
                    },
                ),
            }
        }
        "window.focus" => {
            let app = req.parameters.get("app").and_then(|v| v.as_str());
            let wid = req.parameters.get("window_id").and_then(|v| v.as_u64());
            let target = if let Some(id) = wid {
                serde_json::json!({ "op": "window.focus", "window_id": id })
            } else if let Some(app_id) = app {
                // Resolve via window.list first to get id
                let list = surface_call(serde_json::json!({ "op": "window.list" }));
                if let Ok(v) = list {
                    if let Some(arr) = v["windows"].as_array() {
                        let mut found: Option<u64> = None;
                        for w in arr {
                            let a = w["app"].as_str().unwrap_or("").to_ascii_lowercase();
                            let t = w["title"].as_str().unwrap_or("").to_ascii_lowercase();
                            if a == app_id.to_ascii_lowercase() || t == app_id.to_ascii_lowercase() {
                                found = w["id"].as_u64();
                                break;
                            }
                        }
                        if let Some(id) = found {
                            serde_json::json!({ "op": "window.focus", "window_id": id })
                        } else {
                            return IpcResponse::err(
                                "window.focus",
                                IpcError {
                                    code: "NOT_FOUND".to_string(),
                                    message: format!("no window for '{app_id}'"),
                                },
                            );
                        }
                    } else {
                        return IpcResponse::err(
                            "window.focus",
                            IpcError {
                                code: "INTERNAL".to_string(),
                                message: "bad window.list shape".to_string(),
                            },
                        );
                    }
                } else {
                    return IpcResponse::err(
                        "window.focus",
                        IpcError {
                            code: "SURFACE_UNAVAILABLE".to_string(),
                            message: "surface unavailable".to_string(),
                        },
                    );
                }
            } else {
                return IpcResponse::err(
                    "window.focus",
                    IpcError {
                        code: "INVALID_INPUT".to_string(),
                        message: "'app' or 'window_id' required".to_string(),
                    },
                );
            };
            match surface_call(target) {
                Ok(v) => IpcResponse::ok("window.focus", v),
                Err(e) => IpcResponse::err(
                    "window.focus",
                    IpcError {
                        code: "SURFACE_ERROR".to_string(),
                        message: e,
                    },
                ),
            }
        }
        "window.minimize" => {
            let app = req.parameters.get("app").and_then(|v| v.as_str());
            let wid = req.parameters.get("window_id").and_then(|v| v.as_u64());
            let payload = if let Some(id) = wid {
                serde_json::json!({ "op": "window.minimize", "window_id": id })
            } else if let Some(app_id) = app {
                // resolve
                let list = surface_call(serde_json::json!({ "op": "window.list" }));
                if let Ok(v) = list {
                    let mut found: Option<u64> = None;
                    if let Some(arr) = v["windows"].as_array() {
                        for w in arr {
                            let a = w["app"].as_str().unwrap_or("").to_ascii_lowercase();
                            let t = w["title"].as_str().unwrap_or("").to_ascii_lowercase();
                            if a == app_id.to_ascii_lowercase() || t == app_id.to_ascii_lowercase() {
                                found = w["id"].as_u64();
                                break;
                            }
                        }
                    }
                    if let Some(id) = found {
                        serde_json::json!({ "op": "window.minimize", "window_id": id })
                    } else {
                        return IpcResponse::err(
                            "window.minimize",
                            IpcError {
                                code: "NOT_FOUND".to_string(),
                                message: format!("no window for '{app_id}'"),
                            },
                        );
                    }
                } else {
                    return IpcResponse::err(
                        "window.minimize",
                        IpcError {
                            code: "SURFACE_UNAVAILABLE".to_string(),
                            message: "surface unavailable".to_string(),
                        },
                    );
                }
            } else {
                return IpcResponse::err(
                    "window.minimize",
                    IpcError {
                        code: "INVALID_INPUT".to_string(),
                        message: "'app' or 'window_id' required".to_string(),
                    },
                );
            };
            match surface_call(payload) {
                Ok(v) => IpcResponse::ok("window.minimize", v),
                Err(e) => IpcResponse::err(
                    "window.minimize",
                    IpcError {
                        code: "SURFACE_ERROR".to_string(),
                        message: e,
                    },
                ),
            }
        }
        "window.maximize" => {
            let app = req.parameters.get("app").and_then(|v| v.as_str());
            let wid = req.parameters.get("window_id").and_then(|v| v.as_u64());
            let payload = if let Some(id) = wid {
                serde_json::json!({ "op": "window.maximize", "window_id": id })
            } else if let Some(app_id) = app {
                let list = surface_call(serde_json::json!({ "op": "window.list" }));
                if let Ok(v) = list {
                    let mut found: Option<u64> = None;
                    if let Some(arr) = v["windows"].as_array() {
                        for w in arr {
                            let a = w["app"].as_str().unwrap_or("").to_ascii_lowercase();
                            let t = w["title"].as_str().unwrap_or("").to_ascii_lowercase();
                            if a == app_id.to_ascii_lowercase() || t == app_id.to_ascii_lowercase() {
                                found = w["id"].as_u64();
                                break;
                            }
                        }
                    }
                    if let Some(id) = found {
                        serde_json::json!({ "op": "window.maximize", "window_id": id })
                    } else {
                        return IpcResponse::err(
                            "window.maximize",
                            IpcError {
                                code: "NOT_FOUND".to_string(),
                                message: format!("no window for '{app_id}'"),
                            },
                        );
                    }
                } else {
                    return IpcResponse::err(
                        "window.maximize",
                        IpcError {
                            code: "SURFACE_UNAVAILABLE".to_string(),
                            message: "surface unavailable".to_string(),
                        },
                    );
                }
            } else {
                return IpcResponse::err(
                    "window.maximize",
                    IpcError {
                        code: "INVALID_INPUT".to_string(),
                        message: "'app' or 'window_id' required".to_string(),
                    },
                );
            };
            match surface_call(payload) {
                Ok(v) => IpcResponse::ok("window.maximize", v),
                Err(e) => IpcResponse::err(
                    "window.maximize",
                    IpcError {
                        code: "SURFACE_ERROR".to_string(),
                        message: e,
                    },
                ),
            }
        }
        "window.close" => {
            let app = req.parameters.get("app").and_then(|v| v.as_str());
            let wid = req.parameters.get("window_id").and_then(|v| v.as_u64());
            let payload = if let Some(id) = wid {
                serde_json::json!({ "op": "window.close", "window_id": id })
            } else if let Some(app_id) = app {
                serde_json::json!({ "op": "window.close", "app_id": app_id })
            } else {
                return IpcResponse::err(
                    "window.close",
                    IpcError {
                        code: "INVALID_INPUT".to_string(),
                        message: "'app' or 'window_id' required".to_string(),
                    },
                );
            };
            match surface_call(payload) {
                Ok(v) => IpcResponse::ok("window.close", v),
                Err(e) => IpcResponse::err(
                    "window.close",
                    IpcError {
                        code: "SURFACE_ERROR".to_string(),
                        message: e,
                    },
                ),
            }
        }
        "window.restore" => {
            // Restore is focus (which un-minimizes)
            let app = req.parameters.get("app").and_then(|v| v.as_str()).unwrap_or("");
            let wid = req.parameters.get("window_id").and_then(|v| v.as_u64());
            let payload = if let Some(id) = wid {
                serde_json::json!({ "op": "window.focus", "window_id": id })
            } else {
                let list = surface_call(serde_json::json!({ "op": "window.list" }));
                if let Ok(v) = list {
                    let mut found: Option<u64> = None;
                    if let Some(arr) = v["windows"].as_array() {
                        for w in arr {
                            let a = w["app"].as_str().unwrap_or("").to_ascii_lowercase();
                            let t = w["title"].as_str().unwrap_or("").to_ascii_lowercase();
                            if a == app.to_ascii_lowercase() || t == app.to_ascii_lowercase() {
                                found = w["id"].as_u64();
                                break;
                            }
                        }
                    }
                    if let Some(id) = found {
                        serde_json::json!({ "op": "window.focus", "window_id": id })
                    } else {
                        return IpcResponse::err(
                            "window.restore",
                            IpcError {
                                code: "NOT_FOUND".to_string(),
                                message: format!("no window for '{app}'"),
                            },
                        );
                    }
                } else {
                    return IpcResponse::err(
                        "window.restore",
                        IpcError {
                            code: "SURFACE_UNAVAILABLE".to_string(),
                            message: "surface unavailable".to_string(),
                        },
                    );
                }
            };
            match surface_call(payload) {
                Ok(v) => IpcResponse::ok("window.restore", v),
                Err(e) => IpcResponse::err(
                    "window.restore",
                    IpcError {
                        code: "SURFACE_ERROR".to_string(),
                        message: e,
                    },
                ),
            }
        }
        "file.list" => {
            let path = req.parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
            // Check for confirmation if bulk? For now auto; path validation inside FileManager will handle
            match files.list(path) {
                Ok(entries) => {
                    // Return structured metadata limited to useful fields
                    IpcResponse::ok("file.list", serde_json::json!({ "path": path, "files": entries }))
                }
                Err(e) => IpcResponse::err("file.list", IpcError { code: e.code.to_string(), message: e.message }),
            }
        }
        "file.search" => {
            let query = req.parameters.get("query").and_then(|v| v.as_str()).unwrap_or("");
            match files.search(query) {
                Ok(results) => IpcResponse::ok("file.search", serde_json::json!({ "query": query, "results": results })),
                Err(e) => IpcResponse::err("file.search", IpcError { code: e.code.to_string(), message: e.message }),
            }
        }
        "file.read" => {
            let path = req.parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match files.read(path) {
                Ok(content) => {
                    let size = content.len() as u64;
                    IpcResponse::ok("file.read", serde_json::json!({ "path": path, "content": content, "size": size }))
                }
                Err(e) => IpcResponse::err("file.read", IpcError { code: e.code.to_string(), message: e.message }),
            }
        }
        "file.create" => {
            let path = req.parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = req.parameters.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match files.create(path, content) {
                Ok((rel, bytes)) => IpcResponse::ok("file.create", serde_json::json!({ "path": rel, "bytes_written": bytes })),
                Err(e) => {
                    // Map already exists to requires confirmation for overwrite scenario
                    if e.code == ErrorKind::InvalidInput && e.message.contains("already exists") {
                        IpcResponse::err("file.create", IpcError { code: "REQUIRES_CONFIRMATION".to_string(), message: format!("{} already exists; overwrite requires confirmation", path) })
                    } else {
                        IpcResponse::err("file.create", IpcError { code: e.code.to_string(), message: e.message })
                    }
                }
            }
        }
        "file.write" => {
            let path = req.parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = req.parameters.get("content").and_then(|v| v.as_str()).unwrap_or("");
            // For this phase, writing to existing file auto-executes; future could require confirmation
            match files.write(path, content) {
                Ok((rel, bytes)) => IpcResponse::ok("file.write", serde_json::json!({ "path": rel, "bytes_written": bytes })),
                Err(e) => IpcResponse::err("file.write", IpcError { code: e.code.to_string(), message: e.message }),
            }
        }
        "file.rename" => {
            let from = req.parameters.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = req.parameters.get("to").and_then(|v| v.as_str()).unwrap_or("");
            match files.rename(from, to) {
                Ok(rel) => IpcResponse::ok("file.rename", serde_json::json!({ "from": from, "to": rel })),
                Err(e) => {
                    if e.code == ErrorKind::InvalidInput && e.message.contains("already exists") {
                        IpcResponse::err("file.rename", IpcError { code: "REQUIRES_CONFIRMATION".to_string(), message: e.message })
                    } else {
                        IpcResponse::err("file.rename", IpcError { code: e.code.to_string(), message: e.message })
                    }
                }
            }
        }
        "file.move" => {
            let from = req.parameters.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = req.parameters.get("to").and_then(|v| v.as_str()).unwrap_or("");
            match files.move_file(from, to) {
                Ok(rel) => IpcResponse::ok("file.move", serde_json::json!({ "from": from, "to": rel })),
                Err(e) => {
                    if e.code == ErrorKind::InvalidInput && e.message.contains("already exists") {
                        IpcResponse::err("file.move", IpcError { code: "REQUIRES_CONFIRMATION".to_string(), message: e.message })
                    } else {
                        IpcResponse::err("file.move", IpcError { code: e.code.to_string(), message: e.message })
                    }
                }
            }
        }
        "file.delete" => {
            // Not implemented unrestricted; require confirmation
            IpcResponse::err("file.delete", IpcError { code: "REQUIRES_CONFIRMATION".to_string(), message: "delete requires explicit user confirmation".to_string() })
        }
        "system.info" => {
            let services_val = serde_json::to_value(manager.system_status()).ok().and_then(|v| v.get("services").cloned());
            let info = system_info::system_info(services_val);
            IpcResponse::ok("system.info", info)
        }
        "system.resources" => {
            let res = system_info::system_resources(Some(files.workspace_root()));
            IpcResponse::ok("system.resources", res)
        }
        "system.uptime" => {
            let up = system_info::system_uptime(Some(started_at));
            IpcResponse::ok("system.uptime", up)
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
    let mut files = match FileManager::new(WorkspaceConfig::from_env_or_default()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[system-core] workspace init failed: {e}");
            std::process::exit(1);
        }
    };
    let started_at = SystemTime::now();
    eprintln!(
        "[system-core] {} services running; {} app capabilities registered; workspace at {}; control plane on {bind_addr}:{port}",
        manager.graph().len(),
        apps.registered_count(),
        files.workspace_root().display()
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
                    dispatch(&mut manager, &mut apps, &mut files, started_at, &req)
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
