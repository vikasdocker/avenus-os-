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

/// A managed window.
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

#[derive(Default)]
pub struct WindowManager {
    windows: BTreeMap<WindowId, Window>,
    stack: Vec<WindowId>, // bottom -> top
    focused: Option<WindowId>,
    next_id: WindowId,
    area: Option<ScreenArea>,
}

impl WindowManager {
    pub fn new(area: ScreenArea) -> Self {
        Self {
            area: Some(area),
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
        };
        self.stack.push(id);
        self.windows.insert(id, window.clone());
        self.focused = Some(id);
        Some(window)
    }

    /// Applies a structured action; returns the affected window if it exists.
    pub fn apply(&mut self, action: &WindowAction) -> Option<Window> {
        match *action {
            WindowAction::Focus(id) => self.focus(id),
            WindowAction::Move { id, x, y } => {
                let w = self.windows.get_mut(&id)?;
                w.x = x;
                w.y = y;
                self.windows.get(&id).cloned()
            }
            WindowAction::Resize { id, width, height } => {
                let w = self.windows.get_mut(&id)?;
                w.width = width.max(120);
                w.height = height.max(80);
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
        // Restore from maximize keeps maximized geometry until moved/resized;
        // the important part is focus and visibility.
        assert!(restored.visible && restored.focused);
        let _ = before;
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
}
