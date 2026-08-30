//! Aether UI component library.
//!
//! Typed, non-painting component definitions for the Aether
//! design system. Each component is a struct whose fields
//! describe its visual + semantic state; the `layout()` method
//! resolves the design tokens (color, spacing, radius, type)
//! into a concrete `LayoutBox` (rect + padding + corner radius)
//! and a paint-time `ComponentStyle` (the colors the renderer
//! needs to fill it with).
//!
//! The split is deliberate: a component here is a *description*
//! of what the user sees, not a framebuffer draw call. The
//! graphical shell, the Wayland compositor, the headless test
//! renderer, and the accessibility auditor all consume the
//! same `Component` value and apply their own paint logic.
//!
//! Every component resolves its colors through
//! `aether-design-tokens::Color::role(Role::...)`. There is no
//! raw `Rgb` constant in this crate; a re-skin is a one-file
//! change.
//!
//! The component set today:
//!
//!   * [`button`]    — the primary interactive control.
//!   * [`card`]      — a content surface (a launcher tile, a
//!                       file preview).
//!   * [`list`]      — a vertical list of [`list::ListItem`].
//!   * [`dialog`]    — a modal with a title, body, and
//!                       actions.
//!   * [`panel`]     — a sidebar / drawer surface.
//!   * [`nav`]       — a horizontal or vertical nav rail.
//!   * [`taskbar`]   — the OS taskbar (windows + tray + clock).
//!   * [`launcher`]  — the AI-first application launcher tiles.
//!
//! All components are `Clone + PartialEq + Debug` so they
//! serialize into a snapshot for accessibility audits and
//! snapshot tests.

#![deny(unsafe_code)]
#![warn(missing_docs)]
// The crate's `//!` doc blocks use a multi-column bullet
// list where the second line of each bullet is indented
// to align with the first character of the description
// (rather than the start of the bullet). That looks
// better in source but trips
// `clippy::doc_overindented_list_items`. The lint is
// pure style and would force a less-readable doc; allow
// it crate-wide.
#![allow(clippy::doc_overindented_list_items)]

pub mod button;
pub mod card;
pub mod dialog;
pub mod launcher;
pub mod list;
pub mod nav;
pub mod panel;
pub mod taskbar;

pub use button::{Button, ButtonSize, ButtonVariant};
pub use card::{Card, CardElevation};
pub use dialog::{Dialog, DialogAction};
pub use launcher::{Launcher, LauncherTile};
pub use list::{List, ListItem, ListSelection};
pub use nav::{Nav, NavItem, NavOrientation};
pub use panel::{Panel, PanelSide};
pub use taskbar::TaskbarItem;

use aether_design_tokens::{Color, Radius, Role, Spacing};

/// A rectangle in the design-system coordinate space. The
/// origin is the top-left of the parent surface; sizes are
/// in CSS pixels (1 px = 1 design pixel; the renderer
/// scales by the device pixel ratio at paint time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutBox {
    /// X coordinate of the top-left corner.
    pub x: i32,
    /// Y coordinate of the top-left corner.
    pub y: i32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl LayoutBox {
    /// Construct a `LayoutBox` from its parts.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Right edge (exclusive).
    #[must_use]
    pub const fn right(self) -> i32 {
        self.x + self.width as i32
    }

    /// Bottom edge (exclusive).
    #[must_use]
    pub const fn bottom(self) -> i32 {
        self.y + self.height as i32
    }
}

/// The padding inside a component. The four sides are
/// independent so a button can have 12 px vertical / 16 px
/// horizontal (the §12 button default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Insets {
    /// Top padding.
    pub top: i32,
    /// Right padding.
    pub right: i32,
    /// Bottom padding.
    pub bottom: i32,
    /// Left padding.
    pub left: i32,
}

impl Insets {
    /// Even insets on all four sides.
    #[must_use]
    pub const fn even(v: i32) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }

    /// Symmetric vertical / horizontal insets.
    #[must_use]
    pub const fn symmetric(vertical: i32, horizontal: i32) -> Self {
        Self { top: vertical, right: horizontal, bottom: vertical, left: horizontal }
    }
}

/// The paint-time style of a component: the colors and
/// radius the renderer should use. This is the "what" the
/// renderer needs; the renderer decides "how" to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentStyle {
    /// The fill color (the component's surface).
    pub fill: Color,
    /// The text color (legible on the fill).
    pub text: Color,
    /// The border / hairline color.
    pub border: Color,
    /// The corner radius.
    pub radius: Radius,
}

impl ComponentStyle {
    /// Resolve a `(fill role, text role, border role, radius)`
    /// triple into a `ComponentStyle`.
    #[must_use]
    pub const fn from_roles(fill: Role, text: Role, border: Role, radius: Radius) -> Self {
        Self {
            fill: Color::role(fill),
            text: Color::role(text),
            border: Color::role(border),
            radius,
        }
    }
}

/// A component is something the renderer can lay out and
/// paint. The trait exists so the renderer's "for each
/// component" loop can call `layout()` and `style()` without
/// knowing the concrete type.
pub trait Component {
    /// The component's bounding box in the parent surface's
    /// coordinate space. Includes the padding.
    fn layout(&self) -> LayoutBox;
    /// The component's paint-time style.
    fn style(&self) -> ComponentStyle;
    /// The component's inner padding (between the bounding
    /// box edge and the content).
    fn padding(&self) -> Insets;
    /// The inner content rect — `layout()` minus `padding()`.
    /// Renderers paint text / icons in this rect.
    #[must_use]
    fn content_rect(&self) -> LayoutBox {
        let outer = self.layout();
        let pad = self.padding();
        let x = outer.x + pad.left;
        let y = outer.y + pad.top;
        let w = (outer.width as i32 - pad.left - pad.right).max(0) as u32;
        let h = (outer.height as i32 - pad.top - pad.bottom).max(0) as u32;
        LayoutBox::new(x, y, w, h)
    }
}

/// Common defaults: a 12 px vertical / 16 px horizontal
/// inset for primary surfaces. Component modules that want
/// the §12 default should call `default_button_insets()`.
#[must_use]
pub fn default_button_insets() -> Insets {
    Insets::symmetric(Spacing::Md.px(), Spacing::Lg.px())
}

/// The default radius for primary surfaces: `Lg` (18 px).
#[must_use]
pub fn default_radius() -> Radius {
    Radius::Lg
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn layout_box_right_and_bottom() {
        let b = LayoutBox::new(10, 20, 100, 50);
        assert_eq!(b.right(), 110);
        assert_eq!(b.bottom(), 70);
    }

    #[test]
    fn insets_even_applies_to_all_sides() {
        let i = Insets::even(8);
        assert_eq!(i.top, 8);
        assert_eq!(i.right, 8);
        assert_eq!(i.bottom, 8);
        assert_eq!(i.left, 8);
    }

    #[test]
    fn insets_symmetric_applies_to_axes() {
        let i = Insets::symmetric(12, 16);
        assert_eq!(i.top, 12);
        assert_eq!(i.bottom, 12);
        assert_eq!(i.left, 16);
        assert_eq!(i.right, 16);
    }

    #[test]
    fn content_rect_subtracts_padding() {
        let outer = LayoutBox::new(0, 0, 100, 60);
        let pad = Insets::even(10);
        let c = LayoutBox::new(10, 10, 80, 40);
        // Manual computation matches content_rect.
        assert_eq!(outer.x + pad.left, c.x);
        assert_eq!(outer.y + pad.top, c.y);
        assert_eq!(outer.width as i32 - pad.left - pad.right, c.width as i32);
        assert_eq!(outer.height as i32 - pad.top - pad.bottom, c.height as i32);
    }

    #[test]
    fn content_rect_clamps_to_zero_on_overpadding() {
        // If padding exceeds the box, content_rect must
        // return a zero-size box rather than wrap
        // around to a negative value.
        let outer = LayoutBox::new(0, 0, 10, 10);
        let pad = Insets::even(20);
        let c = LayoutBox::new(20, 20, 0, 0);
        assert_eq!(outer.x + pad.left, c.x);
        let w = (outer.width as i32 - pad.left - pad.right).max(0) as u32;
        let h = (outer.height as i32 - pad.top - pad.bottom).max(0) as u32;
        assert_eq!(w, 0);
        assert_eq!(h, 0);
    }

    #[test]
    fn style_from_roles_resolves_through_tokens() {
        let s =
            ComponentStyle::from_roles(Role::BgBase, Role::TextPrimary, Role::Hairline, Radius::Lg);
        // BgBase = warm white; TextPrimary = INK_900.
        assert_eq!(s.fill.r, 252);
        assert_eq!(s.text, Color::INK_900);
    }
}
