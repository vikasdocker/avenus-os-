//! Panel — a sidebar / drawer surface.
//!
//! A panel is a card that occupies a fixed edge of the
//! parent surface. The four sides are:
//!   * `Left`   — a vertical sidebar (the launcher's
//!                 home column, the AI assistant panel).
//!   * `Right`  — vertical, right-anchored (a system
//!                 tray drawer).
//!   * `Top`    — horizontal bar (a status strip).
//!   * `Bottom` — horizontal bar (a notification
//!                 drawer).
//!
//! The panel carries the side so the renderer can
//! anchor it correctly without re-asking.

extern crate alloc;

use aether_design_tokens::{Radius, Role, Spacing};

use crate::{Component, ComponentStyle, Insets, LayoutBox};

/// Which edge a panel is anchored to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PanelSide {
    /// Left edge.
    Left,
    /// Right edge.
    Right,
    /// Top edge.
    Top,
    /// Bottom edge.
    Bottom,
}

impl PanelSide {
    /// Whether the panel is vertical (Left / Right) or
    /// horizontal (Top / Bottom).
    #[must_use]
    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// A panel. The size is the side-specific dimension
/// (width for left/right, height for top/bottom); the
/// other dimension spans the parent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Panel {
    /// Top-left in the parent surface.
    pub origin: (i32, i32),
    /// Width in pixels (ignored for Top / Bottom, where
    /// the height is the meaningful dimension).
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Which edge this panel is anchored to.
    pub side: PanelSide,
    /// Internal padding.
    pub padding: Insets,
    /// Whether the panel is currently visible (panels
    /// can be hidden behind a toggle, e.g. the launcher
    /// is a Left panel with `visible = false` until
    /// the user opens it).
    pub visible: bool,
}

impl Panel {
    /// Construct a panel.
    #[must_use]
    pub fn new(side: PanelSide) -> Self {
        Self {
            origin: (0, 0),
            width: 0,
            height: 0,
            side,
            padding: Insets::even(Spacing::Lg.px()),
            visible: true,
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

    /// Override the padding.
    #[must_use]
    pub fn with_padding(mut self, p: Insets) -> Self {
        self.padding = p;
        self
    }

    /// Hide the panel.
    #[must_use]
    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }
}

impl Component for Panel {
    fn layout(&self) -> LayoutBox {
        LayoutBox::new(self.origin.0, self.origin.1, self.width, self.height)
    }

    fn style(&self) -> ComponentStyle {
        ComponentStyle::from_roles(Role::BgPanel, Role::TextPrimary, Role::Hairline, Radius::Lg)
    }

    fn padding(&self) -> Insets {
        self.padding
    }
}

/// The default width for a left-anchored panel (the
/// launcher's home column).
#[must_use]
pub fn default_left_width() -> u32 {
    240
}

/// The default height for a top-anchored panel.
#[must_use]
pub fn default_top_height() -> u32 {
    48
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_design_tokens::Color;

    #[test]
    fn left_and_right_are_vertical() {
        assert!(PanelSide::Left.is_vertical());
        assert!(PanelSide::Right.is_vertical());
        assert!(!PanelSide::Top.is_vertical());
        assert!(!PanelSide::Bottom.is_vertical());
    }

    #[test]
    fn default_left_width_is_240() {
        assert_eq!(default_left_width(), 240);
    }

    #[test]
    fn default_top_height_is_48() {
        assert_eq!(default_top_height(), 48);
    }

    #[test]
    fn new_panel_is_visible() {
        let p = Panel::new(PanelSide::Left);
        assert!(p.visible);
    }

    #[test]
    fn hidden_panel_is_not_visible() {
        let p = Panel::new(PanelSide::Left).hidden();
        assert!(!p.visible);
    }

    #[test]
    fn panel_uses_panel_background() {
        let p = Panel::new(PanelSide::Left);
        let s = p.style();
        assert_eq!(s.fill, Color::role(Role::BgPanel));
    }

    #[test]
    fn panel_default_padding_is_lg() {
        let p = Panel::new(PanelSide::Left);
        let pad = p.padding;
        assert_eq!(pad.top, Spacing::Lg.px());
        assert_eq!(pad.left, Spacing::Lg.px());
    }

    #[test]
    fn layout_uses_origin_and_size() {
        let p = Panel::new(PanelSide::Left).at(0, 100).with_size(240, 800);
        let l = p.layout();
        assert_eq!(l.x, 0);
        assert_eq!(l.y, 100);
        assert_eq!(l.width, 240);
        assert_eq!(l.height, 800);
    }
}
