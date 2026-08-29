// Aether Window Manager - window model and management logic.
//
// Pure state machine: no I/O, no rendering. The desktop shell owns an
// instance of [`WindowManager`], feeds it structured actions (user input,
// AI capabilities) and reads back the resulting layout to paint chrome
// around each window's content region.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Monotonic window identifier.
pub type WindowId = u64;

/// Window visibility/state model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
}

impl std::fmt::Display for WindowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Normal => "normal",
            Self::Minimized => "minimized",
            Self::Maximized => "maximized",
            Self::Fullscreen => "fullscreen",
        };
        write!(f, "{s}")
    }
}

/// A managed window with full identity tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    pub id: WindowId,
    pub app_id: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub state: WindowState,
    pub focused: bool,
    pub visible: bool,
    /// Wayland surface identifier (UUID string).
    pub surface_id: Option<String>,
    /// Owning process ID.
    pub process_id: Option<u32>,
    /// Graphical session identifier.
    pub session_id: String,
    /// Workspace this window belongs to.
    pub workspace_id: u32,
    /// Saved geometry before maximize (x, y, w, h).
    pub restore_rect: Option<(i32, i32, u32, u32)>,
}

impl Window {
    /// Content area below the title bar chrome.
    pub fn content_rect(&self) -> (i32, i32, u32, u32) {
        const TITLE_H: u32 = 28;
        match self.state {
            WindowState::Minimized => (0, 0, 0, 0),
            _ => (
                self.x,
                self.y + TITLE_H as i32,
                self.width,
                self.height.saturating_sub(TITLE_H),
            ),
        }
    }
}

/// Screen geometry handed to the manager by the shell.
#[derive(Debug, Clone, Copy)]
pub struct ScreenArea {
    /// Usable region for windows (desktop minus taskbar/header).
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A workspace tracks its own ID and which windows belong to it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: u32,
    pub name: String,
    pub windows: Vec<WindowId>,
}

/// Structured window action — the only way anything moves a window.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowAction {
    Focus(WindowId),
    Move { id: WindowId, x: i32, y: i32 },
    Resize { id: WindowId, width: u32, height: u32 },
    Minimize(WindowId),
    Maximize(WindowId),
    Restore(WindowId),
    Close(WindowId),
}

/// Events emitted by the window manager for the event bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowEvent {
    WindowCreated { id: WindowId, app_id: String },
    WindowClosed { id: WindowId },
    WindowFocused { id: WindowId },
    WindowMoved { id: WindowId, x: i32, y: i32 },
    WindowResized { id: WindowId, w: u32, h: u32 },
    WorkspaceCreated { id: u32, name: String },
    WorkspaceChanged { id: u32 },
    WorkspaceDestroyed { id: u32 },
}

#[derive(Default)]
pub struct WindowManager {
    windows: BTreeMap<WindowId, Window>,
    stack: Vec<WindowId>, // bottom -> top
    focused: Option<WindowId>,
    next_id: WindowId,
    area: Option<ScreenArea>,
    active_workspace: u32,
    workspaces: BTreeMap<u32, Workspace>,
    events: Vec<WindowEvent>,
}

impl WindowManager {
    pub fn new(area: ScreenArea) -> Self {
        let mut workspaces = BTreeMap::new();
        workspaces.insert(
            0,
            Workspace {
                id: 0,
                name: "Desktop".to_string(),
                windows: Vec::new(),
            },
        );
        Self {
            area: Some(area),
            active_workspace: 0,
            workspaces,
            ..Default::default()
        }
    }

    /// CREATE: registers a window, auto-tiled into the next free slot and
    /// focused. Returns the created window.
    pub fn create(
        &mut self,
        app_id: &str,
        title: &str,
        preferred_width: u32,
        preferred_height: u32,
    ) -> Option<Window> {
        let area = self.area?;
        self.next_id += 1;
        let id = self.next_id;

        // Grid tiling: up to four windows share the screen quadrant-style;
        // further windows cascade from the last tile.
        let count = self.windows.len();
        let (col, row) = match count {
            0 => (0u32, 0u32),
            1 => (1u32, 0u32),
            2 => (0u32, 1u32),
            3 => (1u32, 1u32),
            n => ((n % 2) as u32, ((n / 2) % 2) as u32),
        };
        let tile_w = area.width / 2;
        let tile_h = area.height / 2;
        let w = preferred_width.min(tile_w.saturating_sub(16)).max(200);
        let h = preferred_height.min(tile_h.saturating_sub(16)).max(150);
        let x = area.x + (col * tile_w) as i32 + 8;
        let y = area.y + (row * tile_h) as i32 + 8;

        for w_ in self.windows.values_mut() {
            w_.focused = false;
        }
        let ws_id = self.active_workspace;
        let window = Window {
            id,
            app_id: app_id.to_string(),
            title: title.to_string(),
            x,
            y,
            width: w,
            height: h,
            state: WindowState::Normal,
            focused: true,
            visible: true,
            surface_id: None,
            process_id: None,
            session_id: String::new(),
            workspace_id: ws_id,
            restore_rect: None,
        };
        self.stack.push(id);
        self.windows.insert(id, window.clone());
        self.focused = Some(id);

        // Track in workspace.
        if let Some(ws) = self.workspaces.get_mut(&ws_id) {
            ws.windows.push(id);
        }

        self.events
            .push(WindowEvent::WindowCreated { id, app_id: app_id.to_string() });
        Some(window)
    }

    /// CREATE with full identity tracking.
    #[allow(clippy::too_many_arguments)]
    pub fn create_tracked(
        &mut self,
        app_id: &str,
        title: &str,
        preferred_width: u32,
        preferred_height: u32,
        surface_id: Option<String>,
        process_id: Option<u32>,
        session_id: &str,
    ) -> Option<Window> {
        let mut win = self.create(app_id, title, preferred_width, preferred_height)?;
        win.surface_id = surface_id;
        win.process_id = process_id;
        win.session_id = session_id.to_string();
        // Re-insert with the extra fields (create already inserted a copy without them).
        self.windows.insert(win.id, win.clone());
        Some(win)
    }

    /// Applies a structured action; returns the affected window if it exists.
    pub fn apply(&mut self, action: &WindowAction) -> Option<Window> {
        match *action {
            WindowAction::Focus(id) => self.focus(id),
            WindowAction::Move { id, x, y } => {
                let w = self.windows.get_mut(&id)?;
                w.x = x;
                w.y = y;
                self.events.push(WindowEvent::WindowMoved { id, x, y });
                self.windows.get(&id).cloned()
            }
            WindowAction::Resize { id, width, height } => {
                let w = self.windows.get_mut(&id)?;
                w.width = width.max(120);
                w.height = height.max(80);
                self.events.push(WindowEvent::WindowResized {
                    id,
                    w: w.width,
                    h: w.height,
                });
                self.windows.get(&id).cloned()
            }
            WindowAction::Minimize(id) => {
                let w = self.windows.get_mut(&id)?;
                w.state = WindowState::Minimized;
                w.visible = false;
                w.focused = false;
                if self.focused == Some(id) {
                    self.focused = None;
                    // Focus falls through to the top-most visible window.
                    for wid in self.stack.iter().rev() {
                        if let Some(cand) = self.windows.get_mut(wid) {
                            if cand.visible && cand.state != WindowState::Minimized {
                                cand.focused = true;
                                self.focused = Some(*wid);
                                break;
                            }
                        }
                    }
                }
                self.windows.get(&id).cloned()
            }
            WindowAction::Maximize(id) => {
                let area = self.area?;
                {
                    let w = self.windows.get_mut(&id)?;
                    // Save restore geometry only if not already maximized.
                    if w.state != WindowState::Maximized {
                        w.restore_rect = Some((w.x, w.y, w.width, w.height));
                    }
                    w.state = WindowState::Maximized;
                    w.visible = true;
                    w.x = area.x;
                    w.y = area.y;
                    w.width = area.width;
                    w.height = area.height;
                }
                self.focus(id);
                self.windows.get(&id).cloned()
            }
            WindowAction::Restore(id) => {
                let w = self.windows.get_mut(&id)?;
                if w.state == WindowState::Minimized {
                    w.state = WindowState::Normal;
                    w.visible = true;
                } else if w.state == WindowState::Maximized {
                    // Restore previous geometry if available.
                    if let Some((rx, ry, rw, rh)) = w.restore_rect.take() {
                        w.x = rx;
                        w.y = ry;
                        w.width = rw;
                        w.height = rh;
                    }
                    w.state = WindowState::Normal;
                }
                self.focus(id);
                self.windows.get(&id).cloned()
            }
            WindowAction::Close(id) => self.close(id),
        }
    }

    /// DESTROY/CLOSE: removes a window entirely.
    pub fn close(&mut self, id: WindowId) -> Option<Window> {
        let removed = self.windows.remove(&id)?;
        self.stack.retain(|w| *w != id);

        // Remove from workspace.
        if let Some(ws) = self.workspaces.get_mut(&removed.workspace_id) {
            ws.windows.retain(|w| *w != id);
        }

        if self.focused == Some(id) {
            self.focused = None;
            for wid in self.stack.iter().rev() {
                if let Some(cand) = self.windows.get_mut(wid) {
                    if cand.visible && cand.state != WindowState::Minimized {
                        cand.focused = true;
                        self.focused = Some(*wid);
                        break;
                    }
                }
            }
        }
        self.events.push(WindowEvent::WindowClosed { id });
        Some(removed)
    }

    fn focus(&mut self, id: WindowId) -> Option<Window> {
        let w = self.windows.get_mut(&id)?;
        w.focused = true;
        w.visible = true;
        if w.state == WindowState::Minimized {
            w.state = WindowState::Normal;
        }
        if let Some(pos) = self.stack.iter().position(|x| *x == id) {
            self.stack.remove(pos);
        }
        self.stack.push(id);
        for other in self.windows.values_mut() {
            if other.id != id {
                other.focused = false;
            }
        }
        self.focused = Some(id);
        self.events.push(WindowEvent::WindowFocused { id });
        self.windows.get(&id).cloned()
    }

    /// Top-most window whose frame contains the point (hit testing).
    pub fn window_at(&self, px: i32, py: i32) -> Option<WindowId> {
        for id in self.stack.iter().rev() {
            if let Some(w) = self.windows.get(id) {
                if !w.visible || w.state == WindowState::Minimized {
                    continue;
                }
                let in_x = px >= w.x && px < w.x + w.width as i32;
                let in_y = py >= w.y && py < w.y + w.height as i32;
                if in_x && in_y {
                    return Some(*id);
                }
            }
        }
        None
    }

    /// Cycles focus through visible windows (keyboard TAB behaviour).
    pub fn cycle_focus(&mut self) -> Option<Window> {
        let visible: Vec<WindowId> = self
            .windows
            .values()
            .filter(|w| w.visible && w.state != WindowState::Minimized)
            .map(|w| w.id)
            .collect();
        if visible.is_empty() {
            return None;
        }
        let current = self.focused.unwrap_or(0);
        let pos = visible.iter().position(|i| *i > current).unwrap_or(0);
        self.focus(visible[pos])
    }

    pub fn get(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(&id)
    }

    /// Windows in bottom-to-top stacking order.
    pub fn stacked(&self) -> Vec<&Window> {
        self.stack.iter().filter_map(|id| self.windows.get(id)).collect()
    }

    pub fn focused_id(&self) -> Option<WindowId> {
        self.focused
    }

    pub fn count(&self) -> usize {
        self.windows.len()
    }

    pub fn active_workspace_id(&self) -> u32 {
        self.active_workspace
    }

    pub fn workspaces(&self) -> &BTreeMap<u32, Workspace> {
        &self.workspaces
    }

    /// Drains accumulated events (consumed by the event bus each frame).
    pub fn drain_events(&mut self) -> Vec<WindowEvent> {
        std::mem::take(&mut self.events)
    }

    // ---- workspace operations ------------------------------------------------

    /// Creates a new workspace. Returns the new workspace ID.
    pub fn create_workspace(&mut self, name: &str) -> Option<u32> {
        let next_id = self.workspaces.keys().max().map_or(1, |k| k + 1);
        let ws = Workspace {
            id: next_id,
            name: name.to_string(),
            windows: Vec::new(),
        };
        self.events.push(WindowEvent::WorkspaceCreated {
            id: next_id,
            name: name.to_string(),
        });
        self.workspaces.insert(next_id, ws);
        Some(next_id)
    }

    /// Destroys a workspace. Windows on it are moved to workspace 0.
    pub fn destroy_workspace(&mut self, id: u32) -> bool {
        if id == 0 {
            return false; // Cannot destroy root workspace.
        }
        if let Some(ws) = self.workspaces.remove(&id) {
            // Move windows to workspace 0.
            if let Some(default_ws) = self.workspaces.get_mut(&0) {
                for wid in ws.windows {
                    default_ws.windows.push(wid);
                    if let Some(w) = self.windows.get_mut(&wid) {
                        w.workspace_id = 0;
                    }
                }
            }
            if self.active_workspace == id {
                self.active_workspace = 0;
                self.events.push(WindowEvent::WorkspaceChanged { id: 0 });
            }
            self.events.push(WindowEvent::WorkspaceDestroyed { id });
            true
        } else {
            false
        }
    }

    /// Activates (switches to) a workspace. Hides windows on old, shows on new.
    pub fn activate_workspace(&mut self, id: u32) -> bool {
        if !self.workspaces.contains_key(&id) {
            return false;
        }
        let old = self.active_workspace;
        self.active_workspace = id;

        // Hide windows from old workspace.
        if let Some(old_ws) = self.workspaces.get(&old) {
            for wid in &old_ws.windows {
                if let Some(w) = self.windows.get_mut(wid) {
                    if w.state != WindowState::Minimized {
                        w.visible = false;
                    }
                }
            }
        }

        // Show windows on new workspace.
        if let Some(new_ws) = self.workspaces.get(&id) {
            let wids: Vec<WindowId> = new_ws.windows.clone();
            for wid in &wids {
                if let Some(w) = self.windows.get_mut(wid) {
                    w.visible = true;
                }
            }
            // Focus the topmost visible window on the new workspace.
            self.focused = None;
            for wid in self.stack.iter().rev() {
                if let Some(w) = self.windows.get_mut(wid) {
                    if w.visible && w.workspace_id == id && w.state != WindowState::Minimized {
                        w.focused = true;
                        self.focused = Some(*wid);
                        break;
                    }
                }
            }
        }

        self.events.push(WindowEvent::WorkspaceChanged { id });
        true
    }

    /// Returns windows belonging to a specific workspace.
    pub fn windows_in_workspace(&self, ws_id: u32) -> Vec<&Window> {
        self.windows
            .values()
            .filter(|w| w.workspace_id == ws_id)
            .collect()
    }

    /// Assigns a window to a different workspace.
    pub fn move_window_to_workspace(&mut self, wid: WindowId, target_ws: u32) -> bool {
        if !self.workspaces.contains_key(&target_ws) {
            return false;
        }
        let old_ws_id;
        {
            let w = match self.windows.get_mut(&wid) {
                Some(w) => w,
                None => return false,
            };
            old_ws_id = w.workspace_id;
            w.workspace_id = target_ws;
        }
        // Remove from old workspace.
        if let Some(ws) = self.workspaces.get_mut(&old_ws_id) {
            ws.windows.retain(|w| *w != wid);
        }
        // Add to new workspace.
        if let Some(ws) = self.workspaces.get_mut(&target_ws) {
            ws.windows.push(wid);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wm() -> WindowManager {
        WindowManager::new(ScreenArea { x: 0, y: 96, width: 1024, height: 616 })
    }

    #[test]
    fn create_focuses_new_window_on_top() {
        let mut m = wm();
        let a = m.create("app-a", "A", 400, 300).unwrap_or_else(|| panic!("create"));
        let b = m.create("app-b", "B", 400, 300).unwrap_or_else(|| panic!("create"));
        // NOTE: returned Windows are snapshots; re-read live state.
        assert!(!m.get(a.id).unwrap_or_else(|| panic!("gone")).focused);
        assert!(m.get(b.id).unwrap_or_else(|| panic!("gone")).focused);
        assert_eq!(m.stacked().last().map(|w| w.id), Some(b.id));
        assert_eq!(m.window_at(b.x + 5, b.y + 40), Some(b.id));
    }

    #[test]
    fn minimize_moves_focus_to_next_visible() {
        let mut m = wm();
        let a = m.create("a", "A", 400, 300).unwrap_or_else(|| panic!("x"));
        let b = m.create("b", "B", 400, 300).unwrap_or_else(|| panic!("x"));
        m.apply(&WindowAction::Minimize(b.id));
        assert!(!m.get(b.id).unwrap_or_else(|| panic!("gone")).visible);
        assert_eq!(m.focused_id(), Some(a.id));
    }

    #[test]
    fn maximize_fills_area_and_restore_keeps_geometry() {
        let mut m = wm();
        let a = m.create("a", "A", 300, 200).unwrap_or_else(|| panic!("x"));
        let before = (a.x, a.y, a.width, a.height);
        m.apply(&WindowAction::Maximize(a.id));
        let maxed = m.get(a.id).unwrap_or_else(|| panic!("gone"));
        assert_eq!(maxed.state, WindowState::Maximized);
        assert_eq!((maxed.x, maxed.y, maxed.width, maxed.height), (0, 96, 1024, 616));
        m.apply(&WindowAction::Restore(a.id));
        let restored = m.get(a.id).unwrap_or_else(|| panic!("gone"));
        assert!(restored.visible && restored.focused);
        // Restore from maximize should recover the original geometry.
        assert_eq!((restored.x, restored.y, restored.width, restored.height), before);
    }

    #[test]
    fn close_removes_and_refocuses() {
        let mut m = wm();
        let _a = m.create("a", "A", 300, 200).unwrap_or_else(|| panic!("x"));
        let b = m.create("b", "B", 300, 200).unwrap_or_else(|| panic!("x"));
        m.apply(&WindowAction::Close(b.id));
        assert!(m.get(b.id).is_none());
        assert_eq!(m.count(), 1);
    }

    #[test]
    fn hit_test_respects_stack_order() {
        let mut m = wm();
        let a = m.create("a", "A", 400, 300).unwrap_or_else(|| panic!("x"));
        let b = m.create("b", "B", 400, 300).unwrap_or_else(|| panic!("x"));
        // b overlaps a's top-left cascade corner; b is on top.
        assert_eq!(m.window_at(b.x + 2, b.y + 2), Some(b.id));
        // a's left edge is outside b's cascade offset.
        assert_eq!(m.window_at(a.x + 2, a.y + 2), Some(a.id));
    }

    #[test]
    fn cycle_focus_walks_windows() {
        let mut m = wm();
        let a = m.create("a", "A", 300, 200).unwrap_or_else(|| panic!("x"));
        let b = m.create("b", "B", 300, 200).unwrap_or_else(|| panic!("x"));
        assert_eq!(m.focused_id(), Some(b.id));
        m.cycle_focus();
        assert_eq!(m.focused_id(), Some(a.id));
    }

    // ---- new tests for Phase 1.9 Part 2 ----

    // ---- new tests for Phase 1.9 Part 2 ----

    #[test]
    fn create_tracked_sets_identity_fields() {
        let mut m = wm();
        let win = match m.create_tracked(
            "calculator",
            "Calculator",
            400,
            300,
            Some("surface-abc-123".to_string()),
            Some(1234),
            "session-main",
        ) {
            Some(w) => w,
            None => panic!("create_tracked returned None"),
        };
        assert_eq!(win.surface_id.as_deref(), Some("surface-abc-123"));
        assert_eq!(win.process_id, Some(1234));
        assert_eq!(win.session_id, "session-main");
        assert_eq!(win.workspace_id, 0);
    }

    #[test]
    fn create_assigns_active_workspace() {
        let mut m = wm();
        let ws = m.create_workspace("Work");
        assert_eq!(ws, Some(1));
        assert!(m.activate_workspace(1));
        let win = match m.create("app", "App", 300, 200) {
            Some(w) => w,
            None => panic!("create returned None"),
        };
        assert_eq!(win.workspace_id, 1);
    }

    #[test]
    fn workspace_create_and_activate() {
        let mut m = wm();
        let id = match m.create_workspace("Dev") {
            Some(v) => v,
            None => panic!("create_workspace returned None"),
        };
        assert_eq!(id, 1);
        assert!(m.activate_workspace(id));
        assert_eq!(m.active_workspace_id(), 1);
    }

    #[test]
    fn workspace_cannot_destroy_root() {
        let mut m = wm();
        assert!(!m.destroy_workspace(0));
    }

    #[test]
    fn workspace_destroy_moves_windows_to_zero() {
        let mut m = wm();
        let ws_id = match m.create_workspace("Temp") {
            Some(v) => v,
            None => panic!("create_workspace returned None"),
        };
        m.activate_workspace(ws_id);
        let win = match m.create("a", "A", 300, 200) {
            Some(w) => w,
            None => panic!("create returned None"),
        };
        assert_eq!(win.workspace_id, ws_id);
        m.activate_workspace(0);
        assert!(m.destroy_workspace(ws_id));
        let w = match m.get(win.id) {
            Some(v) => v,
            None => panic!("get returned None"),
        };
        assert_eq!(w.workspace_id, 0);
    }

    #[test]
    fn move_window_to_workspace() {
        let mut m = wm();
        let ws2 = match m.create_workspace("Second") {
            Some(v) => v,
            None => panic!("create_workspace returned None"),
        };
        let win = match m.create("a", "A", 300, 200) {
            Some(w) => w,
            None => panic!("create returned None"),
        };
        assert_eq!(win.workspace_id, 0);
        assert!(m.move_window_to_workspace(win.id, ws2));
        let w = match m.get(win.id) {
            Some(v) => v,
            None => panic!("get returned None"),
        };
        assert_eq!(w.workspace_id, ws2);
    }

    #[test]
    fn move_window_to_nonexistent_workspace_fails() {
        let mut m = wm();
        let win = match m.create("a", "A", 300, 200) {
            Some(w) => w,
            None => panic!("create returned None"),
        };
        assert!(!m.move_window_to_workspace(win.id, 99));
    }

    #[test]
    fn windows_in_workspace_filters() {
        let mut m = wm();
        let ws2 = match m.create_workspace("Two") {
            Some(v) => v,
            None => panic!("create_workspace returned None"),
        };
        let a = match m.create("a", "A", 300, 200) {
            Some(w) => w,
            None => panic!("create returned None"),
        };
        match m.create("b", "B", 300, 200) {
            Some(_) => {}
            None => panic!("create returned None"),
        }
        m.move_window_to_workspace(a.id, ws2);
        assert_eq!(m.windows_in_workspace(0).len(), 1);
        assert_eq!(m.windows_in_workspace(ws2).len(), 1);
    }

    #[test]
    fn events_are_drained() {
        let mut m = wm();
        let _ = m.create("a", "A", 300, 200);
        let _ = m.create("b", "B", 300, 200);
        let _ = m.create_workspace("X");
        let events = m.drain_events();
        assert!(events.len() >= 3); // created, created, workspace_created
        assert!(m.drain_events().is_empty());
    }

    #[test]
    fn maximize_saves_restore_rect() {
        let mut m = wm();
        let a = match m.create("a", "A", 300, 200) {
            Some(w) => w,
            None => panic!("create returned None"),
        };
        let before = (a.x, a.y, a.width, a.height);
        m.apply(&WindowAction::Maximize(a.id));
        let maxed = match m.get(a.id) {
            Some(v) => v,
            None => panic!("get returned None"),
        };
        assert_eq!(maxed.restore_rect, Some(before));
    }

    #[test]
    fn double_maximize_does_not_corrupt_restore_rect() {
        let mut m = wm();
        let a = match m.create("a", "A", 300, 200) {
            Some(w) => w,
            None => panic!("create returned None"),
        };
        let before = (a.x, a.y, a.width, a.height);
        m.apply(&WindowAction::Maximize(a.id));
        m.apply(&WindowAction::Maximize(a.id));
        m.apply(&WindowAction::Restore(a.id));
        let restored = match m.get(a.id) {
            Some(v) => v,
            None => panic!("get returned None"),
        };
        assert_eq!((restored.x, restored.y, restored.width, restored.height), before);
    }
}
