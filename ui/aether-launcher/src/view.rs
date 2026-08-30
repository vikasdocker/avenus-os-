//! Launcher view — the resolved state of the launcher
//! at a single moment in time.
//!
//! A `LauncherView` is what the renderer consumes. It
//! carries the active mode, the resolved tile list, the
//! current search query, the selected index, and the
//! keyboard model that maps user input into a
//! `ViewAction`.
//!
//! The view is built once per frame by the launcher's
//! state machine:
//!
//! 1. The user typed or pressed a key.
//! 2. The state machine updated the `query` /
//!    `selected` / `mode`.
//! 3. The view ran the content resolver.
//! 4. The renderer reads the `LauncherView` and paints.

extern crate alloc;

use aether_ui_components::{Launcher, LauncherTile};

use crate::content::{LauncherContent, LauncherMatch};
use crate::mode::LauncherMode;

/// A user-driven action on the launcher. The renderer
/// receives these from the view (or directly from input
/// events) and routes them to the launcher's state
/// machine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ViewAction {
    /// Move the selection up by one tile. Wraps to the
    /// bottom if `wrap = true`.
    MoveUp {
        /// Whether to wrap.
        wrap: bool,
    },
    /// Move the selection down by one tile.
    MoveDown {
        /// Whether to wrap.
        wrap: bool,
    },
    /// Move the selection left by one column. The column
    /// count comes from the `Launcher::grid_columns`.
    MoveLeft {
        /// Whether to wrap.
        wrap: bool,
    },
    /// Move the selection right by one column.
    MoveRight {
        /// Whether to wrap.
        wrap: bool,
    },
    /// Append a character to the query.
    TypeChar(char),
    /// Remove the last character from the query.
    Backspace,
    /// Clear the query entirely.
    ClearQuery,
    /// Submit the current query. In `Apps` mode this
    /// launches the selected tile; in `Files` mode it
    /// opens the selected file; in `Ai` mode it hands
    /// the query to the agent.
    Submit,
    /// Switch to a different mode.
    SwitchMode(LauncherMode),
    /// Open the mode's help / settings.
    ShowHelp,
    /// Close the launcher. The renderer fades the panel
    /// out and the taskbar / desktop return.
    Close,
}

/// The launcher's resolved view. Built by
/// `LauncherView::build`; mutated through `apply` with
/// `ViewAction`s.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LauncherView {
    /// The active mode.
    pub mode: LauncherMode,
    /// The current search query.
    pub query: String,
    /// The selected index in the current filtered list.
    pub selected: usize,
    /// The resolved tile matches for the current mode +
    /// query.
    pub matches: alloc::vec::Vec<LauncherMatch>,
    /// Whether the launcher is currently visible.
    pub visible: bool,
}

impl LauncherView {
    /// Build a fresh view from the active mode, the
    /// current query, the content snapshot, and the
    /// previous selected index.
    #[must_use]
    pub fn build(
        mode: LauncherMode,
        query: &str,
        content: &LauncherContent,
        selected: usize,
    ) -> Self {
        let matches = content.resolve(mode, query);
        let selected = if matches.is_empty() { 0 } else { selected.min(matches.len() - 1) };
        Self { mode, query: String::from(query), selected, matches, visible: true }
    }

    /// Apply a `ViewAction` to the view, returning a new
    /// `LauncherView`. The content snapshot is needed
    /// for `TypeChar` / `Backspace` / `ClearQuery`
    /// because the resolved list changes.
    #[must_use]
    pub fn apply(&self, action: ViewAction, content: &LauncherContent) -> Self {
        match action {
            ViewAction::MoveUp { wrap } => self.move_selection(-1, wrap, content),
            ViewAction::MoveDown { wrap } => self.move_selection(1, wrap, content),
            ViewAction::MoveLeft { wrap } => self.move_columns(-1, wrap, content),
            ViewAction::MoveRight { wrap } => self.move_columns(1, wrap, content),
            ViewAction::TypeChar(c) => {
                let mut q = self.query.clone();
                q.push(c);
                Self::build(self.mode, &q, content, self.selected)
            }
            ViewAction::Backspace => {
                let mut q = self.query.clone();
                q.pop();
                Self::build(self.mode, &q, content, 0)
            }
            ViewAction::ClearQuery => Self::build(self.mode, "", content, 0),
            ViewAction::Submit => self.clone(),
            ViewAction::SwitchMode(m) => Self::build(m, &self.query, content, 0),
            ViewAction::ShowHelp => self.clone(),
            ViewAction::Close => Self { visible: false, ..self.clone() },
        }
    }

    fn move_selection(&self, delta: i32, wrap: bool, content: &LauncherContent) -> Self {
        if self.matches.is_empty() {
            return self.clone();
        }
        let len = self.matches.len() as i32;
        let cur = self.selected as i32;
        let next = if wrap {
            (cur + delta).rem_euclid(len) as usize
        } else {
            (cur + delta).clamp(0, len - 1) as usize
        };
        Self::build(self.mode, &self.query, content, next)
    }

    fn move_columns(&self, delta: i32, wrap: bool, content: &LauncherContent) -> Self {
        if self.matches.is_empty() {
            return self.clone();
        }
        let cols = Launcher::grid_columns() as i32;
        let len = self.matches.len() as i32;
        let cur = self.selected as i32;
        let next = if wrap {
            (cur + delta * cols).rem_euclid(len) as usize
        } else {
            (cur + delta * cols).clamp(0, len - 1) as usize
        };
        Self::build(self.mode, &self.query, content, next)
    }

    /// The currently selected match, if any. Returns
    /// `None` if the match list is empty.
    #[must_use]
    pub fn selected_match(&self) -> Option<&LauncherMatch> {
        self.matches.get(self.selected)
    }

    /// The currently selected tile, for the renderer.
    /// `LauncherView` is a higher-level abstraction than
    /// the `Launcher` component — the renderer bridges
    /// them by building a `Launcher` value with the
    /// resolved tiles + the selected index.
    #[must_use]
    pub fn to_launcher(&self) -> Launcher {
        let tiles: alloc::vec::Vec<LauncherTile> =
            self.matches.iter().map(|m| m.tile.clone()).collect();
        Launcher::new()
            .query(self.query.clone())
            .selected(self.selected)
            .with_tiles(tiles)
            .with_visible(self.visible)
            .with_panel_size(0, 0)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_design_tokens::Color;

    fn tile(app_id: &str, name: &str) -> LauncherTile {
        LauncherTile::new(app_id, name, Color::PASTEL_BLUE, Color::PASTEL_MINT)
    }

    fn content() -> LauncherContent {
        LauncherContent::new()
            .with_installed(tile("a", "Alpha"))
            .with_installed(tile("b", "Beta"))
            .with_installed(tile("c", "Gamma"))
            .with_installed(tile("d", "Delta"))
    }

    #[test]
    fn build_resolves_empty_query() {
        let v = LauncherView::build(LauncherMode::Apps, "", &content(), 0);
        assert_eq!(v.matches.len(), 4);
        assert_eq!(v.query, "");
    }

    #[test]
    fn build_clamps_selected() {
        let v = LauncherView::build(LauncherMode::Apps, "", &content(), 99);
        assert_eq!(v.selected, 3);
    }

    #[test]
    fn build_starts_visible() {
        let v = LauncherView::build(LauncherMode::Apps, "", &content(), 0);
        assert!(v.visible);
    }

    #[test]
    fn type_char_appends_and_refilters() {
        let v0 = LauncherView::build(LauncherMode::Apps, "", &content(), 0);
        let v1 = v0.apply(ViewAction::TypeChar('a'), &content());
        assert_eq!(v1.query, "a");
        // "a" matches Alpha, Beta, Gamma, Delta (all contain 'a' or 'A').
        assert!(!v1.matches.is_empty());
    }

    #[test]
    fn backspace_removes_last_char() {
        let v0 = LauncherView::build(LauncherMode::Apps, "al", &content(), 0);
        let v1 = v0.apply(ViewAction::Backspace, &content());
        assert_eq!(v1.query, "a");
    }

    #[test]
    fn clear_query_resets() {
        let v0 = LauncherView::build(LauncherMode::Apps, "al", &content(), 0);
        let v1 = v0.apply(ViewAction::ClearQuery, &content());
        assert_eq!(v1.query, "");
        assert_eq!(v1.matches.len(), 4);
    }

    #[test]
    fn move_down_increments_selection() {
        let v0 = LauncherView::build(LauncherMode::Apps, "", &content(), 1);
        let v1 = v0.apply(ViewAction::MoveDown { wrap: false }, &content());
        assert_eq!(v1.selected, 2);
    }

    #[test]
    fn move_down_clamps_at_end() {
        let v0 = LauncherView::build(LauncherMode::Apps, "", &content(), 3);
        let v1 = v0.apply(ViewAction::MoveDown { wrap: false }, &content());
        assert_eq!(v1.selected, 3);
    }

    #[test]
    fn move_down_with_wrap() {
        let v0 = LauncherView::build(LauncherMode::Apps, "", &content(), 3);
        let v1 = v0.apply(ViewAction::MoveDown { wrap: true }, &content());
        assert_eq!(v1.selected, 0);
    }

    #[test]
    fn move_right_steps_by_grid_columns() {
        let v0 = LauncherView::build(LauncherMode::Apps, "", &content(), 0);
        let v1 = v0.apply(ViewAction::MoveRight { wrap: false }, &content());
        // grid_columns is 3, so right from 0 lands on 3.
        assert_eq!(v1.selected, 3);
    }

    #[test]
    fn move_left_wraps_or_clamps() {
        let v0 = LauncherView::build(LauncherMode::Apps, "", &content(), 0);
        let v1 = v0.apply(ViewAction::MoveLeft { wrap: false }, &content());
        assert_eq!(v1.selected, 0);
        let v2 = v0.apply(ViewAction::MoveLeft { wrap: true }, &content());
        // 0 - 3 = -3, mod 4 = 1
        assert_eq!(v2.selected, 1);
    }

    #[test]
    fn switch_mode_resets_selection() {
        let v0 = LauncherView::build(LauncherMode::Apps, "", &content(), 2);
        let v1 = v0.apply(ViewAction::SwitchMode(LauncherMode::Files), &content());
        assert_eq!(v1.mode, LauncherMode::Files);
        assert_eq!(v1.selected, 0);
    }

    #[test]
    fn close_makes_invisible() {
        let v0 = LauncherView::build(LauncherMode::Apps, "", &content(), 0);
        let v1 = v0.apply(ViewAction::Close, &content());
        assert!(!v1.visible);
    }

    #[test]
    fn selected_match_returns_some() {
        let v = LauncherView::build(LauncherMode::Apps, "", &content(), 0);
        assert!(v.selected_match().is_some());
    }

    #[test]
    fn selected_match_returns_none_on_empty() {
        let v = LauncherView::build(LauncherMode::Files, "", &content(), 0);
        assert!(v.selected_match().is_none());
    }

    #[test]
    fn to_launcher_carries_query() {
        let v = LauncherView::build(LauncherMode::Apps, "", &content(), 0);
        let l = v.to_launcher();
        assert_eq!(l.query, "");
        assert_eq!(l.tiles.len(), 4);
    }
}
