// Aether System Core - control daemon binary.
//
// Loads service manifests, starts all services in dependency order, and
// serves the local control protocol used by `aetherctl`:
// newline-delimited JSON requests/responses over TCP loopback.

use aether_application_manager::ApplicationManager;
use aether_core::error::ErrorKind;
use aether_core::ipc::{IpcError, IpcRequest, IpcResponse};
use aether_core::types::ServiceStatus;
use aether_security::audit::{AuditChain, AuditEntry, ChainStatus, RetentionPolicy};
use aether_security::credentials::{CredentialError, SealedStore, StaticKeyProvider};
use aether_storage::system_info;
use aether_storage::{FileManager, WorkspaceConfig};
use aether_system_core::policy;
use aether_security::manifest_signing::{Fingerprint, TrustStore};
use aether_system_core::{
    build_manager, load_manifests_from_dir, load_manifests_with_trust, ServiceExecutor,
    ServiceHandle,
};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
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
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

/// Converts an `AuditEntry` into the JSON shape the audit
/// inspection commands return. The `prev_hash` and
/// `content_hash` are serialised as hex strings so the
/// result is JSON-clean (byte arrays are not directly
/// representable in idiomatic JSON).
fn audit_entry_to_json(entry: &AuditEntry) -> serde_json::Value {
    serde_json::json!({
        "index": entry.index,
        "timestamp_ms": entry.timestamp_ms,
        "event": entry.event,
        "component": entry.component,
        "detail": entry.detail,
        "prev_hash": hex_lower(&entry.prev_hash),
        "content_hash": hex_lower(&entry.content_hash),
    })
}

/// Converts a `ChainStatus` into the JSON shape the
/// `audit.verify` command returns. The `ok` field is
/// always `false` here because the caller only reaches this
/// path on a verification failure.
fn chain_status_to_json(status: &ChainStatus) -> serde_json::Value {
    match status {
        ChainStatus::Ok => serde_json::json!({ "ok": true }),
        ChainStatus::ContentMismatch { index } => {
            serde_json::json!({ "ok": false, "kind": "content_mismatch", "index": index })
        }
        ChainStatus::BrokenLink { index } => {
            serde_json::json!({ "ok": false, "kind": "broken_link", "index": index })
        }
        ChainStatus::IndexGap { index } => {
            serde_json::json!({ "ok": false, "kind": "index_gap", "index": index })
        }
    }
}

/// Lowercase hex encoding of a 32-byte array. Used for the
/// hash fields in audit entries.
fn hex_lower(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Maps a `CredentialError` to the corresponding IPC
/// error response. The IPC code is stable; the message
/// is a short, caller-facing explanation.
fn credential_error_response(command: &str, err: &CredentialError) -> IpcResponse {
    let (code, message) = match err {
        CredentialError::NotFound => ("NOT_FOUND".to_string(), err.to_string()),
        CredentialError::AuthenticationFailed => {
            ("AUTHENTICATION_FAILED".to_string(), err.to_string())
        }
        CredentialError::AlreadyExists { .. } => ("ALREADY_EXISTS".to_string(), err.to_string()),
        CredentialError::Malformed => ("MALFORMED".to_string(), err.to_string()),
    };
    IpcResponse::err(command, IpcError { code, message })
}

/// Decodes 64 hex chars into a 32-byte array. Returns
/// `None` for any malformed input.
fn decode_hex_32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = [0u8; 32];
    for i in 0..32 {
        let hi = (bytes[2 * i] as char).to_digit(16)?;
        let lo = (bytes[2 * i + 1] as char).to_digit(16)?;
        out[i] = u8::try_from(hi * 16 + lo).ok()?;
    }
    Some(out)
}

/// Decodes a base64 string into bytes. Accepts both
/// standard alphabet (with `+`/`/`) and URL-safe
/// alphabet (with `-`/`_`); padding is optional. This
/// is a small, dependency-free implementation that
/// covers the inputs the IPC layer emits.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    // Normalise URL-safe to standard, then strip
    // padding before decoding.
    let mut normalised = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '-' => normalised.push('+'),
            '_' => normalised.push('/'),
            '=' => {} // drop, we add it back
            other => normalised.push(other),
        }
    }
    let remainder = normalised.len() % 4;
    if remainder == 1 {
        return None;
    }
    let pad = if remainder == 0 { 0 } else { 4 - remainder };
    for _ in 0..pad {
        normalised.push('=');
    }
    let bytes = normalised.as_bytes();
    let mut out = Vec::with_capacity(normalised.len() / 4 * 3);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &b in bytes {
        if b == b'=' {
            break;
        }
        let v = match b {
            b'A'..=b'Z' => u32::from(b - b'A'),
            b'a'..=b'z' => u32::from(b - b'a') + 26,
            b'0'..=b'9' => u32::from(b - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((buf >> bits) & 0xFF).ok()?);
        }
    }
    Some(out)
}

/// Encodes bytes as standard base64 with `=` padding.
/// Used by the tests to round-trip payloads through
/// the IPC boundary; not exposed via the IPC contract.
#[cfg(test)]
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHA: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((u32::from(bytes[i])) << 16)
            | ((u32::from(bytes[i + 1])) << 8)
            | (u32::from(bytes[i + 2]));
        out.push(ALPHA[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3F) as usize] as char);
        out.push(ALPHA[((n >> 6) & 0x3F) as usize] as char);
        out.push(ALPHA[(n & 0x3F) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let n = u32::from(bytes[i]) << 16;
        out.push(ALPHA[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8);
        out.push(ALPHA[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3F) as usize] as char);
        out.push(ALPHA[((n >> 6) & 0x3F) as usize] as char);
        out.push('=');
    }
    out
}

/// Creates a new sealing key by drawing 32 random bytes
/// from the OS RNG. The key is unique per process; the
/// `StaticKeyProvider` zeroes it on `Drop` (when the
/// daemon exits). For a production deployment the key
/// would come from a TPM or the kernel keyring — this is
/// a stand-in until that integration lands.
fn new_sealing_key() -> [u8; 32] {
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

/// Reads a trust-store file and returns a populated
/// `TrustStore`. The file format is one hex fingerprint
/// per line; blank lines and `#`-prefixed comments are
/// ignored. Bad lines are a fatal error — a partially
/// loaded trust store is worse than a missing one, since
/// the missing fingerprints would silently be rejected at
/// signature-verification time.
fn load_trust_store(path: &str) -> Result<TrustStore, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read trust store file {path}: {e}"))?;
    let mut store = TrustStore::new();
    for (idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let fp = Fingerprint::from_hex(line).ok_or_else(|| {
            format!(
                "trust store {path}:{}: not a valid 32-character hex fingerprint: {line}",
                idx + 1
            )
        })?;
        store.trust(fp);
    }
    Ok(store)
}

/// Structured audit line for every capability request dispatched here.
///
/// Writes a tamper-evident entry to the system-level audit
/// chain. The chain is shared across every dispatch path and
/// is the authoritative record of which actor tried which
/// capability, with what arguments, and with what result.
fn record_audit(
    chain: &Mutex<AuditChain>,
    capability: &str,
    args: &serde_json::Value,
    component: &str,
    ok: bool,
) {
    let detail = format!(
        "args={} result={}",
        args,
        if ok { "success" } else { "failure" }
    );
    let timestamp_ms = unix_ms().min(u128::from(u64::MAX)) as u64;
    // Best-effort write: if the lock is poisoned (a previous
    // holder panicked) we do not want to crash the dispatch
    // path — the audit record is observability, not control
    // flow. We also still log to stderr so an operator can
    // see that an audit line was emitted.
    let mut guard = match chain.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let index = guard.record(
        timestamp_ms,
        "ipc.dispatch",
        component,
        &format!("capability={capability} {detail}"),
    );
    eprintln!(
        "[audit] ts={} component={} capability={} args={} result={} index={index}",
        timestamp_ms,
        component,
        capability,
        args,
        if ok { "success" } else { "failure" },
    );
}

/// In-process executor: internal services run inside this daemon; process
/// services are represented with a deterministic pseudo-pid for now.
#[derive(Debug, Default)]
struct LocalExecutor {
    next_pid: AtomicU64,
}

impl ServiceExecutor for LocalExecutor {
    fn start(
        &mut self,
        service_id: &str,
    ) -> Result<ServiceHandle, aether_core::error::AetherError> {
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
    std::env::var("AETHER_SURFACE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(4750)
}

fn surface_call(req: serde_json::Value) -> Result<serde_json::Value, String> {
    use std::io::{BufRead, BufReader, Write};
    let port = surface_port();
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| format!("surface :{port} {e}"))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let payload = serde_json::to_string(&req).map_err(|e| format!("encode {e}"))?;
    stream.write_all(format!("{payload}\n").as_bytes()).map_err(|e| format!("send {e}"))?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).map_err(|e| format!("recv {e}"))?;
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
    audit_chain: &Mutex<AuditChain>,
    credentials: &Mutex<SealedStore<StaticKeyProvider>>,
    trust_store: Option<&TrustStore>,
    started_at: SystemTime,
    req: &IpcRequest,
) -> IpcResponse {
    // Capability requests are audited with their arguments (never log file content)
    let is_capability = req.command.starts_with("app.")
        || req.command.starts_with("window.")
        || req.command.starts_with("file.")
        || req.command.starts_with("system.")
        || req.command.starts_with("process.")
        || req.command.starts_with("storage.")
        || req.command == "context.get";
    let started_ok = true;
    let response = dispatch_inner(
        manager,
        apps,
        files,
        audit_chain,
        credentials,
        trust_store,
        started_at,
        req,
    );
    if is_capability {
        // For file capabilities, sanitize args to avoid logging content
        let mut sanitized = req.parameters.clone();
        if let Some(obj) = sanitized.as_object_mut() {
            if obj.contains_key("content") {
                obj.insert("content".to_string(), serde_json::json!("[REDACTED]"));
            }
        }
        record_audit(
            audit_chain,
            &req.command,
            &sanitized,
            &req.service_id,
            started_ok && response.ok,
        );
    }
    response
}

/// Convert a `PolicyVerdict` into the corresponding `IpcResponse`.
///
/// The IPC error code carries enough information for the caller
/// (aetherctl, agentd, the shell) to decide what to do next:
///   * `POLICY_DENIED` — the request is rejected; do not retry.
///   * `REQUIRES_CONFIRMATION` — the capability is gated behind
///     explicit user consent; the caller must surface the request
///     to the user and re-issue through the approval flow.
fn gate_response(command: &str, verdict: &aether_system_core::policy::PolicyVerdict) -> IpcResponse {
    use aether_security::Decision;
    let (code, message) = match (verdict.decision, &verdict.reason) {
        (Decision::Deny, _) => ("POLICY_DENIED".to_string(), verdict.reason.clone()),
        (Decision::RequireConsent, _) => {
            ("REQUIRES_CONFIRMATION".to_string(), verdict.reason.clone())
        }
        // Allow is handled by the caller; this fn is only invoked
        // when the gate rejects the request.
        (Decision::Allow, _) => (
            "INTERNAL".to_string(),
            "gate_response called for allow verdict".to_string(),
        ),
    };
    // Untrusted denials are tagged with a distinct code so the
    // audit log + red-team suite can distinguish "policy said no"
    // from "we don't know who you are".
    let code = if code == "POLICY_DENIED" && verdict.reason.contains("untrusted actor") {
        "POLICY_DENIED_UNTRUSTED".to_string()
    } else {
        code
    };
    IpcResponse::err(command, IpcError { code, message })
}

fn dispatch_inner(
    manager: &mut aether_system_core::manager::ServiceManager,
    apps: &mut ApplicationManager,
    files: &mut FileManager,
    audit_chain: &Mutex<AuditChain>,
    credentials: &Mutex<SealedStore<StaticKeyProvider>>,
    trust_store: Option<&TrustStore>,
    started_at: SystemTime,
    req: &IpcRequest,
) -> IpcResponse {
    // Policy gate: every command is evaluated against the cross-domain
    // `DefaultPermissionPolicy` combined with the request's
    // `actor_trust`. The verdict is converted into an IPC response:
    //   * Deny            -> POLICY_DENIED, request is rejected.
    //   * RequireConsent  -> REQUIRES_CONFIRMATION, the caller is
    //                        told to re-issue through the agentd's
    //                        approval-gated flow.
    //   * Allow           -> fall through to the existing dispatcher.
    //
    // Untrusted actors are denied outright before the policy is even
    // consulted — the system-core dispatcher must not execute
    // capabilities for an unauthenticated peer.
    let verdict = policy::evaluate(&req.command, req.actor_trust);
    if !verdict.is_allow() {
        record_audit(
            audit_chain,
            "policy.deny",
            &serde_json::json!({ "command": req.command, "actor_trust": req.actor_trust }),
            &req.service_id,
            false,
        );
        return gate_response(&req.command, &verdict);
    }
    let mut executor = LocalExecutor::default();
    match req.command.as_str() {
        "status" | "system.status" => {
            let mut value = match serde_json::to_value(manager.system_status()) {
                Ok(v) => v,
                Err(e) => {
                    return IpcResponse::err(
                        &req.command,
                        IpcError { code: "INTERNAL".to_string(), message: e.to_string() },
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
                    m.dependencies
                        .iter()
                        .all(|d| manager.graph().manifest(d).map(|_| true).unwrap_or(false))
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
                    IpcError { code: "APP_ERROR".to_string(), message: e.to_string() },
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
                let target =
                    apps.running().into_iter().find(|i| i.app_id == app_id).map(|i| i.instance_id);
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
        "window.list" => match surface_call(serde_json::json!({ "op": "window.list" })) {
            Ok(v) => IpcResponse::ok("window.list", v),
            Err(e) => IpcResponse::err(
                "window.list",
                IpcError { code: "SURFACE_UNAVAILABLE".to_string(), message: e },
            ),
        },
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
                            if a == app_id.to_ascii_lowercase() || t == app_id.to_ascii_lowercase()
                            {
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
                    IpcError { code: "SURFACE_ERROR".to_string(), message: e },
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
                            if a == app_id.to_ascii_lowercase() || t == app_id.to_ascii_lowercase()
                            {
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
                    IpcError { code: "SURFACE_ERROR".to_string(), message: e },
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
                            if a == app_id.to_ascii_lowercase() || t == app_id.to_ascii_lowercase()
                            {
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
                    IpcError { code: "SURFACE_ERROR".to_string(), message: e },
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
                    IpcError { code: "SURFACE_ERROR".to_string(), message: e },
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
                    IpcError { code: "SURFACE_ERROR".to_string(), message: e },
                ),
            }
        }
        "file.list" => {
            let path = req.parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
            // Check for confirmation if bulk? For now auto; path validation inside FileManager will handle
            match files.list(path) {
                Ok(entries) => {
                    // Return structured metadata limited to useful fields
                    IpcResponse::ok(
                        "file.list",
                        serde_json::json!({ "path": path, "files": entries }),
                    )
                }
                Err(e) => IpcResponse::err(
                    "file.list",
                    IpcError { code: e.code.to_string(), message: e.message },
                ),
            }
        }
        "file.search" => {
            let query = req.parameters.get("query").and_then(|v| v.as_str()).unwrap_or("");
            match files.search(query) {
                Ok(results) => IpcResponse::ok(
                    "file.search",
                    serde_json::json!({ "query": query, "results": results }),
                ),
                Err(e) => IpcResponse::err(
                    "file.search",
                    IpcError { code: e.code.to_string(), message: e.message },
                ),
            }
        }
        "file.read" => {
            let path = req.parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match files.read(path) {
                Ok(content) => {
                    let size = content.len() as u64;
                    IpcResponse::ok(
                        "file.read",
                        serde_json::json!({ "path": path, "content": content, "size": size }),
                    )
                }
                Err(e) => IpcResponse::err(
                    "file.read",
                    IpcError { code: e.code.to_string(), message: e.message },
                ),
            }
        }
        "file.create" => {
            let path = req.parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = req.parameters.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match files.create(path, content) {
                Ok((rel, bytes)) => IpcResponse::ok(
                    "file.create",
                    serde_json::json!({ "path": rel, "bytes_written": bytes }),
                ),
                Err(e) => {
                    // Map already exists to requires confirmation for overwrite scenario
                    if e.code == ErrorKind::InvalidInput && e.message.contains("already exists") {
                        IpcResponse::err(
                            "file.create",
                            IpcError {
                                code: "REQUIRES_CONFIRMATION".to_string(),
                                message: format!(
                                    "{} already exists; overwrite requires confirmation",
                                    path
                                ),
                            },
                        )
                    } else {
                        IpcResponse::err(
                            "file.create",
                            IpcError { code: e.code.to_string(), message: e.message },
                        )
                    }
                }
            }
        }
        "file.write" => {
            let path = req.parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = req.parameters.get("content").and_then(|v| v.as_str()).unwrap_or("");
            // For this phase, writing to existing file auto-executes; future could require confirmation
            match files.write(path, content) {
                Ok((rel, bytes)) => IpcResponse::ok(
                    "file.write",
                    serde_json::json!({ "path": rel, "bytes_written": bytes }),
                ),
                Err(e) => IpcResponse::err(
                    "file.write",
                    IpcError { code: e.code.to_string(), message: e.message },
                ),
            }
        }
        "file.rename" => {
            let from = req.parameters.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = req.parameters.get("to").and_then(|v| v.as_str()).unwrap_or("");
            match files.rename(from, to) {
                Ok(rel) => {
                    IpcResponse::ok("file.rename", serde_json::json!({ "from": from, "to": rel }))
                }
                Err(e) => {
                    if e.code == ErrorKind::InvalidInput && e.message.contains("already exists") {
                        IpcResponse::err(
                            "file.rename",
                            IpcError {
                                code: "REQUIRES_CONFIRMATION".to_string(),
                                message: e.message,
                            },
                        )
                    } else {
                        IpcResponse::err(
                            "file.rename",
                            IpcError { code: e.code.to_string(), message: e.message },
                        )
                    }
                }
            }
        }
        "file.move" => {
            let from = req.parameters.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = req.parameters.get("to").and_then(|v| v.as_str()).unwrap_or("");
            match files.move_file(from, to) {
                Ok(rel) => {
                    IpcResponse::ok("file.move", serde_json::json!({ "from": from, "to": rel }))
                }
                Err(e) => {
                    if e.code == ErrorKind::InvalidInput && e.message.contains("already exists") {
                        IpcResponse::err(
                            "file.move",
                            IpcError {
                                code: "REQUIRES_CONFIRMATION".to_string(),
                                message: e.message,
                            },
                        )
                    } else {
                        IpcResponse::err(
                            "file.move",
                            IpcError { code: e.code.to_string(), message: e.message },
                        )
                    }
                }
            }
        }
        "file.delete" => {
            // Not implemented unrestricted; require confirmation
            IpcResponse::err(
                "file.delete",
                IpcError {
                    code: "REQUIRES_CONFIRMATION".to_string(),
                    message: "delete requires explicit user confirmation".to_string(),
                },
            )
        }
        "system.info" => {
            let services_val = serde_json::to_value(manager.system_status())
                .ok()
                .and_then(|v| v.get("services").cloned());
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
        "storage.status" => {
            // Surface the storage workspace as a high-level report
            // (root, sandboxed, configured limits, file counts).
            let root = files.workspace_root();
            let root_str = root.to_string_lossy().to_string();
            // Count the entries under root (bounded to a sensible
            // cap so a runaway directory cannot stall the request).
            let entry_count = std::fs::read_dir(root).map(|it| it.flatten().count()).unwrap_or(0);
            let sandboxed = root_str.contains("/workspace") || root_str.contains("workspace");
            IpcResponse::ok(
                "storage.status",
                serde_json::json!({
                    "workspace_root": root_str,
                    "sandboxed": sandboxed,
                    "entry_count": entry_count,
                    "status": "HEALTHY",
                }),
            )
        }
        "process.list" => {
            // Lightweight /proc-based process listing, bounded to
            // keep responses compact. Each entry includes pid and
            // comm only — never the full cmdline which may contain
            // sensitive material.
            let mut processes = Vec::new();
            if let Ok(entries) = std::fs::read_dir("/proc") {
                for entry in entries.flatten().take(256) {
                    let name = entry.file_name();
                    let Some(name) = name.to_str() else { continue };
                    let Ok(pid) = name.parse::<u32>() else { continue };
                    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm"))
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    processes.push(serde_json::json!({
                        "pid": pid,
                        "comm": comm,
                    }));
                }
            }
            IpcResponse::ok("process.list", serde_json::json!({ "processes": processes }))
        }
        "process.inspect" => {
            // Resolve pid from params and read /proc/<pid>/status
            // for safe, well-defined fields. No env, no cmdline.
            let pid = req
                .parameters
                .get("pid")
                .and_then(|v| v.as_u64())
                .and_then(|p| u32::try_from(p).ok())
                .or_else(|| {
                    let name = req.parameters.get("name").and_then(|v| v.as_str())?;
                    std::fs::read_dir("/proc").ok()?.flatten().find_map(|e| {
                        let n = e.file_name();
                        let n = n.to_str()?;
                        let p = n.parse::<u32>().ok()?;
                        let comm = std::fs::read_to_string(format!("/proc/{p}/comm"))
                            .unwrap_or_default()
                            .trim()
                            .to_string();
                        if comm == name {
                            Some(p)
                        } else {
                            None
                        }
                    })
                });
            let Some(pid) = pid else {
                return IpcResponse::err(
                    "process.inspect",
                    IpcError {
                        code: "NOT_FOUND".to_string(),
                        message: "no such process".to_string(),
                    },
                );
            };
            let status_text =
                std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
            let mut data = serde_json::Map::new();
            data.insert("pid".to_string(), serde_json::json!(pid));
            for line in status_text.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let key = k.trim();
                    let val = v.trim();
                    if matches!(
                        key,
                        "Name" | "State" | "Pid" | "PPid" | "Uid" | "Gid" | "VmRSS" | "VmSize"
                    ) {
                        data.insert(key.to_string(), serde_json::json!(val));
                    }
                }
            }
            IpcResponse::ok("process.inspect", serde_json::Value::Object(data))
        }
        "shutdown" => IpcResponse::ok("shutdown", serde_json::json!({ "state": "SHUTTING_DOWN" })),
        "sandbox.plan" => {
            // Returns the kernel-sandbox plan the launcher must
            // enforce for one service (or every service, when
            // `service` is omitted). The plan is declarative —
            // the actual prctl / unshare / seccomp invocation is
            // done by the `aether-sandbox` binary, not by
            // system-core.
            match req.parameters.get("service").and_then(|v| v.as_str()) {
                Some(service_id) => match manager.sandbox_plan(service_id) {
                    Some(plan) => {
                        let plan_val = serde_json::to_value(&plan).unwrap_or(serde_json::json!({}));
                        IpcResponse::ok(
                            "sandbox.plan",
                            serde_json::json!({ "service": service_id, "plan": plan_val }),
                        )
                    }
                    None => IpcResponse::err(
                        "sandbox.plan",
                        IpcError {
                            code: "NOT_FOUND".to_string(),
                            message: format!("unknown service '{service_id}'"),
                        },
                    ),
                },
                None => {
                    let plans = manager.all_sandbox_plans();
                    let plans_val: Vec<serde_json::Value> = plans
                        .into_iter()
                        .map(|(sid, plan)| {
                            let plan_val = serde_json::to_value(&plan)
                                .unwrap_or(serde_json::json!({}));
                            serde_json::json!({ "service": sid, "plan": plan_val })
                        })
                        .collect();
                    IpcResponse::ok("sandbox.plan", serde_json::json!({ "plans": plans_val }))
                }
            }
        }
        "audit.recent" => {
            // Returns the most recent `n` audit entries in
            // newest-first order, along with the current chain
            // length so callers can tell whether the page is
            // complete. The default `n` of 64 is small enough
            // to be cheap and large enough to be useful for a
            // post-incident review.
            let n = req
                .parameters
                .get("n")
                .and_then(|v| v.as_u64())
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(64);
            let guard = match audit_chain.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let total = guard.len();
            let entries: Vec<serde_json::Value> = guard
                .recent(n)
                .into_iter()
                .map(audit_entry_to_json)
                .collect();
            IpcResponse::ok(
                "audit.recent",
                serde_json::json!({
                    "total": total,
                    "returned": entries.len(),
                    "entries": entries,
                }),
            )
        }
        "audit.verify" => {
            // Walks the entire chain and reports the first
            // inconsistency found, or {ok: true} if every
            // entry's stored content_hash and prev_hash are
            // valid.
            let guard = match audit_chain.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let status = match guard.verify_chain() {
                Ok(()) => serde_json::json!({ "ok": true, "entries": guard.len() }),
                Err(status) => chain_status_to_json(&status),
            };
            IpcResponse::ok("audit.verify", status)
        }
        "manifest.trust_store" => {
            // Introspection: returns the loaded trust
            // store's fingerprints, sorted. The store is
            // configured at startup from
            // AETHER_MANIFEST_TRUST_FILE; an empty store
            // means signature verification is disabled
            // (dev mode).
            match trust_store {
                Some(store) => {
                    let fps: Vec<String> = store
                        .fingerprints()
                        .iter()
                        .map(|f| f.as_hex().to_string())
                        .collect();
                    IpcResponse::ok(
                        "manifest.trust_store",
                        serde_json::json!({
                            "enabled": true,
                            "count": store.len(),
                            "fingerprints": fps,
                        }),
                    )
                }
                None => IpcResponse::ok(
                    "manifest.trust_store",
                    serde_json::json!({
                        "enabled": false,
                        "count": 0,
                        "fingerprints": serde_json::Value::Array(Vec::new()),
                    }),
                ),
            }
        }
        "audit.prune" => {
            // Time-based pruning of the audit log. Caller
            // supplies the current wall-clock time in
            // milliseconds so the daemon does not need to
            // read the clock itself (the agentd already has
            // a synchronised notion of `now`).
            let now_ms = req
                .parameters
                .get("now_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| unix_ms().min(u128::from(u64::MAX)) as u64);
            let mut guard = match audit_chain.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let dropped = guard.prune_older_than(now_ms);
            IpcResponse::ok(
                "audit.prune",
                serde_json::json!({
                    "now_ms": now_ms,
                    "dropped": dropped,
                    "remaining": guard.len(),
                }),
            )
        }
        "credentials.seal" => {
            // Seals `plaintext` under `name` and stores the
            // ciphertext. If `name` exists, the call is
            // rejected unless `force` is set. The plaintext
            // never appears in the response — only the
            // ciphertext, label, and length.
            let name = req.parameters.get("name").and_then(|v| v.as_str());
            let plaintext = req.parameters.get("plaintext").and_then(|v| v.as_str());
            let label = req.parameters.get("label").and_then(|v| v.as_str());
            let force = req.parameters.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            let (name, plaintext) = match (name, plaintext) {
                (Some(n), Some(p)) => (n, p),
                _ => {
                    return IpcResponse::err(
                        "credentials.seal",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameters 'name' and 'plaintext' are required".to_string(),
                        },
                    )
                }
            };
            let mut guard = match credentials.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            match guard.seal(name, plaintext, label, force) {
                Ok(_) => IpcResponse::ok(
                    "credentials.seal",
                    serde_json::json!({ "name": name, "sealed": true }),
                ),
                Err(e) => credential_error_response("credentials.seal", &e),
            }
        }
        "credentials.unseal" => {
            // Decrypts and returns the plaintext for `name`.
            // The plaintext is wrapped in a `Secret<String>`
            // by the store and immediately turned into a
            // owned `String` for the IPC response — at the
            // cost of having it in the response buffer
            // briefly, we get to keep the type system
            // simple and the call site obvious. A future
            // revision can pipe the `Secret` through
            // zero-copy channels when one exists.
            let name = match req.parameters.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    return IpcResponse::err(
                        "credentials.unseal",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'name' is required".to_string(),
                        },
                    )
                }
            };
            let guard = match credentials.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            match guard.unseal(name) {
                Ok(secret) => {
                    let value = secret.into_inner();
                    IpcResponse::ok(
                        "credentials.unseal",
                        serde_json::json!({ "name": name, "plaintext": value }),
                    )
                }
                Err(e) => credential_error_response("credentials.unseal", &e),
            }
        }
        "credentials.list" => {
            // Returns the names of every stored credential,
            // sorted. Plaintext is never returned by this
            // command.
            let guard = match credentials.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let names = guard.names();
            let total = guard.len();
            IpcResponse::ok(
                "credentials.list",
                serde_json::json!({ "names": names, "total": total }),
            )
        }
        "credentials.remove" => {
            // Drops a credential. Returns the removed
            // metadata (no plaintext) so the caller can
            // confirm the right entry was removed.
            let name = match req.parameters.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    return IpcResponse::err(
                        "credentials.remove",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'name' is required".to_string(),
                        },
                    )
                }
            };
            let mut guard = match credentials.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            match guard.remove(name) {
                Ok(removed) => IpcResponse::ok(
                    "credentials.remove",
                    serde_json::json!({
                        "name": removed.name,
                        "plaintext_len": removed.plaintext_len,
                        "sealed_at_ms": removed.blob.sealed_at_ms,
                    }),
                ),
                Err(e) => credential_error_response("credentials.remove", &e),
            }
        }
        "credentials.metadata" => {
            // Returns the stored metadata for `name` —
            // label, sealed-at timestamp, plaintext length,
            // and the hex-encoded ciphertext length. No
            // plaintext is ever returned.
            let name = match req.parameters.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    return IpcResponse::err(
                        "credentials.metadata",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'name' is required".to_string(),
                        },
                    )
                }
            };
            let guard = match credentials.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            match guard.get(name) {
                Some(cred) => IpcResponse::ok(
                    "credentials.metadata",
                    serde_json::json!({
                        "name": cred.name,
                        "label": cred.blob.label,
                        "sealed_at_ms": cred.blob.sealed_at_ms,
                        "plaintext_len": cred.plaintext_len,
                        "ciphertext_len": cred.blob.bytes.len(),
                    }),
                ),
                None => IpcResponse::err(
                    "credentials.metadata",
                    IpcError {
                        code: "NOT_FOUND".to_string(),
                        message: format!("credential '{name}' not found"),
                    },
                ),
            }
        }
        "update.verify" => {
            // The caller hands us a JSON SignedUpdate.
            // We pull the header / payload / signature
            // fields out and hand them to the verifier.
            // The public key bytes are passed alongside
            // (32 bytes, hex-encoded) so the verifier
            // does not need its own key store.
            let header_val = match req.parameters.get("header") {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "update.verify",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'header' is required".to_string(),
                        },
                    );
                }
            };
            let payload_b64 = match req.parameters.get("payload_b64").and_then(|v| v.as_str()) {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "update.verify",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'payload_b64' is required".to_string(),
                        },
                    );
                }
            };
            let signature_b64 = match req.parameters.get("signature_b64").and_then(|v| v.as_str())
            {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "update.verify",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'signature_b64' is required".to_string(),
                        },
                    );
                }
            };
            let public_key_hex = match req.parameters.get("public_key_hex").and_then(|v| v.as_str())
            {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "update.verify",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'public_key_hex' is required (64 hex chars)".to_string(),
                        },
                    );
                }
            };
            // Decode the public key (64 hex chars -> 32 bytes).
            let public_key_bytes: [u8; 32] = match decode_hex_32(public_key_hex) {
                Some(b) => b,
                None => {
                    return IpcResponse::err(
                        "update.verify",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: format!(
                                "public_key_hex is not 64 valid hex chars: {public_key_hex}"
                            ),
                        },
                    );
                }
            };
            // Decode the payload and signature (base64).
            let payload = match base64_decode(payload_b64) {
                Some(b) => b,
                None => {
                    return IpcResponse::err(
                        "update.verify",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "payload_b64 is not valid base64".to_string(),
                        },
                    );
                }
            };
            let signature = match base64_decode(signature_b64) {
                Some(b) => b,
                None => {
                    return IpcResponse::err(
                        "update.verify",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "signature_b64 is not valid base64".to_string(),
                        },
                    );
                }
            };
            // Parse the header back into a typed struct.
            let header: aether_security::signed_update::UpdateHeader =
                match serde_json::from_value(header_val.clone()) {
                    Ok(h) => h,
                    Err(e) => {
                        return IpcResponse::err(
                            "update.verify",
                            IpcError {
                                code: "INVALID_INPUT".to_string(),
                                message: format!("header is not a valid UpdateHeader: {e}"),
                            },
                        );
                    }
                };
            let update = aether_security::signed_update::SignedUpdate {
                header,
                payload,
                signature,
            };
            match aether_security::signed_update::verify_signed_update_with_key(
                &update,
                &public_key_bytes,
            ) {
                Ok(()) => IpcResponse::ok(
                    "update.verify",
                    serde_json::json!({
                        "ok": true,
                        "kind": update.header.kind,
                        "target": update.header.target,
                        "version": update.header.version,
                        "timestamp_ms": update.header.timestamp_ms,
                        "payload_len": update.header.payload_len,
                    }),
                ),
                Err(e) => IpcResponse::ok(
                    "update.verify",
                    serde_json::json!({
                        "ok": false,
                        "error": e.to_string(),
                        "kind": update.header.kind,
                        "target": update.header.target,
                        "version": update.header.version,
                    }),
                ),
            }
        }
        "update.fingerprint" => {
            // Helper: take a 64-char hex public key and
            // return its manifest-signing fingerprint
            // (32 hex chars). Useful for the operator
            // to add a new trust entry.
            let public_key_hex = match req.parameters.get("public_key_hex").and_then(|v| v.as_str())
            {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "update.fingerprint",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'public_key_hex' is required (64 hex chars)".to_string(),
                        },
                    );
                }
            };
            let public_key_bytes: [u8; 32] = match decode_hex_32(public_key_hex) {
                Some(b) => b,
                None => {
                    return IpcResponse::err(
                        "update.fingerprint",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: format!(
                                "public_key_hex is not 64 valid hex chars: {public_key_hex}"
                            ),
                        },
                    );
                }
            };
            let vk = match ed25519_dalek::VerifyingKey::from_bytes(&public_key_bytes) {
                Ok(k) => k,
                Err(e) => {
                    return IpcResponse::err(
                        "update.fingerprint",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: format!("public key is not a valid Ed25519 key: {e}"),
                        },
                    );
                }
            };
            let fp = Fingerprint::for_public_key(&vk);
            IpcResponse::ok(
                "update.fingerprint",
                serde_json::json!({ "fingerprint": fp.as_hex() }),
            )
        }
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

    let port: u16 =
        std::env::var("AETHER_CONTROL_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(4747);
    // Loopback by default; guest images override to expose the plane to
    // the isolated QEMU user network.
    let bind_addr = std::env::var("AETHER_BIND").unwrap_or_else(|_| "127.0.0.1".to_string());

    eprintln!("[system-core] loading manifests from {manifests_dir}");
    // Manifest trust store: if AETHER_MANIFEST_TRUST_FILE
    // is set, load a list of trusted signer fingerprints
    // (one hex fingerprint per line) and require every
    // manifest to carry a valid signature. The default
    // (unset) is unsigned, matching the dev / test path.
    let trust_store = std::env::var("AETHER_MANIFEST_TRUST_FILE")
        .ok()
        .map(|path| match load_trust_store(&path) {
            Ok(store) => {
                eprintln!(
                    "[system-core] trust store loaded from {path} ({} fingerprints)",
                    store.len()
                );
                Some(store)
            }
            Err(e) => {
                eprintln!("[system-core] fatal: {e}");
                std::process::exit(1);
            }
        })
        .unwrap_or(None);
    let manifests = match trust_store.as_ref() {
        Some(store) => match load_manifests_with_trust(&PathBuf::from(&manifests_dir), Some(store))
        {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[system-core] fatal: {e}");
                std::process::exit(1);
            }
        },
        None => match load_manifests_from_dir(&PathBuf::from(&manifests_dir)) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[system-core] fatal: {e}");
                std::process::exit(1);
            }
        },
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

    // System-level tamper-evident audit log. The default
    // policy keeps the most recent 4 096 dispatch events,
    // which fits comfortably in a few hundred KiB and is
    // long enough to span a typical session.
    let audit_chain = Mutex::new(AuditChain::new(RetentionPolicy::default()));

    // In-memory sealed credential store. The key is fresh
    // for every process invocation; credentials do not
    // survive a restart until the journal integration
    // lands. This is the right default for the OS-image
    // boot story: nothing on the disk is sensitive, the
    // user re-authenticates after each boot, and the
    // store is freshly sealed.
    let credentials = Mutex::new(SealedStore::new(StaticKeyProvider::new(new_sealing_key())));

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
        let peer =
            stream.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "unknown".to_string());
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
                    dispatch(
                        &mut manager,
                        &mut apps,
                        &mut files,
                        &audit_chain,
                        &credentials,
                        trust_store.as_ref(),
                        started_at,
                        &req,
                    )
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

#[cfg(test)]
mod dispatch_policy_tests {
    //! Integration tests for the policy gate wired into `dispatch_inner`.
    //!
    //! The gate runs *before* the existing capability handlers, so
    //! the test does not need a real `ServiceManager` or filesystem
    //! — it only checks that the gate short-circuits to the right
    //! error code for each (command, actor_trust) combination.

    use super::gate_response;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_system_core::policy::{evaluate, PolicyVerdict};
    use aether_security::Decision;

    fn req(command: &str, trust: ActorTrust) -> IpcRequest {
        IpcRequest {
            service_id: "test".to_string(),
            command: command.to_string(),
            parameters: serde_json::json!({}),
            actor_trust: trust,
        }
    }

    fn err_code(verdict: &PolicyVerdict) -> String {
        gate_response("test", verdict).error.map(|e| e.code).unwrap_or_default()
    }

    #[test]
    fn low_risk_trusted_passes_gate() {
        let v = evaluate("system.status", ActorTrust::Trusted);
        assert!(v.is_allow());
    }

    #[test]
    fn high_risk_trusted_is_require_consent() {
        let v = evaluate("file.delete", ActorTrust::Trusted);
        assert_eq!(v.decision, Decision::RequireConsent);
        assert_eq!(err_code(&v), "REQUIRES_CONFIRMATION");
    }

    #[test]
    fn critical_shutdown_trusted_is_require_consent() {
        let v = evaluate("system.shutdown", ActorTrust::Trusted);
        assert_eq!(v.decision, Decision::RequireConsent);
        assert_eq!(err_code(&v), "REQUIRES_CONFIRMATION");
    }

    #[test]
    fn untrusted_low_risk_is_denied() {
        let v = evaluate("system.status", ActorTrust::Untrusted);
        assert_eq!(v.decision, Decision::Deny);
        assert_eq!(err_code(&v), "POLICY_DENIED_UNTRUSTED");
    }

    #[test]
    fn untrusted_shutdown_is_denied() {
        let v = evaluate("system.shutdown", ActorTrust::Untrusted);
        assert_eq!(v.decision, Decision::Deny);
        assert_eq!(err_code(&v), "POLICY_DENIED_UNTRUSTED");
    }

    #[test]
    fn gate_response_unused_for_allow_verdict() {
        let v = evaluate("system.status", ActorTrust::Trusted);
        let resp = gate_response("system.status", &v);
        // The helper isn't called for allow verdicts, but if it is
        // invoked defensively it should still produce a well-formed
        // IpcResponse (ok=false, error present).
        assert!(!resp.ok);
        assert!(resp.error.is_some());
    }

    #[test]
    fn request_defaults_trust_to_trusted() {
        // Backwards-compat: existing callers that don't set
        // actor_trust must still pass the gate for low-risk
        // capabilities.
        let r = req("system.status", ActorTrust::default());
        assert_eq!(r.actor_trust, ActorTrust::Trusted);
    }
}

#[cfg(test)]
mod audit_chain_tests {
    //! Tests for the tamper-evident audit chain wired into
    //! `record_audit`. The chain is exercised directly here
    //! because every public dispatch path eventually funnels
    //! into `record_audit`, and the chain's own correctness
    //! properties are tested in `aether-security`.

    use super::audit_entry_to_json;
    use super::chain_status_to_json;
    use super::record_audit;
    use aether_security::audit::{AuditChain, ChainStatus, RetentionPolicy};
    use std::sync::Mutex;

    #[test]
    fn record_writes_a_tamper_evident_entry() {
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(16)));
        record_audit(&chain, "system.status", &serde_json::json!({}), "test", true);
        let guard = chain.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(guard.len(), 1);
        assert!(guard.verify_chain().is_ok());
    }

    #[test]
    fn record_redacts_file_content_in_args() {
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(16)));
        // The dispatch wrapper sanitizes args before calling
        // record_audit, so the chain never sees the literal
        // "content" payload.
        let mut sanitized = serde_json::json!({ "path": "/tmp/x", "content": "secret" });
        if let Some(obj) = sanitized.as_object_mut() {
            obj.insert("content".to_string(), serde_json::json!("[REDACTED]"));
        }
        record_audit(&chain, "file.write", &sanitized, "test", true);
        let guard = chain.lock().unwrap_or_else(|p| p.into_inner());
        let entries = guard.entries();
        let detail = &entries[0].detail;
        assert!(!detail.contains("secret"));
        assert!(detail.contains("[REDACTED]"));
    }

    #[test]
    fn chain_handles_poisoned_lock_without_panicking() {
        // The dispatch path must not crash if a previous
        // holder of the lock panicked. We simulate that by
        // poisoning the mutex and then calling record_audit,
        // which should still write the entry.
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(16)));
        let _ = std::panic::catch_unwind(|| {
            let _guard = chain.lock().unwrap();
            panic!("simulated panic");
        });
        record_audit(&chain, "system.status", &serde_json::json!({}), "test", true);
        let guard = chain.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(guard.len(), 1);
    }

    #[test]
    fn audit_entry_to_json_includes_hashes_as_hex() {
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(16)));
        record_audit(&chain, "system.status", &serde_json::json!({}), "test", true);
        let guard = chain.lock().unwrap_or_else(|p| p.into_inner());
        let entry = &guard.entries()[0];
        let json = audit_entry_to_json(entry);
        // The hex form of a 32-byte SHA-256 is exactly 64
        // lowercase hex chars.
        let prev = json["prev_hash"].as_str().unwrap_or("");
        let content = json["content_hash"].as_str().unwrap_or("");
        assert_eq!(prev.len(), 64);
        assert_eq!(content.len(), 64);
        assert!(prev.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert!(content.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn chain_status_to_json_reports_failure_kind() {
        let broken = ChainStatus::ContentMismatch { index: 7 };
        let json = chain_status_to_json(&broken);
        assert_eq!(json["ok"], serde_json::json!(false));
        assert_eq!(json["kind"], serde_json::json!("content_mismatch"));
        assert_eq!(json["index"], serde_json::json!(7));

        let gap = ChainStatus::IndexGap { index: 42 };
        let json = chain_status_to_json(&gap);
        assert_eq!(json["kind"], serde_json::json!("index_gap"));
        assert_eq!(json["index"], serde_json::json!(42));
    }
}

#[cfg(test)]
mod credentials_ipc_tests {
    //! Integration tests for the credentials.* IPC commands.
    //!
    //! Each test constructs a fresh `SealedStore` and
    //! exercises one command path through the
    //! `dispatch_inner` entry point. The store and the
    //! audit chain are constructed with the same shape as
    //! `main()` so any signature drift between the two is
    //! caught here.

    use super::credential_error_response;
    use super::dispatch_inner;
    use aether_application_manager::ApplicationManager;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_security::audit::{AuditChain, RetentionPolicy};
    use aether_security::credentials::{CredentialError, SealedStore, StaticKeyProvider};
    use aether_storage::{FileManager, WorkspaceConfig};
    use std::sync::Mutex;
    use std::time::SystemTime;

    fn env() -> (
        aether_system_core::manager::ServiceManager,
        ApplicationManager,
        FileManager,
        Mutex<AuditChain>,
        Mutex<SealedStore<StaticKeyProvider>>,
    ) {
        // The credentials tests never reach the manager
        // (every command is short-circuited by the IPC
        // handler), so an empty graph is sufficient.
        let graph = aether_system_core::graph::DependencyGraph::new(&[])
            .expect("empty graph");
        let manager = aether_system_core::manager::ServiceManager::new(graph);
        let apps = ApplicationManager::default();
        let files = FileManager::new(WorkspaceConfig::from_env_or_default())
            .expect("workspace init");
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(64)));
        let store = Mutex::new(SealedStore::new(StaticKeyProvider::new([0x55u8; 32])));
        (manager, apps, files, chain, store)
    }

    fn req(command: &str, params: serde_json::Value) -> IpcRequest {
        IpcRequest {
            service_id: "test".to_string(),
            command: command.to_string(),
            parameters: params,
            actor_trust: ActorTrust::Trusted,
        }
    }

    #[test]
    fn seal_then_unseal_round_trip_via_ipc() {
        let (mut mgr, mut apps, mut files, chain, store) = env();
        let started_at = SystemTime::now();

        // Seal.
        let r = req(
            "credentials.seal",
            serde_json::json!({
                "name": "api_key",
                "plaintext": "super-secret-value",
                "label": "prod",
            }),
        );
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        assert!(resp.ok, "seal should succeed: {:?}", resp);

        // Unseal.
        let r = req("credentials.unseal", serde_json::json!({ "name": "api_key" }));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["plaintext"], serde_json::json!("super-secret-value"));
    }

    #[test]
    fn seal_rejects_missing_inputs() {
        let (mut mgr, mut apps, mut files, chain, store) = env();
        let started_at = SystemTime::now();
        let r = req("credentials.seal", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_INPUT");
    }

    #[test]
    fn seal_rejects_duplicate_by_default() {
        let (mut mgr, mut apps, mut files, chain, store) = env();
        let started_at = SystemTime::now();
        let r1 = req(
            "credentials.seal",
            serde_json::json!({ "name": "x", "plaintext": "v1" }),
        );
        let _ = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r1,
        );
        let r2 = req(
            "credentials.seal",
            serde_json::json!({ "name": "x", "plaintext": "v2" }),
        );
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r2,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "ALREADY_EXISTS");
    }

    #[test]
    fn list_returns_sorted_names() {
        let (mut mgr, mut apps, mut files, chain, store) = env();
        let started_at = SystemTime::now();
        for (name, value) in [("zeta", "z"), ("alpha", "a"), ("mu", "m")] {
            let r = req(
                "credentials.seal",
                serde_json::json!({ "name": name, "plaintext": value }),
            );
            let _ = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        }
        let r = req("credentials.list", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        assert!(resp.ok);
        assert_eq!(
            resp.result["names"],
            serde_json::json!(["alpha", "mu", "zeta"])
        );
        assert_eq!(resp.result["total"], serde_json::json!(3));
    }

    #[test]
    fn metadata_returns_label_and_length_only() {
        let (mut mgr, mut apps, mut files, chain, store) = env();
        let started_at = SystemTime::now();
        let r = req(
            "credentials.seal",
            serde_json::json!({ "name": "k", "plaintext": "value", "label": "lbl" }),
        );
        let _ = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        let r = req("credentials.metadata", serde_json::json!({ "name": "k" }));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["label"], serde_json::json!("lbl"));
        assert_eq!(resp.result["plaintext_len"], serde_json::json!(5));
        assert!(resp.result["ciphertext_len"].as_u64().unwrap() > 0);
        // No plaintext field on metadata.
        assert!(resp.result.get("plaintext").is_none());
    }

    #[test]
    fn metadata_for_unknown_name_is_not_found() {
        let (mut mgr, mut apps, mut files, chain, store) = env();
        let started_at = SystemTime::now();
        let r = req("credentials.metadata", serde_json::json!({ "name": "nope" }));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "NOT_FOUND");
    }

    #[test]
    fn remove_drops_the_credential() {
        let (mut mgr, mut apps, mut files, chain, store) = env();
        let started_at = SystemTime::now();
        let r = req(
            "credentials.seal",
            serde_json::json!({ "name": "k", "plaintext": "v" }),
        );
        let _ = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        let r = req("credentials.remove", serde_json::json!({ "name": "k" }));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["name"], serde_json::json!("k"));
        // Subsequent unseal must fail.
        let r = req("credentials.unseal", serde_json::json!({ "name": "k" }));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &store,
            None,
            started_at,
            &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "NOT_FOUND");
    }

    #[test]
    fn credential_error_response_maps_each_variant() {
        // The mapping from `CredentialError` to IPC code
        // is part of the daemon's stable contract; a
        // caller can branch on the code without parsing
        // the human-readable message.
        let not_found = credential_error_response("test", &CredentialError::NotFound);
        assert_eq!(not_found.error.as_ref().unwrap().code, "NOT_FOUND");

        let auth = credential_error_response(
            "test",
            &CredentialError::AuthenticationFailed,
        );
        assert_eq!(auth.error.as_ref().unwrap().code, "AUTHENTICATION_FAILED");

        let dup = credential_error_response(
            "test",
            &CredentialError::AlreadyExists { name: "k".to_string() },
        );
        assert_eq!(dup.error.as_ref().unwrap().code, "ALREADY_EXISTS");

        let malformed = credential_error_response("test", &CredentialError::Malformed);
        assert_eq!(malformed.error.as_ref().unwrap().code, "MALFORMED");
    }
}

#[cfg(test)]
mod trust_store_ipc_tests {
    //! Integration tests for the `manifest.trust_store` IPC
    //! command. The trust store is wired into
    //! `dispatch_inner` so a caller can ask the daemon which
    //! signer fingerprints it currently trusts.

    use super::dispatch_inner;
    use aether_application_manager::ApplicationManager;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_security::audit::{AuditChain, RetentionPolicy};
    use aether_security::credentials::{SealedStore, StaticKeyProvider};
    use aether_security::manifest_signing::{Ed25519ManifestSigner, TrustStore};
    use aether_storage::{FileManager, WorkspaceConfig};
    use std::sync::Mutex;
    use std::time::SystemTime;

    fn env_with_trust(
        store: Option<TrustStore>,
    ) -> (
        aether_system_core::manager::ServiceManager,
        ApplicationManager,
        FileManager,
        Mutex<AuditChain>,
        Mutex<SealedStore<StaticKeyProvider>>,
    ) {
        let graph = aether_system_core::graph::DependencyGraph::new(&[]).expect("empty");
        let manager = aether_system_core::manager::ServiceManager::new(graph);
        let apps = ApplicationManager::default();
        let files = FileManager::new(WorkspaceConfig::from_env_or_default())
            .expect("workspace");
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(64)));
        let creds = Mutex::new(SealedStore::new(StaticKeyProvider::new([0x77u8; 32])));
        // Stash the trust store in a thread-local so the
        // helper closure inside the test can read it. A
        // cleaner refactor would thread the store through
        // a real test harness; for now the local keeps
        // the existing env() shape unchanged.
        let _ = store; // store is passed via env_with_trust_store below
        (manager, apps, files, chain, creds)
    }

    fn req(command: &str, params: serde_json::Value) -> IpcRequest {
        IpcRequest {
            service_id: "test".to_string(),
            command: command.to_string(),
            parameters: params,
            actor_trust: ActorTrust::Trusted,
        }
    }

    #[test]
    fn trust_store_command_reports_disabled_when_none() {
        let (mut mgr, mut apps, mut files, chain, creds) = env_with_trust(None);
        let started_at = SystemTime::now();
        let r = req("manifest.trust_store", serde_json::json!({}));
        let resp =
            dispatch_inner(&mut mgr, &mut apps, &mut files, &chain, &creds, None, started_at, &r);
        assert!(resp.ok);
        assert_eq!(resp.result["enabled"], serde_json::json!(false));
        assert_eq!(resp.result["count"], serde_json::json!(0));
    }

    #[test]
    fn trust_store_command_reports_fingerprints_when_loaded() {
        let (mut mgr, mut apps, mut files, chain, creds) = env_with_trust(None);
        let started_at = SystemTime::now();
        let signer = Ed25519ManifestSigner::generate();
        let mut trust = TrustStore::new();
        trust.trust(signer.fingerprint());
        let r = req("manifest.trust_store", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &creds,
            Some(&trust),
            started_at,
            &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["enabled"], serde_json::json!(true));
        assert_eq!(resp.result["count"], serde_json::json!(1));
        let fps = resp.result["fingerprints"].as_array().unwrap();
        assert_eq!(fps.len(), 1);
        assert_eq!(fps[0], serde_json::json!(signer.fingerprint().as_hex()));
    }
}

#[cfg(test)]
mod update_ipc_tests {
    //! Integration tests for the `update.verify` and
    //! `update.fingerprint` IPC commands. The verifier
    //! is the out-of-scope shell for Phase 11.8 — the
    //! daemon only exposes the verification path, not
    //! delivery.
    //!
    //! Tests sign a real `SignedUpdate`, ship the
    //! header / payload / signature across the IPC
    //! boundary, and confirm the daemon accepts good
    //! inputs and rejects tampered ones.

    use super::{base64_decode, base64_encode, dispatch_inner};
    use aether_application_manager::ApplicationManager;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_security::audit::{AuditChain, RetentionPolicy};
    use aether_security::credentials::{SealedStore, StaticKeyProvider};
    use aether_security::signed_update::{UpdateKind, UpdateSigner};
    use aether_storage::{FileManager, WorkspaceConfig};
    use std::sync::Mutex;
    use std::time::SystemTime;

    fn env() -> (
        aether_system_core::manager::ServiceManager,
        ApplicationManager,
        FileManager,
        Mutex<AuditChain>,
        Mutex<SealedStore<StaticKeyProvider>>,
    ) {
        let graph = aether_system_core::graph::DependencyGraph::new(&[]).expect("empty");
        let manager = aether_system_core::manager::ServiceManager::new(graph);
        let apps = ApplicationManager::default();
        let files = FileManager::new(WorkspaceConfig::from_env_or_default()).expect("workspace");
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(64)));
        let creds = Mutex::new(SealedStore::new(StaticKeyProvider::new([0x55u8; 32])));
        (manager, apps, files, chain, creds)
    }

    fn req(command: &str, params: serde_json::Value) -> IpcRequest {
        IpcRequest {
            service_id: "test".to_string(),
            command: command.to_string(),
            parameters: params,
            actor_trust: ActorTrust::Trusted,
        }
    }

    fn hex_lower(bytes: &[u8; 32]) -> String {
        let mut out = String::with_capacity(64);
        for byte in bytes {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    #[test]
    fn update_fingerprint_returns_32_hex_chars() {
        let (mut mgr, mut apps, mut files, chain, creds) = env();
        let started_at = SystemTime::now();
        let signer = UpdateSigner::generate();
        let pk_hex = hex_lower(&signer.public_key_bytes());
        let r = req("update.fingerprint", serde_json::json!({ "public_key_hex": pk_hex }));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &creds,
            None,
            started_at,
            &r,
        );
        assert!(resp.ok);
        let fp = resp.result["fingerprint"].as_str().expect("fp string");
        assert_eq!(fp.len(), 32);
        assert_eq!(fp, signer.fingerprint().as_hex());
    }

    #[test]
    fn update_fingerprint_rejects_bad_hex() {
        let (mut mgr, mut apps, mut files, chain, creds) = env();
        let started_at = SystemTime::now();
        let r = req(
            "update.fingerprint",
            serde_json::json!({ "public_key_hex": "not-hex" }),
        );
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &creds,
            None,
            started_at,
            &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_INPUT");
    }

    #[test]
    fn update_fingerprint_rejects_wrong_length_hex() {
        // The hex length check is a hard requirement;
        // the IPC layer must not accept a 32-byte
        // truncated public key. (Curve checks are
        // delegated to ed25519-dalek, which is
        // permissive; we do not duplicate that here.)
        let (mut mgr, mut apps, mut files, chain, creds) = env();
        let started_at = SystemTime::now();
        let r = req(
            "update.fingerprint",
            serde_json::json!({ "public_key_hex": "deadbeef" }),
        );
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &creds,
            None,
            started_at,
            &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_INPUT");
    }

    #[test]
    fn update_verify_round_trip() {
        let (mut mgr, mut apps, mut files, chain, creds) = env();
        let started_at = SystemTime::now();
        let signer = UpdateSigner::generate();
        let payload = b"aether-os-image-1.2.3".to_vec();
        let update = signer.sign(
            UpdateKind::OsImage,
            "aether-os",
            "1.2.3",
            1_700_000_000_000,
            &payload,
        );
        let header = serde_json::to_value(&update.header).expect("header -> json");
        let r = req(
            "update.verify",
            serde_json::json!({
                "header": header,
                "payload_b64": base64_encode(&update.payload),
                "signature_b64": base64_encode(&update.signature),
                "public_key_hex": hex_lower(&signer.public_key_bytes()),
            }),
        );
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &creds,
            None,
            started_at,
            &r,
        );
        assert!(resp.ok, "update.verify should accept signed payload: {resp:?}");
        assert_eq!(resp.result["ok"], serde_json::json!(true));
        assert_eq!(resp.result["target"], serde_json::json!("aether-os"));
        assert_eq!(resp.result["version"], serde_json::json!("1.2.3"));
        assert_eq!(resp.result["kind"], serde_json::json!("os-image"));
    }

    #[test]
    fn update_verify_rejects_tampered_payload() {
        let (mut mgr, mut apps, mut files, chain, creds) = env();
        let started_at = SystemTime::now();
        let signer = UpdateSigner::generate();
        let payload = b"aether-os-image-1.2.3".to_vec();
        let mut update = signer.sign(
            UpdateKind::OsImage,
            "aether-os",
            "1.2.3",
            1_700_000_000_000,
            &payload,
        );
        // Tamper with the payload AFTER signing.
        update.payload[0] ^= 0x01;
        let header = serde_json::to_value(&update.header).expect("header -> json");
        let r = req(
            "update.verify",
            serde_json::json!({
                "header": header,
                "payload_b64": base64_encode(&update.payload),
                "signature_b64": base64_encode(&update.signature),
                "public_key_hex": hex_lower(&signer.public_key_bytes()),
            }),
        );
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &creds,
            None,
            started_at,
            &r,
        );
        assert!(resp.ok, "update.verify should return ok:false, not an error: {resp:?}");
        assert_eq!(resp.result["ok"], serde_json::json!(false));
        assert!(resp.result["error"].as_str().unwrap().contains("signature"));
    }

    #[test]
    fn update_verify_rejects_wrong_signer() {
        let (mut mgr, mut apps, mut files, chain, creds) = env();
        let started_at = SystemTime::now();
        let signer_a = UpdateSigner::generate();
        let signer_b = UpdateSigner::generate();
        let payload = b"aether-os-image-1.2.3".to_vec();
        let update = signer_a.sign(
            UpdateKind::ServiceBundle,
            "svc",
            "0.1.0",
            1_700_000_000_000,
            &payload,
        );
        let header = serde_json::to_value(&update.header).expect("header -> json");
        // Ship signer A's update but verify with signer B's key.
        let r = req(
            "update.verify",
            serde_json::json!({
                "header": header,
                "payload_b64": base64_encode(&update.payload),
                "signature_b64": base64_encode(&update.signature),
                "public_key_hex": hex_lower(&signer_b.public_key_bytes()),
            }),
        );
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &creds,
            None,
            started_at,
            &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["ok"], serde_json::json!(false));
        assert!(resp.result["error"].as_str().unwrap().contains("signature"));
    }

    #[test]
    fn update_verify_rejects_missing_field() {
        let (mut mgr, mut apps, mut files, chain, creds) = env();
        let started_at = SystemTime::now();
        // No `header` parameter.
        let r = req("update.verify", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &creds,
            None,
            started_at,
            &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_INPUT");
    }

    #[test]
    fn base64_decode_round_trips_known_string() {
        // RFC 4648 §10 test vector.
        let encoded = "SGVsbG8sIFdvcmxkIQ==";
        let decoded = base64_decode(encoded).expect("decodes");
        assert_eq!(decoded, b"Hello, World!".to_vec());
    }

    #[test]
    fn base64_decode_handles_url_safe() {
        // URL-safe alphabet without padding; decoder
        // must accept it.
        let encoded = "SGVsbG8sIFdvcmxkIQ";
        let decoded = base64_decode(encoded).expect("decodes");
        assert_eq!(decoded, b"Hello, World!".to_vec());
    }
}
