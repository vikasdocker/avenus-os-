// Surface server: applications register windows and receive key/close
// events. Control clients (the AI agent capability layer) may also connect
// and drive window ops without registering a window.

use aether_wm::{WindowAction, WindowId, WindowManager};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub type Clients = Arc<Mutex<HashMap<u64, std::net::TcpStream>>>;

/// Commands the server sends into the UI loop.
pub enum SurfaceCommand {
    Close(WindowId),
}

pub struct SurfaceServer {
    pub tx: Sender<SurfaceCommand>,
    pub wm: Arc<Mutex<WindowManager>>,
    pub clients: Clients,
}

fn send_line(stream: &mut std::net::TcpStream, value: &Value) {
    let _ = stream.write_all(format!("{value}\n").as_bytes());
    let _ = stream.flush();
}

pub fn notify_close(clients: &Clients, id: u64) {
    if let Ok(mut map) = clients.lock() {
        if let Some(mut s) = map.remove(&id) {
            let _ = s.write_all(b"{\"event\":\"close\"}\n");
            let _ = s.flush();
        }
    }
}

pub fn spawn(
    port: u16,
    server: SurfaceServer,
) -> Result<(), String> {
    let listener =
        TcpListener::bind(("0.0.0.0", port)).map_err(|e| format!("bind :{port}: {e}"))?;
    eprintln!("[surface] listening on 0.0.0.0:{port}");
    std::thread::spawn(move || {
        let SurfaceServer { tx, wm, clients } = server;
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            let wm = Arc::clone(&wm);
            let clients = Arc::clone(&clients);
            std::thread::spawn(move || {
                let Ok(writer) = stream.try_clone() else { return };
                let mut reader = BufReader::new(stream);
                let mut writer = writer;
                let mut registered: Option<u64> = None;

                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                    let Ok(req) = serde_json::from_str::<Value>(line.trim()) else { continue };
                    let op = req["op"].as_str().unwrap_or_default().to_string();

                    if op == "register" {
                        let app_id = req["app"].as_str().unwrap_or("app").to_string();
                        let title = req["title"].as_str().unwrap_or(&app_id).to_string();
                        let w = req["w"].as_u64().unwrap_or(420) as u32;
                        let h = req["h"].as_u64().unwrap_or(320) as u32;
                        let created = {
                            let mut guard = wm.lock().unwrap_or_else(|p| p.into_inner());
                            guard.create(&app_id, &title, w, h)
                        };
                        let Some(win) = created else { break };
                        registered = Some(win.id);
                        if let (Ok(mut map), Some(clone)) =
                            (clients.lock(), writer.try_clone().ok())
                        {
                            map.insert(win.id, clone);
                        }
                        eprintln!("[surface] window {} '{}' for '{app_id}'", win.id, win.title);
                        send_line(
                            &mut writer,
                            &json!({
                                "event": "registered",
                                "window_id": win.id,
                                "rect": [win.x, win.y + 28, win.width, win.height.saturating_sub(28)],
                            }),
                        );
                        continue;
                    }

                    // Control-plane ops (agent capability layer).
                    let mut respond = |v: &Value| send_line(&mut writer, v);
                    match op.as_str() {
                        "window.list" => {
                            let list: Vec<Value> = {
                                let guard = wm.lock().unwrap_or_else(|p| p.into_inner());
                                guard
                                    .stacked()
                                    .into_iter()
                                    .map(|w| {
                                        json!({
                                            "id": w.id, "app": w.app_id, "title": w.title,
                                            "x": w.x, "y": w.y, "w": w.width, "h": w.height,
                                            "state": w.state.to_string(), "focused": w.focused,
                                        })
                                    })
                                    .collect()
                            };
                            respond(&json!({ "ok": true, "windows": list }));
                        }
                        "window.focus" | "window.minimize" | "window.maximize" => {
                            let Some(id) = req["window_id"].as_u64() else {
                                respond(&json!({ "ok": false, "error": "window_id required" }));
                                continue;
                            };
                            let action = match op.as_str() {
                                "window.focus" => WindowAction::Focus(id),
                                "window.minimize" => WindowAction::Minimize(id),
                                _ => WindowAction::Maximize(id),
                            };
                            let applied = {
                                let mut guard = wm.lock().unwrap_or_else(|p| p.into_inner());
                                guard.apply(&action)
                            };
                            if applied.is_some() {
                                respond(&json!({ "ok": true, "window_id": id }));
                            } else {
                                respond(&json!({ "ok": false, "error": "no such window" }));
                            }
                        }
                        "window.close" => {
                            let id = req["window_id"].as_u64();
                            let by_app = req["app_id"].as_str().map(|s| s.to_string());
                            let target = match (id, by_app) {
                                (Some(id), _) => Some(id),
                                (None, Some(ref app)) => find_by_app(&wm, app),
                                _ => None,
                            };
                            match target {
                                Some(wid) => {
                                    notify_close(&clients, wid);
                                    let _ = tx.send(SurfaceCommand::Close(wid));
                                    respond(&json!({ "ok": true, "closed": wid }));
                                }
                                None => {
                                    respond(&json!({ "ok": false, "error": "no such window" }))
                                }
                            }
                        }
                        _ => respond(&json!({
                            "ok": false,
                            "error": format!("unknown op '{op}'"),
                        })),
                    }
                }

                // Disconnected: if it was an app window, close it (lifecycle).
                if let Some(id) = registered {
                    let _ = tx.send(SurfaceCommand::Close(id));
                }
            });
        }
    });
    Ok(())
}

fn find_by_app(wm: &Arc<Mutex<WindowManager>>, app: &str) -> Option<u64> {
    let guard = wm.lock().unwrap_or_else(|p| p.into_inner());
    guard
        .stacked()
        .into_iter()
        .find(|w| w.app_id == app)
        .map(|w| w.id)
}
