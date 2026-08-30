// Aether Agent - intent parsing and capability policy.
//
// Translates user text into a STRUCTURED intent, validates it against the
// capability policy (reuse of aether-core Capability/RiskLevel), and hands
// approved intents to the control plane. The AI provider never sees raw
// system access; it can only trigger capabilities defined here.

use aether_core::capability::{Capability, CapabilityDomain, RiskLevel};
use serde_json::Value;
use std::time::Duration;

/// Capabilities this phase exposes. Read-only ones execute directly;
/// state-changing ones are marked for future approval flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityId {
    SystemStatus,
    AppStatus,
    AppList,
    AppLaunch,
    AppClose,
    // Window / desktop operations
    WindowList,
    WindowFocus,
    WindowMinimize,
    WindowMaximize,
    WindowClose,
    WindowRestore,
    ContextGet,
    // File operations
    FileList,
    FileSearch,
    FileRead,
    FileCreate,
    FileWrite,
    FileRename,
    FileMove,
    FileDelete,
    // System information
    SystemInfo,
    SystemResources,
    SystemUptime,
}

impl CapabilityId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SystemStatus => "system.status",
            Self::AppStatus => "app.status",
            Self::AppList => "app.list",
            Self::AppLaunch => "app.launch",
            Self::AppClose => "app.close",
            Self::WindowList => "window.list",
            Self::WindowFocus => "window.focus",
            Self::WindowMinimize => "window.minimize",
            Self::WindowMaximize => "window.maximize",
            Self::WindowClose => "window.close",
            Self::WindowRestore => "window.restore",
            Self::ContextGet => "context.get",
            Self::FileList => "file.list",
            Self::FileSearch => "file.search",
            Self::FileRead => "file.read",
            Self::FileCreate => "file.create",
            Self::FileWrite => "file.write",
            Self::FileRename => "file.rename",
            Self::FileMove => "file.move",
            Self::FileDelete => "file.delete",
            Self::SystemInfo => "system.info",
            Self::SystemResources => "system.resources",
            Self::SystemUptime => "system.uptime",
        }
    }

    /// The aether-core capability this id maps to (domain + risk).
    pub fn capability(&self) -> Capability {
        match self {
            Self::SystemStatus => {
                Capability::new(CapabilityDomain::System, "status", RiskLevel::Low)
            }
            Self::AppStatus => {
                Capability::new(CapabilityDomain::Application, "status", RiskLevel::Low)
            }
            Self::AppList => Capability::new(CapabilityDomain::Application, "list", RiskLevel::Low),
            Self::AppLaunch => {
                Capability::new(CapabilityDomain::Application, "launch", RiskLevel::Medium)
            }
            Self::AppClose => {
                Capability::new(CapabilityDomain::Application, "close", RiskLevel::Medium)
            }
            Self::WindowList => {
                Capability::new(CapabilityDomain::Application, "window.list", RiskLevel::Low)
            }
            Self::WindowFocus => {
                Capability::new(CapabilityDomain::Application, "window.focus", RiskLevel::Low)
            }
            Self::WindowMinimize => {
                Capability::new(CapabilityDomain::Application, "window.minimize", RiskLevel::Low)
            }
            Self::WindowMaximize => {
                Capability::new(CapabilityDomain::Application, "window.maximize", RiskLevel::Low)
            }
            Self::WindowClose => {
                Capability::new(CapabilityDomain::Application, "window.close", RiskLevel::Medium)
            }
            Self::WindowRestore => {
                Capability::new(CapabilityDomain::Application, "window.restore", RiskLevel::Low)
            }
            Self::ContextGet => {
                Capability::new(CapabilityDomain::System, "context", RiskLevel::Low)
            }
            Self::FileList => Capability::new(CapabilityDomain::Filesystem, "list", RiskLevel::Low),
            Self::FileSearch => {
                Capability::new(CapabilityDomain::Filesystem, "search", RiskLevel::Low)
            }
            Self::FileRead => Capability::new(CapabilityDomain::Filesystem, "read", RiskLevel::Low),
            Self::FileCreate => {
                Capability::new(CapabilityDomain::Filesystem, "create", RiskLevel::Medium)
            }
            Self::FileWrite => {
                Capability::new(CapabilityDomain::Filesystem, "write", RiskLevel::Medium)
            }
            Self::FileRename => {
                Capability::new(CapabilityDomain::Filesystem, "rename", RiskLevel::Medium)
            }
            Self::FileMove => {
                Capability::new(CapabilityDomain::Filesystem, "move", RiskLevel::Medium)
            }
            Self::FileDelete => {
                Capability::new(CapabilityDomain::Filesystem, "delete", RiskLevel::High)
            }
            Self::SystemInfo => Capability::new(CapabilityDomain::System, "info", RiskLevel::Low),
            Self::SystemResources => {
                Capability::new(CapabilityDomain::System, "resources", RiskLevel::Low)
            }
            Self::SystemUptime => {
                Capability::new(CapabilityDomain::System, "uptime", RiskLevel::Low)
            }
        }
    }

    /// Whether this phase executes without an approval dialog.
    pub fn auto_execute(&self) -> bool {
        // All low/medium desktop actions auto-execute in this phase; high-risk
        // would be gated by ConfirmationPolicy.
        true
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(raw: &str) -> Option<Self> {
        match raw {
            "system.status" => Some(Self::SystemStatus),
            "app.status" => Some(Self::AppStatus),
            "app.list" => Some(Self::AppList),
            "app.launch" => Some(Self::AppLaunch),
            "app.close" => Some(Self::AppClose),
            "window.list" => Some(Self::WindowList),
            "window.focus" => Some(Self::WindowFocus),
            "window.minimize" => Some(Self::WindowMinimize),
            "window.maximize" => Some(Self::WindowMaximize),
            "window.close" => Some(Self::WindowClose),
            "window.restore" => Some(Self::WindowRestore),
            "context.get" => Some(Self::ContextGet),
            "file.list" => Some(Self::FileList),
            "file.search" => Some(Self::FileSearch),
            "file.read" => Some(Self::FileRead),
            "file.create" => Some(Self::FileCreate),
            "file.write" => Some(Self::FileWrite),
            "file.rename" => Some(Self::FileRename),
            "file.move" => Some(Self::FileMove),
            "file.delete" => Some(Self::FileDelete),
            "system.info" => Some(Self::SystemInfo),
            "system.resources" => Some(Self::SystemResources),
            "system.uptime" => Some(Self::SystemUptime),
            _ => None,
        }
    }
}

/// A structured intent extracted from user text.
#[derive(Debug, Clone, PartialEq)]
pub struct Intent {
    pub capability: CapabilityId,
    /// Arguments per capability: app.launch/app.close/window.* require their target.
    pub arguments: Value,
}

/// Why an intent was rejected by validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection(pub String);

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

trait NonEmptyOption {
    fn non_empty(self) -> Option<String>;
}
impl NonEmptyOption for String {
    fn non_empty(self) -> Option<String> {
        if self.is_empty() {
            None
        } else {
            Some(self)
        }
    }
}

// ----------------- surface client (direct) -----------------

/// Minimal client for the surface server (window ops).
pub struct SurfaceClient {
    addr: String,
    timeout: Duration,
}

impl SurfaceClient {
    pub fn new(surface_port: u16) -> Self {
        Self { addr: format!("127.0.0.1:{surface_port}"), timeout: Duration::from_secs(5) }
    }

    fn call(&self, req: Value) -> Result<Value, String> {
        use std::io::{BufRead, BufReader, Write};
        let mut stream = std::net::TcpStream::connect(&self.addr)
            .map_err(|e| format!("connect surface {}: {e}", self.addr))?;
        stream.set_read_timeout(Some(self.timeout)).map_err(|e| format!("timeout: {e}"))?;
        let mut payload = serde_json::to_string(&req).map_err(|e| format!("encode: {e}"))?;
        payload.push('\n');
        stream.write_all(payload.as_bytes()).map_err(|e| format!("send: {e}"))?;
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).map_err(|e| format!("recv: {e}"))?;
        if line.trim().is_empty() {
            return Err("empty surface response".to_string());
        }
        let v: Value = serde_json::from_str(line.trim()).map_err(|e| format!("decode: {e}"))?;
        if v["ok"].as_bool().unwrap_or(false) {
            Ok(v)
        } else {
            let err = v["error"].as_str().unwrap_or("surface error").to_string();
            Err(err)
        }
    }

    pub fn window_list(&self) -> Result<Value, String> {
        self.call(serde_json::json!({ "op": "window.list" }))
    }

    pub fn window_focus(
        &self,
        app_or_title: &str,
        window_id: Option<u64>,
    ) -> Result<Value, String> {
        if let Some(id) = window_id {
            self.call(serde_json::json!({ "op": "window.focus", "window_id": id }))
        } else {
            // Resolve via app_id -> id lookup via window.list first, then focus.
            let list = self.window_list()?;
            if let Some(arr) = list["windows"].as_array() {
                for w in arr {
                    let app = w["app"].as_str().unwrap_or("").to_ascii_lowercase();
                    let title = w["title"].as_str().unwrap_or("").to_ascii_lowercase();
                    let target = app_or_title.to_ascii_lowercase();
                    if app == target || title == target {
                        if let Some(id) = w["id"].as_u64() {
                            return self.call(
                                serde_json::json!({ "op": "window.focus", "window_id": id }),
                            );
                        }
                    }
                }
            }
            Err(format!("no such window for '{app_or_title}'"))
        }
    }

    pub fn window_minimize(&self, app: &str, window_id: Option<u64>) -> Result<Value, String> {
        if let Some(id) = window_id {
            self.call(serde_json::json!({ "op": "window.minimize", "window_id": id }))
        } else {
            let list = self.window_list()?;
            if let Some(arr) = list["windows"].as_array() {
                for w in arr {
                    let a = w["app"].as_str().unwrap_or("").to_ascii_lowercase();
                    let t = w["title"].as_str().unwrap_or("").to_ascii_lowercase();
                    let target = app.to_ascii_lowercase();
                    if a == target || t == target {
                        if let Some(id) = w["id"].as_u64() {
                            return self.call(
                                serde_json::json!({ "op": "window.minimize", "window_id": id }),
                            );
                        }
                    }
                }
            }
            Err(format!("no such window for '{app}'"))
        }
    }

    pub fn window_maximize(&self, app: &str, window_id: Option<u64>) -> Result<Value, String> {
        if let Some(id) = window_id {
            self.call(serde_json::json!({ "op": "window.maximize", "window_id": id }))
        } else {
            let list = self.window_list()?;
            if let Some(arr) = list["windows"].as_array() {
                for w in arr {
                    let a = w["app"].as_str().unwrap_or("").to_ascii_lowercase();
                    let t = w["title"].as_str().unwrap_or("").to_ascii_lowercase();
                    let target = app.to_ascii_lowercase();
                    if a == target || t == target {
                        if let Some(id) = w["id"].as_u64() {
                            return self.call(
                                serde_json::json!({ "op": "window.maximize", "window_id": id }),
                            );
                        }
                    }
                }
            }
            Err(format!("no such window for '{app}'"))
        }
    }

    pub fn window_close(&self, app: &str, window_id: Option<u64>) -> Result<Value, String> {
        if let Some(id) = window_id {
            self.call(serde_json::json!({ "op": "window.close", "window_id": id }))
        } else {
            self.call(serde_json::json!({ "op": "window.close", "app_id": app }))
        }
    }
}

// ----------------- parsing -----------------

const FILLER: &[&str] = &["THE", "MY", "A", "AN", "THIS", "THAT"];
const CONJ: &[&str] = &["AND", "THEN", "PLUS", ",", "&", ";"];
const VERBS: &[&str] = &[
    "OPEN",
    "LAUNCH",
    "START",
    "RUN",
    "CLOSE",
    "FOCUS",
    "BRING",
    "SHOW",
    "MINIMIZE",
    "HIDE",
    "MAXIMIZE",
    "MAXIMISE",
    "FULLSCREEN",
    "EXPAND",
    "RESTORE",
    "FRONT",
    "BACK",
];

fn is_filler(w: &str) -> bool {
    FILLER.contains(&w)
}
fn is_conj(w: &str) -> bool {
    CONJ.contains(&w) || w == "AND" || w == "," || w == "&"
}
fn is_verb(w: &str) -> bool {
    VERBS.contains(&w)
}

fn normalize_token(raw: &str) -> Option<String> {
    let t = raw
        .trim_end_matches(|c: char| !c.is_ascii_alphanumeric())
        .trim_start_matches(|c: char| !c.is_ascii_alphanumeric())
        .to_ascii_lowercase();
    if t.is_empty() { None } else { Some(t) }.filter(|s| {
        s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
    })
}

/// Collect app-like targets after a verb index. Returns list of app ids.
fn collect_targets(words: &[&str], verb_idx: usize) -> Vec<String> {
    let mut targets = Vec::new();
    let mut i = verb_idx + 1;
    while i < words.len() {
        let w = words[i];
        // Stop at next verb that is not conjunction-like? Actually CLOSE/OPEN etc indicates new clause.
        if is_verb(w) && w != "AND" && w != "THEN" {
            // If this verb is itself an action verb, stop collecting for previous.
            if w == "OPEN"
                || w == "LAUNCH"
                || w == "START"
                || w == "CLOSE"
                || w == "MINIMIZE"
                || w == "HIDE"
                || w == "MAXIMIZE"
                || w == "MAXIMISE"
                || w == "BRING"
                || w == "FOCUS"
            {
                break;
            }
        }
        if is_conj(w) || is_filler(w) {
            // Skip conjunction/filler but allow continuing
            i += 1;
            continue;
        }
        // Skip words like TO, THE, FRONT, BACK etc that are not app names but part of phrase
        if w == "TO"
            || w == "THE"
            || w == "FRONT"
            || w == "BACK"
            || w == "FORE"
            || w == "GROUND"
            || w == "UP"
            || w == "IT"
            || w == "THAT"
            || w == "THIS"
            || w == "THEM"
        {
            i += 1;
            continue;
        }
        if let Some(norm) = normalize_token(w) {
            // Heuristic: ignore common english words that are not app ids if they are not known apps.
            let ignore = norm == "whats"
                || norm == "what"
                || norm == "is"
                || norm == "are"
                || norm == "open"
                || norm == "app"
                || norm == "apps"
                || norm == "window"
                || norm == "windows"
                || norm == "please"
                || norm == "bring"
                || norm == "minimize"
                || norm == "maximize"
                || norm == "close"
                || norm == "focus"
                || norm == "me"
                || norm == "installed"
                || norm == "show"
                || norm == "are"
                || norm == "the";
            if ignore {
                i += 1;
                continue;
            }
            targets.push(norm);
        }
        i += 1;
        if targets.len() >= 4 {
            break;
        }
    }
    targets
}

fn extract_quoted_content(text: &str) -> Option<String> {
    // Find content inside single or double quotes
    for quote in ['\'', '"'] {
        if let Some(start) = text.find(quote) {
            if let Some(end) = text[start + 1..].find(quote) {
                let content = text[start + 1..start + 1 + end].to_string();
                if !content.trim().is_empty() {
                    return Some(content);
                }
            }
        }
    }
    None
}

fn find_file_paths(text: &str) -> Vec<String> {
    // Split by whitespace and extract tokens that look like file paths
    // Keep leading '/' and '.' for absolute and hidden, trim trailing punctuation
    let mut paths = Vec::new();
    for token in text.split_whitespace() {
        let clean = token
            .trim_end_matches([',', ';', ':', ')', '(', '!', '?'])
            .trim_start_matches(['(', '"', '\''])
            .trim_end_matches(['"', '\'', ')'])
            .to_string();
        let lower = clean.to_ascii_lowercase();
        // Heuristic: contains '.' with extension 1-4 chars, or contains '/', or ends with .md/.txt etc
        let is_path = (clean.contains('.') && clean.len() >= 3 && !clean.starts_with('.')
            || clean.contains('/'))
            && clean.chars().any(|c| c.is_ascii_alphanumeric())
            && !clean.eq_ignore_ascii_case("aether")
            && !clean.eq_ignore_ascii_case("os");
        // Also accept absolute paths like /etc/shadow
        let is_absolute = clean.starts_with('/') && clean.len() > 1;
        if is_path || is_absolute {
            // Normalize to remove trailing punctuation
            let norm = clean.trim_end_matches(['.', ',', ';']).to_string();
            if !norm.is_empty() {
                paths.push(norm);
            }
        }
        let _ = lower;
    }
    paths
}

fn parse_file_intent(lower: &str, original: &str, last_file: Option<&str>) -> Option<Intent> {
    // Security: always parse file intents even for absolute paths so validation can reject
    let _has_files = lower.contains("file")
        || lower.contains("files")
        || lower.contains("document")
        || lower.contains("documents")
        || lower.contains("notes") && lower.contains("find");
    // File list: show/list files
    if (lower.contains("show") || lower.contains("list") || lower.contains("display"))
        && lower.contains("file")
    {
        // Extract path after "in" if present
        let mut path = "";
        if let Some(idx) = lower.find(" in ") {
            let after = original[idx + 4..].trim();
            // Take first token as folder
            if let Some(folder) = after.split_whitespace().next() {
                let clean = folder.trim_matches(|c: char| {
                    !c.is_ascii_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-'
                });
                if !clean.is_empty()
                    && (clean.eq_ignore_ascii_case("documents")
                        || clean.eq_ignore_ascii_case("downloads")
                        || clean.eq_ignore_ascii_case("projects")
                        || clean.eq_ignore_ascii_case("notes")
                        || clean.contains('/'))
                {
                    path = clean;
                }
            }
        }
        // Also handle "show my files" -> root
        if lower.contains("show my files")
            || lower == "show my files."
            || lower.contains("show my files.")
        {
            path = "";
        }
        return Some(Intent {
            capability: CapabilityId::FileList,
            arguments: serde_json::json!({ "path": path }),
        });
    }
    // Special case: "Show my files." exact
    if lower.trim() == "show my files" || lower.trim() == "show my files." {
        return Some(Intent {
            capability: CapabilityId::FileList,
            arguments: serde_json::json!({ "path": "" }),
        });
    }
    // File search: find/search files
    if (lower.contains("find") || lower.contains("search")) && lower.contains("file") {
        // Extract query: word before "files" that is not filler
        let words: Vec<&str> = lower.split_whitespace().collect();
        let mut query = "";
        for (i, w) in words.iter().enumerate() {
            if (*w == "files" || *w == "file") && i > 0 {
                let mut cand = words[i - 1].trim_matches(|c: char| !c.is_ascii_alphanumeric());
                // Skip fillers
                if matches!(cand, "my" | "all" | "the" | "a" | "an") && i > 1 {
                    cand = words[i - 2].trim_matches(|c: char| !c.is_ascii_alphanumeric());
                }
                if !cand.is_empty()
                    && cand != "find"
                    && cand != "search"
                    && cand != "all"
                    && cand != "my"
                {
                    query = cand;
                    break;
                }
            }
        }
        // Fallback: if query empty, try to find word after find/search
        if query.is_empty() {
            for (i, w) in words.iter().enumerate() {
                if (*w == "find" || *w == "search") && i + 1 < words.len() {
                    let mut cand = words[i + 1].trim_matches(|c: char| !c.is_ascii_alphanumeric());
                    if matches!(cand, "my" | "all" | "the") && i + 2 < words.len() {
                        cand = words[i + 2].trim_matches(|c: char| !c.is_ascii_alphanumeric());
                    }
                    if !cand.is_empty() && cand != "files" && cand != "file" {
                        query = cand;
                        break;
                    }
                }
            }
        }
        if query.is_empty() {
            query = "all";
        }
        // Map markdown -> md etc
        if query == "markdown" {
            query = "md";
        }
        return Some(Intent {
            capability: CapabilityId::FileSearch,
            arguments: serde_json::json!({ "query": query }),
        });
    }
    // Find my project files -> also search
    if lower.contains("find my project files") {
        return Some(Intent {
            capability: CapabilityId::FileSearch,
            arguments: serde_json::json!({ "query": "project" }),
        });
    }
    // File read: read <path>
    if lower.contains("read") {
        let paths = find_file_paths(original);
        if let Some(p) = paths.first() {
            return Some(Intent {
                capability: CapabilityId::FileRead,
                arguments: serde_json::json!({ "path": p }),
            });
        }
        // Also handle "Read /etc/shadow" where path is absolute with slash
        // find_file_paths should already capture /etc/shadow
        // If not found, try to extract after "read"
        if let Some(idx) = lower.find("read") {
            let after = original[idx + 4..].trim();
            let token = after
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '.' | ';'));
            if !token.is_empty() {
                return Some(Intent {
                    capability: CapabilityId::FileRead,
                    arguments: serde_json::json!({ "path": token }),
                });
            }
        }
    }
    // File create: create a file called <path>
    if lower.contains("create") && lower.contains("file") {
        let paths = find_file_paths(original);
        if let Some(p) = paths.first() {
            let content = extract_quoted_content(original).unwrap_or_default();
            // If create has content via "with content" etc, but for now empty
            return Some(Intent {
                capability: CapabilityId::FileCreate,
                arguments: serde_json::json!({ "path": p, "content": content }),
            });
        }
        // Fallback: look for "called" or "named"
        for marker in ["called", "named"] {
            if let Some(idx) = lower.find(marker) {
                let after = original[idx + marker.len()..].trim();
                if let Some(tok) = after.split_whitespace().next() {
                    let clean =
                        tok.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '.' | ';'));
                    if !clean.is_empty() {
                        return Some(Intent {
                            capability: CapabilityId::FileCreate,
                            arguments: serde_json::json!({ "path": clean, "content": "" }),
                        });
                    }
                }
            }
        }
    }
    // File write: write <content> into <path>  OR write "content" into file
    if lower.contains("write") {
        let paths = find_file_paths(original);
        // Content is quoted
        let content = extract_quoted_content(original).unwrap_or_default();
        if let Some(p) = paths.last().cloned() {
            // If content empty, try to extract after "write"
            let final_content = if content.is_empty() {
                // Try to find content between write and into
                if let Some(write_idx) = lower.find("write") {
                    if let Some(into_idx) = lower.find("into") {
                        let between = original[write_idx + 5..into_idx].trim();
                        let c = between.trim_matches(|c: char| matches!(c, '"' | '\'' | ' '));
                        if !c.is_empty() {
                            c.to_string()
                        } else {
                            content
                        }
                    } else {
                        content
                    }
                } else {
                    content
                }
            } else {
                content
            };
            return Some(Intent {
                capability: CapabilityId::FileWrite,
                arguments: serde_json::json!({ "path": p, "content": final_content }),
            });
        }
    }
    // File rename: rename <from> to <to>
    if lower.contains("rename") {
        let paths = find_file_paths(original);
        if paths.len() >= 2 {
            return Some(Intent {
                capability: CapabilityId::FileRename,
                arguments: serde_json::json!({ "from": paths[0], "to": paths[1] }),
            });
        }
        // Fallback: parse "rename A to B"
        if let Some(rename_idx) = lower.find("rename") {
            if let Some(to_idx) = lower.find(" to ") {
                let from_part = original[rename_idx + 6..to_idx].trim();
                let to_part = original[to_idx + 4..].trim();
                let from_tok = from_part
                    .split_whitespace()
                    .last()
                    .unwrap_or("")
                    .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '.' | ';'));
                let to_tok = to_part
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '.' | ';'));
                if !from_tok.is_empty() && !to_tok.is_empty() {
                    return Some(Intent {
                        capability: CapabilityId::FileRename,
                        arguments: serde_json::json!({ "from": from_tok, "to": to_tok }),
                    });
                }
            }
        }
    }
    // File move: move <from> into <to>  or move <from> to <to>
    if lower.contains("move") {
        let paths = find_file_paths(original);
        // Handle pronoun "this note" / "it" -> use last_file if available
        let is_pronoun_move = lower.contains("this")
            && (lower.contains("note") || lower.contains("file"))
            || lower.contains(" this ") && lower.contains("move");
        if is_pronoun_move {
            if let Some(last) = last_file {
                // Extract destination after "into" or "to"
                let mut to = String::new();
                if let Some(into_idx) = lower.find(" into ") {
                    let after = original[into_idx + 6..].trim();
                    if let Some(tok) = after.split_whitespace().next() {
                        to = tok
                            .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '.' | ';'))
                            .to_string();
                    }
                } else if let Some(to_idx) = lower.rfind(" to ") {
                    let after = original[to_idx + 4..].trim();
                    if let Some(tok) = after.split_whitespace().next() {
                        to = tok
                            .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '.' | ';'))
                            .to_string();
                    }
                }
                if !to.is_empty() {
                    let dest = if to.contains('.') {
                        to.clone()
                    } else {
                        format!(
                            "{}/{}",
                            to.trim_end_matches('/'),
                            last.split('/').next_back().unwrap_or(last)
                        )
                    };
                    return Some(Intent {
                        capability: CapabilityId::FileMove,
                        arguments: serde_json::json!({ "from": last, "to": dest }),
                    });
                }
                // Fallback if no to found but we have last_file and a path (maybe destination)
                if let Some(dest_candidate) = paths.first() {
                    let dest = if dest_candidate.contains('.') {
                        dest_candidate.clone()
                    } else {
                        format!(
                            "{}/{}",
                            dest_candidate.trim_end_matches('/'),
                            last.split('/').next_back().unwrap_or(last)
                        )
                    };
                    return Some(Intent {
                        capability: CapabilityId::FileMove,
                        arguments: serde_json::json!({ "from": last, "to": dest }),
                    });
                }
            }
        }
        // For "Move project-ideas.md into Documents" -> from is first path, to is Documents
        if lower.contains(" into ") || lower.contains(" to ") {
            // Try to extract from and to folder
            let from = paths.first().cloned().unwrap_or_default();
            // If from is actually the destination (when pronoun was used but last_file not available), try alternative
            let mut to = String::new();
            if let Some(into_idx) = lower.find(" into ") {
                let after = original[into_idx + 6..].trim();
                if let Some(tok) = after.split_whitespace().next() {
                    to = tok
                        .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '.' | ';'))
                        .to_string();
                }
            } else if let Some(to_idx) = lower.rfind(" to ") {
                let after = original[to_idx + 4..].trim();
                if let Some(tok) = after.split_whitespace().next() {
                    to = tok
                        .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | '.' | ';'))
                        .to_string();
                }
            }
            if !from.is_empty() && !to.is_empty() && from != to {
                // If to is a directory like Documents, construct full dest
                let dest = if to.contains('.') {
                    to
                } else {
                    format!(
                        "{}/{}",
                        to.trim_end_matches('/'),
                        from.split('/').next_back().unwrap_or(&from)
                    )
                };
                return Some(Intent {
                    capability: CapabilityId::FileMove,
                    arguments: serde_json::json!({ "from": from, "to": dest }),
                });
            }
            if paths.len() >= 2 {
                return Some(Intent {
                    capability: CapabilityId::FileMove,
                    arguments: serde_json::json!({ "from": paths[0], "to": paths[1] }),
                });
            }
        } else if paths.len() >= 2 {
            return Some(Intent {
                capability: CapabilityId::FileMove,
                arguments: serde_json::json!({ "from": paths[0], "to": paths[1] }),
            });
        }
    }
    // File delete: delete <path> or delete all files
    if lower.contains("delete") {
        let paths = find_file_paths(original);
        if let Some(p) = paths.first() {
            return Some(Intent {
                capability: CapabilityId::FileDelete,
                arguments: serde_json::json!({ "path": p }),
            });
        }
        if lower.contains("all files") || lower.contains("delete all") {
            return Some(Intent {
                capability: CapabilityId::FileDelete,
                arguments: serde_json::json!({ "path": "*" }),
            });
        }
        // Generic delete without path -> treat as bulk
        return Some(Intent {
            capability: CapabilityId::FileDelete,
            arguments: serde_json::json!({ "path": "*" }),
        });
    }
    None
}

fn parse_system_intent(lower: &str) -> Option<Intent> {
    if (lower.contains("ram")
        || lower.contains("memory")
        || (lower.contains("resources") && lower.contains("system")))
        && (lower.contains("how much")
            || lower.contains("available")
            || lower.contains("memory")
            || lower.contains("ram")
            || lower.contains("resources"))
    {
        return Some(Intent {
            capability: CapabilityId::SystemResources,
            arguments: serde_json::json!({}),
        });
    }
    if lower.contains("uptime")
        || (lower.contains("how long") && lower.contains("running"))
        || lower.contains("been running")
    {
        return Some(Intent {
            capability: CapabilityId::SystemUptime,
            arguments: serde_json::json!({}),
        });
    }
    if lower.contains("system info")
        || lower.contains("os version")
        || lower.contains("system information")
        || (lower.contains("system") && lower.contains("info"))
    {
        return Some(Intent {
            capability: CapabilityId::SystemInfo,
            arguments: serde_json::json!({}),
        });
    }
    // Also handle "How much RAM is available?" specifically
    if lower.contains("ram") && lower.contains("available") {
        return Some(Intent {
            capability: CapabilityId::SystemResources,
            arguments: serde_json::json!({}),
        });
    }
    if lower.contains("how long has aether been running") {
        return Some(Intent {
            capability: CapabilityId::SystemUptime,
            arguments: serde_json::json!({}),
        });
    }
    None
}

/// Keyword-based intent classifier (deterministic stand-in for model-based
/// intent detection; the capability layer below is unchanged either way).
pub fn parse_intent(text: &str) -> Option<Intent> {
    let intents = parse_intents(text, &crate::context::SystemContext::empty(), None);
    intents.into_iter().next()
}

/// Extended parser: returns all intents in text (multi-step), using optional context & last app/file for pronouns.
pub fn parse_intents(
    text: &str,
    ctx: &crate::context::SystemContext,
    convo_last_app: Option<&str>,
) -> Vec<Intent> {
    parse_intents_with_file(text, ctx, convo_last_app, None)
}

pub fn parse_intents_with_file(
    text: &str,
    ctx: &crate::context::SystemContext,
    convo_last_app: Option<&str>,
    convo_last_file: Option<&str>,
) -> Vec<Intent> {
    let upper = text.to_uppercase();
    let raw_cleaned: Vec<String> = upper
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_ascii_alphanumeric()).to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if raw_cleaned.is_empty() {
        return Vec::new();
    }
    // Use cleaned upper tokens for parsing (punctuation stripped)
    let words_str: Vec<String> = raw_cleaned.clone();
    let words: Vec<&str> = words_str.iter().map(|s| s.as_str()).collect();

    // Special: "what's open?" / "what is open?" / "which windows" etc -> window.list (highest priority)
    // Keep this before generic OPEN detection to avoid misclassifying.
    let normalized = upper.replace('\'', "").replace(['?', '.', ','], " ");
    let norm_words: Vec<&str> = normalized.split_whitespace().collect();
    let joined = norm_words.join(" ");
    let is_window_list_query = (joined.contains("WHAT") && joined.contains("OPEN"))
        || joined.contains("WHATS OPEN")
        || joined.contains("WHAT IS OPEN")
        || (joined.contains("WHICH") && joined.contains("WINDOW"))
        || (joined.contains("LIST") && joined.contains("WINDOW"))
        || (joined.contains("SHOW") && joined.contains("WINDOW") && joined.contains("OPEN"))
        || joined == "WHAT IS OPEN"
        || joined == "WHATS OPEN";

    if is_window_list_query {
        // But don't trigger on "open calculator" which also contains OPEN.
        // Heuristic: if phrase is short and contains WHAT/WHICH/LIST, it's a query.
        if norm_words.contains(&"WHAT")
            || norm_words.contains(&"WHICH")
            || norm_words.contains(&"LIST")
        {
            return vec![Intent {
                capability: CapabilityId::WindowList,
                arguments: serde_json::json!({}),
            }];
        }
    }

    // Also handle explicit "what's open" as window.list even without verb check.
    if upper.contains("WHAT") && upper.contains("OPEN?")
        || upper.trim() == "WHAT'S OPEN?"
        || upper.trim() == "WHATS OPEN"
    {
        return vec![Intent {
            capability: CapabilityId::WindowList,
            arguments: serde_json::json!({}),
        }];
    }

    // More precise: if text matches patterns like "what's open" ignoring punctuation/case
    let lower = text.to_ascii_lowercase();
    let lower_nopunct: String =
        lower.chars().filter(|c| c.is_ascii_alphabetic() || c.is_ascii_whitespace()).collect();
    let lower_trim = lower_nopunct.trim().replace("  ", " ");
    if matches!(
        lower_trim.as_str(),
        "whats open"
            | "what is open"
            | "what is open "
            | "which windows are open"
            | "list windows"
            | "show open windows"
            | "what windows are open"
    ) {
        return vec![Intent {
            capability: CapabilityId::WindowList,
            arguments: serde_json::json!({}),
        }];
    }
    // Also detect "what's open?" with apostrophe removed -> already.
    if lower.contains("what's open")
        || lower.contains("whats open")
        || (lower.contains("what")
            && lower.contains("open")
            && lower.split_whitespace().count() <= 4
            && !lower.contains("calculator")
            && !lower.contains("notes")
            && !lower.contains("files"))
    {
        // Only if not containing a specific app name that suggests launch.
        if !lower.contains("calculator")
            && !lower.contains("notes")
            && !lower.contains("files")
            && !lower.contains("open calculator")
        {
            return vec![Intent {
                capability: CapabilityId::WindowList,
                arguments: serde_json::json!({}),
            }];
        }
    }

    let mut intents: Vec<Intent> = Vec::new();

    // Track pronouns to resolve.
    let has_pronoun = lower.split_whitespace().any(|w| {
        let t = w.trim_matches(|c: char| !c.is_ascii_alphabetic());
        t == "it" || t == "that" || t == "this" || t == "them"
    }) || lower.contains(" it ")
        || lower.contains(" it.")
        || lower.trim_end_matches(|c: char| !c.is_ascii_alphabetic()).ends_with(" it");

    // File and system capabilities have priority when their keywords are present.
    // Check file intents first so "Show my files" doesn't become WindowFocus.
    if let Some(intent) = parse_file_intent(&lower, text, convo_last_file) {
        return vec![intent];
    }
    if let Some(intent) = parse_system_intent(&lower) {
        return vec![intent];
    }

    // Iterate over verb positions to collect actions.
    let mut idx = 0;
    while idx < words.len() {
        let verb = words[idx];
        match verb {
            "CLOSE" => {
                let targets = collect_targets(&words, idx);
                if targets.is_empty() && has_pronoun {
                    if let Some(last) = convo_last_app.or(ctx.focused_app()) {
                        // For close, pronoun likely refers to last app; if no context, skip.
                        // But we still produce intent with resolved app.
                        intents.push(Intent {
                            capability: CapabilityId::AppClose,
                            arguments: serde_json::json!({ "app": last }),
                        });
                    } else if let Some(last) = convo_last_app {
                        intents.push(Intent {
                            capability: CapabilityId::AppClose,
                            arguments: serde_json::json!({ "app": last }),
                        });
                    }
                } else {
                    for t in targets {
                        // Disambiguate window.close vs app.close: for now use AppClose for "close X"
                        // unless phrase contains "window".
                        let is_window_close = upper[idx..].to_string().contains("WINDOW");
                        let cap = if is_window_close {
                            CapabilityId::WindowClose
                        } else {
                            CapabilityId::AppClose
                        };
                        intents.push(Intent {
                            capability: cap,
                            arguments: serde_json::json!({ "app": t }),
                        });
                    }
                }
                // Also detect extra close verbs further? Need to advance idx past collected targets.
                // Simplify: jump to next verb position.
                idx += 1;
                continue;
            }
            "OPEN" | "LAUNCH" | "START" | "RUN" => {
                let targets = collect_targets(&words, idx);
                if targets.is_empty() && has_pronoun {
                    if let Some(last) = convo_last_app {
                        intents.push(Intent {
                            capability: CapabilityId::AppLaunch,
                            arguments: serde_json::json!({ "app": last }),
                        });
                    }
                } else {
                    for t in targets {
                        intents.push(Intent {
                            capability: CapabilityId::AppLaunch,
                            arguments: serde_json::json!({ "app": t }),
                        });
                    }
                }
                idx += 1;
                continue;
            }
            "BRING" | "FOCUS" | "SHOW" => {
                // BRING ... TO FRONT, FOCUS ... etc
                let targets = collect_targets(&words, idx);
                let mut resolved = targets;
                if resolved.is_empty() && has_pronoun {
                    if let Some(last) =
                        convo_last_app.or(ctx.active_window.as_deref()).or(ctx.focused_app())
                    {
                        // Normalize to app id
                        let norm = last.to_ascii_lowercase();
                        resolved.push(norm);
                    }
                }
                for t in resolved {
                    intents.push(Intent {
                        capability: CapabilityId::WindowFocus,
                        arguments: serde_json::json!({ "app": t }),
                    });
                }
                idx += 1;
                continue;
            }
            "FRONT" => {
                // "to the front" without explicit BRING/FOCUS verb? Handle via preceding BRING already.
                idx += 1;
                continue;
            }
            "MINIMIZE" | "HIDE" => {
                let mut targets = collect_targets(&words, idx);
                if targets.is_empty() && has_pronoun {
                    if let Some(last) = convo_last_app.or(ctx.focused_app()) {
                        targets.push(last.to_string());
                    }
                }
                for t in targets {
                    intents.push(Intent {
                        capability: CapabilityId::WindowMinimize,
                        arguments: serde_json::json!({ "app": t }),
                    });
                }
                idx += 1;
                continue;
            }
            "MAXIMIZE" | "MAXIMISE" | "EXPAND" | "FULLSCREEN" => {
                let mut targets = collect_targets(&words, idx);
                if targets.is_empty() && has_pronoun {
                    if let Some(last) = convo_last_app.or(ctx.focused_app()) {
                        targets.push(last.to_string());
                    }
                }
                for t in targets {
                    intents.push(Intent {
                        capability: CapabilityId::WindowMaximize,
                        arguments: serde_json::json!({ "app": t }),
                    });
                }
                idx += 1;
                continue;
            }
            "RESTORE" => {
                let mut targets = collect_targets(&words, idx);
                if targets.is_empty() && has_pronoun {
                    if let Some(last) = convo_last_app.or(ctx.focused_app()) {
                        targets.push(last.to_string());
                    }
                }
                for t in targets {
                    intents.push(Intent {
                        capability: CapabilityId::WindowRestore,
                        arguments: serde_json::json!({ "app": t }),
                    });
                }
                idx += 1;
                continue;
            }
            _ => {}
        }
        idx += 1;
    }

    // If we found window/app intents via verb scanning, return them.
    if !intents.is_empty() {
        return intents;
    }

    // Fallback to legacy single-intent heuristics for phrases not captured above (e.g., "Is Calculator running?" etc)
    // Keep backwards compatibility: check for APPS list, RUNNING, STATUS etc.

    // app.list
    if words.contains(&"APPS") || upper.contains("APPLICATIONS") {
        // But also check if it's a window list query already handled; fallback to app list.
        if !intents.iter().any(|i| i.capability == CapabilityId::WindowList) {
            return vec![Intent {
                capability: CapabilityId::AppList,
                arguments: serde_json::json!({}),
            }];
        }
    }

    // app.status: "<APP> RUNNING?" / "IS <APP> RUNNING"
    if upper.contains("RUNNING") {
        let running_idx = words.iter().position(|w| w.contains("RUNNING"));
        let target = running_idx.and_then(|ri| {
            words[..ri]
                .iter()
                .rev()
                .find(|w| !matches!(**w, "IS" | "THE" | "MY" | "STILL"))
                .map(|w| {
                    w.trim_end_matches(|c: char| !c.is_ascii_alphanumeric()).to_ascii_lowercase()
                })
                .and_then(|w| w.non_empty())
        });
        return vec![Intent {
            capability: CapabilityId::AppStatus,
            arguments: serde_json::json!({ "app": target }),
        }];
    }

    // system.status
    if upper.contains("STATUS") || upper.contains("HEALTH") {
        return vec![Intent {
            capability: CapabilityId::SystemStatus,
            arguments: serde_json::json!({}),
        }];
    }

    Vec::new()
}

/// Validates an intent against the capability policy. Unknown capabilities
/// and malformed arguments are rejected here, before anything executes.
pub fn validate(intent: &Intent) -> Result<(), Rejection> {
    // Unknown capability names can only arrive via forged requests; the
    // enum makes them unrepresentable, but keep an explicit guard anyway.
    if CapabilityId::from_str(intent.capability.as_str()).is_none() {
        return Err(Rejection("UNKNOWN_CAPABILITY".to_string()));
    }

    let needs_target = matches!(
        intent.capability,
        CapabilityId::AppLaunch
            | CapabilityId::AppClose
            | CapabilityId::AppStatus
            | CapabilityId::WindowFocus
            | CapabilityId::WindowMinimize
            | CapabilityId::WindowMaximize
            | CapabilityId::WindowClose
            | CapabilityId::WindowRestore
    );

    if needs_target {
        let app_value = intent.arguments.get("app").cloned().unwrap_or(Value::Null);
        let ok = match app_value {
            Value::String(s) => {
                !s.trim().is_empty()
                    && s.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_')
                    })
            }
            _ => false,
        };
        if !ok {
            return Err(Rejection(
                "MALFORMED_ARGUMENTS: 'app' must be a registered app id".to_string(),
            ));
        }
    }

    // File capabilities validation
    match intent.capability {
        CapabilityId::FileList => {
            if let Some(p) = intent.arguments.get("path") {
                if !p.is_string() {
                    return Err(Rejection(
                        "MALFORMED_ARGUMENTS: 'path' must be string".to_string(),
                    ));
                }
                let s = p.as_str().unwrap_or_default();
                if s.contains('\0') {
                    return Err(Rejection("MALFORMED_ARGUMENTS: path contains null".to_string()));
                }
            }
        }
        CapabilityId::FileSearch => {
            let q = intent.arguments.get("query").and_then(|v| v.as_str()).unwrap_or("");
            if q.trim().is_empty() || q.len() > 100 {
                return Err(Rejection(
                    "MALFORMED_ARGUMENTS: 'query' must be 1..100 chars".to_string(),
                ));
            }
        }
        CapabilityId::FileRead | CapabilityId::FileCreate | CapabilityId::FileDelete => {
            let p = intent.arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if p.trim().is_empty() {
                return Err(Rejection("MALFORMED_ARGUMENTS: 'path' is required".to_string()));
            }
            if p.contains('\0') {
                return Err(Rejection("MALFORMED_ARGUMENTS: path contains null".to_string()));
            }
        }
        CapabilityId::FileWrite => {
            let p = intent.arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if p.trim().is_empty() {
                return Err(Rejection("MALFORMED_ARGUMENTS: 'path' is required".to_string()));
            }
            if !intent.arguments.get("content").is_some_and(|v| v.is_string()) {
                return Err(Rejection("MALFORMED_ARGUMENTS: 'content' must be string".to_string()));
            }
        }
        CapabilityId::FileRename | CapabilityId::FileMove => {
            let from = intent.arguments.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = intent.arguments.get("to").and_then(|v| v.as_str()).unwrap_or("");
            if from.trim().is_empty() || to.trim().is_empty() {
                return Err(Rejection("MALFORMED_ARGUMENTS: 'from' and 'to' required".to_string()));
            }
        }
        _ => {}
    }

    if !intent.capability.auto_execute() {
        return Err(Rejection("APPROVAL_REQUIRED".to_string()));
    }
    Ok(())
}

/// Executes a validated intent against the control plane via the SDK client.
pub fn execute(intent: &Intent, client: &aether_sdk::AetherClient) -> Result<Value, String> {
    execute_extended(intent, client, &SurfaceClient::new(4750))
}

/// Extended executor handling both control plane and surface server.
pub fn execute_extended(
    intent: &Intent,
    client: &aether_sdk::AetherClient,
    surface: &SurfaceClient,
) -> Result<Value, String> {
    // Local helper: build a control-plane request attributed to the
    // agent (trusted). The actor_trust field was added in Phase 11
    // alongside the system-core policy gate.
    let req = |command: &str, parameters: serde_json::Value| aether_sdk::IpcRequest {
        service_id: "aether-system-core".to_string(),
        command: command.to_string(),
        parameters,
        actor_trust: aether_sdk::ActorTrust::Trusted,
    };
    let response = match intent.capability {
        CapabilityId::SystemStatus => client.status()?,
        CapabilityId::AppStatus => {
            client.request(&req("app.status", serde_json::json!({ "app": intent.arguments["app"] })))?
        }
        CapabilityId::AppList => client.request(&req("app.list", serde_json::json!({})))?,
        CapabilityId::AppLaunch => {
            client.request(&req("app.launch", serde_json::json!({ "app": intent.arguments["app"] })))?
        }
        CapabilityId::AppClose => {
            client.request(&req("app.close", serde_json::json!({ "app": intent.arguments["app"] })))?
        }
        CapabilityId::WindowList => {
            // Try control plane proxy first, fallback to direct surface.
            let via_control = client.request(&req("window.list", serde_json::json!({})));
            match via_control {
                Ok(r) if r.ok => return Ok(r.result),
                _ => {
                    // Direct surface
                    let v = surface.window_list()?;
                    return Ok(v);
                }
            }
        }
        CapabilityId::WindowFocus => {
            let app = intent.arguments["app"].as_str().unwrap_or_default();
            // Try control plane first
            let via_control = client.request(&req("window.focus", serde_json::json!({ "app": app })));
            match via_control {
                Ok(r) if r.ok => return Ok(r.result),
                _ => {
                    let v = surface.window_focus(app, None)?;
                    return Ok(v);
                }
            }
        }
        CapabilityId::WindowMinimize => {
            let app = intent.arguments["app"].as_str().unwrap_or_default();
            let via_control =
                client.request(&req("window.minimize", serde_json::json!({ "app": app })));
            match via_control {
                Ok(r) if r.ok => return Ok(r.result),
                _ => {
                    let v = surface.window_minimize(app, None)?;
                    return Ok(v);
                }
            }
        }
        CapabilityId::WindowMaximize => {
            let app = intent.arguments["app"].as_str().unwrap_or_default();
            let via_control =
                client.request(&req("window.maximize", serde_json::json!({ "app": app })));
            match via_control {
                Ok(r) if r.ok => return Ok(r.result),
                _ => {
                    let v = surface.window_maximize(app, None)?;
                    return Ok(v);
                }
            }
        }
        CapabilityId::WindowClose => {
            let app = intent.arguments["app"].as_str().unwrap_or_default();
            let via_control =
                client.request(&req("window.close", serde_json::json!({ "app": app })));
            match via_control {
                Ok(r) if r.ok => return Ok(r.result),
                _ => {
                    let v = surface.window_close(app, None)?;
                    return Ok(v);
                }
            }
        }
        CapabilityId::WindowRestore => {
            let app = intent.arguments["app"].as_str().unwrap_or_default();
            let via_control =
                client.request(&req("window.restore", serde_json::json!({ "app": app })));
            match via_control {
                Ok(r) if r.ok => return Ok(r.result),
                _ => {
                    // Restore is focus with restore semantics; direct surface has no restore op but focus un-minimizes.
                    let v = surface.window_focus(app, None)?;
                    return Ok(v);
                }
            }
        }
        CapabilityId::ContextGet => client.request(&req("context.get", serde_json::json!({})))?,
        CapabilityId::FileList => client.request(&req(
            "file.list",
            serde_json::json!({ "path": intent.arguments.get("path").cloned().unwrap_or(Value::String(String::new())) }),
        ))?,
        CapabilityId::FileSearch => {
            client.request(&req("file.search", serde_json::json!({ "query": intent.arguments["query"] })))?
        }
        CapabilityId::FileRead => {
            client.request(&req("file.read", serde_json::json!({ "path": intent.arguments["path"] })))?
        }
        CapabilityId::FileCreate => client.request(&req(
            "file.create",
            serde_json::json!({ "path": intent.arguments["path"], "content": intent.arguments.get("content").cloned().unwrap_or(Value::String(String::new())) }),
        ))?,
        CapabilityId::FileWrite => client.request(&req(
            "file.write",
            serde_json::json!({ "path": intent.arguments["path"], "content": intent.arguments["content"] }),
        ))?,
        CapabilityId::FileRename => client.request(&req(
            "file.rename",
            serde_json::json!({ "from": intent.arguments["from"], "to": intent.arguments["to"] }),
        ))?,
        CapabilityId::FileMove => client.request(&req(
            "file.move",
            serde_json::json!({ "from": intent.arguments["from"], "to": intent.arguments["to"] }),
        ))?,
        CapabilityId::FileDelete => {
            client.request(&req("file.delete", serde_json::json!({ "path": intent.arguments["path"] })))?
        }
        CapabilityId::SystemInfo => client.request(&req("system.info", serde_json::json!({})))?,
        CapabilityId::SystemResources => {
            client.request(&req("system.resources", serde_json::json!({})))?
        }
        CapabilityId::SystemUptime => client.request(&req("system.uptime", serde_json::json!({})))?,
    };

    if response.ok {
        Ok(response.result)
    } else {
        Err(response
            .error
            .map(|e| format!("{}: {}", e.code, e.message))
            .unwrap_or_else(|| "unknown error".to_string()))
    }
}

/// Client used by the agent's capability executor.
pub fn control_client(port: u16) -> aether_sdk::AetherClient {
    aether_sdk::AetherClient::new(format!("127.0.0.1:{port}"), Duration::from_secs(5))
}

/// Human-readable (uppercase-rendered later) summary of a capability result.
pub fn format_result(capability: CapabilityId, result: &Value) -> String {
    match capability {
        CapabilityId::SystemStatus => {
            let health = result["overall_health"].as_str().unwrap_or("UNKNOWN");
            let count = result["services"].as_array().map(|a| a.len()).unwrap_or(0);
            format!("SYSTEM {health} - {count} SERVICES REGISTERED")
        }
        CapabilityId::AppList => {
            let names: Vec<String> = result["apps"]
                .as_array()
                .map(|apps| {
                    apps.iter().map(|a| a["id"].as_str().unwrap_or("?").to_uppercase()).collect()
                })
                .unwrap_or_default();
            if names.is_empty() {
                "NO APPLICATIONS INSTALLED".to_string()
            } else {
                format!("INSTALLED APPS: {}", names.join(", "))
            }
        }
        CapabilityId::AppLaunch => {
            let app = result["app"].as_str().unwrap_or("APP").to_uppercase();
            let pid = result["instance"]["pid"].as_u64();
            match pid {
                Some(p) => format!("LAUNCHED {app} (PID {p})"),
                None => format!("LAUNCHED {app}"),
            }
        }
        CapabilityId::AppClose => "APPLICATION CLOSED".to_string(),
        CapabilityId::AppStatus => {
            let app = result["report"]["app"].as_str().unwrap_or("APP").to_uppercase();
            let state = result["report"]["state"].as_str().unwrap_or("UNKNOWN");
            format!("{app} STATE: {state}")
        }
        CapabilityId::WindowList => {
            let wins = result["windows"].as_array();
            if let Some(arr) = wins {
                if arr.is_empty() {
                    return "NO WINDOWS OPEN".to_string();
                }
                let titles: Vec<String> =
                    arr.iter().map(|w| w["title"].as_str().unwrap_or("?").to_uppercase()).collect();
                format!("OPEN WINDOWS: {}", titles.join(", "))
            } else {
                // Fallback if result is directly window list array
                "NO WINDOWS OPEN".to_string()
            }
        }
        CapabilityId::WindowFocus => {
            let app = result["app"].as_str().unwrap_or("WINDOW").to_uppercase();
            format!("FOCUSED {app}")
        }
        CapabilityId::WindowMinimize => "WINDOW MINIMIZED".to_string(),
        CapabilityId::WindowMaximize => "WINDOW MAXIMIZED".to_string(),
        CapabilityId::WindowClose => "WINDOW CLOSED".to_string(),
        CapabilityId::WindowRestore => "WINDOW RESTORED".to_string(),
        CapabilityId::ContextGet => {
            // Pretty print context grounding
            format!("CONTEXT: {result}")
        }
        CapabilityId::FileList => {
            let files = result["files"].as_array().or_else(|| result["entries"].as_array());
            if let Some(arr) = files {
                if arr.is_empty() {
                    return "NO FILES FOUND".to_string();
                }
                let names: Vec<String> = arr
                    .iter()
                    .map(|f| {
                        f["relative_path"]
                            .as_str()
                            .or_else(|| f["filename"].as_str())
                            .unwrap_or("?")
                            .to_string()
                    })
                    .collect();
                format!("FILES: {}", names.join(", "))
            } else {
                format!("FILES: {}", result)
            }
        }
        CapabilityId::FileSearch => {
            let arr = result["results"].as_array().or_else(|| result["files"].as_array());
            if let Some(files) = arr {
                if files.is_empty() {
                    return "NO FILES FOUND".to_string();
                }
                let names: Vec<String> = files
                    .iter()
                    .map(|f| {
                        f["relative_path"]
                            .as_str()
                            .or_else(|| f["filename"].as_str())
                            .unwrap_or("?")
                            .to_string()
                    })
                    .collect();
                format!("FOUND {} FILES: {}", files.len(), names.join(", "))
            } else {
                format!("SEARCH RESULTS: {}", result)
            }
        }
        CapabilityId::FileRead => {
            let path = result["path"].as_str().unwrap_or("FILE");
            let preview =
                result["content"].as_str().unwrap_or("").chars().take(80).collect::<String>();
            if preview.is_empty() {
                format!(
                    "READ {}: {} bytes",
                    path.to_uppercase(),
                    result["size"].as_u64().unwrap_or(0)
                )
            } else {
                format!("READ {}: {}", path.to_uppercase(), preview)
            }
        }
        CapabilityId::FileCreate => {
            let path = result["path"].as_str().unwrap_or("FILE").to_uppercase();
            let bytes =
                result["bytes_written"].as_u64().or_else(|| result["size"].as_u64()).unwrap_or(0);
            format!("CREATED {path} ({bytes} bytes)")
        }
        CapabilityId::FileWrite => {
            let path = result["path"].as_str().unwrap_or("FILE").to_uppercase();
            let bytes = result["bytes_written"].as_u64().unwrap_or(0);
            format!("WROTE {bytes} bytes to {path}")
        }
        CapabilityId::FileRename => {
            let from = result["from"].as_str().unwrap_or("FILE").to_uppercase();
            let to = result["to"].as_str().unwrap_or("FILE").to_uppercase();
            format!("RENAMED {from} -> {to}")
        }
        CapabilityId::FileMove => {
            let from = result["from"].as_str().unwrap_or("FILE").to_uppercase();
            let to = result["to"].as_str().unwrap_or("FILE").to_uppercase();
            format!("MOVED {from} -> {to}")
        }
        CapabilityId::FileDelete => "FILE DELETED".to_string(),
        CapabilityId::SystemInfo => {
            let os = result["os_version"].as_str().unwrap_or("UNKNOWN");
            let kernel = result["kernel_version"].as_str().unwrap_or("");
            format!("SYSTEM {os} {kernel}").trim().to_string()
        }
        CapabilityId::SystemResources => {
            let mem_total = result["memory"]["total_kib"].as_u64().unwrap_or(0);
            let mem_avail = result["memory"]["available_kib"].as_u64().unwrap_or(0);
            let cpu = result["cpu_count"].as_u64().unwrap_or(0);
            format!("RESOURCES CPU:{cpu} MEM:{mem_total}KiB avail:{mem_avail}KiB")
        }
        CapabilityId::SystemUptime => {
            let human = result["uptime_human"].as_str().unwrap_or("");
            let ms = result["uptime_ms"].as_u64().unwrap_or(0);
            if !human.is_empty() {
                format!("UPTIME {human} ({ms}ms)")
            } else {
                format!("UPTIME {ms}ms")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SystemContext;

    #[test]
    fn open_sentence_maps_to_app_launch() {
        let intent = parse_intent("Open the calculator.")
            .unwrap_or_else(|| panic!("expected launch intent"));
        assert_eq!(intent.capability, CapabilityId::AppLaunch);
        assert_eq!(intent.arguments["app"], "calculator");
    }

    #[test]
    fn apps_list_sentence_maps_to_app_list() {
        let intent = parse_intent("Show me what apps are installed.")
            .unwrap_or_else(|| panic!("expected list intent"));
        assert_eq!(intent.capability, CapabilityId::AppList);
    }

    #[test]
    fn status_sentence_maps_to_system_status() {
        let intent = parse_intent("what is the system health?")
            .unwrap_or_else(|| panic!("expected status intent"));
        assert_eq!(intent.capability, CapabilityId::SystemStatus);
    }

    #[test]
    fn plain_chat_has_no_intent() {
        assert!(parse_intent("tell me a joke").is_none());
        assert!(parse_intent("").is_none());
    }

    #[test]
    fn validate_rejects_malformed_launch_arguments() {
        let intent =
            Intent { capability: CapabilityId::AppLaunch, arguments: serde_json::json!({}) };
        assert_eq!(
            validate(&intent),
            Err(Rejection("MALFORMED_ARGUMENTS: 'app' must be a registered app id".to_string()))
        );
    }

    #[test]
    fn validate_accepts_wellformed_launch() {
        let intent = parse_intent("open calculator").unwrap_or_else(|| panic!("expected intent"));
        assert!(validate(&intent).is_ok());
    }

    #[test]
    fn close_sentence_maps_to_app_close() {
        let intent =
            parse_intent("Close Calculator.").unwrap_or_else(|| panic!("expected close intent"));
        assert_eq!(intent.capability, CapabilityId::AppClose);
        assert_eq!(intent.arguments["app"], "calculator");
    }

    #[test]
    fn running_question_maps_to_app_status() {
        let intent = parse_intent("Is Calculator running?")
            .unwrap_or_else(|| panic!("expected status intent"));
        assert_eq!(intent.capability, CapabilityId::AppStatus);
        assert_eq!(intent.arguments["app"], "calculator");
        assert!(validate(&intent).is_ok());
    }

    #[test]
    fn unknown_capability_names_are_unrepresentable_but_guarded() {
        assert!(CapabilityId::from_str("shell.exec").is_none());
    }

    #[test]
    fn whats_open_maps_to_window_list() {
        let intents = parse_intents("What's open?", &SystemContext::empty(), None);
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].capability, CapabilityId::WindowList);
    }

    #[test]
    fn open_calculator_and_notes_produces_two_launches() {
        let intents = parse_intents("Open Calculator and Notes.", &SystemContext::empty(), None);
        assert_eq!(intents.len(), 2);
        assert_eq!(intents[0].capability, CapabilityId::AppLaunch);
        assert_eq!(intents[0].arguments["app"], "calculator");
        assert_eq!(intents[1].capability, CapabilityId::AppLaunch);
        assert_eq!(intents[1].arguments["app"], "notes");
    }

    #[test]
    fn bring_notes_to_front_maps_to_window_focus() {
        let intents = parse_intents("Bring Notes to the front.", &SystemContext::empty(), None);
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].capability, CapabilityId::WindowFocus);
        assert_eq!(intents[0].arguments["app"], "notes");
    }

    #[test]
    fn minimize_calculator_maps_correctly() {
        let intents = parse_intents("Minimize Calculator.", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::WindowMinimize);
        assert_eq!(intents[0].arguments["app"], "calculator");
    }

    #[test]
    fn maximize_notes_maps_correctly() {
        let intents = parse_intents("Maximize Notes.", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::WindowMaximize);
        assert_eq!(intents[0].arguments["app"], "notes");
    }

    #[test]
    fn pronoun_it_resolves_to_last_app() {
        let ctx = SystemContext::empty();
        let intents = parse_intents("Bring it to the front.", &ctx, Some("notes"));
        assert_eq!(intents[0].capability, CapabilityId::WindowFocus);
        assert_eq!(intents[0].arguments["app"], "notes");
    }

    #[test]
    fn window_list_and_app_launch_are_distinct() {
        let a = parse_intents("What's open?", &SystemContext::empty(), None);
        let b = parse_intents("Open Calculator", &SystemContext::empty(), None);
        assert_eq!(a[0].capability, CapabilityId::WindowList);
        assert_eq!(b[0].capability, CapabilityId::AppLaunch);
    }

    #[test]
    fn show_my_files_maps_to_file_list() {
        let intents = parse_intents("Show my files.", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::FileList);
    }

    #[test]
    fn find_markdown_maps_to_file_search() {
        let intents = parse_intents("Find all markdown files.", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::FileSearch);
        assert_eq!(intents[0].arguments["query"], "md");
    }

    #[test]
    fn read_roadmap_maps_to_file_read() {
        let intents = parse_intents("Read roadmap.md.", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::FileRead);
        assert_eq!(intents[0].arguments["path"], "roadmap.md");
    }

    #[test]
    fn create_ideas_maps_to_file_create() {
        let intents =
            parse_intents("Create a file called ideas.md.", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::FileCreate);
        assert_eq!(intents[0].arguments["path"], "ideas.md");
    }

    #[test]
    fn write_ideas_maps_to_file_write() {
        let intents =
            parse_intents("Write 'Aether OS idea' into ideas.md.", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::FileWrite);
        assert_eq!(intents[0].arguments["path"], "ideas.md");
        assert_eq!(intents[0].arguments["content"], "Aether OS idea");
    }

    #[test]
    fn rename_maps_correctly() {
        let intents =
            parse_intents("Rename ideas.md to project-ideas.md.", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::FileRename);
        assert_eq!(intents[0].arguments["from"], "ideas.md");
        assert_eq!(intents[0].arguments["to"], "project-ideas.md");
    }

    #[test]
    fn move_maps_correctly() {
        let intents =
            parse_intents("Move project-ideas.md into Documents.", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::FileMove);
        assert_eq!(intents[0].arguments["from"], "project-ideas.md");
        let to = intents[0].arguments["to"].as_str().unwrap_or("");
        assert!(to.contains("Documents"), "expected 'to' to contain 'Documents', got: {to}");
    }

    #[test]
    fn system_resources_maps_correctly() {
        let intents = parse_intents("How much RAM is available?", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::SystemResources);
    }

    #[test]
    fn system_uptime_maps_correctly() {
        let intents =
            parse_intents("How long has Aether been running?", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::SystemUptime);
    }

    #[test]
    fn security_absolute_path_still_parses_for_validation() {
        let intents = parse_intents("Read /etc/shadow.", &SystemContext::empty(), None);
        assert_eq!(intents[0].capability, CapabilityId::FileRead);
        assert_eq!(intents[0].arguments["path"], "/etc/shadow");
        // Validation should pass (path is string), execution will reject via FileManager
        assert!(validate(&intents[0]).is_ok());
    }

    #[test]
    fn delete_requires_high_risk_confirmation() {
        let intent = parse_intents("Delete all files.", &SystemContext::empty(), None)[0].clone();
        assert_eq!(intent.capability, CapabilityId::FileDelete);
        assert_eq!(intent.capability.capability().risk_level, aether_core::RiskLevel::High);
        assert!(!crate::confirmation::ConfirmationPolicy::is_auto(&intent.capability.capability()));
    }
}
