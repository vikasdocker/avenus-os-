//! List — a vertical list of `ListItem`s.
//!
//! The list is a single surface (the `BgPanel` cream); each
//! `ListItem` is its own row inside it. The list tracks
//! the active selection so renderers can draw a focus
//! ring on the right row.
//!
//! Item heights default to `Lg` (16 px) padding on each
//! side + the body type's line height, so a row is
//! `Body.line_height_px() + 2 * Spacing::Lg.px()` = 52 px.

extern crate alloc;

use aether_design_tokens::{Color, Radius, Role, Spacing, TypeScale};

use crate::{Component, ComponentStyle, Insets, LayoutBox};

/// Selection state for a list. A list is either
/// single-select (the common case) or multi-select
/// (file pickers). The component layer is
/// selection-agnostic; it just carries the active
/// index set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ListSelection {
    /// No selection.
    #[default]
    None,
    /// Single selection, the given index.
    Single(usize),
    /// Multi-selection, the set of active indices.
    /// Kept as a `Vec` so the type is `Eq`; for large
    /// lists the renderer can swap in a bitset
    /// without changing the public API.
    Multi(alloc::vec::Vec<usize>),
}

/// A single list row. The list itself owns the geometry
/// (origin + height); each row carries its content +
/// its selected / focused state.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListItem {
    /// The row's primary label.
    pub label: String,
    /// Optional secondary text (right-aligned metadata,
    /// e.g. file size or timestamp).
    pub secondary: Option<String>,
    /// Whether this row is currently selected.
    pub selected: bool,
    /// Whether this row is the keyboard-focused row.
    pub focused: bool,
}

impl ListItem {
    /// Construct a row.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), secondary: None, selected: false, focused: false }
    }

    /// Set secondary text.
    #[must_use]
    pub fn secondary(mut self, s: impl Into<String>) -> Self {
        self.secondary = Some(s.into());
        self
    }

    /// Mark selected.
    #[must_use]
    pub fn selected(mut self) -> Self {
        self.selected = true;
        self
    }

    /// Mark focused.
    #[must_use]
    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }
}

/// A list. Height is `items.len() * row_height`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct List {
    /// Top-left in the parent surface.
    pub origin: (i32, i32),
    /// Width in pixels.
    pub width: u32,
    /// The list's items, in display order.
    pub items: alloc::vec::Vec<ListItem>,
    /// Selection state.
    pub selection: ListSelection,
}

impl List {
    /// Construct an empty list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: (0, 0),
            width: 0,
            items: alloc::vec::Vec::new(),
            selection: ListSelection::None,
        }
    }

    /// Set the origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.origin = (x, y);
        self
    }

    /// Set the width.
    #[must_use]
    pub fn with_width(mut self, w: u32) -> Self {
        self.width = w;
        self
    }

    /// Set the items.
    #[must_use]
    pub fn items(mut self, items: alloc::vec::Vec<ListItem>) -> Self {
        self.items = items;
        self
    }

    /// Set the selection.
    #[must_use]
    pub fn selection(mut self, sel: ListSelection) -> Self {
        self.selection = sel;
        self
    }

    /// Per-row height in pixels. The §12 default:
    /// body line height + 2 * Lg padding.
    #[must_use]
    pub fn row_height_px() -> u32 {
        TypeScale::Body.line_height_px() + 2 * Spacing::Lg.px_u32()
    }

    /// Per-row layout, indexed from 0.
    #[must_use]
    pub fn row_layout(&self, index: usize) -> LayoutBox {
        LayoutBox::new(
            self.origin.0,
            self.origin.1 + (index as i32) * Self::row_height_px() as i32,
            self.width,
            Self::row_height_px(),
        )
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for List {
    fn layout(&self) -> LayoutBox {
        let h = (self.items.len() as u32) * Self::row_height_px();
        LayoutBox::new(self.origin.0, self.origin.1, self.width, h)
    }

    fn style(&self) -> ComponentStyle {
        ComponentStyle::from_roles(Role::BgPanel, Role::TextPrimary, Role::Hairline, Radius::Lg)
    }

    fn padding(&self) -> Insets {
        Insets::even(0)
    }
}

/// The style of a single row. Renderers consume this
/// row-by-row; the list exposes `row_style(index)` so
/// the row's selected / focused state is computed at
/// the same place as the row's geometry.
pub fn row_style(list: &List, index: usize) -> ComponentStyle {
    let item = match list.items.get(index) {
        Some(i) => i,
        None => {
            return ComponentStyle::from_roles(
                Role::BgPanel,
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
            Radius::Md,
        );
    }
    if item.selected {
        return ComponentStyle::from_roles(
            Role::BgPanelHover,
            Role::TextPrimary,
            Role::Hairline,
            Radius::Md,
        );
    }
    // Default: transparent fill so the list's own
    // BgPanel shows through.
    ComponentStyle::from_roles(Role::BgBase, Role::TextPrimary, Role::Hairline, Radius::Md)
}

/// The text color for a list row's secondary column.
pub fn secondary_text_color() -> Color {
    Color::role(Role::TextSecondary)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn row_height_is_body_line_plus_double_lg() {
        let expected = TypeScale::Body.line_height_px() + 2 * Spacing::Lg.px_u32();
        assert_eq!(List::row_height_px(), expected);
    }

    #[test]
    fn empty_list_has_zero_height() {
        let l = List::new();
        let layout = l.layout();
        assert_eq!(layout.height, 0);
    }

    #[test]
    fn height_grows_with_items() {
        let l = List::new().with_width(200).items(alloc::vec![
            ListItem::new("a"),
            ListItem::new("b"),
            ListItem::new("c")
        ]);
        let layout = l.layout();
        assert_eq!(layout.height, 3 * List::row_height_px());
    }

    #[test]
    fn row_layout_indexes() {
        let l = List::new()
            .at(10, 20)
            .with_width(200)
            .items(alloc::vec![ListItem::new("a"), ListItem::new("b")]);
        assert_eq!(l.row_layout(0).y, 20);
        assert_eq!(l.row_layout(1).y, 20 + List::row_height_px() as i32);
    }

    #[test]
    fn list_uses_panel_background() {
        let l = List::new();
        let s = l.style();
        assert_eq!(s.fill, Color::role(Role::BgPanel));
    }

    #[test]
    fn focused_row_uses_lavender() {
        let l = List::new().items(alloc::vec![ListItem::new("a").focused()]);
        let s = row_style(&l, 0);
        assert_eq!(s.fill, Color::role(Role::AccentLavender));
    }

    #[test]
    fn selected_row_uses_panel_hover() {
        let l = List::new().items(alloc::vec![ListItem::new("a").selected()]);
        let s = row_style(&l, 0);
        assert_eq!(s.fill, Color::role(Role::BgPanelHover));
    }

    #[test]
    fn out_of_bounds_row_returns_disabled_style() {
        let l = List::new();
        let s = row_style(&l, 0);
        assert_eq!(s.text, Color::role(Role::TextDisabled));
    }

    #[test]
    fn secondary_text_is_in_text_secondary_role() {
        assert_eq!(secondary_text_color(), Color::role(Role::TextSecondary));
    }

    #[test]
    fn single_selection_equality() {
        let a = ListSelection::Single(2);
        let b = ListSelection::Single(2);
        assert_eq!(a, b);
    }
}
