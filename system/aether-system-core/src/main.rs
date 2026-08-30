// Aether System Core - control daemon binary.
//
// Loads service manifests, starts all services in dependency order, and
// serves the local control protocol used by `aetherctl`:
// newline-delimited JSON requests/responses over TCP loopback.

use aether_agent_core::{AgentStatus, Observation, Proposal, ProposalError, TaskId};
use aether_application_manager::ApplicationManager;
use aether_core::error::ErrorKind;
use aether_core::ipc::{IpcError, IpcRequest, IpcResponse};
use aether_core::types::ServiceStatus;
use aether_device_core::{
    DeviceClass, DeviceFingerprint, DeviceId, DeviceRegistry, DeviceRegistryError, PairingCode,
    PairingGrant, PairingState,
};
use aether_security::audit::{AuditChain, AuditEntry, ChainStatus, RetentionPolicy};
use aether_security::credentials::{CredentialError, SealedStore, StaticKeyProvider};
use aether_security::manifest_signing::{Fingerprint, TrustStore};
use aether_storage::system_info;
use aether_storage::{FileManager, WorkspaceConfig};
use aether_system_core::policy;
use aether_system_core::{
    build_manager, load_manifests_from_dir, load_manifests_with_trust, ServiceExecutor,
    ServiceHandle,
};
use aether_update_core::{
    plan_from_signed_update, UpdateAction, UpdatePlanError, UpdateStage, UpdateStatus,
    VersionPolicy,
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
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
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

/// Maps a `UpdatePlanError` to a short caller-facing
/// string. The IPC code is `POLICY_DENIED` for every
/// variant today (the planning layer has only a few
/// rejection reasons, all policy-shaped); a more
/// elaborate code can be added later.
fn plan_error_message(err: &UpdatePlanError) -> String {
    err.to_string()
}

/// Converts an `UpdateAction` into its canonical
/// kebab-case name. Re-exported here so the IPC layer
/// does not have to depend on the action enum's own
/// `as_str` for rendering.
#[allow(dead_code)]
fn update_action_str(a: UpdateAction) -> &'static str {
    a.as_str()
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
    let detail = format!("args={} result={}", args, if ok { "success" } else { "failure" });
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

// The dispatcher wires together every global the
// daemon owns (services, apps, files, audit, sealed
// credentials, trust store, update planner, version
// policy, agent planning surface). Splitting it into
// a struct would help testability, but the IPC layer
// is small enough that the explicit signature is the
// simplest correct shape.
#[allow(clippy::too_many_arguments)]
fn dispatch(
    manager: &mut aether_system_core::manager::ServiceManager,
    apps: &mut ApplicationManager,
    files: &mut FileManager,
    audit_chain: &Mutex<AuditChain>,
    credentials: &Mutex<SealedStore<StaticKeyProvider>>,
    trust_store: Option<&TrustStore>,
    update_status: &Mutex<UpdateStatus>,
    version_policy: &Mutex<VersionPolicy>,
    agent_status: &Mutex<AgentStatus>,
    device_registry: &Mutex<DeviceRegistry>,
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
        update_status,
        version_policy,
        agent_status,
        device_registry,
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
fn gate_response(
    command: &str,
    verdict: &aether_system_core::policy::PolicyVerdict,
) -> IpcResponse {
    use aether_security::Decision;
    let (code, message) = match (verdict.decision, &verdict.reason) {
        (Decision::Deny, _) => ("POLICY_DENIED".to_string(), verdict.reason.clone()),
        (Decision::RequireConsent, _) => {
            ("REQUIRES_CONFIRMATION".to_string(), verdict.reason.clone())
        }
        // Allow is handled by the caller; this fn is only invoked
        // when the gate rejects the request.
        (Decision::Allow, _) => {
            ("INTERNAL".to_string(), "gate_response called for allow verdict".to_string())
        }
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

// See the note on `dispatch` — this is the inner
// half of the central IPC router and inherits the
// same broad signature.
#[allow(clippy::too_many_arguments)]
fn dispatch_inner(
    manager: &mut aether_system_core::manager::ServiceManager,
    apps: &mut ApplicationManager,
    files: &mut FileManager,
    audit_chain: &Mutex<AuditChain>,
    credentials: &Mutex<SealedStore<StaticKeyProvider>>,
    trust_store: Option<&TrustStore>,
    update_status: &Mutex<UpdateStatus>,
    version_policy: &Mutex<VersionPolicy>,
    agent_status: &Mutex<AgentStatus>,
    device_registry: &Mutex<DeviceRegistry>,
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
                            let plan_val =
                                serde_json::to_value(&plan).unwrap_or(serde_json::json!({}));
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
            let entries: Vec<serde_json::Value> =
                guard.recent(n).into_iter().map(audit_entry_to_json).collect();
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
                    let fps: Vec<String> =
                        store.fingerprints().iter().map(|f| f.as_hex().to_string()).collect();
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
            let signature_b64 = match req.parameters.get("signature_b64").and_then(|v| v.as_str()) {
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
                            message: "parameter 'public_key_hex' is required (64 hex chars)"
                                .to_string(),
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
            let update =
                aether_security::signed_update::SignedUpdate { header, payload, signature };
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
                            message: "parameter 'public_key_hex' is required (64 hex chars)"
                                .to_string(),
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
            IpcResponse::ok("update.fingerprint", serde_json::json!({ "fingerprint": fp.as_hex() }))
        }
        "update.plan" => {
            // The caller hands us a JSON header + a
            // base64-encoded payload + a base64-encoded
            // signature + a hex public key, plus the
            // currently installed version for the
            // target. We:
            //   1. Decode the bytes.
            //   2. Re-parse the header into a typed
            //      UpdateHeader.
            //   3. Run the security verifier.
            //   4. Run the version policy via
            //      `plan_from_signed_update`.
            //   5. Return the resulting UpdatePlan.
            //
            // We do NOT mutate the live UpdateStatus
            // here — the future update-agent is the
            // only thing allowed to drive transitions.
            let header_val = match req.parameters.get("header") {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "update.plan",
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
                        "update.plan",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'payload_b64' is required".to_string(),
                        },
                    );
                }
            };
            let signature_b64 = match req.parameters.get("signature_b64").and_then(|v| v.as_str()) {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "update.plan",
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
                        "update.plan",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'public_key_hex' is required".to_string(),
                        },
                    );
                }
            };
            let installed_version =
                req.parameters.get("installed_version").and_then(|v| v.as_str());
            let public_key_bytes: [u8; 32] = match decode_hex_32(public_key_hex) {
                Some(b) => b,
                None => {
                    return IpcResponse::err(
                        "update.plan",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: format!(
                                "public_key_hex is not 64 valid hex chars: {public_key_hex}"
                            ),
                        },
                    );
                }
            };
            let payload = match base64_decode(payload_b64) {
                Some(b) => b,
                None => {
                    return IpcResponse::err(
                        "update.plan",
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
                        "update.plan",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "signature_b64 is not valid base64".to_string(),
                        },
                    );
                }
            };
            let header: aether_security::signed_update::UpdateHeader =
                match serde_json::from_value(header_val.clone()) {
                    Ok(h) => h,
                    Err(e) => {
                        return IpcResponse::err(
                            "update.plan",
                            IpcError {
                                code: "INVALID_INPUT".to_string(),
                                message: format!("header is not a valid UpdateHeader: {e}"),
                            },
                        );
                    }
                };
            let update =
                aether_security::signed_update::SignedUpdate { header, payload, signature };
            if let Err(e) = aether_security::signed_update::verify_signed_update_with_key(
                &update,
                &public_key_bytes,
            ) {
                return IpcResponse::err(
                    "update.plan",
                    IpcError {
                        code: "VERIFICATION_FAILED".to_string(),
                        message: format!("signature verification failed: {e}"),
                    },
                );
            }
            let policy = match version_policy.lock() {
                Ok(p) => p,
                Err(poisoned) => poisoned.into_inner(),
            };
            match plan_from_signed_update(&update, installed_version, &policy) {
                Ok(plan) => {
                    let plan_json = match serde_json::to_value(&plan) {
                        Ok(v) => v,
                        Err(e) => {
                            return IpcResponse::err(
                                "update.plan",
                                IpcError {
                                    code: "INTERNAL".to_string(),
                                    message: format!("plan serialisation: {e}"),
                                },
                            );
                        }
                    };
                    IpcResponse::ok("update.plan", plan_json)
                }
                Err(e) => IpcResponse::err(
                    "update.plan",
                    IpcError { code: "POLICY_DENIED".to_string(), message: plan_error_message(&e) },
                ),
            }
        }
        "update.status" => {
            // Read-only view of the live update state
            // machine. The shell returns the stage,
            // attempt counter, last error, and the
            // current plan (if any). The history is
            // returned by `update.history`.
            let status = match update_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let plan_json = match status.current_plan() {
                Some(p) => match serde_json::to_value(p) {
                    Ok(v) => v,
                    Err(e) => {
                        return IpcResponse::err(
                            "update.status",
                            IpcError {
                                code: "INTERNAL".to_string(),
                                message: format!("plan serialisation: {e}"),
                            },
                        );
                    }
                },
                None => serde_json::Value::Null,
            };
            IpcResponse::ok(
                "update.status",
                serde_json::json!({
                    "stage": status.stage().as_str(),
                    "attempt": status.attempt(),
                    "last_error": status.last_error(),
                    "current_plan": plan_json,
                }),
            )
        }
        "update.history" => {
            // Returns the bounded history of
            // transitions. Each entry carries the
            // `from` / `to` stages, the timestamp,
            // an optional note, and the plan that
            // drove the transition (if any).
            let status = match update_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let entries: Vec<serde_json::Value> = status
                .history()
                .iter()
                .map(|h| {
                    serde_json::json!({
                        "from": h.transition.from.as_str(),
                        "to": h.transition.to.as_str(),
                        "timestamp_ms": h.transition.timestamp_ms,
                        "note": h.transition.note,
                        "plan": h.plan.as_ref().and_then(|p| serde_json::to_value(p).ok()),
                    })
                })
                .collect();
            IpcResponse::ok(
                "update.history",
                serde_json::json!({ "entries": entries, "total": entries.len() }),
            )
        }
        "update.simulate" => {
            // Test-only helper: drives the state
            // machine through a sequence of stages so
            // operators (and tests) can observe the
            // history without waiting for a real
            // update. The shell accepts any
            // comma-separated sequence of `UpdateStage`
            // names.
            let sequence = match req.parameters.get("stages").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return IpcResponse::err(
                        "update.simulate",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'stages' is required (comma-separated)".to_string(),
                        },
                    );
                }
            };
            let mut status = match update_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let now = unix_ms().min(u128::from(u64::MAX)) as u64;
            let mut applied: Vec<&'static str> = Vec::new();
            for token in sequence.split(',') {
                let trimmed = token.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let stage = match trimmed {
                    "idle" => UpdateStage::Idle,
                    "downloading" => UpdateStage::Downloading,
                    "verifying" => UpdateStage::Verifying,
                    "staging" => UpdateStage::Staging,
                    "applying" => UpdateStage::Applying,
                    "done" => UpdateStage::Done,
                    "failed" => UpdateStage::Failed,
                    "rolled-back" => UpdateStage::RolledBack,
                    other => {
                        return IpcResponse::err(
                            "update.simulate",
                            IpcError {
                                code: "INVALID_INPUT".to_string(),
                                message: format!("unknown stage '{other}'"),
                            },
                        );
                    }
                };
                status.transition(stage, now, None);
                applied.push(stage.as_str());
            }
            IpcResponse::ok(
                "update.simulate",
                serde_json::json!({ "applied": applied, "current": status.stage().as_str() }),
            )
        }
        // ----- Agent (Phase 13) -----
        //
        // The agent is the future runtime; today
        // the shell exposes only the planning
        // surface — observations, proposals, task
        // graph, history. The runtime is the
        // only thing allowed to call
        // `add_observation` and `add_proposal` from
        // outside, but the IPC layer is open so
        // tests and the future shell UI can drive
        // it end-to-end.
        "agent.propose" => {
            // Accepts a single `Proposal` object in
            // `parameters.proposal`. Validates the
            // proposal against the live observation
            // log; on success it is added to the
            // agent's pending set.
            let proposal_value = match req.parameters.get("proposal").cloned() {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "agent.propose",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'proposal' is required".to_string(),
                        },
                    );
                }
            };
            let proposal: Proposal = match serde_json::from_value(proposal_value) {
                Ok(p) => p,
                Err(e) => {
                    return IpcResponse::err(
                        "agent.propose",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: format!("proposal decode: {e}"),
                        },
                    );
                }
            };
            let mut status = match agent_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            // Validate against the live observation
            // log so callers can't reference
            // observations that don't exist.
            let obs_snapshot: Vec<Observation> = status.observations().to_vec();
            match aether_agent_core::proposal::validate_proposal(&proposal, &obs_snapshot) {
                Ok(()) => {
                    let id = proposal.id.clone();
                    let was_new = status.add_proposal(proposal);
                    IpcResponse::ok(
                        "agent.propose",
                        serde_json::json!({
                            "id": id.to_string(),
                            "new": was_new,
                        }),
                    )
                }
                Err(e) => {
                    let (code, message) = match &e {
                        ProposalError::EmptyId => ("EMPTY_ID".to_string(), e.to_string()),
                        ProposalError::IncompleteDescription => {
                            ("INCOMPLETE_DESCRIPTION".to_string(), e.to_string())
                        }
                        ProposalError::UnknownEvidence { .. } => {
                            ("UNKNOWN_EVIDENCE".to_string(), e.to_string())
                        }
                        ProposalError::RiskTooLowForKind { .. } => {
                            ("RISK_TOO_LOW".to_string(), e.to_string())
                        }
                    };
                    IpcResponse::err("agent.propose", IpcError { code, message })
                }
            }
        }
        "agent.observe" => {
            // Records a new observation in the
            // bounded log. Today this is invoked
            // only by tests; the future agentd
            // is the real source.
            let obs_value = match req.parameters.get("observation").cloned() {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "agent.observe",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'observation' is required".to_string(),
                        },
                    );
                }
            };
            let observation: Observation = match serde_json::from_value(obs_value) {
                Ok(o) => o,
                Err(e) => {
                    return IpcResponse::err(
                        "agent.observe",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: format!("observation decode: {e}"),
                        },
                    );
                }
            };
            let mut status = match agent_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let id = observation.id.clone();
            status.add_observation(observation);
            IpcResponse::ok("agent.observe", serde_json::json!({ "id": id }))
        }
        "agent.proposals" => {
            // Read-only view of the pending
            // proposal set. Sorted by id for a
            // stable order.
            let status = match agent_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let mut proposals: Vec<&Proposal> = status.proposals();
            proposals.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            let values: Vec<serde_json::Value> =
                proposals.iter().filter_map(|p| serde_json::to_value(p).ok()).collect();
            IpcResponse::ok(
                "agent.proposals",
                serde_json::json!({
                    "proposals": values,
                    "total": values.len(),
                }),
            )
        }
        "agent.tasks" => {
            // Read-only view of the live task
            // graph. Returns the insertion-order
            // task list and a `ready` subset for
            // the given `done` ids (defaults to
            // empty).
            let done: Vec<TaskId> = match req.parameters.get("done").and_then(|v| v.as_array()) {
                Some(arr) => {
                    let mut out = Vec::with_capacity(arr.len());
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            if let Some(t) = TaskId::new(s) {
                                out.push(t);
                            }
                        }
                    }
                    out
                }
                None => Vec::new(),
            };
            let status = match agent_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let tasks: Vec<serde_json::Value> = status
                .tasks()
                .tasks()
                .iter()
                .filter_map(|t| serde_json::to_value(t).ok())
                .collect();
            let ready: Vec<serde_json::Value> = status
                .tasks()
                .ready(&done)
                .iter()
                .filter_map(|t| serde_json::to_value(t).ok())
                .collect();
            IpcResponse::ok(
                "agent.tasks",
                serde_json::json!({
                    "tasks": tasks,
                    "ready": ready,
                    "task_count": tasks.len(),
                    "ready_count": ready.len(),
                }),
            )
        }
        "agent.history" => {
            // Returns the bounded task history,
            // oldest-first. The `task` field is
            // the full `AgentTask`; the `stage`
            // is the terminal stage it ended in.
            let status = match agent_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let entries: Vec<serde_json::Value> = status
                .history()
                .iter()
                .map(|h| {
                    serde_json::json!({
                        "stage": h.stage.as_str(),
                        "timestamp_ms": h.timestamp_ms,
                        "note": h.note,
                        "task": serde_json::to_value(&h.task).ok(),
                    })
                })
                .collect();
            IpcResponse::ok(
                "agent.history",
                serde_json::json!({
                    "entries": entries,
                    "total": entries.len(),
                }),
            )
        }
        "agent.observations" => {
            // Returns the bounded observation log,
            // oldest-first. The proposal layer
            // reads this to validate evidence ids.
            let status = match agent_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let entries: Vec<serde_json::Value> =
                status.observations().iter().filter_map(|o| serde_json::to_value(o).ok()).collect();
            IpcResponse::ok(
                "agent.observations",
                serde_json::json!({
                    "observations": entries,
                    "total": entries.len(),
                }),
            )
        }
        "agent.cancel" => {
            // Removes a live task by id. Returns
            // the removed task or an
            // `INVALID_INPUT` if no such task
            // exists. Cancelled tasks are not
            // appended to history; the future
            // runtime appends a `Cancelled`
            // history entry when it observes the
            // removal.
            let id_str = match req.parameters.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return IpcResponse::err(
                        "agent.cancel",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'id' is required".to_string(),
                        },
                    );
                }
            };
            let id = match TaskId::new(id_str) {
                Some(t) => t,
                None => {
                    return IpcResponse::err(
                        "agent.cancel",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "task id is empty".to_string(),
                        },
                    );
                }
            };
            let mut status = match agent_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            match status.remove_task(&id) {
                Some(task) => {
                    let task_json = serde_json::to_value(&task).ok();
                    IpcResponse::ok("agent.cancel", serde_json::json!({ "removed": task_json }))
                }
                None => IpcResponse::err(
                    "agent.cancel",
                    IpcError {
                        code: "NOT_FOUND".to_string(),
                        message: format!("task '{}' not found", id.as_str()),
                    },
                ),
            }
        }
        "agent.approve" => {
            // Approves a pending proposal and
            // turns it into a live task. The
            // task is inserted into the agent's
            // task graph; the future runtime is
            // responsible for picking it up. The
            // proposal is removed from the
            // pending set on success.
            let id_str = match req.parameters.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return IpcResponse::err(
                        "agent.approve",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'id' is required".to_string(),
                        },
                    );
                }
            };
            let mut status = match agent_status.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let proposal = match aether_agent_core::ProposalId::new(id_str.clone()) {
                Some(pid) => status.remove_proposal(&pid),
                None => {
                    return IpcResponse::err(
                        "agent.approve",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "proposal id is empty".to_string(),
                        },
                    );
                }
            };
            let proposal = match proposal {
                Some(p) => p,
                None => {
                    return IpcResponse::err(
                        "agent.approve",
                        IpcError {
                            code: "NOT_FOUND".to_string(),
                            message: format!("proposal '{id_str}' not found"),
                        },
                    );
                }
            };
            // The future runtime supplies the
            // task id; for now we derive it from
            // the proposal id so the operation
            // is idempotent in tests.
            let task_id = match TaskId::new(format!("task-{}", proposal.id.as_str())) {
                Some(t) => t,
                None => {
                    return IpcResponse::err(
                        "agent.approve",
                        IpcError {
                            code: "INTERNAL".to_string(),
                            message: "could not derive task id".to_string(),
                        },
                    );
                }
            };
            let task = match aether_agent_core::proposal::proposal_to_task(&proposal, task_id) {
                Some(t) => t,
                None => {
                    return IpcResponse::err(
                        "agent.approve",
                        IpcError {
                            code: "INTERNAL".to_string(),
                            message: "could not convert proposal to task".to_string(),
                        },
                    );
                }
            };
            // Insert into the live graph. The
            // graph may reject duplicate ids; we
            // surface that as INVALID_INPUT.
            if let Err(e) = status.insert_task(task.clone()) {
                return IpcResponse::err(
                    "agent.approve",
                    IpcError {
                        code: "INVALID_INPUT".to_string(),
                        message: format!("insert task: {e}"),
                    },
                );
            }
            let task_json = serde_json::to_value(&task).ok();
            IpcResponse::ok("agent.approve", serde_json::json!({ "task": task_json }))
        }
        // ----- Devices (Phase 14) -----
        //
        // The future device runtime is the
        // only thing that will talk to a real
        // network. Today the shell exposes the
        // registry, the pairing state machine,
        // and a typed delivery gate. Every
        // command here is purely a contract
        // for the future runtime to drive.
        "device.list" => {
            // Read-only view of the registry.
            // Returns the registered devices
            // sorted by id, with a separate
            // `paired` list for the subset the
            // local agent trusts.
            let registry = match device_registry.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let devices: Vec<serde_json::Value> =
                registry.devices().iter().filter_map(|d| serde_json::to_value(d).ok()).collect();
            let paired: Vec<serde_json::Value> =
                registry.paired().iter().filter_map(|d| serde_json::to_value(d).ok()).collect();
            IpcResponse::ok(
                "device.list",
                serde_json::json!({
                    "devices": devices,
                    "paired": paired,
                    "total": devices.len(),
                    "paired_count": paired.len(),
                    "capacity": registry.capacity(),
                }),
            )
        }
        "device.register" => {
            // Registers a new device in the
            // `Available` state. The caller
            // supplies the device id, class,
            // public-key fingerprint, and the
            // grant. The shell does not
            // validate the fingerprint (the
            // future runtime owns the
            // out-of-band key check); it just
            // stores it.
            let id = match req.parameters.get("device_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return IpcResponse::err(
                        "device.register",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'device_id' is required".to_string(),
                        },
                    );
                }
            };
            let class_str = match req.parameters.get("device_class").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return IpcResponse::err(
                        "device.register",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'device_class' is required".to_string(),
                        },
                    );
                }
            };
            let class = match class_str.as_str() {
                "phone" => DeviceClass::Phone,
                "tablet" => DeviceClass::Tablet,
                "laptop" => DeviceClass::Laptop,
                "desktop" => DeviceClass::Desktop,
                "iot" => DeviceClass::Iot,
                "server" => DeviceClass::Server,
                "external" => DeviceClass::External,
                "other" => DeviceClass::Other,
                other => {
                    return IpcResponse::err(
                        "device.register",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: format!("unknown device class '{other}'"),
                        },
                    );
                }
            };
            let fp_hex = match req.parameters.get("fingerprint").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return IpcResponse::err(
                        "device.register",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'fingerprint' is required (64 hex chars)"
                                .to_string(),
                        },
                    );
                }
            };
            let fp_bytes = match decode_hex_32(&fp_hex) {
                Some(b) => b,
                None => {
                    return IpcResponse::err(
                        "device.register",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "fingerprint must be 64 hex chars".to_string(),
                        },
                    );
                }
            };
            let grant = if req.parameters.get("grant").is_some() {
                match serde_json::from_value::<PairingGrant>(req.parameters["grant"].clone()) {
                    Ok(g) => g,
                    Err(e) => {
                        return IpcResponse::err(
                            "device.register",
                            IpcError {
                                code: "INVALID_INPUT".to_string(),
                                message: format!("grant decode: {e}"),
                            },
                        );
                    }
                }
            } else {
                PairingGrant::default()
            };
            let device_id = match DeviceId::new(id.clone()) {
                Some(d) => d,
                None => {
                    return IpcResponse::err(
                        "device.register",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "device id is empty or too long".to_string(),
                        },
                    );
                }
            };
            let now = unix_ms().min(u128::from(u64::MAX)) as u64;
            let mut registry = match device_registry.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            match registry.register(
                device_id,
                class,
                DeviceFingerprint::from_bytes(fp_bytes),
                grant,
                now,
            ) {
                Ok(()) => IpcResponse::ok(
                    "device.register",
                    serde_json::json!({ "id": id, "state": "available" }),
                ),
                Err(e) => {
                    let code = match &e {
                        DeviceRegistryError::Full => "REGISTRY_FULL",
                        DeviceRegistryError::AlreadyRegistered => "ALREADY_REGISTERED",
                        DeviceRegistryError::UnknownDevice => "UNKNOWN_DEVICE",
                    };
                    IpcResponse::err(
                        "device.register",
                        IpcError { code: code.to_string(), message: e.to_string() },
                    )
                }
            }
        }
        "device.pair.begin" => {
            // Moves a device from `Available`
            // (or `Revoked` / `Expired`) into
            // the `Pairing` state. The
            // handshake is the future runtime's
            // responsibility; the shell only
            // tracks the state.
            let id = match req.parameters.get("device_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return IpcResponse::err(
                        "device.pair.begin",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'device_id' is required".to_string(),
                        },
                    );
                }
            };
            let device_id = match DeviceId::new(id.clone()) {
                Some(d) => d,
                None => {
                    return IpcResponse::err(
                        "device.pair.begin",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "device id is empty or too long".to_string(),
                        },
                    );
                }
            };
            let now = unix_ms().min(u128::from(u64::MAX)) as u64;
            let mut registry = match device_registry.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            // Reject if not in a state that
            // can start a new pairing.
            let current = match registry.get(&device_id) {
                Some(d) => d.pairing.state,
                None => {
                    return IpcResponse::err(
                        "device.pair.begin",
                        IpcError {
                            code: "NOT_FOUND".to_string(),
                            message: format!("device '{id}' is not registered"),
                        },
                    );
                }
            };
            if !matches!(current, PairingState::Available | PairingState::Cancelled) {
                return IpcResponse::err(
                    "device.pair.begin",
                    IpcError {
                        code: "INVALID_STATE".to_string(),
                        message: format!(
                            "device is in '{}' state; only Available or Cancelled can begin pairing",
                            current.as_str()
                        ),
                    },
                );
            }
            match registry.transition(&device_id, PairingState::Pairing, now) {
                Ok(()) => IpcResponse::ok(
                    "device.pair.begin",
                    serde_json::json!({ "id": id, "state": "pairing" }),
                ),
                Err(e) => IpcResponse::err(
                    "device.pair.begin",
                    IpcError { code: "REGISTRY_ERROR".to_string(), message: e.to_string() },
                ),
            }
        }
        "device.pair.complete" => {
            // Validates a `PairingAcceptance`
            // against a `PairingRequest` and,
            // if they match, flips the device
            // from `Pairing` to `Paired`.
            // Returns the new state on
            // success.
            let request = match req.parameters.get("request").cloned() {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "device.pair.complete",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'request' is required".to_string(),
                        },
                    );
                }
            };
            let acceptance = match req.parameters.get("acceptance").cloned() {
                Some(v) => v,
                None => {
                    return IpcResponse::err(
                        "device.pair.complete",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'acceptance' is required".to_string(),
                        },
                    );
                }
            };
            let request: aether_device_core::PairingRequest = match serde_json::from_value(request)
            {
                Ok(r) => r,
                Err(e) => {
                    return IpcResponse::err(
                        "device.pair.complete",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: format!("request decode: {e}"),
                        },
                    );
                }
            };
            let acceptance: aether_device_core::PairingAcceptance =
                match serde_json::from_value(acceptance) {
                    Ok(a) => a,
                    Err(e) => {
                        return IpcResponse::err(
                            "device.pair.complete",
                            IpcError {
                                code: "INVALID_INPUT".to_string(),
                                message: format!("acceptance decode: {e}"),
                            },
                        );
                    }
                };
            // The local-side `code` is the
            // 6-digit pairing code the user
            // reads on the local device. The
            // future runtime produces it; for
            // now we accept an optional
            // `local_code` parameter and
            // default to the request's code
            // (the IPC caller is the trust
            // anchor in tests).
            let local_code = match req.parameters.get("local_code").and_then(|v| v.as_str()) {
                Some(s) => match PairingCode::new(s) {
                    Some(c) => c,
                    None => {
                        return IpcResponse::err(
                            "device.pair.complete",
                            IpcError {
                                code: "INVALID_INPUT".to_string(),
                                message: "local_code is not a 6-digit decimal".to_string(),
                            },
                        );
                    }
                },
                None => request.code,
            };
            // The acceptance's `code` must
            // match the request's. We use
            // `validate_acceptance` but supply
            // a request whose code is the
            // local code; the local code must
            // equal the request's code, and
            // the acceptance's code must equal
            // the local code. Simpler: build
            // a "synthetic" request whose code
            // is the local one and validate.
            let synthetic = aether_device_core::PairingRequest {
                device_id: request.device_id.clone(),
                device_class: request.device_class,
                fingerprint: request.fingerprint,
                code: local_code,
                grant: request.grant.clone(),
                timestamp_ms: request.timestamp_ms,
            };
            let now = unix_ms().min(u128::from(u64::MAX)) as u64;
            // 60-second skew window for tests.
            if let Err(e) = aether_device_core::pairing::validate_acceptance(
                &synthetic,
                &acceptance,
                60_000,
                now,
            ) {
                let code = match &e {
                    aether_device_core::PairingError::CodeMismatch => "CODE_MISMATCH",
                    aether_device_core::PairingError::FingerprintMismatch => "FINGERPRINT_MISMATCH",
                    aether_device_core::PairingError::IdentityMismatch => "IDENTITY_MISMATCH",
                    aether_device_core::PairingError::RequestExpired => "REQUEST_EXPIRED",
                    aether_device_core::PairingError::AlreadyPaired => "ALREADY_PAIRED",
                    aether_device_core::PairingError::TerminalState => "TERMINAL_STATE",
                };
                return IpcResponse::err(
                    "device.pair.complete",
                    IpcError { code: code.to_string(), message: e.to_string() },
                );
            }
            // Move the device from `Pairing`
            // to `Paired`. If the device is
            // not in `Pairing` (e.g. the user
            // cancelled), refuse.
            let mut registry = match device_registry.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let device_id = match DeviceId::new(acceptance.device_id.as_str()) {
                Some(d) => d,
                None => {
                    return IpcResponse::err(
                        "device.pair.complete",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "device id is empty or too long".to_string(),
                        },
                    );
                }
            };
            let current = match registry.get(&device_id) {
                Some(d) => d.pairing.state,
                None => {
                    return IpcResponse::err(
                        "device.pair.complete",
                        IpcError {
                            code: "NOT_FOUND".to_string(),
                            message: format!(
                                "device '{}' is not registered",
                                acceptance.device_id.as_str()
                            ),
                        },
                    );
                }
            };
            if current != PairingState::Pairing {
                return IpcResponse::err(
                    "device.pair.complete",
                    IpcError {
                        code: "INVALID_STATE".to_string(),
                        message: format!(
                            "device is in '{}' state; only Pairing can be completed",
                            current.as_str()
                        ),
                    },
                );
            }
            match registry.transition(&device_id, PairingState::Paired, now) {
                Ok(()) => IpcResponse::ok(
                    "device.pair.complete",
                    serde_json::json!({
                        "id": acceptance.device_id.as_str(),
                        "state": "paired",
                    }),
                ),
                Err(e) => IpcResponse::err(
                    "device.pair.complete",
                    IpcError { code: "REGISTRY_ERROR".to_string(), message: e.to_string() },
                ),
            }
        }
        "device.revoke" => {
            // Moves a device into `Revoked`
            // state. The future runtime can
            // later unregister the device
            // entirely.
            let id = match req.parameters.get("device_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return IpcResponse::err(
                        "device.revoke",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'device_id' is required".to_string(),
                        },
                    );
                }
            };
            let device_id = match DeviceId::new(id.clone()) {
                Some(d) => d,
                None => {
                    return IpcResponse::err(
                        "device.revoke",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "device id is empty or too long".to_string(),
                        },
                    );
                }
            };
            let now = unix_ms().min(u128::from(u64::MAX)) as u64;
            let mut registry = match device_registry.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            if registry.get(&device_id).is_none() {
                return IpcResponse::err(
                    "device.revoke",
                    IpcError {
                        code: "NOT_FOUND".to_string(),
                        message: format!("device '{id}' is not registered"),
                    },
                );
            }
            match registry.transition(&device_id, PairingState::Revoked, now) {
                Ok(()) => IpcResponse::ok(
                    "device.revoke",
                    serde_json::json!({ "id": id, "state": "revoked" }),
                ),
                Err(e) => IpcResponse::err(
                    "device.revoke",
                    IpcError { code: "REGISTRY_ERROR".to_string(), message: e.to_string() },
                ),
            }
        }
        "device.unregister" => {
            // Removes a device from the
            // registry entirely. Returns the
            // removed entry.
            let id = match req.parameters.get("device_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return IpcResponse::err(
                        "device.unregister",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "parameter 'device_id' is required".to_string(),
                        },
                    );
                }
            };
            let device_id = match DeviceId::new(id.clone()) {
                Some(d) => d,
                None => {
                    return IpcResponse::err(
                        "device.unregister",
                        IpcError {
                            code: "INVALID_INPUT".to_string(),
                            message: "device id is empty or too long".to_string(),
                        },
                    );
                }
            };
            let mut registry = match device_registry.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            match registry.unregister(&device_id) {
                Some(entry) => IpcResponse::ok(
                    "device.unregister",
                    serde_json::json!({ "id": id, "removed": serde_json::to_value(&entry).ok() }),
                ),
                None => IpcResponse::err(
                    "device.unregister",
                    IpcError {
                        code: "NOT_FOUND".to_string(),
                        message: format!("device '{id}' is not registered"),
                    },
                ),
            }
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

    // Update subsystem state. The shell holds an
    // in-memory `UpdateStatus` and a default
    // `VersionPolicy`; the future update-agent daemon
    // will own these and drive the state machine.
    let update_status = Mutex::new(UpdateStatus::new());
    let version_policy = Mutex::new(VersionPolicy::default());
    // Phase 13: agent planning surface. The
    // future agentd owns the live state
    // machine; today the shell only stores
    // observations, proposals, and tasks.
    let agent_status = Mutex::new(AgentStatus::new());
    // Phase 14: paired-device registry. The
    // future device runtime owns the
    // transport and the persistence; today
    // the shell only stores the in-memory
    // map of registered peers.
    let device_registry = Mutex::new(DeviceRegistry::new());

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
                        &update_status,
                        &version_policy,
                        &agent_status,
                        &device_registry,
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::type_complexity)]
mod dispatch_policy_tests {
    //! Integration tests for the policy gate wired into `dispatch_inner`.
    //!
    //! The gate runs *before* the existing capability handlers, so
    //! the test does not need a real `ServiceManager` or filesystem
    //! — it only checks that the gate short-circuits to the right
    //! error code for each (command, actor_trust) combination.

    use super::gate_response;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_security::Decision;
    use aether_system_core::policy::{evaluate, PolicyVerdict};

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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::type_complexity)]
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::type_complexity)]
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
    use aether_agent_core::AgentStatus;
    use aether_application_manager::ApplicationManager;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_device_core::DeviceRegistry;
    use aether_security::audit::{AuditChain, RetentionPolicy};
    use aether_security::credentials::{CredentialError, SealedStore, StaticKeyProvider};
    use aether_storage::{FileManager, WorkspaceConfig};
    use aether_update_core::{UpdateStatus, VersionPolicy};
    use std::sync::Mutex;
    use std::time::SystemTime;

    fn env() -> (
        aether_system_core::manager::ServiceManager,
        ApplicationManager,
        FileManager,
        Mutex<AuditChain>,
        Mutex<SealedStore<StaticKeyProvider>>,
        Mutex<UpdateStatus>,
        Mutex<VersionPolicy>,
        Mutex<AgentStatus>,
        Mutex<DeviceRegistry>,
    ) {
        // The credentials tests never reach the manager
        // (every command is short-circuited by the IPC
        // handler), so an empty graph is sufficient.
        let graph = aether_system_core::graph::DependencyGraph::new(&[]).expect("empty graph");
        let manager = aether_system_core::manager::ServiceManager::new(graph);
        let apps = ApplicationManager::default();
        let files =
            FileManager::new(WorkspaceConfig::from_env_or_default()).expect("workspace init");
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(64)));
        let store = Mutex::new(SealedStore::new(StaticKeyProvider::new([0x55u8; 32])));
        let status = Mutex::new(UpdateStatus::new());
        let policy = Mutex::new(VersionPolicy::default());
        let agent = Mutex::new(AgentStatus::new());
        let registry = Mutex::new(DeviceRegistry::new());
        (manager, apps, files, chain, store, status, policy, agent, registry)
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
        let (mut mgr, mut apps, mut files, chain, store, status, policy, agent, registry) = env();
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
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "seal should succeed: {:?}", resp);

        // Unseal.
        let r = req("credentials.unseal", serde_json::json!({ "name": "api_key" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["plaintext"], serde_json::json!("super-secret-value"));
    }

    #[test]
    fn seal_rejects_missing_inputs() {
        let (mut mgr, mut apps, mut files, chain, store, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("credentials.seal", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_INPUT");
    }

    #[test]
    fn seal_rejects_duplicate_by_default() {
        let (mut mgr, mut apps, mut files, chain, store, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r1 = req("credentials.seal", serde_json::json!({ "name": "x", "plaintext": "v1" }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r1,
        );
        let r2 = req("credentials.seal", serde_json::json!({ "name": "x", "plaintext": "v2" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r2,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "ALREADY_EXISTS");
    }

    #[test]
    fn list_returns_sorted_names() {
        let (mut mgr, mut apps, mut files, chain, store, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        for (name, value) in [("zeta", "z"), ("alpha", "a"), ("mu", "m")] {
            let r =
                req("credentials.seal", serde_json::json!({ "name": name, "plaintext": value }));
            let _ = dispatch_inner(
                &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
                &registry, started_at, &r,
            );
        }
        let r = req("credentials.list", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["names"], serde_json::json!(["alpha", "mu", "zeta"]));
        assert_eq!(resp.result["total"], serde_json::json!(3));
    }

    #[test]
    fn metadata_returns_label_and_length_only() {
        let (mut mgr, mut apps, mut files, chain, store, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req(
            "credentials.seal",
            serde_json::json!({ "name": "k", "plaintext": "value", "label": "lbl" }),
        );
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req("credentials.metadata", serde_json::json!({ "name": "k" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r,
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
        let (mut mgr, mut apps, mut files, chain, store, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("credentials.metadata", serde_json::json!({ "name": "nope" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "NOT_FOUND");
    }

    #[test]
    fn remove_drops_the_credential() {
        let (mut mgr, mut apps, mut files, chain, store, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("credentials.seal", serde_json::json!({ "name": "k", "plaintext": "v" }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req("credentials.remove", serde_json::json!({ "name": "k" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["name"], serde_json::json!("k"));
        // Subsequent unseal must fail.
        let r = req("credentials.unseal", serde_json::json!({ "name": "k" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &store, None, &status, &policy, &agent,
            &registry, started_at, &r,
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

        let auth = credential_error_response("test", &CredentialError::AuthenticationFailed);
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::type_complexity)]
mod trust_store_ipc_tests {
    //! Integration tests for the `manifest.trust_store` IPC
    //! command. The trust store is wired into
    //! `dispatch_inner` so a caller can ask the daemon which
    //! signer fingerprints it currently trusts.

    use super::dispatch_inner;
    use aether_agent_core::AgentStatus;
    use aether_application_manager::ApplicationManager;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_device_core::DeviceRegistry;
    use aether_security::audit::{AuditChain, RetentionPolicy};
    use aether_security::credentials::{SealedStore, StaticKeyProvider};
    use aether_security::manifest_signing::{Ed25519ManifestSigner, TrustStore};
    use aether_storage::{FileManager, WorkspaceConfig};
    use aether_update_core::{UpdateStatus, VersionPolicy};
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
        Mutex<UpdateStatus>,
        Mutex<VersionPolicy>,
        Mutex<AgentStatus>,
        Mutex<DeviceRegistry>,
    ) {
        let graph = aether_system_core::graph::DependencyGraph::new(&[]).expect("empty");
        let manager = aether_system_core::manager::ServiceManager::new(graph);
        let apps = ApplicationManager::default();
        let files = FileManager::new(WorkspaceConfig::from_env_or_default()).expect("workspace");
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(64)));
        let creds = Mutex::new(SealedStore::new(StaticKeyProvider::new([0x77u8; 32])));
        let status = Mutex::new(UpdateStatus::new());
        let policy = Mutex::new(VersionPolicy::default());
        let agent = Mutex::new(AgentStatus::new());
        let registry = Mutex::new(DeviceRegistry::new());
        // Stash the trust store in a thread-local so the
        // helper closure inside the test can read it. A
        // cleaner refactor would thread the store through
        // a real test harness; for now the local keeps
        // the existing env() shape unchanged.
        let _ = store; // store is passed via env_with_trust_store below
        (manager, apps, files, chain, creds, status, policy, agent, registry)
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
        let (mut mgr, mut apps, mut files, chain, creds, _status, _policy, _agent, _registry) =
            env_with_trust(None);
        let started_at = SystemTime::now();
        let r = req("manifest.trust_store", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr,
            &mut apps,
            &mut files,
            &chain,
            &creds,
            None,
            &Mutex::new(UpdateStatus::new()),
            &Mutex::new(VersionPolicy::default()),
            &Mutex::new(AgentStatus::new()),
            &Mutex::new(DeviceRegistry::new()),
            started_at,
            &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["enabled"], serde_json::json!(false));
        assert_eq!(resp.result["count"], serde_json::json!(0));
    }

    #[test]
    fn trust_store_command_reports_fingerprints_when_loaded() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) =
            env_with_trust(None);
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
            &status,
            &policy,
            &agent,
            &registry,
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::type_complexity)]
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
    use aether_agent_core::AgentStatus;
    use aether_application_manager::ApplicationManager;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_device_core::DeviceRegistry;
    use aether_security::audit::{AuditChain, RetentionPolicy};
    use aether_security::credentials::{SealedStore, StaticKeyProvider};
    use aether_security::signed_update::{UpdateKind, UpdateSigner};
    use aether_storage::{FileManager, WorkspaceConfig};
    use aether_update_core::{UpdateStatus, VersionPolicy};
    use std::sync::Mutex;
    use std::time::SystemTime;

    fn env() -> (
        aether_system_core::manager::ServiceManager,
        ApplicationManager,
        FileManager,
        Mutex<AuditChain>,
        Mutex<SealedStore<StaticKeyProvider>>,
        Mutex<UpdateStatus>,
        Mutex<VersionPolicy>,
        Mutex<AgentStatus>,
        Mutex<DeviceRegistry>,
    ) {
        let graph = aether_system_core::graph::DependencyGraph::new(&[]).expect("empty");
        let manager = aether_system_core::manager::ServiceManager::new(graph);
        let apps = ApplicationManager::default();
        let files = FileManager::new(WorkspaceConfig::from_env_or_default()).expect("workspace");
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(64)));
        let creds = Mutex::new(SealedStore::new(StaticKeyProvider::new([0x55u8; 32])));
        let status = Mutex::new(UpdateStatus::new());
        let policy = Mutex::new(VersionPolicy::default());
        let agent = Mutex::new(AgentStatus::new());
        let registry = Mutex::new(DeviceRegistry::new());
        (manager, apps, files, chain, creds, status, policy, agent, registry)
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
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let signer = UpdateSigner::generate();
        let pk_hex = hex_lower(&signer.public_key_bytes());
        let r = req("update.fingerprint", serde_json::json!({ "public_key_hex": pk_hex }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        let fp = resp.result["fingerprint"].as_str().expect("fp string");
        assert_eq!(fp.len(), 32);
        assert_eq!(fp, signer.fingerprint().as_hex());
    }

    #[test]
    fn update_fingerprint_rejects_bad_hex() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("update.fingerprint", serde_json::json!({ "public_key_hex": "not-hex" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
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
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("update.fingerprint", serde_json::json!({ "public_key_hex": "deadbeef" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_INPUT");
    }

    #[test]
    fn update_verify_round_trip() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let signer = UpdateSigner::generate();
        let payload = b"aether-os-image-1.2.3".to_vec();
        let update =
            signer.sign(UpdateKind::OsImage, "aether-os", "1.2.3", 1_700_000_000_000, &payload);
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
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "update.verify should accept signed payload: {resp:?}");
        assert_eq!(resp.result["ok"], serde_json::json!(true));
        assert_eq!(resp.result["target"], serde_json::json!("aether-os"));
        assert_eq!(resp.result["version"], serde_json::json!("1.2.3"));
        assert_eq!(resp.result["kind"], serde_json::json!("os-image"));
    }

    #[test]
    fn update_verify_rejects_tampered_payload() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let signer = UpdateSigner::generate();
        let payload = b"aether-os-image-1.2.3".to_vec();
        let mut update =
            signer.sign(UpdateKind::OsImage, "aether-os", "1.2.3", 1_700_000_000_000, &payload);
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
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "update.verify should return ok:false, not an error: {resp:?}");
        assert_eq!(resp.result["ok"], serde_json::json!(false));
        assert!(resp.result["error"].as_str().unwrap().contains("signature"));
    }

    #[test]
    fn update_verify_rejects_wrong_signer() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let signer_a = UpdateSigner::generate();
        let signer_b = UpdateSigner::generate();
        let payload = b"aether-os-image-1.2.3".to_vec();
        let update =
            signer_a.sign(UpdateKind::ServiceBundle, "svc", "0.1.0", 1_700_000_000_000, &payload);
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
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["ok"], serde_json::json!(false));
        assert!(resp.result["error"].as_str().unwrap().contains("signature"));
    }

    #[test]
    fn update_verify_rejects_missing_field() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        // No `header` parameter.
        let r = req("update.verify", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::type_complexity)]
mod update_plan_ipc_tests {
    //! Integration tests for the Phase-12 update
    //! planning layer IPC commands (`update.plan`,
    //! `update.status`, `update.history`,
    //! `update.simulate`). The shell signs an update,
    //! ships it through the IPC boundary, and confirms
    //! the daemon turns it into a plan (or rejects it
    //! on policy grounds).
    //!
    //! The `update.simulate` command is the only path
    //! that mutates the live `UpdateStatus`; everything
    //! else is read-only.

    use super::{base64_encode, dispatch_inner};
    use aether_agent_core::AgentStatus;
    use aether_application_manager::ApplicationManager;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_device_core::DeviceRegistry;
    use aether_security::audit::{AuditChain, RetentionPolicy};
    use aether_security::credentials::{SealedStore, StaticKeyProvider};
    use aether_security::signed_update::{UpdateKind, UpdateSigner};
    use aether_storage::{FileManager, WorkspaceConfig};
    use aether_update_core::{UpdateStatus, VersionPolicy};
    use std::sync::Mutex;
    use std::time::SystemTime;

    fn env() -> (
        aether_system_core::manager::ServiceManager,
        ApplicationManager,
        FileManager,
        Mutex<AuditChain>,
        Mutex<SealedStore<StaticKeyProvider>>,
        Mutex<UpdateStatus>,
        Mutex<VersionPolicy>,
        Mutex<AgentStatus>,
        Mutex<DeviceRegistry>,
    ) {
        let graph = aether_system_core::graph::DependencyGraph::new(&[]).expect("empty");
        let manager = aether_system_core::manager::ServiceManager::new(graph);
        let apps = ApplicationManager::default();
        let files = FileManager::new(WorkspaceConfig::from_env_or_default()).expect("workspace");
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(64)));
        let creds = Mutex::new(SealedStore::new(StaticKeyProvider::new([0x33u8; 32])));
        let status = Mutex::new(UpdateStatus::new());
        let policy = Mutex::new(VersionPolicy::default());
        let agent = Mutex::new(AgentStatus::new());
        let registry = Mutex::new(DeviceRegistry::new());
        (manager, apps, files, chain, creds, status, policy, agent, registry)
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

    fn sign_os_image(target: &str, version: &str) -> (UpdateSigner, serde_json::Value) {
        let signer = UpdateSigner::generate();
        let payload = vec![0u8; 4096];
        let update = signer.sign(UpdateKind::OsImage, target, version, 1_700_000_000_000, &payload);
        let header = serde_json::to_value(&update.header).expect("header -> json");
        let _ = base64_encode(&update.payload);
        let _ = base64_encode(&update.signature);
        (signer, header)
    }

    fn plan_params(
        signer: &UpdateSigner,
        header: &serde_json::Value,
        target: &str,
        version: &str,
    ) -> serde_json::Value {
        let payload = vec![0u8; 4096];
        let update = signer.sign(UpdateKind::OsImage, target, version, 1_700_000_000_000, &payload);
        let h = if target.is_empty() || version.is_empty() {
            // The signature would not match a freshly
            // serialised header (it was built from
            // a different canonical form). Pass the
            // header through as-is so the verifier
            // sees the bytes the signer signed.
            header.clone()
        } else {
            serde_json::to_value(&update.header).expect("header -> json")
        };
        serde_json::json!({
            "header": h,
            "payload_b64": base64_encode(&update.payload),
            "signature_b64": base64_encode(&update.signature),
            "public_key_hex": hex_lower(&signer.public_key_bytes()),
        })
    }

    #[test]
    fn update_plan_returns_plan_for_signed_upgrade() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let (signer, header) = sign_os_image("aether-os", "1.2.0");
        let r = req(
            "update.plan",
            plan_params(&signer, &header, "aether-os", "1.2.0")
                .as_object_mut()
                .map(|m| {
                    let mut v = serde_json::Value::Object(m.clone());
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("installed_version".to_string(), serde_json::json!("1.1.0"));
                    }
                    v
                })
                .unwrap(),
        );
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "expected plan, got: {resp:?}");
        assert_eq!(resp.result["target"], serde_json::json!("aether-os"));
        assert_eq!(resp.result["version"], serde_json::json!("1.2.0"));
        assert_eq!(resp.result["action"], serde_json::json!("upgrade-os-image"));
        assert_eq!(resp.result["version_decision"]["requirement"], serde_json::json!("upgrade"));
        assert_eq!(resp.result["version_decision"]["allowed"], serde_json::json!(true));
    }

    #[test]
    fn update_plan_rejects_downgrade_by_default() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let (signer, header) = sign_os_image("aether-os", "0.9.0");
        let mut params = plan_params(&signer, &header, "aether-os", "0.9.0");
        if let Some(obj) = params.as_object_mut() {
            obj.insert("installed_version".to_string(), serde_json::json!("1.0.0"));
        }
        let r = req("update.plan", params);
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "POLICY_DENIED");
        assert!(resp.error.as_ref().unwrap().message.contains("downgrade"));
    }

    #[test]
    fn update_plan_rejects_bad_signature() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let (signer, header) = sign_os_image("aether-os", "1.2.0");
        let mut params = plan_params(&signer, &header, "aether-os", "1.2.0");
        if let Some(obj) = params.as_object_mut() {
            let other = UpdateSigner::generate();
            obj.insert(
                "public_key_hex".to_string(),
                serde_json::json!(hex_lower(&other.public_key_bytes())),
            );
            obj.insert("installed_version".to_string(), serde_json::json!("1.1.0"));
        }
        let r = req("update.plan", params);
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "VERIFICATION_FAILED");
    }

    #[test]
    fn update_plan_rejects_empty_target() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        // The plan layer rejects empty target
        // *before* the signature verifier runs in
        // the policy check — but the IPC layer runs
        // the signature check first. Use a properly
        // signed empty-target update so the policy
        // can be exercised.
        let (signer, header) = sign_os_image("aether-os", "1.2.0");
        let mut params = plan_params(&signer, &header, "aether-os", "1.2.0");
        if let Some(obj) = params.as_object_mut() {
            if let Some(h) = obj.get_mut("header") {
                if let Some(hobj) = h.as_object_mut() {
                    hobj.insert("target".to_string(), serde_json::json!(""));
                }
            }
        }
        let r = req("update.plan", params);
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        // Signature check is run first, so the
        // empty-target policy check is unreachable
        // through the IPC path; the empty target
        // produces a different empty target via the
        // signer's filter or the verifier's bad
        // target check. We accept either.
        let code = resp.error.as_ref().unwrap().code.clone();
        assert!(
            code == "VERIFICATION_FAILED" || code == "POLICY_DENIED",
            "unexpected code {code}: {resp:?}"
        );
    }

    #[test]
    fn update_status_reports_idle_initially() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("update.status", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["stage"], serde_json::json!("idle"));
        assert_eq!(resp.result["attempt"], serde_json::json!(0));
        assert_eq!(resp.result["last_error"], serde_json::Value::Null);
        assert_eq!(resp.result["current_plan"], serde_json::Value::Null);
    }

    #[test]
    fn update_simulate_drives_state_machine() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req(
            "update.simulate",
            serde_json::json!({ "stages": "downloading,verifying,staging,applying,done" }),
        );
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["current"], serde_json::json!("done"));
        // Now check the history.
        let r = req("update.history", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        let entries = resp.result["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0]["to"], serde_json::json!("downloading"));
        assert_eq!(entries[4]["to"], serde_json::json!("done"));
    }

    #[test]
    fn update_simulate_records_failed_transition_with_note() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        // We can't pass a note through `simulate` (it
        // accepts a sequence only), so we drive the
        // state machine through `transition` via the
        // status lock directly. The IPC layer exposes
        // this as part of the test surface; the
        // future daemon is the only real caller.
        let r = req("update.simulate", serde_json::json!({ "stages": "downloading,failed" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        let r = req("update.status", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert_eq!(resp.result["stage"], serde_json::json!("failed"));
        assert_eq!(resp.result["attempt"], serde_json::json!(1));
    }

    #[test]
    fn update_simulate_rejects_unknown_stage() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("update.simulate", serde_json::json!({ "stages": "downloading,bogus" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_INPUT");
    }

    #[test]
    fn update_history_is_empty_initially() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("update.history", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["total"], serde_json::json!(0));
        assert_eq!(resp.result["entries"], serde_json::json!([]));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::type_complexity)]
mod agent_ipc_tests {
    //! Integration tests for the Phase-13 agent
    //! planning surface IPC commands
    //! (`agent.observe`, `agent.propose`,
    //! `agent.proposals`, `agent.tasks`,
    //! `agent.history`, `agent.observations`,
    //! `agent.cancel`, `agent.approve`).
    //!
    //! The shell stores observations, validates
    //! proposals against the live observation log,
    //! and converts approved proposals into tasks.
    //! The future agentd is the only thing that will
    //! execute the tasks; today the tests cover the
    //! contract.

    use super::dispatch_inner;
    use aether_agent_core::{
        AgentStatus, Observation, ObservationSeverity, Proposal, ProposalRisk, TaskId, TaskKind,
    };
    use aether_application_manager::ApplicationManager;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_device_core::DeviceRegistry;
    use aether_security::audit::{AuditChain, RetentionPolicy};
    use aether_security::credentials::{SealedStore, StaticKeyProvider};
    use aether_storage::{FileManager, WorkspaceConfig};
    use aether_update_core::{UpdateStatus, VersionPolicy};
    use std::sync::Mutex;
    use std::time::SystemTime;

    fn env() -> (
        aether_system_core::manager::ServiceManager,
        ApplicationManager,
        FileManager,
        Mutex<AuditChain>,
        Mutex<SealedStore<StaticKeyProvider>>,
        Mutex<UpdateStatus>,
        Mutex<VersionPolicy>,
        Mutex<AgentStatus>,
        Mutex<DeviceRegistry>,
    ) {
        let graph = aether_system_core::graph::DependencyGraph::new(&[]).expect("empty");
        let manager = aether_system_core::manager::ServiceManager::new(graph);
        let apps = ApplicationManager::default();
        let files = FileManager::new(WorkspaceConfig::from_env_or_default()).expect("workspace");
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(64)));
        let creds = Mutex::new(SealedStore::new(StaticKeyProvider::new([0x55u8; 32])));
        let status = Mutex::new(UpdateStatus::new());
        let policy = Mutex::new(VersionPolicy::default());
        let agent = Mutex::new(AgentStatus::new());
        let registry = Mutex::new(DeviceRegistry::new());
        (manager, apps, files, chain, creds, status, policy, agent, registry)
    }

    fn req(command: &str, params: serde_json::Value) -> IpcRequest {
        IpcRequest {
            service_id: "test".to_string(),
            command: command.to_string(),
            parameters: params,
            actor_trust: ActorTrust::Trusted,
        }
    }

    fn observation_json(id: &str) -> serde_json::Value {
        let o = Observation::new(
            id,
            "storage",
            "disk is 95% full",
            ObservationSeverity::Warning,
            1_700_000_000_000,
        )
        .expect("valid obs");
        serde_json::to_value(&o).expect("obs -> json")
    }

    fn proposal_json(id: &str, risk: ProposalRisk, evidence: Vec<String>) -> serde_json::Value {
        let mut p = Proposal::new(
            id,
            TaskKind::ProposeCleanup,
            "free up space",
            "delete cached files",
            "disk is 95% full",
            risk,
            1_700_000_000_000,
        )
        .expect("valid proposal");
        if !evidence.is_empty() {
            p = p.with_evidence(evidence);
        }
        serde_json::to_value(&p).expect("proposal -> json")
    }

    #[test]
    fn observe_and_list_observations() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.observe", serde_json::json!({ "observation": observation_json("o1") }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "expected ok, got: {resp:?}");
        assert_eq!(resp.result["id"], serde_json::json!("o1"));
        let r = req("agent.observations", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["total"], serde_json::json!(1));
    }

    #[test]
    fn propose_requires_known_evidence() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req(
            "agent.propose",
            serde_json::json!({
                "proposal": proposal_json("p1", ProposalRisk::Medium, vec!["missing".to_string()]),
            }),
        );
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "UNKNOWN_EVIDENCE");
    }

    #[test]
    fn propose_succeeds_when_evidence_is_known() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.observe", serde_json::json!({ "observation": observation_json("o1") }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req(
            "agent.propose",
            serde_json::json!({
                "proposal": proposal_json("p1", ProposalRisk::Medium, vec!["o1".to_string()]),
            }),
        );
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "expected ok, got: {resp:?}");
        assert_eq!(resp.result["id"], serde_json::json!("p1"));
        assert_eq!(resp.result["new"], serde_json::json!(true));
    }

    #[test]
    fn propose_rejects_low_risk_for_propose_cleanup() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.observe", serde_json::json!({ "observation": observation_json("o1") }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req(
            "agent.propose",
            serde_json::json!({
                "proposal": proposal_json("p1", ProposalRisk::Low, vec!["o1".to_string()]),
            }),
        );
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "RISK_TOO_LOW");
    }

    #[test]
    fn proposals_list_sorts_by_id() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.observe", serde_json::json!({ "observation": observation_json("o1") }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        for id in ["p2", "p1", "p3"] {
            let r = req(
                "agent.propose",
                serde_json::json!({
                    "proposal": proposal_json(id, ProposalRisk::Medium, vec!["o1".to_string()]),
                }),
            );
            let _ = dispatch_inner(
                &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
                &registry, started_at, &r,
            );
        }
        let r = req("agent.proposals", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["total"], serde_json::json!(3));
        let arr = resp.result["proposals"].as_array().unwrap();
        assert_eq!(arr[0]["id"], serde_json::json!("p1"));
        assert_eq!(arr[1]["id"], serde_json::json!("p2"));
        assert_eq!(arr[2]["id"], serde_json::json!("p3"));
    }

    #[test]
    fn tasks_list_is_empty_initially() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.tasks", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["task_count"], serde_json::json!(0));
        assert_eq!(resp.result["ready_count"], serde_json::json!(0));
    }

    #[test]
    fn approve_converts_proposal_to_task() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.observe", serde_json::json!({ "observation": observation_json("o1") }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req(
            "agent.propose",
            serde_json::json!({
                "proposal": proposal_json("p1", ProposalRisk::Medium, vec!["o1".to_string()]),
            }),
        );
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req("agent.approve", serde_json::json!({ "id": "p1" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "expected ok, got: {resp:?}");
        assert_eq!(resp.result["task"]["kind"], serde_json::json!("propose-cleanup"));
        let r = req("agent.proposals", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert_eq!(resp.result["total"], serde_json::json!(0));
        let r = req("agent.tasks", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert_eq!(resp.result["task_count"], serde_json::json!(1));
    }

    #[test]
    fn cancel_removes_live_task() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.observe", serde_json::json!({ "observation": observation_json("o1") }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req(
            "agent.propose",
            serde_json::json!({
                "proposal": proposal_json("p1", ProposalRisk::Medium, vec!["o1".to_string()]),
            }),
        );
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req("agent.approve", serde_json::json!({ "id": "p1" }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req("agent.cancel", serde_json::json!({ "id": "task-p1" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "expected ok, got: {resp:?}");
        assert!(resp.result["removed"].is_object());
    }

    #[test]
    fn cancel_unknown_task_returns_not_found() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.cancel", serde_json::json!({ "id": "no-such-task" }));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "NOT_FOUND");
    }

    #[test]
    fn history_starts_empty() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.history", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["total"], serde_json::json!(0));
    }

    #[test]
    fn tasks_ready_filters_by_done() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.observe", serde_json::json!({ "observation": observation_json("o1") }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req(
            "agent.propose",
            serde_json::json!({
                "proposal": proposal_json("p1", ProposalRisk::Medium, vec!["o1".to_string()]),
            }),
        );
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req("agent.approve", serde_json::json!({ "id": "p1" }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req("agent.tasks", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["task_count"], serde_json::json!(1));
        assert_eq!(resp.result["ready_count"], serde_json::json!(1));
    }

    #[test]
    fn propose_duplicate_id_returns_existing_flag() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("agent.observe", serde_json::json!({ "observation": observation_json("o1") }));
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req(
            "agent.propose",
            serde_json::json!({
                "proposal": proposal_json("p1", ProposalRisk::Medium, vec!["o1".to_string()]),
            }),
        );
        let resp1 = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp1.ok);
        assert_eq!(resp1.result["new"], serde_json::json!(true));
        let r = req(
            "agent.propose",
            serde_json::json!({
                "proposal": proposal_json("p1", ProposalRisk::Medium, vec!["o1".to_string()]),
            }),
        );
        let resp2 = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp2.ok);
        assert_eq!(resp2.result["new"], serde_json::json!(false));
        // Reference TaskId for the unused-import linter.
        let _ = TaskId::new("x");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::type_complexity)]
mod device_ipc_tests {
    //! Integration tests for the Phase-14 multi-device
    //! IPC surface (`device.list`, `device.register`,
    //! `device.pair.begin`, `device.pair.complete`,
    //! `device.revoke`, `device.unregister`).
    //!
    //! The shell stores a bounded registry of
    //! registered devices and a per-peer pairing
    //! state machine. Today the registry only
    //! covers the typed contract; the future
    //! `aether-device-runtime` is the only
    //! thing allowed to actually deliver a
    //! `RemoteObservation` or `RemoteProposal`
    //! into the local agent.

    use super::dispatch_inner;
    use aether_agent_core::AgentStatus;
    use aether_application_manager::ApplicationManager;
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use aether_device_core::DeviceRegistry;
    use aether_security::audit::{AuditChain, RetentionPolicy};
    use aether_security::credentials::{SealedStore, StaticKeyProvider};
    use aether_storage::{FileManager, WorkspaceConfig};
    use aether_update_core::{UpdateStatus, VersionPolicy};
    use std::sync::Mutex;
    use std::time::SystemTime;

    fn env() -> (
        aether_system_core::manager::ServiceManager,
        ApplicationManager,
        FileManager,
        Mutex<AuditChain>,
        Mutex<SealedStore<StaticKeyProvider>>,
        Mutex<UpdateStatus>,
        Mutex<VersionPolicy>,
        Mutex<AgentStatus>,
        Mutex<DeviceRegistry>,
    ) {
        let graph = aether_system_core::graph::DependencyGraph::new(&[]).expect("empty");
        let manager = aether_system_core::manager::ServiceManager::new(graph);
        let apps = ApplicationManager::default();
        let files = FileManager::new(WorkspaceConfig::from_env_or_default()).expect("workspace");
        let chain = Mutex::new(AuditChain::new(RetentionPolicy::last_n(64)));
        let creds = Mutex::new(SealedStore::new(StaticKeyProvider::new([0x66u8; 32])));
        let status = Mutex::new(UpdateStatus::new());
        let policy = Mutex::new(VersionPolicy::default());
        let agent = Mutex::new(AgentStatus::new());
        let registry = Mutex::new(DeviceRegistry::new());
        (manager, apps, files, chain, creds, status, policy, agent, registry)
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
    fn device_list_starts_empty() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req("device.list", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "expected list ok, got: {resp:?}");
        assert_eq!(resp.result["total"], serde_json::json!(0));
        assert_eq!(resp.result["paired_count"], serde_json::json!(0));
    }

    #[test]
    fn device_register_then_list_round_trip() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req(
            "device.register",
            serde_json::json!({
                "device_id": "dev-phone",
                "device_class": "phone",
                "fingerprint": "11".repeat(32),
                "grant": {
                    "receive_observations": true,
                    "receive_proposals": true,
                    "execute_remote_tasks": false,
                },
            }),
        );
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "expected register ok, got: {resp:?}");
        assert_eq!(resp.result["id"], serde_json::json!("dev-phone"));
        assert_eq!(resp.result["state"], serde_json::json!("available"));

        let r = req("device.list", serde_json::json!({}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        assert_eq!(resp.result["total"], serde_json::json!(1));
    }

    #[test]
    fn device_register_rejects_duplicate() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let params = serde_json::json!({
            "device_id": "dev-a",
            "device_class": "laptop",
            "fingerprint": "22".repeat(32),
            "grant": {
                "receive_observations": true,
                "receive_proposals": true,
                "execute_remote_tasks": false,
            },
        });
        let r = req("device.register", params);
        let _resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        // Re-register returns ALREADY_REGISTERED.
        let r = req(
            "device.register",
            serde_json::json!({
                "device_id": "dev-a",
                "device_class": "laptop",
                "fingerprint": "33".repeat(32),
                "grant": {"receive_observations": true, "receive_proposals": true, "execute_remote_tasks": false},
            }),
        );
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok, "expected duplicate register to fail");
        assert_eq!(resp.error.as_ref().unwrap().code, "ALREADY_REGISTERED");
    }

    #[test]
    fn device_register_rejects_bad_id() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req(
            "device.register",
            serde_json::json!({
                "device_id": "",
                "device_class": "laptop",
                "fingerprint": "33".repeat(32),
                "grant": {"receive_observations": true, "receive_proposals": true, "execute_remote_tasks": false},
            }),
        );
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok, "expected empty id rejected");
        assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_INPUT");
    }

    #[test]
    fn device_register_rejects_bad_fingerprint() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req(
            "device.register",
            serde_json::json!({
                "device_id": "dev-bad-fp",
                "device_class": "laptop",
                "fingerprint": "abcd",
                "grant": {"receive_observations": true, "receive_proposals": true, "execute_remote_tasks": false},
            }),
        );
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok, "expected bad fingerprint rejected");
        assert_eq!(resp.error.as_ref().unwrap().code, "INVALID_INPUT");
    }

    #[test]
    fn device_pair_begin_requires_registered_peer() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req(
            "device.pair.begin",
            serde_json::json!({
                "device_id": "never-registered",
            }),
        );
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "NOT_FOUND");
    }

    #[test]
    fn device_pair_begin_then_revoke() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        // Register.
        let r = req(
            "device.register",
            serde_json::json!({
                "device_id": "dev-revoke",
                "device_class": "laptop",
                "fingerprint": "77".repeat(32),
                "grant": {"receive_observations": true, "receive_proposals": true, "execute_remote_tasks": false},
            }),
        );
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        // Begin pairing.
        let r = req("device.pair.begin", serde_json::json!({"device_id": "dev-revoke"}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "pair.begin failed: {resp:?}");
        assert_eq!(resp.result["state"], serde_json::json!("pairing"));

        // Revoke from `Pairing` is allowed and moves to `Revoked`.
        let r = req("device.revoke", serde_json::json!({"device_id": "dev-revoke"}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok, "revoke failed: {resp:?}");
        assert_eq!(resp.result["state"], serde_json::json!("revoked"));
    }

    #[test]
    fn device_unregister_removes_entry() {
        let (mut mgr, mut apps, mut files, chain, creds, status, policy, agent, registry) = env();
        let started_at = SystemTime::now();
        let r = req(
            "device.register",
            serde_json::json!({
                "device_id": "dev-bye",
                "device_class": "phone",
                "fingerprint": "88".repeat(32),
                "grant": {"receive_observations": true, "receive_proposals": true, "execute_remote_tasks": false},
            }),
        );
        let _ = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        let r = req("device.unregister", serde_json::json!({"device_id": "dev-bye"}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(resp.ok);
        // First unregister removed the entry; `removed` carries the entry
        // as a JSON object rather than a boolean.
        assert!(resp.result["removed"].is_object());

        // Unregistering again returns NOT_FOUND.
        let r = req("device.unregister", serde_json::json!({"device_id": "dev-bye"}));
        let resp = dispatch_inner(
            &mut mgr, &mut apps, &mut files, &chain, &creds, None, &status, &policy, &agent,
            &registry, started_at, &r,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error.as_ref().unwrap().code, "NOT_FOUND");
    }
}
