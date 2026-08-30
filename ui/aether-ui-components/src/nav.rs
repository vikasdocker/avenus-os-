//! Nav — a horizontal or vertical nav rail.
//!
//! A nav is a row (or column) of `NavItem`s with a
//! single active item highlighted. The launcher uses
//! a vertical Nav for the mode switcher (apps, files,
//! AI); the AI command bar uses a horizontal one.

extern crate alloc;

use aether_design_tokens::{Radius, Role, Spacing, TypeScale};

use crate::{Component, ComponentStyle, Insets, LayoutBox};

/// Whether a nav is laid out horizontally (the AI
/// command bar's mode tabs) or vertically (the
/// launcher's mode rail).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NavOrientation {
    /// Left-to-right.
    Horizontal,
    /// Top-to-bottom.
    Vertical,
}

/// A single nav item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NavItem {
    /// The label shown next to the icon.
    pub label: String,
    /// Whether this is the active item.
    pub active: bool,
    /// Whether the keyboard focus is on this item.
    pub focused: bool,
}

impl NavItem {
    /// Construct a nav item.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), active: false, focused: false }
    }

    /// Mark active.
    #[must_use]
    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }

    /// Mark focused.
    #[must_use]
    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }
}

/// A nav rail.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Nav {
    /// Top-left in the parent surface.
    pub origin: (i32, i32),
    /// Length of the rail along the orientation axis
    /// (width for horizontal, height for vertical).
    pub length: u32,
    /// The items.
    pub items: alloc::vec::Vec<NavItem>,
    /// Horizontal / vertical.
    pub orientation: NavOrientation,
}

impl Nav {
    /// Construct a nav.
    #[must_use]
    pub fn new(orientation: NavOrientation) -> Self {
        Self { origin: (0, 0), length: 0, items: alloc::vec::Vec::new(), orientation }
    }

    /// Set the origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.origin = (x, y);
        self
    }

    /// Set the length.
    #[must_use]
    pub fn with_length(mut self, length: u32) -> Self {
        self.length = length;
        self
    }

    /// Set the items.
    #[must_use]
    pub fn items(mut self, items: alloc::vec::Vec<NavItem>) -> Self {
        self.items = items;
        self
    }

    /// Item dimension along the orientation axis
    /// (width for horizontal, height for vertical).
    #[must_use]
    pub fn item_length_px() -> u32 {
        // Body line height + 2 * Lg padding.
        TypeScale::Body.line_height_px() + 2 * Spacing::Lg.px_u32()
    }
}

impl Component for Nav {
    fn layout(&self) -> LayoutBox {
        let total = (self.items.len() as u32) * Self::item_length_px();
        match self.orientation {
            NavOrientation::Horizontal => {
                LayoutBox::new(self.origin.0, self.origin.1, total, self.length)
            }
            NavOrientation::Vertical => {
                LayoutBox::new(self.origin.0, self.origin.1, self.length, total)
            }
        }
    }

    fn style(&self) -> ComponentStyle {
        // The nav itself is transparent — the items
        // paint their own background.
        ComponentStyle::from_roles(Role::BgBase, Role::TextPrimary, Role::Hairline, Radius::Md)
    }

    fn padding(&self) -> Insets {
        Insets::even(0)
    }
}

/// The style of a single nav item. The nav exposes
/// `item_style(index)` so the active / focused state
/// is computed alongside the geometry.
pub fn item_style(nav: &Nav, index: usize) -> ComponentStyle {
    let item = match nav.items.get(index) {
        Some(i) => i,
        None => {
            return ComponentStyle::from_roles(
                Role::BgBase,
                Role::TextDisabled,
                Role::Hairline,
                Radius::Md,
            )
        }
    };
    if item.active {
        return ComponentStyle::from_roles(
            Role::AccentLavender,
            Role::TextPrimary,
            Role::AccentLavenderStrong,
            Radius::Md,
        );
    }
    if item.focused {
        return ComponentStyle::from_roles(
            Role::BgPanelHover,
            Role::TextPrimary,
            Role::Hairline,
            Radius::Md,
        );
    }
    ComponentStyle::from_roles(Role::BgBase, Role::TextPrimary, Role::Hairline, Radius::Md)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_design_tokens::Color;

    #[test]
    fn item_length_is_body_line_plus_double_lg() {
        let expected = TypeScale::Body.line_height_px() + 2 * Spacing::Lg.px_u32();
        assert_eq!(Nav::item_length_px(), expected);
    }

    #[test]
    fn horizontal_nav_is_wide() {
        let n = Nav::new(NavOrientation::Horizontal)
            .with_length(40)
            .items(alloc::vec![NavItem::new("Apps"), NavItem::new("AI")]);
        let l = n.layout();
        assert_eq!(l.width, 2 * Nav::item_length_px());
        assert_eq!(l.height, 40);
    }

    #[test]
    fn vertical_nav_is_tall() {
        let n = Nav::new(NavOrientation::Vertical)
            .with_length(40)
            .items(alloc::vec![NavItem::new("Apps"), NavItem::new("AI")]);
        let l = n.layout();
        assert_eq!(l.width, 40);
        assert_eq!(l.height, 2 * Nav::item_length_px());
    }

    #[test]
    fn active_item_uses_lavender() {
        let n =
            Nav::new(NavOrientation::Horizontal).items(alloc::vec![NavItem::new("Apps").active()]);
        let s = item_style(&n, 0);
        assert_eq!(s.fill, Color::role(Role::AccentLavender));
    }

    #[test]
    fn focused_item_uses_panel_hover() {
        let n =
            Nav::new(NavOrientation::Horizontal).items(alloc::vec![NavItem::new("Apps").focused()]);
        let s = item_style(&n, 0);
        assert_eq!(s.fill, Color::role(Role::BgPanelHover));
    }

    #[test]
    fn inactive_item_uses_base() {
        let n = Nav::new(NavOrientation::Horizontal).items(alloc::vec![NavItem::new("Apps")]);
        let s = item_style(&n, 0);
        assert_eq!(s.fill, Color::role(Role::BgBase));
    }

    #[test]
    fn out_of_bounds_returns_disabled_style() {
        let n = Nav::new(NavOrientation::Horizontal);
        let s = item_style(&n, 0);
        assert_eq!(s.text, Color::role(Role::TextDisabled));
    }
}
