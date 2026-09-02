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

use aether_a11y::KeyboardNav;
use aether_animation::Animation;
use aether_design_tokens::{AiVisualState, Color, Role};
use aether_renderer::{centered_x, draw_text, ComponentRenderer, PixelBuffer};
use aether_ui_components::system_monitor::{StatRow, SystemMonitor, SystemSnapshot};
use aether_wm::{ScreenArea, WindowAction, WindowManager};
use fb::{Rgb, Screen};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

// Design-token-backed colors (the §12 Aether identity).
// Dark Crystal theme: deep navy canvas.
const BG: Rgb = Rgb(12, 14, 24); // DARK_CRYSTAL_CANVAS

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
    color: Color,
    text: String,
}

// Color variants for ChatEntry — dark crystal palette.
const CHAT_CYAN: Color = Color::rgb(140, 180, 255); // GLOW_BLUE
const CHAT_FG: Color = Color::rgb(235, 232, 245); // DARK_CRYSTAL_TEXT
const CHAT_GREEN: Color = Color::rgb(140, 230, 190); // GLOW_MINT
const CHAT_RED: Color = Color::rgb(220, 80, 80); // DARK_CRYSTAL_DANGER
const CHAT_YELLOW: Color = Color::rgb(240, 180, 60); // DARK_CRYSTAL_WARNING
const CHAT_DIM: Color = Color::rgb(140, 136, 160); // DARK_CRYSTAL_MUTED

// --------------------------------------------------------------- app info

#[derive(Debug, Clone)]
struct AppInfo {
    id: String,
    name: String,
}

/// Toast notification severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

/// A frosted glass toast notification.
#[derive(Debug, Clone)]
struct Toast {
    message: String,
    kind: ToastKind,
    /// Time when the toast was created (Instant::now() as u64 millis).
    created_ms: u64,
}

impl ToastKind {
    fn color(self) -> Color {
        match self {
            Self::Info => Color::role(Role::DcAccent),
            Self::Success => Color::role(Role::DcSuccess),
            Self::Warning => Color::role(Role::DcWarning),
            Self::Error => Color::role(Role::DcDanger),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Success => "OK",
            Self::Warning => "WARN",
            Self::Error => "ERR",
        }
    }
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
            let name = a["name"].as_str().unwrap_or(&id).to_string();
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
    /// Animation queue for smooth transitions.
    animations: Vec<(String, Animation, f32)>,
    /// Keyboard navigation state (Tab/Shift+Tab focus cycling).
    keyboard_nav: KeyboardNav,
    /// System monitor panel.
    monitor: SystemMonitor,
    /// Current AI visual state for the orb.
    ai_state: AiVisualState,
    /// Orb pulse phase (0.0..=2*PI), advances each frame.
    orb_phase: f32,
    /// Hovered launcher item index (usize::MAX = none).
    hovered_launcher: usize,
    /// Active toast notifications (bottom-right overlay).
    toasts: Vec<Toast>,
    /// Monotonic frame counter for toast timing.
    frame_ms: u64,
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
            animations: Vec::new(),
            keyboard_nav: KeyboardNav::new(),
            monitor: SystemMonitor::new(),
            ai_state: AiVisualState::Idle,
            orb_phase: 0.0,
            hovered_launcher: usize::MAX,
            toasts: Vec::new(),
            frame_ms: 0,
        }
    }

    /// Start a named animation.
    fn start_animation(&mut self, name: &str, anim: Animation) {
        self.animations.retain(|(n, _, _)| n != name);
        self.animations.push((name.to_string(), anim, 0.0));
    }

    /// Advance all running animations by `delta_ms`.
    fn tick_animations(&mut self, delta_ms: u32) {
        for (_, anim, elapsed) in &mut self.animations {
            *elapsed = (*elapsed + delta_ms as f32).min(anim.duration.as_ms() as f32);
        }
        self.animations.retain(|(_, anim, elapsed)| *elapsed < anim.duration.as_ms() as f32);
        // Advance orb pulse phase (sine wave, ~2s period).
        self.orb_phase += delta_ms as f32 * 0.003;
        if self.orb_phase > std::f32::consts::TAU {
            self.orb_phase -= std::f32::consts::TAU;
        }
        // Advance frame counter and expire old toasts (5s lifetime).
        self.frame_ms += delta_ms as u64;
        self.toasts.retain(|t| self.frame_ms.saturating_sub(t.created_ms) < 5000);
    }

    /// Push a toast notification.
    fn push_toast(&mut self, message: &str, kind: ToastKind) {
        self.toasts.push(Toast { message: message.to_string(), kind, created_ms: self.frame_ms });
        // Keep at most 5 toasts.
        while self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
    }

    /// Get the current progress (0.0..=1.0) of a named
    /// animation, or 1.0 if not running.
    fn animation_progress(&self, name: &str) -> f32 {
        self.animations
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, anim, elapsed)| anim.progress(*elapsed as u16))
            .unwrap_or(1.0)
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

fn draw_desktop(fb: &mut Screen, ui: &UiState, wm: &mut WindowManager, cursor: Option<(i32, i32)>) {
    // Wrap the screen buffer in a PixelBuffer for the renderer.
    let buf_slice = &mut fb.buf[..];
    let mut pbuf = PixelBuffer::from_raw(fb.width, fb.height, fb.stride, buf_slice);
    let mut renderer = ComponentRenderer::new(&mut pbuf);

    renderer.buf.fill(Color::role(Role::BgBase));

    // Ambient prismatic glow — top-left corner.
    renderer.buf.gradient_glow(0, 0, 300, 200, Color::role(Role::CrystalPrism), 0.06);
    // Ambient glow — bottom-right corner.
    renderer.buf.gradient_glow(
        fb.width as i64 - 300,
        fb.height as i64 - 200,
        300,
        200,
        Color::role(Role::CrystalRefract),
        0.04,
    );

    // Header bar — glass panel with crystal accent.
    renderer.buf.glass_panel(0, 0, fb.width, HEADER_H as u32, 0, 0.6);
    renderer.buf.glow_line(0, HEADER_H - 1, fb.width, Color::role(Role::CrystalEdge), 0.4);

    // AETHER OS label.
    draw_text(renderer.buf, "AETHER OS", 20, 10, 2, Color::role(Role::TextPrimary));

    // AI Orb — prismatic centerpiece in the header.
    let orb_x = fb.width as i64 / 2 - 80;
    let orb_y = HEADER_H / 2;
    let orb_color = ui.ai_state.color();
    // Pulse animation: sine wave, 0.0..=1.0.
    let orb_pulse = ui.orb_phase.sin() * 0.5 + 0.5;
    renderer.buf.ai_orb(orb_x, orb_y, 10, orb_color, orb_pulse, 0.9);

    // Clock (center of header).
    let clock_label = &ui.clock;
    let clock_x = centered_x(0, fb.width, clock_label, 2);
    draw_text(renderer.buf, clock_label, clock_x, 10, 2, Color::role(Role::AccentLavender));

    // Workspace indicators.
    let ws_start_x = fb.width as i64 / 2 + 80;
    let mut wpx = ws_start_x;
    for ws_id in &ui.workspaces {
        let label = format!("[{}]", ws_id);
        let color = if *ws_id == ui.active_workspace {
            Color::role(Role::DcAccent)
        } else {
            Color::role(Role::DcDisabled)
        };
        draw_text(renderer.buf, &label, wpx, 12, 1, color);
        wpx += (label.len() as i64 + 1) * 6;
    }

    // Status pills right-aligned in header.
    let mut px = fb.width as i64 - 16;
    for (name, up) in ui.status.iter().rev() {
        let label = format!("{name}:{}", if *up { "UP" } else { "DOWN" });
        let w = label.len() as u32 * 6 * 2 + 12;
        px -= w as i64;
        let color = if *up { Color::role(Role::DcSuccess) } else { Color::role(Role::DcDanger) };
        draw_text(renderer.buf, &label, px + 6, 12, 2, color);
        px -= 16;
    }

    // Network / Storage pills.
    let net_label = format!("NET:{}", if ui.network_up { "UP" } else { "DOWN" });
    let stor_label = format!("STOR:{}", if ui.storage_up { "UP" } else { "DOWN" });
    let net_w = net_label.len() as i64 * 12 + 12;
    let stor_w = stor_label.len() as i64 * 12 + 12;
    draw_text(
        renderer.buf,
        &net_label,
        fb.width as i64 - 16 - net_w,
        HEADER_H - 16,
        1,
        if ui.network_up { Color::role(Role::DcSuccess) } else { Color::role(Role::DcDanger) },
    );
    draw_text(
        renderer.buf,
        &stor_label,
        fb.width as i64 - 16 - net_w - stor_w - 8,
        HEADER_H - 16,
        1,
        if ui.storage_up { Color::role(Role::DcSuccess) } else { Color::role(Role::DcDanger) },
    );

    // Window chrome — glass window with crystal frame.
    for w in wm
        .stacked()
        .into_iter()
        .filter(|w| w.visible && w.state != aether_wm::WindowState::Minimized)
    {
        let (wx, wy) = (i64::from(w.x), i64::from(w.y));
        let ww = i64::from(w.width);

        // Glass window body with layered shadow.
        renderer.buf.glass_window(
            wx, wy, w.width, w.height, 12, // Md radius
            w.focused, 0.7,
        );

        // Title bar text.
        draw_text(
            renderer.buf,
            &w.title.to_uppercase(),
            wx + 10,
            wy + 9,
            2,
            Color::role(Role::TextPrimary),
        );
        // Buttons right side: [_] [O] [X].
        let bx = wx + ww - BOX_W * 3 - 8;
        draw_text(renderer.buf, "_", bx, wy + 8, 2, Color::role(Role::TextSecondary));
        draw_text(renderer.buf, "O", bx + BOX_W, wy + 8, 2, Color::role(Role::TextSecondary));
        let close_color =
            if w.focused { Color::role(Role::DcDanger) } else { Color::role(Role::TextDisabled) };
        draw_text(renderer.buf, "X", bx + BOX_W * 2, wy + 8, 2, close_color);

        // Clean canvas for app content.
        let (cx, cy, cw, ch) = w.content_rect();
        renderer.buf.rect(i64::from(cx), i64::from(cy), cw, ch, Color::role(Role::BgBase));
    }

    // Application launcher (left side panel).
    if ui.launcher_open {
        draw_launcher(&mut renderer.buf, ui);
    }

    // System monitor panel (right side).
    if ui.monitor.visible {
        draw_system_monitor(&mut renderer.buf, ui);
    }

    // Taskbar — glass panel with crystal accent.
    let tb_y = fb.height as i64 - TASKBAR_H;
    renderer.buf.glass_panel(0, tb_y, fb.width, TASKBAR_H as u32, 0, 0.5);
    renderer.buf.glow_line(0, tb_y, fb.width, Color::role(Role::CrystalEdge), 0.3);

    // Workspace quick-switch buttons in taskbar.
    let mut ws_tx = 16i64;
    for ws_id in &ui.workspaces {
        let label = format!("WS{}", ws_id);
        let bw = 40u32;
        let bg = if *ws_id == ui.active_workspace {
            Color::role(Role::DcSurfaceHover)
        } else {
            Color::role(Role::DcSurface)
        };
        renderer.buf.rounded_rect(ws_tx, tb_y + 6, bw, TASKBAR_H as u32 - 12, 6, bg);
        let text_color = if *ws_id == ui.active_workspace {
            Color::role(Role::DcAccent)
        } else {
            Color::role(Role::DcDisabled)
        };
        draw_text(renderer.buf, &label, ws_tx + 4, tb_y + 15, 2, text_color);
        ws_tx += bw as i64 + 4;
    }

    // Window taskbar buttons.
    let mut tx = ws_tx + 8;
    for w in wm
        .stacked()
        .into_iter()
        .filter(|w| w.visible && w.state != aether_wm::WindowState::Minimized)
    {
        let label = w.title.to_uppercase();
        let is_focused = w.focused;
        let bw = 150u32;
        let bg = if is_focused {
            Color::role(Role::DcSurfaceHover)
        } else {
            Color::role(Role::DcSurface)
        };
        renderer.buf.rounded_rect(tx, tb_y + 6, bw, TASKBAR_H as u32 - 12, 6, bg);
        let label = if label.len() > 18 { format!("{}...", &label[..17]) } else { label };
        let text_color =
            if is_focused { Color::role(Role::DcAccent) } else { Color::role(Role::DcDisabled) };
        draw_text(renderer.buf, &label, tx + 8, tb_y + 15, 2, text_color);
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
    draw_text(
        renderer.buf,
        &active,
        fb.width as i64 - 16 - active.len() as i64 * 12,
        tb_y + 14,
        2,
        Color::role(Role::DcSuccess),
    );

    // AI conversation strip.
    let ai_y = tb_y - 24 * (ui.chat.len() as i64).min(3) - 8;
    for (i, entry) in ui.chat.iter().rev().take(3).enumerate() {
        let y = ai_y + (2 - i as i64) * 24;
        draw_text(renderer.buf, entry.prefix, 16, y, 2, entry.color);
        draw_text(renderer.buf, &entry.text.to_uppercase(), 70, y, 2, Color::role(Role::DcText));
    }

    // Frosted glass toast notifications (bottom-right).
    draw_toasts(&mut renderer.buf, ui);

    // Cursor sprite last — glowing crystal cursor.
    if let Some((mx, my)) = cursor {
        let (cx, cy) = (i64::from(mx), i64::from(my));
        // Cursor glow.
        renderer.buf.gradient_glow(cx - 4, cy - 4, 16, 16, Color::role(Role::CrystalPrism), 0.3);
        renderer.buf.rect(cx, cy, 3, 12, Color::role(Role::CrystalShine));
        renderer.buf.rect(cx, cy, 12, 3, Color::role(Role::CrystalShine));
        renderer.buf.rect(cx, cy, 9, 9, Color::role(Role::DcAccent));
    }
}

fn draw_launcher(buf: &mut PixelBuffer, ui: &UiState) {
    let lx: i64 = 4;
    let ly: i64 = HEADER_H + 4;
    let lw = LAUNCHER_W as u32;
    let apps = &ui.launcher_apps;
    let lh = (apps.len() as i64 * LAUNCHER_ITEM_H as i64 + 32)
        .min(buf.height as i64 - ly - TASKBAR_H - 8);

    // Launcher background — frosted glass panel.
    buf.glass_panel(lx, ly, lw, lh as u32, 12, 0.7);
    buf.glow_line(lx + 8, ly, lw.saturating_sub(16), Color::role(Role::CrystalEdge), 0.3);
    draw_text(buf, "APPLICATIONS", lx + 8, ly + 6, 2, Color::role(Role::DcAccent));

    if apps.is_empty() {
        draw_text(buf, "NO APPS", lx + 8, ly + 28, 1, Color::role(Role::DcDisabled));
        return;
    }

    for (i, app) in apps.iter().enumerate() {
        let iy = ly + 24 + i as i64 * LAUNCHER_ITEM_H;
        if iy + LAUNCHER_ITEM_H > ly + lh {
            break;
        }
        let selected = i == ui.selected_launcher;
        let hovered = i == ui.hovered_launcher;
        let item_w = lw - 4;
        let item_h = LAUNCHER_ITEM_H as u32 - 2;

        // Hover illumination: glow ring around the item.
        if hovered && !selected {
            buf.glass_panel_hover(lx + 2, iy, item_w, item_h, 6, 0.4, 0.6);
        } else {
            let bg = if selected {
                Color::role(Role::DcSurfaceHover)
            } else {
                Color::role(Role::DcSurface)
            };
            buf.rounded_rect(lx + 2, iy, item_w, item_h, 6, bg);
        }

        // Selected item gets a crystal accent line.
        if selected {
            buf.glow_line(lx + 2, iy, item_w, Color::role(Role::DcAccent), 0.5);
        }

        let label =
            if app.name.len() > 20 { format!("{}...", &app.name[..19]) } else { app.name.clone() };
        let text_color = if selected {
            Color::role(Role::DcAccent)
        } else if hovered {
            Color::role(Role::CrystalShine)
        } else {
            Color::role(Role::DcText)
        };
        draw_text(buf, &label.to_uppercase(), lx + 10, iy + 7, 2, text_color);
    }
}

fn draw_system_monitor(buf: &mut PixelBuffer, ui: &UiState) {
    let snap = &ui.monitor.snapshot;
    let mx = buf.width as i64 - ui.monitor.width as i64 - 4;
    let my = HEADER_H + 4;
    let mw = ui.monitor.width as u32;
    let mh = ui.monitor.height as u32;

    // Panel background — frosted glass.
    buf.crystal_panel(mx, my, mw, mh, 12, 0.7);

    // Header.
    buf.rounded_rect(mx + 2, my + 2, mw - 4, 24, 8, Color::role(Role::DcSurfaceStrong));
    draw_text(buf, "SYSTEM MONITOR", mx + 8, my + 7, 2, Color::role(Role::DcAccent));

    let mut y = my + 32;
    let row_h: i64 = 18;

    // CPU bar.
    draw_stat_bar(
        buf,
        mx + 8,
        y,
        mw - 16,
        "CPU",
        &format!("{:.0}%", snap.cpu_percent),
        snap.cpu_percent / 100.0,
    );
    y += row_h + 8;

    // Memory bar.
    draw_stat_bar(buf, mx + 8, y, mw - 16, "MEM", &snap.memory, snap.memory_fraction);
    y += row_h + 8;

    // Uptime + process count.
    draw_text(buf, &format!("UP: {}", snap.uptime), mx + 8, y, 1, Color::role(Role::DcMuted));
    y += row_h;
    draw_text(
        buf,
        &format!("PROC: {}", snap.process_count),
        mx + 8,
        y,
        1,
        Color::role(Role::DcMuted),
    );
    y += row_h + 4;

    // Disks section.
    if !snap.disks.is_empty() {
        draw_text(buf, "DISK", mx + 8, y, 2, Color::role(Role::DcAccent));
        y += row_h;
        for disk in &snap.disks {
            let frac = disk.fraction.unwrap_or(0.0);
            draw_stat_bar(buf, mx + 8, y, mw - 16, &disk.label, &disk.value, frac);
            y += row_h + 4;
        }
    }

    // Network section.
    if !snap.networks.is_empty() {
        draw_text(buf, "NET", mx + 8, y, 2, Color::role(Role::DcAccent));
        y += row_h;
        for net in &snap.networks {
            let color = if net.value == "UP" {
                Color::role(Role::DcSuccess)
            } else {
                Color::role(Role::DcDanger)
            };
            draw_text(buf, &net.label, mx + 8, y, 1, Color::role(Role::DcMuted));
            draw_text(buf, &net.value, mx + mw as i64 - 40, y, 1, color);
            y += row_h;
        }
    }
}

fn draw_stat_bar(
    buf: &mut PixelBuffer,
    x: i64,
    y: i64,
    max_w: u32,
    label: &str,
    value: &str,
    fraction: f32,
) {
    let bar_x = x + 40;
    let bar_w = (max_w as i64 - 48) as u32;
    let bar_h: u32 = 10;

    // Label.
    draw_text(buf, label, x, y, 1, Color::role(Role::DcMuted));

    // Value.
    draw_text(
        buf,
        value,
        x + max_w as i64 - value.len() as i64 * 6,
        y,
        1,
        Color::role(Role::DcText),
    );

    // Bar background.
    let by = y + 12;
    buf.rect(bar_x, by, bar_w, bar_h, Color::role(Role::DcSurfaceStrong));

    // Bar fill.
    let fill_w = (bar_w as f32 * fraction.clamp(0.0, 1.0)) as u32;
    if fill_w > 0 {
        let color = if fraction > 0.85 {
            Color::role(Role::DcDanger)
        } else if fraction > 0.6 {
            Color::role(Role::DcAccent)
        } else {
            Color::role(Role::DcSuccess)
        };
        buf.rect(bar_x, by, fill_w, bar_h, color);
    }

    // Bar outline.
    aether_renderer::draw_rect_outline(
        buf,
        bar_x,
        by,
        bar_w,
        bar_h,
        4,
        Color::role(Role::DcBorder),
    );
}

fn draw_toasts(buf: &mut PixelBuffer, ui: &UiState) {
    if ui.toasts.is_empty() {
        return;
    }
    let toast_w: u32 = 260;
    let toast_h: u32 = 36;
    let gap: i64 = 6;
    let margin: i64 = 12;
    let base_y = buf.height as i64 - margin;

    for (i, toast) in ui.toasts.iter().enumerate() {
        let age_ms = ui.frame_ms.saturating_sub(toast.created_ms);
        // Fade in over 200ms, fade out after 4000ms.
        let alpha = if age_ms < 200 {
            age_ms as f32 / 200.0
        } else if age_ms > 4000 {
            1.0 - (age_ms - 4000) as f32 / 1000.0
        } else {
            1.0
        };
        let alpha = alpha.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            continue;
        }

        let ty = base_y - (i as i64 + 1) * (toast_h as i64 + gap);
        let tx = buf.width as i64 - toast_w as i64 - margin;

        // Frosted glass background.
        buf.glass_panel(tx, ty, toast_w, toast_h, 10, 0.75 * alpha);

        // Kind accent line on the left.
        let kind_color = toast.kind.color();
        buf.rect(tx + 2, ty + 6, 3, toast_h - 12, kind_color);

        // Kind label.
        draw_text(buf, toast.kind.label(), tx + 10, ty + 6, 2, kind_color);

        // Message text.
        let msg = if toast.message.len() > 30 {
            format!("{}...", &toast.message[..29])
        } else {
            toast.message.clone()
        };
        draw_text(buf, &msg.to_uppercase(), tx + 10, ty + 20, 1, Color::role(Role::DcText));
    }
}

// ----------------------------------------------------------- monitor data

fn refresh_monitor(ui: &mut UiState) {
    use std::io::{BufRead, BufReader, Write};

    let call_json = |port: u16, v: Value| -> Option<Value> {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
        s.set_read_timeout(Some(Duration::from_secs(2))).ok();
        s.write_all(format!("{v}\n").as_bytes()).ok()?;
        let mut line = String::new();
        BufReader::new(s).read_line(&mut line).ok()?;
        serde_json::from_str(line.trim()).ok()
    };

    // Memory from system.info.
    let (mem_used, mem_total) = call_json(
        4747,
        json!({
            "service_id": "aether-system-core",
            "command": "info",
            "parameters": {},
        }),
    )
    .and_then(|v| {
        let used = v["result"]["memory_used_mib"].as_f64().unwrap_or(0.0);
        let total = v["result"]["memory_total_mib"].as_f64().unwrap_or(1.0);
        Some((used, total))
    })
    .unwrap_or((0.0, 1.0));

    let mem_frac = if mem_total > 0.0 { (mem_used / mem_total) as f32 } else { 0.0 };
    let mem_str = format!("{:.1} / {:.1} MiB", mem_used, mem_total);

    // Storage from storage.status.
    let disks = call_json(
        4747,
        json!({
            "service_id": "aether-system-core",
            "command": "status",
            "parameters": { "type": "storage" },
        }),
    )
    .and_then(|v| {
        let mounts = v["result"]["mounts"].as_array()?;
        let mut rows = Vec::new();
        for m in mounts {
            let name = m["mount_point"].as_str().unwrap_or("?");
            let used = m["used_bytes"].as_f64().unwrap_or(0.0);
            let total = m["total_bytes"].as_f64().unwrap_or(1.0);
            let frac = if total > 0.0 { (used / total) as f32 } else { 0.0 };
            let used_gb = used / 1_073_741_824.0;
            let total_gb = total / 1_073_741_824.0;
            rows.push(StatRow {
                label: name.to_string(),
                value: format!("{:.1} / {:.1} GB", used_gb, total_gb),
                fraction: Some(frac),
            });
        }
        Some(rows)
    })
    .unwrap_or_default();

    // Network.
    let networks = call_json(
        4747,
        json!({
            "service_id": "aether-system-core",
            "command": "status",
            "parameters": { "type": "network" },
        }),
    )
    .and_then(|v| {
        let ifaces = v["result"]["interfaces"].as_array()?;
        let mut rows = Vec::new();
        for iface in ifaces {
            let name = iface["name"].as_str().unwrap_or("?");
            let up = iface["up"].as_bool().unwrap_or(false);
            rows.push(StatRow {
                label: name.to_string(),
                value: if up { "UP" } else { "DOWN" }.to_string(),
                fraction: None,
            });
        }
        Some(rows)
    })
    .unwrap_or_default();

    // Process count.
    let proc_count = call_json(
        4747,
        json!({
            "service_id": "aether-system-core",
            "command": "status",
            "parameters": { "type": "process" },
        }),
    )
    .and_then(|v| v["result"]["processes"].as_array().map(|a| a.len() as u32))
    .unwrap_or(0);

    // Uptime from system.info.
    let uptime_ms = call_json(
        4747,
        json!({
            "service_id": "aether-system-core",
            "command": "info",
            "parameters": {},
        }),
    )
    .and_then(|v| v["result"]["uptime_ms"].as_f64())
    .unwrap_or(0.0);
    let secs = (uptime_ms / 1000.0) as u64;
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let uptime_str = if hours > 0 { format!("{}h {}m", hours, mins) } else { format!("{}m", mins) };

    // CPU (estimate from process count — real CPU requires /proc/stat).
    // In QEMU we don't have /proc/stat, so we use a heuristic.
    let cpu_est = (proc_count as f32 * 0.3).min(100.0);

    ui.monitor.update(SystemSnapshot {
        cpu_percent: cpu_est,
        memory: mem_str,
        memory_fraction: mem_frac,
        disks,
        networks,
        uptime: uptime_str,
        process_count: proc_count,
    });
}

// ------------------------------------------------------------ input decode

fn handle_key(
    code: u16,
    wm: &mut WindowManager,
    tx: &Sender<UiEvent>,
    ui: &mut UiState,
) -> Option<char> {
    const TAB: u16 = 15;
    const F2: u16 = 60;
    const F3: u16 = 61;
    const F4: u16 = 62;
    const F5: u16 = 63;
    const F6: u16 = 64;
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
            ui.start_animation("focus_ring", Animation::tap());
            None
        }
        F2 => {
            if let Some(id) = focused {
                wm.apply(&WindowAction::Minimize(id));
                ui.start_animation("window_state", Animation::window_state());
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
                ui.start_animation("window_state", Animation::window_state());
            }
            None
        }
        F4 => {
            if let Some(id) = focused {
                m_close(wm, id, tx);
                ui.start_animation("window_state", Animation::window_state());
            }
            None
        }
        F5 => {
            // Toggle launcher.
            ui.launcher_open = !ui.launcher_open;
            if ui.launcher_open {
                ui.launcher_apps = query_registered_apps();
                ui.selected_launcher = 0;
                ui.start_animation("launcher", Animation::nav());
            }
            None
        }
        F6 => {
            // Toggle system monitor.
            ui.monitor.visible = !ui.monitor.visible;
            ui.start_animation("monitor", Animation::tap());
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
            color: CHAT_RED,
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
            color: CHAT_RED,
            text: "LAUNCH FAILED - send error".to_string(),
        }));
        return;
    }
    let mut line = String::new();
    match BufReader::new(s).read_line(&mut line) {
        Ok(0) | Err(_) => {
            let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                prefix: "SYS>",
                color: CHAT_YELLOW,
                text: format!("LAUNCHING {}", app_id.to_uppercase()),
            }));
        }
        Ok(_) => {
            let v: Value = serde_json::from_str(line.trim()).unwrap_or(json!({}));
            let ok = v["ok"].as_bool().unwrap_or(false);
            if ok {
                let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                    prefix: "OK >",
                    color: CHAT_GREEN,
                    text: format!("LAUNCHED {}", app_id.to_uppercase()),
                }));
            } else {
                let err = v["error"].as_str().unwrap_or("unknown");
                let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                    prefix: "ERR>",
                    color: CHAT_RED,
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
    s.write_all(format!("{req}\n").as_bytes()).map_err(|e| format!("send: {e}"))?;
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).map_err(|e| format!("recv: {e}"))?;
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
                            if let Some(ev) = input::decode(&byte_buf, &mut kind) {
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
                                    color: CHAT_FG,
                                    text: prompt.clone(),
                                }));
                                let result = agent_chat(&prompt, port);
                                match result {
                                    Ok(reply) => {
                                        if let Some(actions) = reply.actions.clone() {
                                            let mut first_msg: Option<String> = None;
                                            for act in &actions {
                                                let cap =
                                                    act["capability"].as_str().unwrap_or("action");
                                                let status =
                                                    act["status"].as_str().unwrap_or("Success");
                                                let msg = act["message"].as_str().unwrap_or("");
                                                if first_msg.is_none() {
                                                    first_msg = Some(msg.to_string());
                                                }
                                                let (prefix, color) = match status {
                                                    "Success" => ("OK >", CHAT_GREEN),
                                                    "Failed" => ("ERR>", CHAT_RED),
                                                    "Rejected" => ("!! >", CHAT_RED),
                                                    "RequiresConsent" => (".. >", CHAT_DIM),
                                                    _ => (" * >", CHAT_CYAN),
                                                };
                                                let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                                                    prefix,
                                                    color,
                                                    text: format!(
                                                        "{}: {}",
                                                        cap.to_ascii_uppercase(),
                                                        msg
                                                    ),
                                                }));
                                            }
                                            let need_summary = match &first_msg {
                                                Some(f) => f != &reply.response,
                                                None => true,
                                            };
                                            if need_summary && !reply.response.is_empty() {
                                                let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                                                    prefix: "AI >",
                                                    color: CHAT_CYAN,
                                                    text: reply.response,
                                                }));
                                            }
                                        } else {
                                            let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                                                prefix: "AI >",
                                                color: CHAT_CYAN,
                                                text: reply.response,
                                            }));
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx.send(UiEvent::ChatReply(ChatEntry {
                                            prefix: "SYS>",
                                            color: CHAT_RED,
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

/// Headless mode: surface server runs but no framebuffer is available.
/// The shell stays alive so app windows can still register via IPC.
fn headless_mode() -> Result<(), String> {
    eprintln!("[desktop] headless mode: surface server active on :{SURFACE_PORT}");

    let area = ScreenArea { x: 0, y: 0, width: 1024, height: 768 };
    let wm = Arc::new(Mutex::new(WindowManager::new(area)));
    let clients: surface_server::Clients = Arc::new(Mutex::new(HashMap::new()));

    let (stx, srx): (
        Sender<surface_server::SurfaceCommand>,
        Receiver<surface_server::SurfaceCommand>,
    ) = channel();

    surface_server::spawn(
        SURFACE_PORT,
        surface_server::SurfaceServer {
            tx: stx,
            wm: Arc::clone(&wm),
            clients: Arc::clone(&clients),
        },
    )?;
    eprintln!("[desktop] surface server ready on :{SURFACE_PORT}");

    let (_tx, rx): (Sender<UiEvent>, Receiver<UiEvent>) = channel();
    spawn_status_thread(_tx.clone());

    loop {
        while let Ok(surface_server::SurfaceCommand::Close(id)) = srx.try_recv() {
            let _ = wm.lock().unwrap_or_else(|p| p.into_inner()).apply(&WindowAction::Close(id));
        }
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(UiEvent::StatusTick) => {}
            _ => {}
        }
    }
}

fn run() -> Result<(), String> {
    let mut fb = match Screen::open() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[desktop][WARN] no framebuffer available ({e}); running in headless mode");
            return headless_mode();
        }
    };
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
    let (stx, srx): (
        Sender<surface_server::SurfaceCommand>,
        Receiver<surface_server::SurfaceCommand>,
    ) = channel();

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
        // Advance animations every frame (~60ms tick).
        ui.tick_animations(60);

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
                let rect =
                    wm.lock().unwrap_or_else(|p| p.into_inner()).get(id).map(|w| w.content_rect());
                let _ =
                    wm.lock().unwrap_or_else(|p| p.into_inner()).apply(&WindowAction::Close(id));
                if let Some((cx, cy, cw, ch)) = rect {
                    fb.rect(i64::from(cx), i64::from(cy), cw, ch, BG);
                }
            }
            Ok(UiEvent::StatusTick) => {
                refresh_status(&mut ui);
                if ui.monitor.visible {
                    refresh_monitor(&mut ui);
                }
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
            let rect =
                wm.lock().unwrap_or_else(|p| p.into_inner()).get(id).map(|w| w.content_rect());
            let _ = wm.lock().unwrap_or_else(|p| p.into_inner()).apply(&WindowAction::Close(id));
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
