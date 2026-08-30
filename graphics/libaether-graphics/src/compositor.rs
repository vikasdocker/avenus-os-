// Aether Graphics - Compositor module for the graphics stack
//
// Maps Wayland surfaces to managed windows and forwards lifecycle events.

use std::collections::HashMap;
use uuid::Uuid;

/// Metadata about a window tracked by the compositor.
#[derive(Debug, Clone)]
pub struct CompositorWindow {
    pub surface_id: Uuid,
    pub app_id: String,
    pub process_id: Option<u32>,
    pub session_id: String,
    pub workspace_id: u32,
}

/// Events emitted by the compositor for the event bus.
#[derive(Debug, Clone)]
pub enum CompositorEvent {
    SurfaceCreated { surface_id: Uuid, app_id: String },
    SurfaceDestroyed { surface_id: Uuid },
    SurfaceResized { surface_id: Uuid, width: u32, height: u32 },
    FrameSubmitted { surface_id: Uuid, frame: u64 },
}

/// The compositor manages surface-to-window mapping, layer ordering, and
/// frame rendering coordination.
pub struct Compositor {
    /// Active windows keyed by surface UUID.
    windows: HashMap<Uuid, CompositorWindow>,
    /// Maps app_id to surface_id for fast lookup.
    by_app: HashMap<String, Uuid>,
    /// Maps process_id to surface_id.
    by_pid: HashMap<u32, Uuid>,
    /// Frame counters per surface.
    frame_counter: u64,
    /// Accumulated events for the event bus.
    events: Vec<CompositorEvent>,
}

impl Compositor {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            by_app: HashMap::new(),
            by_pid: HashMap::new(),
            frame_counter: 0,
            events: Vec::new(),
        }
    }

    /// Registers a new surface with full identity tracking.
    pub fn register_surface(
        &mut self,
        surface_id: Uuid,
        app_id: &str,
        process_id: Option<u32>,
        session_id: &str,
        workspace_id: u32,
    ) {
        let win = CompositorWindow {
            surface_id,
            app_id: app_id.to_string(),
            process_id,
            session_id: session_id.to_string(),
            workspace_id,
        };
        self.by_app.insert(app_id.to_string(), surface_id);
        if let Some(pid) = process_id {
            self.by_pid.insert(pid, surface_id);
        }
        self.events
            .push(CompositorEvent::SurfaceCreated { surface_id, app_id: app_id.to_string() });
        self.windows.insert(surface_id, win);
    }

    /// Removes a surface.
    pub fn remove_surface(&mut self, surface_id: Uuid) -> Option<CompositorWindow> {
        if let Some(win) = self.windows.remove(&surface_id) {
            self.by_app.remove(&win.app_id);
            if let Some(pid) = win.process_id {
                self.by_pid.remove(&pid);
            }
            self.events.push(CompositorEvent::SurfaceDestroyed { surface_id });
            Some(win)
        } else {
            None
        }
    }

    /// Returns the metadata for a surface.
    pub fn get_window(&self, surface_id: Uuid) -> Option<&CompositorWindow> {
        self.windows.get(&surface_id)
    }

    /// Finds a surface by app_id.
    pub fn surface_by_app(&self, app_id: &str) -> Option<Uuid> {
        self.by_app.get(app_id).copied()
    }

    /// Finds a surface by process ID.
    pub fn surface_by_pid(&self, pid: u32) -> Option<Uuid> {
        self.by_pid.get(&pid).copied()
    }

    /// Returns the current frame counter (global).
    pub fn get_frame(&self) -> u64 {
        self.frame_counter
    }

    /// Records a frame submission.
    pub fn submit_frame(&mut self, surface_id: Uuid) {
        self.frame_counter += 1;
        self.events.push(CompositorEvent::FrameSubmitted { surface_id, frame: self.frame_counter });
    }

    /// Returns the number of active surfaces.
    pub fn surface_count(&self) -> usize {
        self.windows.len()
    }

    /// Returns all active surfaces as a list.
    pub fn surfaces(&self) -> Vec<&CompositorWindow> {
        self.windows.values().collect()
    }

    /// Drains accumulated events.
    pub fn drain_events(&mut self) -> Vec<CompositorEvent> {
        std::mem::take(&mut self.events)
    }
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_lookup_surface() {
        let mut c = Compositor::new();
        let sid = Uuid::new_v4();
        c.register_surface(sid, "calculator", Some(100), "s1", 0);
        assert_eq!(c.surface_count(), 1);
        let win = c.get_window(sid).unwrap();
        assert_eq!(win.app_id, "calculator");
        assert_eq!(win.process_id, Some(100));
    }

    #[test]
    fn lookup_by_app() {
        let mut c = Compositor::new();
        let sid = Uuid::new_v4();
        c.register_surface(sid, "notes", None, "s1", 0);
        assert_eq!(c.surface_by_app("notes"), Some(sid));
        assert_eq!(c.surface_by_app("missing"), None);
    }

    #[test]
    fn remove_surface_cleans_up() {
        let mut c = Compositor::new();
        let sid = Uuid::new_v4();
        c.register_surface(sid, "app", Some(42), "s1", 0);
        assert!(c.remove_surface(sid).is_some());
        assert_eq!(c.surface_count(), 0);
        assert!(c.surface_by_pid(42).is_none());
    }

    #[test]
    fn frame_submission_increments() {
        let mut c = Compositor::new();
        let sid = Uuid::new_v4();
        c.register_surface(sid, "app", None, "s1", 0);
        assert_eq!(c.get_frame(), 0);
        c.submit_frame(sid);
        assert_eq!(c.get_frame(), 1);
    }

    #[test]
    fn events_are_drained() {
        let mut c = Compositor::new();
        let sid = Uuid::new_v4();
        c.register_surface(sid, "app", None, "s1", 0);
        c.submit_frame(sid);
        let events = c.drain_events();
        assert!(events.len() >= 2);
        assert!(c.drain_events().is_empty());
    }
}
