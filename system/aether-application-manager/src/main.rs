// Aether Application Manager - REPL daemon entry point.
//
// Newline-delimited JSON commands on stdin, JSON responses on stdout.
// Commands: list, launch <id>, close <instance>, running.

use aether_application_manager::ApplicationManager;
use std::io::{BufRead, Write};

fn main() {
    let mut manager = ApplicationManager::new();
    // Seed the registry from a JSON config if provided: {"apps":[{"id","name","command"},...]}
    if let Ok(seed) = std::env::var("AETHER_APP_SEED") {
        if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&seed) {
            if let Some(apps) = doc.get("apps").and_then(|a| a.as_array()) {
                for app in apps {
                    let id = app.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                    let name = app
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let command = app
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    if let Err(e) = manager.register_json(id, name, command) {
                        eprintln!("[app-manager] seed rejected '{id}': {e}");
                    }
                }
            }
        }
    }

    eprintln!("[app-manager] ready; {} apps registered", manager.list().len());

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let mut tokens = line.split_whitespace();
        let response = match (tokens.next(), tokens.next(), tokens.next()) {
            (Some("list"), None, None) => serde_json::json!({
                "ok": true,
                "result": manager.list().into_iter().map(|(id, _, _)| id).collect::<Vec<_>>(),
            }),
            (Some("inspect"), Some(id), None) => match manager.list().into_iter().find(|(aid, _, _)| aid == id) {
                Some((_, name, command)) => serde_json::json!({
                    "ok": true,
                    "result": { "id": id, "name": name, "command": command },
                }),
                None => serde_json::json!({ "ok": false, "error": format!("unknown app '{id}'") }),
            },
            (Some("launch"), Some(id), None) => match manager.launch(id) {
                Ok(instance) => serde_json::json!({ "ok": true, "result": instance }),
                Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
            },
            (Some("close"), Some(instance), None) => match instance.parse::<u64>() {
                Ok(n) => match manager.close(n) {
                    Ok(closed) => serde_json::json!({ "ok": true, "result": closed }),
                    Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
                },
                Err(_) => serde_json::json!({ "ok": false, "error": "instance must be numeric" }),
            },
            (Some("running"), None, None) => serde_json::json!({
                "ok": true,
                "result": manager.running(),
            }),
            (None, None, None) => continue,
            _ => serde_json::json!({
                "ok": false,
                "error": "commands: list | inspect <id> | launch <id> | close <n> | running",
            }),
        };

        let mut payload = serde_json::to_string(&response).unwrap_or_default();
        payload.push('\n');
        let mut out = stdout.lock();
        if out.write_all(payload.as_bytes()).is_err() || out.flush().is_err() {
            break;
        }
    }
}
