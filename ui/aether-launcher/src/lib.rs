//! Aether Launcher — the AI-first central UI surface.
//!
//! The launcher is the home of Aether. It is what the
//! user sees when they open the start tile on the
//! taskbar. It is the first thing that shows when no app
//! is in focus. It is the "where do I go next?" surface
//! for the entire operating system.
//!
//! Per §12, the launcher is "AI-first" — the three-mode
//! rail at the top of the left column is **apps**, **files**,
//! **AI**. The user opens it expecting to launch
//! *something*, and Aether is one of those somethings.
//!
//! Composition:
//!
//! ```text
//!   +---------+---------------------------+
//!   |   Nav   |                           |
//!   | (apps / |       search box          |
//!   |  files  |                           |
//!   |  / ai)  +---------------------------+
//!   |         |                           |
//!   |         |       launcher grid       |
//!   |         |    (3 cols of tiles)      |
//!   |         |                           |
//!   +---------+---------------------------+
//! ```
//!
//! The launcher is built from the [`aether_ui_components`]
//! primitives — a `Nav` for the mode rail, a `Card` for
//! the search box, a `Launcher` for the grid — and adds
//! the *state machine* and the *content model* that
//! turns the primitives into a working surface:
//!
//!   * the three `LauncherMode` values (Apps / Files / Ai),
//!   * a content resolver (`LauncherContent`) that maps a
//!     mode + query into a list of `LauncherTile`s,
//!   * a keyboard model: arrow keys move selection,
//!     Enter launches, typing filters.
//!
//! The crate is *non-painting*: it returns a
//! `LauncherView` value that the renderer / layout pass
//! resolves into actual pixels. The same value drives
//! the headless test renderer, the accessibility auditor,
//! and the snapshot tests.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

pub mod content;
pub mod mode;
pub mod view;

pub use content::{LauncherContent, LauncherMatch};
pub use mode::LauncherMode;
pub use view::{LauncherView, ViewAction};

use aether_design_tokens::{Color, Spacing};
use aether_ui_components::{
    Component, ComponentStyle, Insets, LayoutBox, Nav, NavItem, NavOrientation, Panel, PanelSide,
};

use alloc::string::String;
use alloc::vec::Vec;

/// The launcher's mode rail: three vertical tabs (Apps,
/// Files, AI). The active mode drives what the tile grid
/// shows. Aether is one of the three on purpose — the AI
/// is not a chatbot, it is a *place* the user goes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModeRail {
    /// The vertical `Nav` of mode tabs.
    pub nav: Nav,
}

impl ModeRail {
    /// Construct a mode rail for the given active mode.
    /// The list always contains the three modes in the
    /// canonical order: Apps, Files, AI.
    #[must_use]
    pub fn new(active: LauncherMode) -> Self {
        let mut apps = NavItem::new("Apps");
        let mut files = NavItem::new("Files");
        let mut ai = NavItem::new("AI");
        match active {
            LauncherMode::Apps => apps = apps.active(),
            LauncherMode::Files => files = files.active(),
            LauncherMode::Ai => ai = ai.active(),
        }
        let items = alloc::vec![apps, files, ai];
        // The mode rail is a narrow vertical column
        // (~64 px) on the left of the launcher.
        let nav = Nav::new(NavOrientation::Vertical).with_length(64).items(items);
        Self { nav }
    }

    /// Set the launcher's top-left origin. The rail is
    /// 64 px wide; the height is the parent surface's.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.nav = self.nav.at(x, y);
        self
    }

    /// The rail's width in pixels.
    #[must_use]
    pub fn width_px() -> u32 {
        64
    }
}

impl Component for ModeRail {
    fn layout(&self) -> LayoutBox {
        self.nav.layout()
    }

    fn style(&self) -> ComponentStyle {
        self.nav.style()
    }

    fn padding(&self) -> Insets {
        self.nav.padding()
    }
}

/// The launcher's search box. Plain text, with a small
/// placeholder. The renderer paints the placeholder when
/// the field is empty and hides the cursor when the field
/// is not focused.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchBox {
    /// Top-left in the parent surface.
    pub origin: (i32, i32),
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels. The §12 default is 40 px.
    pub height: u32,
    /// Current text. Empty string = no query.
    pub text: String,
    /// Whether the field has keyboard focus.
    pub focused: bool,
    /// Placeholder shown when `text` is empty. The default
    /// is mode-aware ("Search apps", "Search files", "Ask Aether").
    pub placeholder: String,
}

impl SearchBox {
    /// Construct a search box with the §12 default height
    /// and an empty query.
    #[must_use]
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            origin: (0, 0),
            width: 0,
            height: Self::default_height_px(),
            text: String::new(),
            focused: true,
            placeholder: placeholder.into(),
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

    /// Override the current text.
    #[must_use]
    pub fn text(mut self, t: impl Into<String>) -> Self {
        self.text = t.into();
        self
    }

    /// Mark the field as not focused.
    #[must_use]
    pub fn blurred(mut self) -> Self {
        self.focused = false;
        self
    }

    /// §12 default search-box height.
    #[must_use]
    pub fn default_height_px() -> u32 {
        40
    }
}

impl Component for SearchBox {
    fn layout(&self) -> LayoutBox {
        LayoutBox::new(self.origin.0, self.origin.1, self.width, self.height)
    }

    fn style(&self) -> ComponentStyle {
        use aether_design_tokens::Role;
        // The search box is a flat surface inside the
        // launcher panel. Focused: lavender hairline.
        // Unfocused: hairline hairline.
        if self.focused {
            ComponentStyle::from_roles(
                Role::BgBase,
                Role::TextPrimary,
                Role::AccentLavenderStrong,
                aether_design_tokens::Radius::Lg,
            )
        } else {
            ComponentStyle::from_roles(
                Role::BgBase,
                Role::TextPrimary,
                Role::Hairline,
                aether_design_tokens::Radius::Lg,
            )
        }
    }

    fn padding(&self) -> Insets {
        Insets::symmetric(Spacing::Sm.px(), Spacing::Lg.px())
    }
}

/// The launcher surface as a whole: a `Panel::Left` of
/// the mode rail + the search box + the tile grid. The
/// renderer reads the resolved `LauncherView` and paints
/// each region.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LauncherSurface {
    /// The backing panel (the chrome).
    pub panel: Panel,
    /// The mode rail (left edge of the launcher).
    pub mode_rail: ModeRail,
    /// The search box (top-right of the launcher).
    pub search: SearchBox,
    /// The active mode.
    pub mode: LauncherMode,
    /// The current search query.
    pub query: String,
}

impl LauncherSurface {
    /// Construct the §12 default launcher surface for the
    /// given mode. The search placeholder is mode-aware.
    #[must_use]
    pub fn new(mode: LauncherMode) -> Self {
        let panel = Panel::new(PanelSide::Left);
        let mode_rail = ModeRail::new(mode);
        let placeholder = mode.search_placeholder();
        let search = SearchBox::new(placeholder);
        Self { panel, mode_rail, search, mode, query: String::new() }
    }

    /// Set the search query.
    #[must_use]
    pub fn query(mut self, q: impl Into<String>) -> Self {
        let s: String = q.into();
        self.query = s.clone();
        self.search = self.search.text(s);
        self
    }

    /// Set the launcher's top-left origin and total size.
    /// The internal layout divides the box into the mode
    /// rail (left), the search bar (top-right), and the
    /// tile grid (the rest).
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.panel = self.panel.at(x, y);
        self
    }

    /// Set the launcher's total size.
    #[must_use]
    pub fn with_size(mut self, w: u32, h: u32) -> Self {
        self.panel = self.panel.with_size(w, h);
        self
    }

    /// The mode rail's `LayoutBox`, given the panel origin.
    #[must_use]
    pub fn mode_rail_box(&self) -> LayoutBox {
        LayoutBox::new(
            self.panel.origin.0,
            self.panel.origin.1,
            ModeRail::width_px(),
            self.panel.height,
        )
    }

    /// The content region's `LayoutBox` (everything to the
    /// right of the mode rail).
    #[must_use]
    pub fn content_box(&self) -> LayoutBox {
        LayoutBox::new(
            self.panel.origin.0 + ModeRail::width_px() as i32,
            self.panel.origin.1,
            self.panel.width.saturating_sub(ModeRail::width_px()),
            self.panel.height,
        )
    }

    /// The search box's `LayoutBox`, given the panel size.
    #[must_use]
    pub fn search_box(&self) -> LayoutBox {
        let content = self.content_box();
        LayoutBox::new(
            content.x,
            content.y + Spacing::Md.px(),
            content.width.saturating_sub(2 * Spacing::Md.px_u32()),
            SearchBox::default_height_px(),
        )
    }

    /// The tile grid's `LayoutBox`, given the panel size.
    #[must_use]
    pub fn grid_box(&self) -> LayoutBox {
        let content = self.content_box();
        let search = self.search_box();
        LayoutBox::new(
            content.x,
            search.bottom() + Spacing::Lg.px(),
            content.width,
            content
                .height
                .saturating_sub((search.bottom() - content.y) as u32 + Spacing::Lg.px_u32()),
        )
    }
}

impl Component for LauncherSurface {
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

/// Color used for the AI's pulsing "listening" indicator
/// on the launcher. Renderers may use this for an
/// outline or a corner mark. Pulled from the design
/// tokens so it matches every other AI surface.
#[must_use]
pub fn ai_launcher_accent() -> Color {
    aether_design_tokens::AiVisualState::Listening.color()
}

/// The default sort order for the launcher's three
/// modes. The renderer's mode rail reads this to lay
/// out the tabs in the canonical order.
#[must_use]
pub fn default_mode_order() -> Vec<LauncherMode> {
    alloc::vec![LauncherMode::Apps, LauncherMode::Files, LauncherMode::Ai]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn new_launcher_surface_uses_left_panel() {
        let l = LauncherSurface::new(LauncherMode::Apps);
        assert_eq!(l.panel.side, PanelSide::Left);
    }

    #[test]
    fn new_launcher_starts_with_empty_query() {
        let l = LauncherSurface::new(LauncherMode::Apps);
        assert!(l.query.is_empty());
        assert!(l.search.text.is_empty());
    }

    #[test]
    fn new_launcher_search_is_focused_by_default() {
        let l = LauncherSurface::new(LauncherMode::Apps);
        assert!(l.search.focused);
    }

    #[test]
    fn mode_rail_is_64px_wide() {
        assert_eq!(ModeRail::width_px(), 64);
    }

    #[test]
    fn mode_rail_has_three_tabs() {
        let r = ModeRail::new(LauncherMode::Files);
        assert_eq!(r.nav.items.len(), 3);
        assert_eq!(r.nav.items[0].label, "Apps");
        assert_eq!(r.nav.items[1].label, "Files");
        assert_eq!(r.nav.items[2].label, "AI");
    }

    #[test]
    fn active_mode_is_marked() {
        let r = ModeRail::new(LauncherMode::Ai);
        assert!(r.nav.items[2].active);
        assert!(!r.nav.items[0].active);
    }

    #[test]
    fn mode_rail_is_vertical() {
        let r = ModeRail::new(LauncherMode::Apps);
        assert_eq!(r.nav.orientation, NavOrientation::Vertical);
    }

    #[test]
    fn search_box_default_height_is_40() {
        assert_eq!(SearchBox::default_height_px(), 40);
    }

    #[test]
    fn search_box_focused_uses_lavender_border() {
        let s = SearchBox::new("Search apps");
        let st = s.style();
        assert_eq!(st.border, Color::role(aether_design_tokens::Role::AccentLavenderStrong));
    }

    #[test]
    fn search_box_unfocused_uses_hairline() {
        let s = SearchBox::new("Search apps").blurred();
        let st = s.style();
        assert_eq!(st.border, Color::role(aether_design_tokens::Role::Hairline));
    }

    #[test]
    fn mode_placeholder_changes_per_mode() {
        let apps = LauncherMode::Apps.search_placeholder();
        let files = LauncherMode::Files.search_placeholder();
        let ai = LauncherMode::Ai.search_placeholder();
        assert_ne!(apps, files);
        assert_ne!(files, ai);
        assert_ne!(apps, ai);
    }

    #[test]
    fn surface_lays_out_mode_rail_on_left() {
        let l = LauncherSurface::new(LauncherMode::Apps).at(100, 200).with_size(400, 600);
        let rail = l.mode_rail_box();
        assert_eq!(rail.x, 100);
        assert_eq!(rail.y, 200);
        assert_eq!(rail.width, ModeRail::width_px());
        assert_eq!(rail.height, 600);
    }

    #[test]
    fn surface_lays_out_content_right_of_rail() {
        let l = LauncherSurface::new(LauncherMode::Apps).at(100, 200).with_size(400, 600);
        let c = l.content_box();
        assert_eq!(c.x, 100 + ModeRail::width_px() as i32);
        assert_eq!(c.width, 400 - ModeRail::width_px());
    }

    #[test]
    fn search_sits_at_top_of_content() {
        let l = LauncherSurface::new(LauncherMode::Apps).at(0, 0).with_size(400, 600);
        let s = l.search_box();
        assert!(s.y > 0);
        assert!(s.height > 0);
    }

    #[test]
    fn grid_sits_below_search() {
        let l = LauncherSurface::new(LauncherMode::Apps).at(0, 0).with_size(400, 600);
        let s = l.search_box();
        let g = l.grid_box();
        assert!(g.y >= s.bottom());
    }

    #[test]
    fn ai_launcher_accent_is_listening_pink() {
        assert_eq!(ai_launcher_accent(), Color::PASTEL_PINK);
    }

    #[test]
    fn default_mode_order_is_apps_files_ai() {
        let order = default_mode_order();
        assert_eq!(order.len(), 3);
        assert_eq!(order[0], LauncherMode::Apps);
        assert_eq!(order[1], LauncherMode::Files);
        assert_eq!(order[2], LauncherMode::Ai);
    }

    #[test]
    fn surface_query_propagates_to_search() {
        let l = LauncherSurface::new(LauncherMode::Apps).query("calc");
        assert_eq!(l.query, "calc");
        assert_eq!(l.search.text, "calc");
    }
}
