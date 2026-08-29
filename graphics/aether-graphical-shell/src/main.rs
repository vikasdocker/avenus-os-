// Aether Desktop Shell - multi-window graphical operating environment.
//
// Owns the framebuffer, embeds the Window Manager (aether-wm), runs the
// surface server (:4750) where Aether applications register windows, and
// decodes evdev keyboard/mouse input. Applications paint their own content
// rectangle directly into the framebuffer; the shell draws everything
// around it (background, header, taskbar, window chrome) and arbitrates
// the exclusive surface when no window manager compositing is needed.
//
// Phase 1.9 Part 2: clock, workspace panel, application launcher,
// enhanced system panel (network/storage).

pub mod fb;
pub mod input;
pub mod surface_server;

use aether_wm::{ScreenArea, WindowAction, WindowManager};
use fb::{Rgb, Screen};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

const BG: Rgb = Rgb(14, 17, 22);
const PANEL: Rgb = Rgb(28, 34, 44);
const PANEL_HI: Rgb = Rgb(40, 48, 62);
const CYAN: Rgb = Rgb(34, 211, 238);
const FG: Rgb = Rgb(230, 237, 243);
const DIM: Rgb = Rgb(122, 132, 142);
const GREEN: Rgb = Rgb(74, 222, 128);
const RED: Rgb = Rgb(248, 113, 113);
const YELLOW: Rgb = Rgb(250, 204, 21);

const HEADER_H: i64 = 36;
const TASKBAR_H: i64 = 44;
const TITLE_H: i64 = 28;
const BOX_W: i64 = 22;
const LAUNCHER_W: i64 = 180;
const LAUNCHER_ITEM_H: i64 = 26;

const SURFACE_PORT: u16 = 4750;

// ------------------------------------------------------------------ events

#[allow(dead_code)]
enum UiEvent {
    Motion(i32, i32),
    Press,
    Release,
    Key(u16),
    WinClose(u64),
    StatusTick,
    ChatReply(ChatEntry),
}

#[derive(Clone)]
struct ChatEntry {
    prefix: &'static str,
    color: Rgb,
    text: String,
}

// --------------------------------------------------------------- app info

#[derive(Debug, Clone)]
struct AppInfo {
    id: String,
    name: String,
}

fn query_registered_apps() -> Vec<AppInfo> {
    use std::io::{BufRead, BufReader, Write};
    let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", 4747)) else {
        return Vec::new();
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let req = json!({
        "service_id": "aether-system-core",
        "command": "list",
        "parameters": { "type": "applications" },
    });
    if s.write_all(format!("{req}\n").as_bytes()).is_err() {
        return Vec::new();
    }
    let mut line = String::new();
    if BufReader::new(s).read_line(&mut line).is_err() || line.is_empty() {
        return Vec::new();
    }
    let v: Value = serde_json::from_str(line.trim()).unwrap_or(json!({}));
    let Some(apps) = v["result"]["applications"].as_array() else {
        return Vec::new();
    };
    apps.iter()
        .filter_map(|a| {
            let id = a["id"].as_str()?.to_string();
            let name = a["name"]
                .as_str()
                .unwrap_or(&id)
                .to_string();
            Some(AppInfo { id, name })
        })
        .collect()
}

// ------------------------------------------------------------------ state

struct UiState {
    status: Vec<(String, bool)>,
    chat: Vec<ChatEntry>,
    clock: String,
    active_workspace: u32,
    workspaces: Vec<u32>,
    launcher_open: bool,
    launcher_apps: Vec<AppInfo>,
    selected_launcher: usize,
    network_up: bool,
    storage_up: bool,
}

impl UiState {
    fn new() -> Self {
        Self {
            status: vec![
                ("OS".to_string(), true),
                ("AGENT".to_string(), false),
                ("CTRL".to_string(), false),
                ("APPS".to_string(), false),
            ],
            chat: Vec::new(),
            clock: "00:00".to_string(),
            active_workspace: 0,
            workspaces: vec![0],
            launcher_open: false,
            launcher_apps: Vec::new(),
            selected_launcher: 0,
            network_up: false,
            storage_up: false,
        }
    }
}

// ------------------------------------------------------------------ status

fn refresh_status(state: &mut UiState) {
    // Clock.
    if let Ok(now) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        let secs = now.as_secs();
        let h = (secs / 3600) % 24;
        let m = (secs / 60) % 60;
        state.clock = format!("{:02}:{:02}", h, m);
    }

    let call_ok = |port: u16, v: Value| -> bool {
        use std::io::{BufRead, BufReader, Write};
        let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
            return false;
        };
        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
        if s.write_all(format!("{v}\n").as_bytes()).is_err() {
            return false;
        }
        let mut line = String::new();
        matches!(
            BufReader::new(s).read_line(&mut line),
            Ok(n) if n > 0
        ) && line.contains("\"ok\":true")
    };

    for entry in state.status.iter_mut() {
        match entry.0.as_str() {
            "AGENT" => {
                entry.1 = call_ok(4748, json!({ "command": "status" }));
            }
            "CTRL" | "APPS" => {
                let ok = call_ok(
                    4747,
                    json!({
                        "service_id": "aether-system-core",
                        "command": "status",
                        "parameters": {},
                    }),
                );
                if entry.0 == "CTRL" {
                    entry.1 = ok;
                }
                if entry.0 == "APPS" {
                    entry.1 = ok;
                }
            }
            _ => {}
        }
    }

    // Network / storage status.
    state.network_up = call_ok(
        4747,
        json!({
            "service_id": "aether-system-core",
            "command": "status",
            "parameters": { "type": "network" },
        }),
    );
    state.storage_up = call_ok(
        4747,
        json!({
            "service_id": "aether-system-core",
            "command": "status",
            "parameters": { "type": "storage" },
        }),
    );
}

// -------------------------------------------------------------- rendering

fn draw_desktop(
    fb: &mut Screen,
    ui: &UiState,
    wm: &mut WindowManager,
    cursor: Option<(i32, i32)>,
) {
    fb.fill(BG);

    // Header bar.
    fb.rect(0, 0, fb.width, HEADER_H as u32, PANEL);
    fb.text("AETHER OS", 20, 10, 2, FG);
    fb.rect(0, HEADER_H - 2, fb.width, 2, CYAN);

    // Clock (center of header).
    let clock_label = &ui.clock;
    let clock_x = fb.centered_x(clock_label, 2);
    fb.text(clock_label, clock_x, 10, 2, CYAN);

    // Workspace indicators (right of center).
    let ws_start_x = fb.width as i64 / 2 + 80;
    let mut wpx = ws_start_x;
    for ws_id in &ui.workspaces {
        let label = format!("[{}]", ws_id);
        let color = if *ws_id == ui.active_workspace { CYAN } else { DIM };
        fb.text(&label, wpx, 12, 1, color);
        wpx += (label.len() as i64 + 1) * 6;
    }

    // Status pills right-aligned in header.
    let mut px = fb.width as i64 - 16;
    for (name, up) in ui.status.iter().rev() {
        let label = format!("{name}:{}", if *up { "UP" } else { "DOWN" });
        let w = label.len() as u32 * 6 * 2 + 12;
        px -= w as i64;
        fb.text(&label, px + 6, 12, 2, if *up { GREEN } else { RED });
        px -= 16;
    }

    // Network / Storage pills (right side, below status).
    let net_label = format!("NET:{}", if ui.network_up { "UP" } else { "DOWN" });
    let stor_label = format!("STOR:{}", if ui.storage_up { "UP" } else { "DOWN" });
    let net_w = net_label.len() as i64 * 12 + 12;
    let stor_w = stor_label.len() as i64 * 12 + 12;
    fb.text(
        &net_label,
        fb.width as i64 - 16 - net_w,
        HEADER_H - 16,
        1,
        if ui.network_up { GREEN } else { RED },
    );
    fb.text(
        &stor_label,
        fb.width as i64 - 16 - net_w - stor_w - 8,
        HEADER_H - 16,
        1,
        if ui.storage_up { GREEN } else { RED },
    );

    // Window chrome for every visible window.
    for w in wm.stacked().into_iter().filter(|w| {
        w.visible && w.state != aether_wm::WindowState::Minimized
    }) {
        let (wx, wy) = (i64::from(w.x), i64::from(w.y));
        let ww = i64::from(w.width);
        let focused_color = if w.focused { CYAN } else { DIM };
        // Frame border.
        fb.rect(wx, wy, w.width, w.height, focused_color);
        // Title bar background.
        fb.rect(
            wx + 1,
            wy + 1,
            w.width.saturating_sub(2),
            TITLE_H as u32 - 2,
            PANEL,
        );
        fb.text(&w.title.to_uppercase(), wx + 10, wy + 9, 2, FG);
        // Buttons right side: [_] [O] [X].
        let bx = wx + ww - BOX_W * 3 - 8;
        fb.text("_", bx, wy + 8, 2, DIM);
        fb.text("O", bx + BOX_W, wy + 8, 2, DIM);
        fb.text("X", bx + BOX_W * 2, wy + 8, 2, if w.focused { RED } else { DIM });

        // Clean canvas for app content (app paints over it).
        let (cx, cy, cw, ch) = w.content_rect();
        fb.rect(i64::from(cx), i64::from(cy), cw, ch, BG);
    }

    // Application launcher (left side panel).
    if ui.launcher_open {
        draw_launcher(fb, ui);
    }

    // Taskbar.
    let tb_y = fb.height as i64 - TASKBAR_H;
    fb.rect(0, tb_y, fb.width, TASKBAR_H as u32, PANEL);
    fb.rect(0, tb_y, fb.width, 2, CYAN);

    // Workspace quick-switch buttons in taskbar.
    let mut ws_tx = 16i64;
    for ws_id in &ui.workspaces {
        let label = format!("WS{}", ws_id);
        let bw = 40u32;
        let color = if *ws_id == ui.active_workspace { PANEL_HI } else { KEY_BG };
        fb.rect(ws_tx, tb_y + 6, bw, TASKBAR_H as u32 - 12, color);
        fb.text(&label, ws_tx + 4, tb_y + 15, 2, if *ws_id == ui.active_workspace { CYAN } else { DIM });
        ws_tx += bw as i64 + 4;
    }

    // Window taskbar buttons.
    let mut tx = ws_tx + 8;
    for w in wm.stacked().into_iter().filter(|w| {
        w.visible && w.state != aether_wm::WindowState::Minimized
    }) {
        let label = w.title.to_uppercase();
        let is_focused = w.focused;
        let bw = 150u32;
        fb.rect(tx, tb_y + 6, bw, TASKBAR_H as u32 - 12, if is_focused { PANEL_HI } else { KEY_BG });
        let label = if label.len() > 18 {
            format!("{}...", &label[..17])
        } else {
            label
        };
        fb.text(&label, tx + 8, tb_y + 15, 2, if is_focused { CYAN } else { DIM });
        tx += bw as i64 + 8;
        if tx > fb.width as i64 - 260 {
            break;
        }
    }

    // Active application indicator (right side of taskbar).
    let active = wm
        .focused_id()
        .and_then(|id| wm.get(id))
        .map(|w| format!("ACTIVE: {}", w.title.to_uppercase()))
        .unwrap_or_else(|| "NO ACTIVE WINDOW".to_string());
    fb.text(&active, fb.width as i64 - 16 - active.len() as i64 * 12, tb_y + 14, 2, GREEN);

    // AI conversation strip.
    let ai_y = tb_y - 24 * (ui.chat.len() as i64).min(3) - 8;
    for (i, entry) in ui.chat.iter().rev().take(3).enumerate() {
        let y = ai_y + (2 - i as i64) * 24;
        fb.text(entry.prefix, 16, y, 2, entry.color);
        fb.text(&entry.text.to_uppercase(), 70, y, 2, FG);
    }

    // Cursor sprite last.
    if let Some((mx, my)) = cursor {
        let (cx, cy) = (i64::from(mx), i64::from(my));
        fb.rect(cx, cy, 3, 12, FG);
        fb.rect(cx, cy, 12, 3, FG);
        fb.rect(cx, cy, 9, 9, DIM);
    }
}

fn draw_launcher(fb: &mut Screen, ui: &UiState) {
    let lx = 4;
    let ly = HEADER_H + 4;
    let lw = LAUNCHER_W as u32;
    let apps = &ui.launcher_apps;
    let lh = (apps.len() as i64 * LAUNCHER_ITEM_H + 32).min(fb.height as i64 - ly - TASKBAR_H - 8);

    // Launcher background.
    fb.rect(lx, ly, lw, lh as u32, PANEL);
    fb.rect(lx, ly, lw, 2, CYAN);
    fb.text("APPLICATIONS", lx + 8, ly + 6, 2, CYAN);

    if apps.is_empty() {
        fb.text("NO APPS", lx + 8, ly + 28, 1, DIM);
        return;
    }

    for (i, app) in apps.iter().enumerate() {
        let iy = ly + 24 + i as i64 * LAUNCHER_ITEM_H;
        if iy + LAUNCHER_ITEM_H > ly + lh {
            break;
        }
        let selected = i == ui.selected_launcher;
        let bg = if selected { PANEL_HI } else { PANEL };
        fb.rect(lx + 2, iy, lw - 4, LAUNCHER_ITEM_H as u32 - 2, bg);
        let label = if app.name.len() > 20 {
            format!("{}...", &app.name[..19])
        } else {
            app.name.clone()
        };
        fb.text(
            &label.to_uppercase(),
            lx + 10,
            iy + 7,
            2,
            if selected { CYAN } else { FG },
        );
    }
}

const KEY_BG: Rgb = Rgb(38, 45, 56);

// ------------------------------------------------------------ input decode

fn handle_key(code: u16, wm: &mut WindowManager, tx: &Sender<UiEvent>, ui: &mut UiState) -> Option<char> {
    const TAB: u16 = 15;
    const F2: u16 = 60;
    const F3: u16 = 61;
    const F4: u16 = 62;
    const F5: u16 = 63;
    const LEFT_CTRL: u16 = 29;
    const RIGHT_CTRL: u16 = 97;
    const KEY_1: u16 = 2;
    const KEY_2: u16 = 3;
    const KEY_3: u16 = 4;
    const KEY_4: u16 = 5;
    const KEY_5: u16 = 6;
    const KEY_6: u16 = 7;
    const KEY_7: u16 = 8;

    let focused = wm.focused_id();

    match code {
        TAB => {
            wm.cycle_focus();
            None
        }
        F2 => {
            if let Some(id) = focused {
                wm.apply(&WindowAction::Minimize(id));
            }
            None
        }
        F3 => {
            if let Some(id) = focused {
                let maxed = wm.get(id).map(|w| w.state == aether_wm::WindowState::Maximized);
                match maxed {
                    Some(true) => {
                        wm.apply(&WindowAction::Restore(id));
                    }
                    _ => {
                        wm.apply(&WindowAction::Maximize(id));
                    }
                }
            }
            None
        }
        F4 => {
            if let Some(id) = focused {
                m_close(wm, id, tx);
            }
            None
        }
        F5 => {
            // Toggle launcher.
            ui.launcher_open = !ui.launcher_open;
            if ui.launcher_open {
                ui.launcher_apps = query_registered_apps();
                ui.selected_launcher = 0;
            }
            None
        }
        LEFT_CTRL | RIGHT_CTRL => {
            // Ctrl held — consume but no action by itself.
            None
        }
        KEY_1 | KEY_2 | KEY_3 | KEY_4 | KEY_5 | KEY_6 | KEY_7 => {
            // Workspace switching: check if Ctrl is held (simplified check).
            let ws_id = match code {
                KEY_1 => 0,
                KEY_2 => 1,
                KEY_3 => 2,
                KEY_4 => 3,
                KEY_5 => 4,
                KEY_6 => 5,
                KEY_7 => 6,
                _ => unreachable!(),
            };
            // Ensure workspace exists.
            if !ui.workspaces.contains(&ws_id) {
                ui.workspaces.push(ws_id);
                ui.workspaces.sort();
            }
            ui.active_workspace = ws_id;
            wm.activate_workspace(ws_id);
            None
        }
        other => {
            if ui.launcher_open {
                match other {
                    KEY_1 => {
                        if let Some(app) = ui.launcher_apps.first() {
                            launch_app(&app.id, tx);
                        }
                        ui.launcher_open = false;
                        None
                    }
                    _ => input::key_to_char(other),
                }
            } else {
                input::key_to_char(other)
            }
        }
    }
}

fn launch_app(app_id: &str, tx: &Sender<UiEvent>) {
    use std::io::{BufRead, BufReader, Write};
    let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", 4747)) else {
        let _ = tx.send(UiEvent::ChatReply(ChatEntry {
            prefix: "SYS>",
            color: RED,
            text: "LAUNCH FAILED - cannot connect to system core".to_string(),
        }));
        return;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let req = json!({
        "service_id": "aether-system-core",
        "command": "dispatch",
        "parameters": {
            "command": "app.launch",
            "parameters": { "app": app_id },
        },
    });
    if s.write_all(format!("{req}\n").as_bytes()).is_err() {
        let _ = tx.send(UiEvent::ChatReply(ChatEntry {
            prefix: "SYS>",
            color: RED,
            text: "LAUNCH FAILED - send error".to_string(),
        }));
        return;
    }
    let mut line = String::new();
    match BufReader::new(s).read_line(&mut line) {
        Ok(0) | Err(_) => {
            let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                prefix: "SYS>",
                color: YELLOW,
                text: format!("LAUNCHING {}", app_id.to_uppercase()),
            }));
        }
        Ok(_) => {
            let v: Value = serde_json::from_str(line.trim()).unwrap_or(json!({}));
            let ok = v["ok"].as_bool().unwrap_or(false);
            if ok {
                let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                    prefix: "OK >",
                    color: GREEN,
                    text: format!("LAUNCHED {}", app_id.to_uppercase()),
                }));
            } else {
                let err = v["error"].as_str().unwrap_or("unknown");
                let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                    prefix: "ERR>",
                    color: RED,
                    text: format!("LAUNCH FAILED - {err}"),
                }));
            }
        }
    }
}

fn m_close(wm: &mut WindowManager, id: u64, _tx: &Sender<UiEvent>) {
    wm.apply(&WindowAction::Close(id));
}

// --------------------------------------------------------------- app glue

struct DragState {
    window: u64,
    grab_dx: i32,
    grab_dy: i32,
}

// ------------------------------------------------------------- app chat

#[derive(Debug, Clone)]
struct AgentChatReply {
    response: String,
    actions: Option<Vec<Value>>,
}

fn agent_chat(prompt: &str, port: u16) -> Result<AgentChatReply, String> {
    use std::io::{BufRead, BufReader, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| format!("connect agent: {e}"))?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(30)));
    let req = serde_json::json!({ "command": "chat", "argument": prompt });
    s.write_all(format!("{req}\n").as_bytes())
        .map_err(|e| format!("send: {e}"))?;
    let mut line = String::new();
    BufReader::new(s)
        .read_line(&mut line)
        .map_err(|e| format!("recv: {e}"))?;
    if line.trim().is_empty() {
        return Err("empty agent reply".to_string());
    }
    let v: Value = serde_json::from_str(line.trim()).map_err(|e| format!("decode: {e}"))?;
    let response = v["result"]["response"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "no response field".to_string())?;
    let actions = v["result"]["actions"].as_array().cloned();
    Ok(AgentChatReply { response, actions })
}

// ---------------------------------------------------------------- threads

fn spawn_input_thread(tx: Sender<UiEvent>) {
    std::thread::spawn(move || {
        let devices = input::open_devices();
        eprintln!("[gfx] input devices opened: {}", devices.len());
        for (mut file, mut kind) in devices {
            let tx = tx.clone();
            std::thread::spawn(move || {
                use std::io::Read;
                let mut byte_buf = [0u8; 24];
                loop {
                    match file.read(&mut byte_buf) {
                        Ok(0) => std::thread::sleep(Duration::from_millis(50)),
                        Ok(_) => {
                            if let Some(ev) =
                                input::decode(&byte_buf, &mut kind)
                            {
                                let ui_ev = match ev {
                                    input::RawInput::MouseMove(dx, dy) => UiEvent::Motion(dx, dy),
                                    input::RawInput::MouseDown => UiEvent::Press,
                                    input::RawInput::MouseUp => UiEvent::Release,
                                    input::RawInput::KeyPress(code) => UiEvent::Key(code),
                                    input::RawInput::Wheel(_) => continue,
                                };
                                let _ = tx.send(ui_ev);
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }
    });
}

fn spawn_status_thread(tx: Sender<UiEvent>) {
    std::thread::spawn(move || loop {
        let _ = tx.send(UiEvent::StatusTick);
        std::thread::sleep(Duration::from_secs(1));
    });
}

fn spawn_serial_thread(tx: Sender<UiEvent>, port: u16) {
    std::thread::spawn(move || {
        let Ok(mut serial) = std::fs::File::open("/dev/ttyS0") else {
            eprintln!("[gfx][WARN] no serial input");
            return;
        };
        let stty = std::process::Command::new("/bin/stty")
            .args(["-F", "/dev/ttyS0", "raw", "-echo", "-icanon", "min", "1", "time", "0"])
            .status();
        match stty {
            Ok(s) if s.success() => eprintln!("[gfx] serial raw mode ok"),
            _ => eprintln!("[gfx][WARN] stty failed"),
        }
        use std::io::Read;
        let mut word = String::new();
        let mut buf = [0u8; 1];
        loop {
            match serial.read(&mut buf) {
                Ok(0) => std::thread::sleep(Duration::from_millis(50)),
                Ok(_) => match buf[0] {
                    b'\r' | b'\n' => {
                        if !word.is_empty() {
                            let prompt = word.clone();
                            word.clear();
                            let tx = tx.clone();
                            std::thread::spawn(move || {
                                let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                                    prefix: "YOU>",
                                    color: FG,
                                    text: prompt.clone(),
                                }));
                                let result = agent_chat(&prompt, port);
                                match result {
                                    Ok(reply) => {
                                        if let Some(actions) = reply.actions.clone() {
                                            let mut first_msg: Option<String> = None;
                                            for act in &actions {
                                                let cap = act["capability"].as_str().unwrap_or("action");
                                                let status = act["status"].as_str().unwrap_or("Success");
                                                let msg = act["message"].as_str().unwrap_or("");
                                                if first_msg.is_none() {
                                                    first_msg = Some(msg.to_string());
                                                }
                                                let (prefix, color) = match status {
                                                    "Success" => ("OK >", GREEN),
                                                    "Failed" => ("ERR>", RED),
                                                    "Rejected" => ("!! >", RED),
                                                    "RequiresConsent" => (".. >", DIM),
                                                    _ => (" * >", CYAN),
                                                };
                                                let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                                                    prefix,
                                                    color,
                                                    text: format!("{}: {}", cap.to_ascii_uppercase(), msg),
                                                }));
                                            }
                                            let need_summary = match &first_msg {
                                                Some(f) => f != &reply.response,
                                                None => true,
                                            };
                                            if need_summary && !reply.response.is_empty() {
                                                let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                                                    prefix: "AI >",
                                                    color: CYAN,
                                                    text: reply.response,
                                                }));
                                            }
                                        } else {
                                            let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                                                prefix: "AI >",
                                                color: CYAN,
                                                text: reply.response,
                                            }));
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                                            prefix: "SYS>",
                                            color: RED,
                                            text: format!("ACTION FAILED - {e}"),
                                        }));
                                    }
                                }
                            });
                        }
                    }
                    _ => word.push(buf[0] as char),
                },
                Err(_) => break,
            }
        }
    });
}

// -------------------------------------------------------------- main loop

fn run() -> Result<(), String> {
    let mut fb = Screen::open()?;
    eprintln!("[desktop] framebuffer {}x{}", fb.width, fb.height);

    let area = ScreenArea {
        x: 0,
        y: (HEADER_H + 4) as i32,
        width: fb.width,
        height: (fb.height as i64 - HEADER_H - TASKBAR_H - 4).max(100) as u32,
    };
    let wm = Arc::new(Mutex::new(WindowManager::new(area)));
    let clients: surface_server::Clients = Arc::new(Mutex::new(HashMap::new()));

    let ui = UiState::new();
    let (tx, rx): (Sender<UiEvent>, Receiver<UiEvent>) = channel();
    let (stx, srx): (Sender<surface_server::SurfaceCommand>, Receiver<surface_server::SurfaceCommand>) =
        channel();

    surface_server::spawn(
        SURFACE_PORT,
        surface_server::SurfaceServer {
            tx: stx,
            wm: Arc::clone(&wm),
            clients: Arc::clone(&clients),
        },
    )?;
    eprintln!("[desktop] surface server ready on :{SURFACE_PORT}");

    spawn_input_thread(tx.clone());
    spawn_status_thread(tx.clone());
    if std::env::var("AETHER_GFX_INPUT").as_deref() == Ok("1") {
        spawn_serial_thread(tx.clone(), 4748);
        eprintln!("[desktop] serial ai-input armed");
    }

    let mut mx = (fb.width / 2) as i32;
    let mut my = (fb.height / 2) as i32;
    let dragging: Option<DragState> = None;
    let mut last_repaint = Instant::now() - Duration::from_secs(1);
    let mut cursor_dirty = true;
    let mut ui = ui;

    // Initial full paint.
    {
        let mut guard = wm.lock().unwrap_or_else(|p| p.into_inner());
        draw_desktop(&mut fb, &ui, &mut guard, Some((mx, my)));
        fb.flush()?;
    }

    loop {
        let motion_due = cursor_dirty && last_repaint.elapsed() >= Duration::from_millis(70);

        match rx.recv_timeout(Duration::from_millis(60)) {
            Ok(UiEvent::Motion(dx, dy)) => {
                if let Some(drag) = dragging.as_ref() {
                    let action = WindowAction::Move {
                        id: drag.window,
                        x: mx + dx - drag.grab_dx,
                        y: my + dy - drag.grab_dy,
                    };
                    let _ = wm.lock().unwrap_or_else(|p| p.into_inner()).apply(&action);
                    cursor_dirty = true;
                } else {
                    mx = (mx + dx).clamp(0, fb.width as i32 - 2);
                    my = (my + dy).clamp(0, fb.height as i32 - 2);
                    cursor_dirty = true;
                }
            }
            Ok(UiEvent::Press) | Ok(UiEvent::Release) => {
                cursor_dirty = true;
            }
            Ok(UiEvent::Key(code)) => {
                if let Some(ch) = handle_key(
                    code,
                    &mut wm.lock().unwrap_or_else(|p| p.into_inner()),
                    &tx,
                    &mut ui,
                ) {
                    if let Some(id) = wm.lock().unwrap_or_else(|p| p.into_inner()).focused_id() {
                        if let Ok(mut map) = clients.lock() {
                            if let Some(mut s) = map.remove(&id) {
                                use std::io::Write;
                                let _ = s.write_all(
                                    serde_json::json!({"event":"key","key":ch.to_string()})
                                        .to_string()
                                        .as_bytes(),
                                );
                                let _ = s.write_all(b"\n");
                                let _ = s.flush();
                                map.insert(id, s);
                            }
                        }
                    }
                }
                cursor_dirty = true;
            }
            Ok(UiEvent::WinClose(id)) => {
                let rect = wm
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .get(id)
                    .map(|w| w.content_rect());
                let _ = wm
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .apply(&WindowAction::Close(id));
                if let Some((cx, cy, cw, ch)) = rect {
                    fb.rect(i64::from(cx), i64::from(cy), cw, ch, BG);
                }
            }
            Ok(UiEvent::StatusTick) => {
                refresh_status(&mut ui);
                cursor_dirty = true;
            }
            Ok(UiEvent::ChatReply(entry)) => {
                ui.chat.push(entry);
                let keep = 8;
                while ui.chat.len() > keep {
                    ui.chat.remove(0);
                }
                cursor_dirty = true;
            }
            Err(_) => {}
        }

        // Surface commands from applications / AI window capabilities.
        while let Ok(surface_server::SurfaceCommand::Close(id)) = srx.try_recv() {
            let rect = wm
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get(id)
                .map(|w| w.content_rect());
            let _ = wm
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .apply(&WindowAction::Close(id));
            if let Some((cx, cy, cw, ch)) = rect {
                fb.rect(i64::from(cx), i64::from(cy), cw, ch, BG);
            }
            cursor_dirty = true;
        }

        if motion_due {
            let mut guard = wm.lock().unwrap_or_else(|p| p.into_inner());
            draw_desktop(&mut fb, &ui, &mut guard, Some((mx, my)));
            fb.flush()?;
            last_repaint = Instant::now();
            cursor_dirty = false;
        }
    }
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[desktop][FAIL] {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
