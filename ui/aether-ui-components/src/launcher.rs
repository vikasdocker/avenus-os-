//! Launcher — the AI-first application launcher.
//!
//! The launcher is a `Panel::Left` of `LauncherTile`s.
//! Each tile carries the app's id, name, and a
//! 4-pastel gradient (the §12 "soft gradients" rule
//! for app icons). The launcher also carries the
//! current search query so the renderer can dim
//! non-matching tiles.
//!
//! Tile width: 96 px. Tile height: 96 px. Tile padding:
//! `Md`. The grid is 3 columns wide.

extern crate alloc;

use aether_design_tokens::{Color, Radius, Role, Spacing, TypeScale};

use crate::{Component, ComponentStyle, Insets, LayoutBox, Panel, PanelSide};

/// One tile in the launcher's grid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LauncherTile {
    /// The app id (reverse-DNS).
    pub app_id: String,
    /// The display name.
    pub name: String,
    /// The tile's primary pastel color (the
    /// gradient's "top" stop).
    pub gradient_top: Color,
    /// The tile's secondary pastel color (the
    /// gradient's "bottom" stop).
    pub gradient_bottom: Color,
    /// Whether the tile is currently focused.
    pub focused: bool,
    /// Whether the app is currently installed.
    pub installed: bool,
    /// Whether the app is currently running.
    pub running: bool,
}

impl LauncherTile {
    /// Construct a tile.
    #[must_use]
    pub fn new(
        app_id: impl Into<String>,
        name: impl Into<String>,
        gradient_top: Color,
        gradient_bottom: Color,
    ) -> Self {
        Self {
            app_id: app_id.into(),
            name: name.into(),
            gradient_top,
            gradient_bottom,
            focused: false,
            installed: true,
            running: false,
        }
    }

    /// Mark focused.
    #[must_use]
    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }

    /// Mark not yet installed.
    #[must_use]
    pub fn not_installed(mut self) -> Self {
        self.installed = false;
        self
    }

    /// Mark running.
    #[must_use]
    pub fn running(mut self) -> Self {
        self.running = true;
        self
    }
}

/// The launcher.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Launcher {
    /// The left panel that holds the chrome.
    pub panel: Panel,
    /// The tiles, in display order (top-to-bottom,
    /// left-to-right).
    pub tiles: alloc::vec::Vec<LauncherTile>,
    /// The current search query. Tiles whose name
    /// doesn't contain this (case-insensitive) are
    /// rendered dim.
    pub query: String,
    /// The selected index in the current filtered
    /// set; the renderer draws a focus ring on it.
    pub selected: usize,
}

impl Launcher {
    /// Construct a launcher with the §12 default
    /// left-anchored panel.
    #[must_use]
    pub fn new() -> Self {
        let panel = Panel::new(PanelSide::Left)
            .with_size(Self::default_width_px(), 0)
            .with_padding(Insets::even(Spacing::Md.px()));
        Self { panel, tiles: alloc::vec::Vec::new(), query: String::new(), selected: 0 }
    }

    /// Add a tile.
    #[must_use]
    pub fn push(mut self, tile: LauncherTile) -> Self {
        self.tiles.push(tile);
        self
    }

    /// Set the tile list wholesale. Used by the launcher
    /// surface when the content resolver returns a
    /// fresh set of matches.
    #[must_use]
    pub fn with_tiles(mut self, tiles: alloc::vec::Vec<LauncherTile>) -> Self {
        self.tiles = tiles;
        self
    }

    /// Override the backing panel's size. The launcher
    /// surface sets this to the resolved grid box
    /// dimensions; the underlying `Panel` carries the
    /// size for the renderer's anchor pass.
    #[must_use]
    pub fn with_panel_size(mut self, width: u32, height: u32) -> Self {
        self.panel = self.panel.with_size(width, height);
        self
    }

    /// Override the backing panel's visibility. The
    /// launcher surface sets `visible = false` when
    /// the user has dismissed the launcher.
    #[must_use]
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.panel.visible = visible;
        self
    }

    /// Set the search query.
    #[must_use]
    pub fn query(mut self, q: impl Into<String>) -> Self {
        self.query = q.into();
        self
    }

    /// Set the selected index.
    #[must_use]
    pub fn selected(mut self, idx: usize) -> Self {
        self.selected = idx;
        self
    }

    /// §12 default launcher width: 3 columns of 96-px
    /// tiles + 4 * Md padding.
    #[must_use]
    pub fn default_width_px() -> u32 {
        3 * Self::tile_size_px() + 4 * Spacing::Md.px_u32()
    }

    /// Per-tile size in pixels.
    #[must_use]
    pub fn tile_size_px() -> u32 {
        96
    }

    /// Number of columns in the grid.
    #[must_use]
    pub fn grid_columns() -> u32 {
        3
    }

    /// The layout box for a specific tile index.
    #[must_use]
    pub fn tile_layout(&self, index: usize) -> LayoutBox {
        let col = (index as u32) % Self::grid_columns();
        let row = (index as u32) / Self::grid_columns();
        let pad = Spacing::Md.px();
        let size = Self::tile_size_px();
        LayoutBox::new(
            self.panel.origin.0 + Spacing::Md.px() + (col as i32) * (size as i32 + pad),
            self.panel.origin.1 + Spacing::Md.px() + (row as i32) * (size as i32 + pad),
            size,
            size,
        )
    }

    /// Whether a tile matches the current search query.
    /// An empty query matches every tile.
    #[must_use]
    pub fn tile_matches(&self, index: usize) -> bool {
        if self.query.is_empty() {
            return true;
        }
        self.tiles
            .get(index)
            .map(|t| t.name.to_lowercase().contains(&self.query.to_lowercase()))
            .unwrap_or(false)
    }
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Launcher {
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

/// The type scale the tile's label uses.
#[must_use]
pub fn tile_label_type() -> TypeScale {
    TypeScale::Caption
}

/// The corner radius of a launcher tile.
#[must_use]
pub fn tile_radius() -> Radius {
    Radius::Lg
}

/// The focus-ring color for the selected tile.
#[must_use]
pub fn tile_focus_color() -> Color {
    Color::role(Role::AccentLavenderStrong)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn sample_tile() -> LauncherTile {
        LauncherTile::new(
            "com.example.calc",
            "Aether Calculator",
            Color::PASTEL_PINK,
            Color::PASTEL_PEACH,
        )
    }

    #[test]
    fn default_launcher_is_left_anchored() {
        let l = Launcher::new();
        assert_eq!(l.panel.side, PanelSide::Left);
    }

    #[test]
    fn default_launcher_is_visible() {
        let l = Launcher::new();
        assert!(l.panel.visible);
    }

    #[test]
    fn default_query_is_empty() {
        let l = Launcher::new();
        assert!(l.query.is_empty());
    }

    #[test]
    fn default_selected_is_zero() {
        let l = Launcher::new();
        assert_eq!(l.selected, 0);
    }

    #[test]
    fn grid_columns_is_3() {
        assert_eq!(Launcher::grid_columns(), 3);
    }

    #[test]
    fn tile_size_is_96() {
        assert_eq!(Launcher::tile_size_px(), 96);
    }

    #[test]
    fn default_width_is_three_columns() {
        let expected = 3 * 96 + 4 * Spacing::Md.px_u32();
        assert_eq!(Launcher::default_width_px(), expected);
    }

    #[test]
    fn tile_layout_first_tile() {
        let l = Launcher::new().push(sample_tile());
        let box0 = l.tile_layout(0);
        assert_eq!(box0.x, Spacing::Md.px());
        assert_eq!(box0.y, Spacing::Md.px());
        assert_eq!(box0.width, 96);
        assert_eq!(box0.height, 96);
    }

    #[test]
    fn tile_layout_third_tile_wraps_to_second_row() {
        let l = Launcher::new().push(sample_tile()).push(sample_tile()).push(sample_tile());
        let box2 = l.tile_layout(2);
        // Column 2, row 0 (third tile, but 0-indexed)
        assert_eq!(box2.x, Spacing::Md.px() + 2 * (96 + Spacing::Md.px()));
        assert_eq!(box2.y, Spacing::Md.px());
    }

    #[test]
    fn tile_layout_fourth_tile_wraps() {
        let l = Launcher::new()
            .push(sample_tile())
            .push(sample_tile())
            .push(sample_tile())
            .push(sample_tile());
        let box3 = l.tile_layout(3);
        // Column 0, row 1
        assert_eq!(box3.x, Spacing::Md.px());
        assert_eq!(box3.y, Spacing::Md.px() + 96 + Spacing::Md.px());
    }

    #[test]
    fn empty_query_matches_every_tile() {
        let l = Launcher::new().push(sample_tile()).push(sample_tile());
        assert!(l.tile_matches(0));
        assert!(l.tile_matches(1));
    }

    #[test]
    fn query_filters_by_name_case_insensitive() {
        let l = Launcher::new().push(sample_tile()).query("CALC");
        assert!(l.tile_matches(0));
    }

    #[test]
    fn query_misses_non_matching_tile() {
        let l = Launcher::new().push(sample_tile()).query("notes");
        assert!(!l.tile_matches(0));
    }

    #[test]
    fn out_of_bounds_query_returns_false() {
        let l = Launcher::new().push(sample_tile()).query("c");
        assert!(!l.tile_matches(5));
    }

    #[test]
    fn tile_label_uses_caption_type() {
        assert_eq!(tile_label_type(), TypeScale::Caption);
    }

    #[test]
    fn tile_radius_is_lg() {
        assert_eq!(tile_radius(), Radius::Lg);
    }

    #[test]
    fn tile_focus_color_is_lavender_strong() {
        assert_eq!(tile_focus_color(), Color::role(Role::AccentLavenderStrong));
    }

    #[test]
    fn running_flag_persists() {
        let l = Launcher::new().push(sample_tile().running());
        assert!(l.tiles[0].running);
    }

    #[test]
    fn not_installed_flag_persists() {
        let l = Launcher::new().push(sample_tile().not_installed());
        assert!(!l.tiles[0].installed);
    }
}
