//! Taskbar — the OS taskbar (windows + tray + clock).
//!
//! The taskbar is a `Panel::Bottom` with three sections:
//!   * a row of running-window indicators (each one a
//!     `TaskbarItem`),
//!   * a system tray (network, volume, battery, AI state,
//!     clock),
//!   * a centered AI launcher entry-point (per §12: the
//!     "Aether AI launcher" is the start of the taskbar).
//!
//! The component layer splits the bar into three regions;
//! the renderer / layout pass positions each region.

extern crate alloc;

use aether_design_tokens::{AiVisualState, Color, Radius, Role, Spacing, TypeScale};

use crate::{Component, ComponentStyle, Insets, LayoutBox, Panel, PanelSide};

/// One item in the running-windows section.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskbarItem {
    /// The window's title (truncated by the renderer to
    /// fit the slot).
    pub title: String,
    /// Whether the window is currently focused.
    pub focused: bool,
    /// Whether the window is minimized (the renderer
    /// draws a smaller / dimmer chip).
    pub minimized: bool,
    /// Whether the window is currently running an
    /// active task (the renderer can show a small
    /// progress dot).
    pub busy: bool,
}

impl TaskbarItem {
    /// Construct a taskbar item.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), focused: false, minimized: false, busy: false }
    }

    /// Mark focused.
    #[must_use]
    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }

    /// Mark minimized.
    #[must_use]
    pub fn minimized(mut self) -> Self {
        self.minimized = true;
        self
    }

    /// Mark busy.
    #[must_use]
    pub fn busy(mut self) -> Self {
        self.busy = true;
        self
    }
}

/// The taskbar. Built on top of a `Panel::Bottom` for
/// the chrome, with the AI launcher entry-point as
/// the first item in the running-windows section.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Taskbar {
    /// The bottom panel that holds the chrome.
    pub panel: Panel,
    /// The running-window chips.
    pub items: alloc::vec::Vec<TaskbarItem>,
    /// The AI's current visual state (drives the AI
    /// tray icon's color).
    pub ai_state: Option<AiVisualState>,
    /// Whether the network tray icon shows "up" or
    /// "down". The renderer paints the icon's color.
    pub network_up: bool,
    /// Whether the volume tray icon shows muted.
    pub volume_muted: bool,
    /// Whether the battery tray icon is in
    /// low-battery mode (red).
    pub battery_low: bool,
    /// The clock string ("14:32"). The renderer paints
    /// this with the caption type scale.
    pub clock: String,
}

impl Taskbar {
    /// Construct a taskbar with the §12 default
    /// bottom-anchored panel.
    #[must_use]
    pub fn new() -> Self {
        let panel = Panel::new(PanelSide::Bottom)
            .with_padding(Insets::symmetric(Spacing::Sm.px(), Spacing::Md.px()));
        Self {
            panel,
            items: alloc::vec::Vec::new(),
            ai_state: None,
            network_up: true,
            volume_muted: false,
            battery_low: false,
            clock: String::new(),
        }
    }

    /// Set the AI's current visual state.
    #[must_use]
    pub fn ai_state(mut self, s: AiVisualState) -> Self {
        self.ai_state = Some(s);
        self
    }

    /// Set the network state.
    #[must_use]
    pub fn network(mut self, up: bool) -> Self {
        self.network_up = up;
        self
    }

    /// Set the volume state.
    #[must_use]
    pub fn volume_muted(mut self) -> Self {
        self.volume_muted = true;
        self
    }

    /// Set the battery state.
    #[must_use]
    pub fn battery_low(mut self) -> Self {
        self.battery_low = true;
        self
    }

    /// Set the clock.
    #[must_use]
    pub fn clock(mut self, c: impl Into<String>) -> Self {
        self.clock = c.into();
        self
    }

    /// Add a taskbar item.
    #[must_use]
    pub fn push(mut self, item: TaskbarItem) -> Self {
        self.items.push(item);
        self
    }

    /// The default taskbar height. The §12 spec says
    /// 48 px; we use 48 so the AI launcher tile can
    /// fit comfortably.
    #[must_use]
    pub fn default_height_px() -> u32 {
        48
    }

    /// The default taskbar item chip width.
    #[must_use]
    pub fn item_chip_width_px() -> u32 {
        160
    }
}

impl Default for Taskbar {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Taskbar {
    fn layout(&self) -> LayoutBox {
        self.panel.layout()
    }

    fn style(&self) -> ComponentStyle {
        self.panel.style()
    }

    fn padding(&self) -> Insets {
        self.panel.padding
    }
}

/// The color the running-window chip should be painted
/// with. Focused chips use the lavender accent;
/// minimized chips use the disabled ink; busy chips
/// add the AI-state color (if any) as a dot.
pub fn item_style(t: &Taskbar, index: usize) -> ComponentStyle {
    let item = match t.items.get(index) {
        Some(i) => i,
        None => {
            return ComponentStyle::from_roles(
                Role::BgPanelHover,
                Role::TextDisabled,
                Role::Hairline,
                Radius::Md,
            )
        }
    };
    if item.focused {
        return ComponentStyle::from_roles(
            Role::AccentLavender,
            Role::TextPrimary,
            Role::AccentLavenderStrong,
            Radius::Lg,
        );
    }
    if item.minimized {
        return ComponentStyle::from_roles(
            Role::BgPanel,
            Role::TextSecondary,
            Role::Hairline,
            Radius::Md,
        );
    }
    ComponentStyle::from_roles(Role::BgPanel, Role::TextPrimary, Role::Hairline, Radius::Md)
}

/// The color of the AI tray icon, given the taskbar's
/// `ai_state`. Returns `None` if no AI state is set.
#[must_use]
pub fn ai_tray_color(t: &Taskbar) -> Option<Color> {
    t.ai_state.map(AiVisualState::color)
}

/// The color of the network tray icon.
#[must_use]
pub fn network_tray_color(t: &Taskbar) -> Color {
    if t.network_up {
        Color::role(Role::AccentMint)
    } else {
        Color::role(Role::AccentPeachStrong)
    }
}

/// The color of the volume tray icon.
#[must_use]
pub fn volume_tray_color(t: &Taskbar) -> Color {
    if t.volume_muted {
        Color::role(Role::TextDisabled)
    } else {
        Color::role(Role::TextPrimary)
    }
}

/// The color of the battery tray icon.
#[must_use]
pub fn battery_tray_color(t: &Taskbar) -> Color {
    if t.battery_low {
        Color::role(Role::AccentPeachStrong)
    } else {
        Color::role(Role::AccentMint)
    }
}

/// The type scale the clock uses.
#[must_use]
pub fn clock_type() -> TypeScale {
    TypeScale::Caption
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn default_taskbar_has_no_ai_state() {
        let t = Taskbar::new();
        assert!(t.ai_state.is_none());
    }

    #[test]
    fn default_taskbar_assumes_network_up() {
        let t = Taskbar::new();
        assert!(t.network_up);
        assert!(!t.volume_muted);
        assert!(!t.battery_low);
    }

    #[test]
    fn default_height_is_48() {
        assert_eq!(Taskbar::default_height_px(), 48);
    }

    #[test]
    fn default_item_chip_width_is_160() {
        assert_eq!(Taskbar::item_chip_width_px(), 160);
    }

    #[test]
    fn focused_item_uses_lavender() {
        let t = Taskbar::new().push(TaskbarItem::new("Calc").focused());
        let s = item_style(&t, 0);
        assert_eq!(s.fill, Color::role(Role::AccentLavender));
    }

    #[test]
    fn minimized_item_uses_secondary_text() {
        let t = Taskbar::new().push(TaskbarItem::new("Calc").minimized());
        let s = item_style(&t, 0);
        assert_eq!(s.text, Color::role(Role::TextSecondary));
    }

    #[test]
    fn normal_item_uses_panel() {
        let t = Taskbar::new().push(TaskbarItem::new("Calc"));
        let s = item_style(&t, 0);
        assert_eq!(s.fill, Color::role(Role::BgPanel));
    }

    #[test]
    fn out_of_bounds_returns_disabled() {
        let t = Taskbar::new();
        let s = item_style(&t, 0);
        assert_eq!(s.text, Color::role(Role::TextDisabled));
    }

    #[test]
    fn ai_tray_color_reflects_ai_state() {
        let t = Taskbar::new().ai_state(AiVisualState::Thinking);
        assert_eq!(ai_tray_color(&t), Some(Color::PASTEL_BLUE));
    }

    #[test]
    fn ai_tray_color_is_none_when_no_state() {
        let t = Taskbar::new();
        assert_eq!(ai_tray_color(&t), None);
    }

    #[test]
    fn network_tray_uses_mint_when_up() {
        let t = Taskbar::new().network(true);
        assert_eq!(network_tray_color(&t), Color::role(Role::AccentMint));
    }

    #[test]
    fn network_tray_uses_peach_deep_when_down() {
        let t = Taskbar::new().network(false);
        assert_eq!(network_tray_color(&t), Color::role(Role::AccentPeachStrong));
    }

    #[test]
    fn volume_tray_uses_disabled_when_muted() {
        let t = Taskbar::new().volume_muted();
        assert_eq!(volume_tray_color(&t), Color::role(Role::TextDisabled));
    }

    #[test]
    fn volume_tray_uses_primary_when_unmuted() {
        let t = Taskbar::new();
        assert_eq!(volume_tray_color(&t), Color::role(Role::TextPrimary));
    }

    #[test]
    fn battery_tray_uses_peach_deep_when_low() {
        let t = Taskbar::new().battery_low();
        assert_eq!(battery_tray_color(&t), Color::role(Role::AccentPeachStrong));
    }

    #[test]
    fn clock_type_is_caption() {
        assert_eq!(clock_type(), TypeScale::Caption);
    }

    #[test]
    fn clock_field_is_set() {
        let t = Taskbar::new().clock("14:32");
        assert_eq!(t.clock, "14:32");
    }
}
