//! Launcher mode — what kind of thing the user is
//! looking for.
//!
//! The launcher's three modes are **Apps**, **Files**,
//! and **AI**. The mode drives the content resolver
//! and the search-box placeholder.

use aether_ui_components::LauncherTile;

/// What the user is currently looking at in the launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LauncherMode {
    /// Installed applications (and the Aether Store
    /// catalog, when no app matches the query).
    Apps,
    /// Files on the local filesystem. The content
    /// resolver joins the indexer to surface matches.
    Files,
    /// The Aether assistant. Submitting the query in
    /// this mode hands the question to the agent.
    Ai,
}

impl LauncherMode {
    /// The mode's human-readable label. The renderer's
    /// mode rail uses this for the tab title.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Apps => "Apps",
            Self::Files => "Files",
            Self::Ai => "AI",
        }
    }

    /// The mode's search-box placeholder. The renderer
    /// shows this when the search field is empty.
    #[must_use]
    pub const fn search_placeholder(self) -> &'static str {
        match self {
            Self::Apps => "Search apps",
            Self::Files => "Search files",
            Self::Ai => "Ask Aether",
        }
    }

    /// The default accent color for the mode. Renderers
    /// can use this to tint the mode rail's active tab.
    /// The colors are pulled from the design tokens so
    /// they match the rest of the system.
    #[must_use]
    pub fn accent(self) -> aether_design_tokens::Color {
        use aether_design_tokens::Color;
        match self {
            // Apps = blue (the canonical "go" color).
            Self::Apps => Color::PASTEL_BLUE,
            // Files = mint (calm, document-like).
            Self::Files => Color::PASTEL_MINT,
            // AI = lavender (the AI identity).
            Self::Ai => Color::PASTEL_LAVENDER,
        }
    }

    /// The set of all modes, in canonical display order.
    /// The mode rail iterates this.
    #[must_use]
    pub fn all() -> [Self; 3] {
        [Self::Apps, Self::Files, Self::Ai]
    }
}

/// A `LauncherTile` is what the launcher grid shows. The
/// mode doesn't change the tile type; it changes the
/// *source* of the tiles.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModeTile {
    /// The wrapped `LauncherTile`.
    pub tile: LauncherTile,
    /// The mode the tile came from. Renderers may use
    /// this to render a mode-specific badge.
    pub mode: LauncherMode,
}

impl ModeTile {
    /// Construct a mode tile.
    #[must_use]
    pub fn new(tile: LauncherTile, mode: LauncherMode) -> Self {
        Self { tile, mode }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_design_tokens::Color;

    #[test]
    fn all_returns_three_modes() {
        let m = LauncherMode::all();
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn apps_label_is_apps() {
        assert_eq!(LauncherMode::Apps.label(), "Apps");
    }

    #[test]
    fn files_label_is_files() {
        assert_eq!(LauncherMode::Files.label(), "Files");
    }

    #[test]
    fn ai_label_is_ai() {
        assert_eq!(LauncherMode::Ai.label(), "AI");
    }

    #[test]
    fn apps_search_placeholder() {
        assert_eq!(LauncherMode::Apps.search_placeholder(), "Search apps");
    }

    #[test]
    fn files_search_placeholder() {
        assert_eq!(LauncherMode::Files.search_placeholder(), "Search files");
    }

    #[test]
    fn ai_search_placeholder_is_ask_aether() {
        assert_eq!(LauncherMode::Ai.search_placeholder(), "Ask Aether");
    }

    #[test]
    fn apps_accent_is_blue() {
        assert_eq!(LauncherMode::Apps.accent(), Color::PASTEL_BLUE);
    }

    #[test]
    fn files_accent_is_mint() {
        assert_eq!(LauncherMode::Files.accent(), Color::PASTEL_MINT);
    }

    #[test]
    fn ai_accent_is_lavender() {
        assert_eq!(LauncherMode::Ai.accent(), Color::PASTEL_LAVENDER);
    }

    #[test]
    fn mode_equality() {
        assert_eq!(LauncherMode::Apps, LauncherMode::Apps);
        assert_ne!(LauncherMode::Apps, LauncherMode::Files);
    }
}
