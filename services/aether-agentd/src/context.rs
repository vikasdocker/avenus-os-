// Aether Context Engine - structured system state for the agent.
//
// Aggregates running apps, installed apps, window state, and service health
// into a minimal, predictable snapshot. The agent queries this via the
// existing control plane (context.get) or directly via the surface server
// for windows. Raw system data is never sent to the model; only the
// filtered snapshot required for the current task.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// Single window as seen by the context engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowSnapshot {
    pub id: u64,
    pub app_id: String,
    pub title: String,
    pub state: String,
    pub focused: bool,
}

/// Single service as seen by the context engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceSnapshot {
    pub service_id: String,
    pub status: String,
    pub health: String,
}

/// Minimal system context - only what the agent needs to ground intents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemContext {
    /// Title of the currently focused window, if any.
    pub active_window: Option<String>,
    /// Id of focused window.
    pub focused_window_id: Option<u64>,
    /// All windows (stacked order, bottom->top).
    pub windows: Vec<WindowSnapshot>,
    /// Titles of windows that are minimized.
    pub minimized_windows: Vec<String>,
    /// App ids currently running (have a live instance).
    pub running_apps: Vec<String>,
    /// All registered app ids.
    pub installed_apps: Vec<String>,
    /// Per-app state: RUNNING / INSTALLED / etc.
    pub app_states: BTreeMap<String, String>,
    /// Overall health string.
    pub overall_health: String,
    /// Service health snapshots.
    pub services: Vec<ServiceSnapshot>,
}

impl SystemContext {
    /// Empty context used when the control plane is unreachable.
    pub fn empty() -> Self {
        Self {
            active_window: None,
            focused_window_id: None,
            windows: Vec::new(),
            minimized_windows: Vec::new(),
            running_apps: Vec::new(),
            installed_apps: Vec::new(),
            app_states: BTreeMap::new(),
            overall_health: "UNKNOWN".to_string(),
            services: Vec::new(),
        }
    }

    /// Focused app id if any.
    pub fn focused_app(&self) -> Option<&str> {
        self.windows.iter().find(|w| w.focused).map(|w| w.app_id.as_str())
    }

    /// Whether an app is running.
    pub fn is_running(&self, app: &str) -> bool {
        self.running_apps.iter().any(|a| a == app)
    }

    /// Whether an app is installed (registered).
    pub fn is_installed(&self, app: &str) -> bool {
        self.installed_apps.iter().any(|a| a == app)
    }

    /// Find window by app id (case-insensitive).
    pub fn window_for_app(&self, app: &str) -> Option<&WindowSnapshot> {
        let lower = app.to_ascii_lowercase();
        self.windows.iter().find(|w| {
            w.app_id.to_ascii_lowercase() == lower || w.title.to_ascii_lowercase() == lower
        })
    }

    /// Is a window minimized for app.
    pub fn is_minimized(&self, app: &str) -> bool {
        self.minimized_windows.iter().any(|t| t.eq_ignore_ascii_case(app))
    }

    /// Titles of open windows (excluding minimized if needed).
    pub fn open_titles(&self) -> Vec<String> {
        self.windows.iter().map(|w| w.title.clone()).collect()
    }

    /// Minimal summary string for AI grounding (not raw dump).
    pub fn grounding_text(&self) -> String {
        let mut parts = Vec::new();
        if let Some(active) = &self.active_window {
            parts.push(format!("active_window: {active}"));
        } else {
            parts.push("active_window: none".to_string());
        }
        if self.running_apps.is_empty() {
            parts.push("running_apps: none".to_string());
        } else {
            parts.push(format!("running_apps: {}", self.running_apps.join(", ")));
        }
        if self.windows.is_empty() {
            parts.push("windows: none".to_string());
        } else {
            let titles: Vec<String> = self.windows.iter().map(|w| w.title.clone()).collect();
            parts.push(format!("windows: {}", titles.join(", ")));
        }
        if !self.minimized_windows.is_empty() {
            parts.push(format!("minimized: {}", self.minimized_windows.join(", ")));
        }
        parts.push(format!("health: {}", self.overall_health));
        parts.join("\n")
    }
}

/// Fetch helpers - used by AgentState to build context via control plane
/// and surface server. Each is best-effort; on failure fields stay empty.
/// Query system.status via control plane.
pub fn fetch_system_status(control_port: u16) -> Option<serde_json::Value> {
    use std::io::{BufRead, BufReader, Write};
    let req = serde_json::json!({
        "service_id": "aether-system-core",
        "command": "status",
        "parameters": {}
    });
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", control_port)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    stream.write_all(format!("{req}\n").as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let resp: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if resp["ok"].as_bool().unwrap_or(false) {
        Some(resp["result"].clone())
    } else {
        None
    }
}

/// Query app.list via control plane.
pub fn fetch_app_list(control_port: u16) -> Vec<serde_json::Value> {
    use std::io::{BufRead, BufReader, Write};
    let req = serde_json::json!({
        "service_id": "aether-system-core",
        "command": "app.list",
        "parameters": {}
    });
    let mut stream = match std::net::TcpStream::connect(("127.0.0.1", control_port)) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if stream.write_all(format!("{req}\n").as_bytes()).is_err() {
        return Vec::new();
    }
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return Vec::new();
    }
    let resp: serde_json::Value = match serde_json::from_str(line.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    if !resp["ok"].as_bool().unwrap_or(false) {
        return Vec::new();
    }
    resp["result"]["apps"].as_array().cloned().unwrap_or_default()
}

/// Query window.list via surface server (fallback via control plane proxy if surface direct fails).
pub fn fetch_windows(surface_port: u16, control_port: u16) -> Vec<WindowSnapshot> {
    // First try direct surface server.
    if let Some(wins) = fetch_windows_direct(surface_port) {
        return wins;
    }
    // Fallback: ask system-core proxy (context.get / window.list).
    if let Some(wins) = fetch_windows_via_control(control_port) {
        return wins;
    }
    Vec::new()
}

fn fetch_windows_direct(surface_port: u16) -> Option<Vec<WindowSnapshot>> {
    use std::io::{BufRead, BufReader, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", surface_port)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let req = serde_json::json!({ "op": "window.list" });
    stream.write_all(format!("{req}\n").as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let resp: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if !resp["ok"].as_bool().unwrap_or(false) {
        return None;
    }
    let arr = resp["windows"].as_array()?;
    let mut out = Vec::new();
    for v in arr {
        out.push(WindowSnapshot {
            id: v["id"].as_u64().unwrap_or(0),
            app_id: v["app"].as_str().unwrap_or("?").to_string(),
            title: v["title"].as_str().unwrap_or("?").to_string(),
            state: v["state"].as_str().unwrap_or("normal").to_string(),
            focused: v["focused"].as_bool().unwrap_or(false),
        });
    }
    Some(out)
}

fn fetch_windows_via_control(control_port: u16) -> Option<Vec<WindowSnapshot>> {
    use std::io::{BufRead, BufReader, Write};
    let req = serde_json::json!({
        "service_id": "aether-system-core",
        "command": "window.list",
        "parameters": {}
    });
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", control_port)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    stream.write_all(format!("{req}\n").as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let resp: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if !resp["ok"].as_bool().unwrap_or(false) {
        return None;
    }
    let arr = resp["result"]["windows"].as_array()?;
    let mut out = Vec::new();
    for v in arr {
        out.push(WindowSnapshot {
            id: v["id"].as_u64().unwrap_or(0),
            app_id: v["app"].as_str().unwrap_or("?").to_string(),
            title: v["title"].as_str().unwrap_or("?").to_string(),
            state: v["state"].as_str().unwrap_or("normal").to_string(),
            focused: v["focused"].as_bool().unwrap_or(false),
        });
    }
    Some(out)
}

/// Build a SystemContext by querying control plane + surface server.
/// Best-effort: missing data stays empty, never panics.
pub fn build_context(control_port: u16, surface_port: u16) -> SystemContext {
    let mut ctx = SystemContext::empty();

    // System status -> services + health.
    if let Some(status) = fetch_system_status(control_port) {
        ctx.overall_health = status["overall_health"].as_str().unwrap_or("UNKNOWN").to_string();
        if let Some(services) = status["services"].as_array() {
            for s in services {
                ctx.services.push(ServiceSnapshot {
                    service_id: s["service_id"].as_str().unwrap_or("?").to_string(),
                    status: s["status"].as_str().unwrap_or("UNKNOWN").to_string(),
                    health: s["health"].as_str().unwrap_or("UNKNOWN").to_string(),
                });
            }
        }
        // Also available via applications field.
        // Installed / running will be filled via app.list below.
    }

    // App list -> installed + running inference via app_state? For now running via windows or app states?
    // We get discovery.
    let apps = fetch_app_list(control_port);
    for a in &apps {
        if let Some(id) = a["id"].as_str() {
            ctx.installed_apps.push(id.to_string());
        }
    }

    // Windows -> active + minimized etc, also infers running_apps.
    let windows = fetch_windows(surface_port, control_port);
    for w in &windows {
        if w.focused {
            ctx.active_window = Some(w.title.clone());
            ctx.focused_window_id = Some(w.id);
        }
        if w.state == "minimized" {
            ctx.minimized_windows.push(w.title.clone());
        }
    }
    // Running apps are those with a window or with RUNNING state via app_state? We use windows as proxy + explicit check.
    // For precision, query each app's state if we have time; cheap to just infer from windows for now.
    let mut running_from_windows: Vec<String> = windows.iter().map(|w| w.app_id.clone()).collect();
    running_from_windows.sort();
    running_from_windows.dedup();
    // Additionally, fetch per-app state for installed apps to catch running without window? (e.g., headless)
    // Keep it simple: running_apps = running_from_windows.
    ctx.windows = windows.clone();
    ctx.running_apps = running_from_windows;

    // Fill app_states by querying each app (best-effort, limited to installed list to avoid spam).
    for app_id in ctx.installed_apps.clone() {
        if let Some(report) = fetch_app_state(control_port, &app_id) {
            let is_running = report == "RUNNING";
            ctx.app_states.insert(app_id.clone(), report);
            if is_running && !ctx.running_apps.contains(&app_id) {
                ctx.running_apps.push(app_id);
            }
        } else {
            ctx.app_states.insert(app_id, "UNKNOWN".to_string());
        }
    }

    ctx
}

fn fetch_app_state(control_port: u16, app_id: &str) -> Option<String> {
    use std::io::{BufRead, BufReader, Write};
    let req = serde_json::json!({
        "service_id": "aether-system-core",
        "command": "app.status",
        "parameters": { "app": app_id }
    });
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", control_port)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    stream.write_all(format!("{req}\n").as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let resp: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if !resp["ok"].as_bool().unwrap_or(false) {
        return None;
    }
    resp["result"]["report"]["state"].as_str().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_context_grounding_is_stable() {
        let ctx = SystemContext::empty();
        let text = ctx.grounding_text();
        assert!(text.contains("active_window: none"));
        assert!(text.contains("running_apps: none"));
        assert!(text.contains("windows: none"));
    }

    #[test]
    fn window_lookup_is_case_insensitive() {
        let ctx = SystemContext {
            windows: vec![WindowSnapshot {
                id: 1,
                app_id: "calculator".to_string(),
                title: "Calculator".to_string(),
                state: "normal".to_string(),
                focused: true,
            }],
            ..SystemContext::empty()
        };
        assert!(ctx.window_for_app("Calculator").is_some());
        assert!(ctx.window_for_app("CALCULATOR").is_some());
        assert!(ctx.window_for_app("notes").is_none());
    }

    #[test]
    fn grounding_includes_minimized() {
        let ctx = SystemContext {
            active_window: Some("Notes".to_string()),
            running_apps: vec!["notes".to_string(), "calculator".to_string()],
            windows: vec![
                WindowSnapshot {
                    id: 1,
                    app_id: "calculator".to_string(),
                    title: "Calculator".to_string(),
                    state: "minimized".to_string(),
                    focused: false,
                },
                WindowSnapshot {
                    id: 2,
                    app_id: "notes".to_string(),
                    title: "Notes".to_string(),
                    state: "normal".to_string(),
                    focused: true,
                },
            ],
            minimized_windows: vec!["Calculator".to_string()],
            ..SystemContext::empty()
        };
        let t = ctx.grounding_text();
        assert!(t.contains("active_window: Notes"));
        assert!(t.contains("minimized: Calculator"));
    }
}
