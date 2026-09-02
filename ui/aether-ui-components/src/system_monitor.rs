//! System Monitor — a real-time panel showing CPU, memory, disk, and network.
//!
//! The monitor is a right-anchored panel that displays live
//! system statistics. It is purely declarative: the component
//! holds the latest data snapshot, and the renderer paints it.
//! The graphical shell polls the system and updates the
//! component each tick.

use aether_design_tokens::{Radius, Role, Spacing};

use crate::{Component, ComponentStyle, Insets, LayoutBox};

/// A single stat row (label + value + optional bar fraction).
#[derive(Debug, Clone, PartialEq)]
pub struct StatRow {
    /// Label (e.g. "CPU", "MEM", "/").
    pub label: String,
    /// Human-readable value (e.g. "42%", "1.2 GB").
    pub value: String,
    /// Optional bar fraction 0.0..=1.0 (for progress bars).
    pub fraction: Option<f32>,
}

/// System resource snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemSnapshot {
    /// CPU usage percent (0..=100).
    pub cpu_percent: f32,
    /// Memory used / total (e.g. "1.2 / 4.0 GB").
    pub memory: String,
    /// Memory fraction for bar (0.0..=1.0).
    pub memory_fraction: f32,
    /// Per-mount disk rows.
    pub disks: Vec<StatRow>,
    /// Network interfaces with up/down status.
    pub networks: Vec<StatRow>,
    /// Uptime string (e.g. "2h 14m").
    pub uptime: String,
    /// Process count.
    pub process_count: u32,
}

impl Default for SystemSnapshot {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory: String::new(),
            memory_fraction: 0.0,
            disks: Vec::new(),
            networks: Vec::new(),
            uptime: String::new(),
            process_count: 0,
        }
    }
}

/// A system monitor panel (right-anchored).
#[derive(Debug, Clone, PartialEq)]
pub struct SystemMonitor {
    /// Top-left in the parent surface.
    pub origin: (i32, i32),
    /// Panel width.
    pub width: u32,
    /// Panel height.
    pub height: u32,
    /// Whether the panel is visible.
    pub visible: bool,
    /// The latest system snapshot.
    pub snapshot: SystemSnapshot,
}

impl SystemMonitor {
    /// Construct a hidden system monitor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: (0, 0),
            width: 220,
            height: 300,
            visible: false,
            snapshot: SystemSnapshot::default(),
        }
    }

    /// Set the origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.origin = (x, y);
        self
    }

    /// Set the size.
    #[must_use]
    pub fn with_size(mut self, w: u32, h: u32) -> Self {
        self.width = w;
        self.height = h;
        self
    }

    /// Toggle visibility.
    #[must_use]
    pub fn toggled(mut self) -> Self {
        self.visible = !self.visible;
        self
    }

    /// Show the panel.
    #[must_use]
    pub fn shown(mut self) -> Self {
        self.visible = true;
        self
    }

    /// Hide the panel.
    #[must_use]
    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    /// Update the snapshot.
    pub fn update(&mut self, snapshot: SystemSnapshot) {
        self.snapshot = snapshot;
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SystemMonitor {
    fn layout(&self) -> LayoutBox {
        LayoutBox::new(self.origin.0, self.origin.1, self.width, self.height)
    }

    fn style(&self) -> ComponentStyle {
        ComponentStyle::from_roles(Role::BgPanel, Role::TextPrimary, Role::Hairline, Radius::Lg)
    }

    fn padding(&self) -> Insets {
        Insets::even(Spacing::Md.px())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_design_tokens::Color;

    #[test]
    fn new_monitor_is_hidden() {
        let m = SystemMonitor::new();
        assert!(!m.visible);
    }

    #[test]
    fn shown_monitor_is_visible() {
        let m = SystemMonitor::new().shown();
        assert!(m.visible);
    }

    #[test]
    fn toggled_flips_visibility() {
        let m = SystemMonitor::new().toggled();
        assert!(m.visible);
        let m2 = m.toggled();
        assert!(!m2.visible);
    }

    #[test]
    fn default_snapshot_is_zeroed() {
        let s = SystemSnapshot::default();
        assert_eq!(s.cpu_percent, 0.0);
        assert!(s.memory.is_empty());
        assert_eq!(s.memory_fraction, 0.0);
        assert!(s.disks.is_empty());
        assert!(s.networks.is_empty());
        assert!(s.uptime.is_empty());
        assert_eq!(s.process_count, 0);
    }

    #[test]
    fn update_replaces_snapshot() {
        let mut m = SystemMonitor::new();
        let snap = SystemSnapshot {
            cpu_percent: 42.5,
            memory: "1.2 / 4.0 GB".into(),
            memory_fraction: 0.3,
            disks: vec![StatRow {
                label: "/".into(),
                value: "12 / 64 GB".into(),
                fraction: Some(0.1875),
            }],
            networks: vec![StatRow { label: "eth0".into(), value: "UP".into(), fraction: None }],
            uptime: "2h 14m".into(),
            process_count: 128,
        };
        m.update(snap.clone());
        assert_eq!(m.snapshot.cpu_percent, 42.5);
        assert_eq!(m.snapshot.memory, "1.2 / 4.0 GB");
        assert_eq!(m.snapshot.disks.len(), 1);
        assert_eq!(m.snapshot.networks.len(), 1);
    }

    #[test]
    fn layout_uses_origin_and_size() {
        let m = SystemMonitor::new().at(800, 40).with_size(220, 600);
        let l = m.layout();
        assert_eq!(l.x, 800);
        assert_eq!(l.y, 40);
        assert_eq!(l.width, 220);
        assert_eq!(l.height, 600);
    }

    #[test]
    fn style_uses_panel_background() {
        let m = SystemMonitor::new();
        let s = m.style();
        assert_eq!(s.fill, Color::role(Role::BgPanel));
    }

    #[test]
    fn stat_row_fraction_bar() {
        let row = StatRow { label: "MEM".into(), value: "50%".into(), fraction: Some(0.5) };
        assert_eq!(row.fraction, Some(0.5));
    }

    #[test]
    fn stat_row_no_bar() {
        let row = StatRow { label: "eth0".into(), value: "UP".into(), fraction: None };
        assert!(row.fraction.is_none());
    }

    #[test]
    fn multiple_disks_and_networks() {
        let snap = SystemSnapshot {
            disks: vec![
                StatRow { label: "/".into(), value: "10GB".into(), fraction: Some(0.2) },
                StatRow { label: "/home".into(), value: "50GB".into(), fraction: Some(0.5) },
            ],
            networks: vec![
                StatRow { label: "eth0".into(), value: "UP".into(), fraction: None },
                StatRow { label: "wlan0".into(), value: "DOWN".into(), fraction: None },
            ],
            ..SystemSnapshot::default()
        };
        assert_eq!(snap.disks.len(), 2);
        assert_eq!(snap.networks.len(), 2);
    }
}
