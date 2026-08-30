//! Launcher content — the resolver that turns a mode +
//! query into a list of tiles.
//!
//! The launcher's job is to answer the question "what
//! should I show in the grid right now?" The answer
//! depends on the active mode and the search query:
//!
//!   * `Apps` mode with an empty query: every installed
//!     app tile.
//!   * `Apps` mode with a query: every installed app
//!     whose name (case-insensitively) contains the
//!     query. If the user has typed something that
//!     doesn't match any installed app, the resolver
//!     falls back to the Aether Store catalog (apps
//!     that are not yet installed).
//!   * `Files` mode: not implemented yet — the resolver
//!     returns an empty list. The file indexer will
//!     populate it in a later milestone.
//!   * `Ai` mode: the resolver returns no tiles; the
//!     search box becomes the primary surface and
//!     pressing Enter hands the query to the agent.
//!
//! The resolver is pure: it takes the mode, the query,
//! and a list of installed + catalog tiles, and returns
//! a `Vec<LauncherMatch>`. There is no IO.

extern crate alloc;

use aether_ui_components::{Launcher, LauncherTile};

use crate::mode::LauncherMode;

/// A single resolved match — a tile plus a relevance
/// hint (lower score = better match; the launcher
/// sorts ascending).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LauncherMatch {
    /// The matched tile.
    pub tile: LauncherTile,
    /// Relevance. `0` is a prefix match, `1` is a
    /// substring match, `2` is a fuzzy / catalog
    /// match. Renderers may render the score as a
    /// small "best match" badge.
    pub score: u8,
    /// Whether the match is in the user's installed
    /// set. Catalog (uninstalled) matches have
    /// `installed = false` so the renderer can show an
    /// "Install" badge.
    pub installed: bool,
}

impl LauncherMatch {
    /// Construct a match.
    #[must_use]
    pub fn new(tile: LauncherTile, score: u8, installed: bool) -> Self {
        Self { tile, score, installed }
    }
}

/// The launcher's content resolver. The caller wires it
/// to the installed-app set and the store catalog; the
/// resolver joins them by mode + query.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct LauncherContent {
    /// The installed apps. These are matched first.
    pub installed: alloc::vec::Vec<LauncherTile>,
    /// The store catalog (apps available but not yet
    /// installed). These are matched only when nothing
    /// in `installed` matches the query.
    pub catalog: alloc::vec::Vec<LauncherTile>,
}

impl LauncherContent {
    /// Construct an empty content resolver. The caller
    /// populates `installed` and `catalog` separately.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an installed tile.
    #[must_use]
    pub fn with_installed(mut self, tile: LauncherTile) -> Self {
        self.installed.push(tile);
        self
    }

    /// Add a catalog tile.
    #[must_use]
    pub fn with_catalog(mut self, tile: LauncherTile) -> Self {
        self.catalog.push(tile);
        self
    }

    /// Resolve the content for the given mode + query.
    /// The returned list is sorted by `score` (ascending)
    /// so the best match is at index 0.
    #[must_use]
    pub fn resolve(&self, mode: LauncherMode, query: &str) -> alloc::vec::Vec<LauncherMatch> {
        match mode {
            LauncherMode::Apps => self.resolve_apps(query),
            LauncherMode::Files => self.resolve_files(query),
            LauncherMode::Ai => alloc::vec::Vec::new(),
        }
    }

    /// Apps-mode resolver. The query, if any, is matched
    /// against the `name` field of every installed tile
    /// (case-insensitive). If nothing in `installed`
    /// matches, fall back to the catalog.
    fn resolve_apps(&self, query: &str) -> alloc::vec::Vec<LauncherMatch> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self
                .installed
                .iter()
                .cloned()
                .map(|t| LauncherMatch::new(t, 1, true))
                .collect();
        }
        let mut matches: alloc::vec::Vec<LauncherMatch> = self
            .installed
            .iter()
            .filter_map(|t| {
                let name = t.name.to_lowercase();
                if name.starts_with(&q) {
                    Some(LauncherMatch::new(t.clone(), 0, true))
                } else if name.contains(&q) {
                    Some(LauncherMatch::new(t.clone(), 1, true))
                } else {
                    None
                }
            })
            .collect();
        if matches.is_empty() {
            matches = self
                .catalog
                .iter()
                .filter_map(|t| {
                    let name = t.name.to_lowercase();
                    if name.contains(&q) {
                        Some(LauncherMatch::new(t.clone(), 2, false))
                    } else {
                        None
                    }
                })
                .collect();
        }
        matches.sort_by_key(|m| m.score);
        matches
    }

    /// Files-mode resolver. Returns an empty list for
    /// now — the file indexer will populate it later.
    fn resolve_files(&self, _query: &str) -> alloc::vec::Vec<LauncherMatch> {
        alloc::vec::Vec::new()
    }
}

/// Build a `Launcher` value from a content snapshot. The
/// `selected` index is clamped to the filtered list.
#[must_use]
pub fn launcher_from_matches(matches: &[LauncherMatch], query: &str, selected: usize) -> Launcher {
    let tiles: alloc::vec::Vec<LauncherTile> = matches.iter().map(|m| m.tile.clone()).collect();
    let selected = if tiles.is_empty() { 0 } else { selected.min(tiles.len() - 1) };
    Launcher::new()
        .query(query)
        .selected(selected)
        .with_tiles(tiles)
        .with_visible(true)
        .with_panel_size(0, 0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_design_tokens::Color;

    fn tile(app_id: &str, name: &str) -> LauncherTile {
        LauncherTile::new(app_id, name, Color::PASTEL_BLUE, Color::PASTEL_MINT)
    }

    #[test]
    fn empty_apps_query_returns_everything_installed() {
        let c = LauncherContent::new()
            .with_installed(tile("com.aether.notes", "Notes"))
            .with_installed(tile("com.aether.calc", "Calculator"));
        let m = c.resolve(LauncherMode::Apps, "");
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn empty_apps_query_preserves_order() {
        let c = LauncherContent::new()
            .with_installed(tile("a", "Alpha"))
            .with_installed(tile("b", "Beta"));
        let m = c.resolve(LauncherMode::Apps, "");
        assert_eq!(m[0].tile.app_id, "a");
        assert_eq!(m[1].tile.app_id, "b");
    }

    #[test]
    fn query_matches_prefix() {
        let c = LauncherContent::new()
            .with_installed(tile("com.aether.notes", "Notes"))
            .with_installed(tile("com.aether.calc", "Calculator"));
        let m = c.resolve(LauncherMode::Apps, "calc");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].tile.app_id, "com.aether.calc");
        assert_eq!(m[0].score, 0);
    }

    #[test]
    fn query_matches_substring() {
        let c = LauncherContent::new().with_installed(tile("com.aether.notes", "Aether Notes"));
        let m = c.resolve(LauncherMode::Apps, "notes");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].score, 1);
    }

    #[test]
    fn query_is_case_insensitive() {
        let c = LauncherContent::new().with_installed(tile("com.aether.notes", "Aether Notes"));
        let m = c.resolve(LauncherMode::Apps, "NOTES");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn prefix_match_outranks_substring_match() {
        let c = LauncherContent::new()
            .with_installed(tile("a", "Calculator Plus"))
            .with_installed(tile("b", "MegaCalc"));
        let m = c.resolve(LauncherMode::Apps, "calc");
        // Both match, but Calculator Plus starts with
        // "calc" — it sorts first.
        assert_eq!(m[0].tile.app_id, "a");
        assert_eq!(m[0].score, 0);
        assert_eq!(m[1].tile.app_id, "b");
        assert_eq!(m[1].score, 1);
    }

    #[test]
    fn no_match_falls_back_to_catalog() {
        let c = LauncherContent::new()
            .with_catalog(tile("com.store.unknown", "Mystery App"))
            .with_installed(tile("com.aether.notes", "Notes"));
        let m = c.resolve(LauncherMode::Apps, "mystery");
        assert_eq!(m.len(), 1);
        assert!(!m[0].installed);
        assert_eq!(m[0].score, 2);
    }

    #[test]
    fn installed_match_takes_priority_over_catalog() {
        let c = LauncherContent::new()
            .with_catalog(tile("com.store.calc", "Calculator (Pro)"))
            .with_installed(tile("com.aether.calc", "Calculator"));
        let m = c.resolve(LauncherMode::Apps, "calc");
        assert_eq!(m.len(), 1);
        assert!(m[0].installed);
        assert_eq!(m[0].tile.app_id, "com.aether.calc");
    }

    #[test]
    fn files_mode_returns_empty_for_now() {
        let c = LauncherContent::new().with_installed(tile("a", "Alpha"));
        let m = c.resolve(LauncherMode::Files, "alpha");
        assert!(m.is_empty());
    }

    #[test]
    fn ai_mode_returns_empty() {
        let c = LauncherContent::new().with_installed(tile("a", "Alpha"));
        let m = c.resolve(LauncherMode::Ai, "anything");
        assert!(m.is_empty());
    }

    #[test]
    fn query_is_trimmed() {
        let c = LauncherContent::new().with_installed(tile("a", "Calculator"));
        let m = c.resolve(LauncherMode::Apps, "   calc  ");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn empty_query_does_not_match_catalog() {
        let c = LauncherContent::new().with_catalog(tile("a", "Alpha"));
        let m = c.resolve(LauncherMode::Apps, "");
        assert!(m.is_empty());
    }

    #[test]
    fn launcher_from_matches_carries_query() {
        let c = LauncherContent::new().with_installed(tile("a", "Alpha"));
        let m = c.resolve(LauncherMode::Apps, "alp");
        let l = launcher_from_matches(&m, "alp", 0);
        assert_eq!(l.query, "alp");
        assert_eq!(l.tiles.len(), 1);
    }

    #[test]
    fn launcher_from_matches_clamps_selected_to_empty() {
        let l = launcher_from_matches(&[], "x", 5);
        assert_eq!(l.selected, 0);
    }
}
