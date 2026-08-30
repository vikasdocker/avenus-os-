//! Command bar state — what mode the bar is in and
//! what the current text is.

use aether_design_tokens::Color;

/// What kind of thing the user is asking the command
/// bar to do. The three modes mirror the launcher's
/// three modes (apps / files / AI), but the AI mode
/// is the default because the command bar is the
/// type-to-AI surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CommandMode {
    /// Send the prompt to an installed app (the apps
    /// mode shows the picker).
    Apps,
    /// Search the file system (the files mode shows
    /// the indexer results).
    Files,
    /// Hand the prompt to the Aether agent. This is
    /// the default mode.
    Ai,
}

impl CommandMode {
    /// The mode's human-readable label. The renderer's
    /// tab row uses this for the tab title.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Apps => "Apps",
            Self::Files => "Files",
            Self::Ai => "AI",
        }
    }

    /// The mode's prompt placeholder. The renderer
    /// shows this when the prompt field is empty.
    #[must_use]
    pub const fn search_placeholder(self) -> &'static str {
        match self {
            Self::Apps => "Open an app or type a command",
            Self::Files => "Find a file",
            Self::Ai => "Ask Aether anything",
        }
    }

    /// The mode's accent color. The renderer's tab
    /// indicator uses this to tint the active tab.
    #[must_use]
    pub fn accent(self) -> Color {
        match self {
            Self::Apps => Color::PASTEL_BLUE,
            Self::Files => Color::PASTEL_MINT,
            Self::Ai => Color::PASTEL_LAVENDER,
        }
    }

    /// The set of all modes, in canonical display order.
    #[must_use]
    pub fn all() -> [Self; 3] {
        [Self::Apps, Self::Files, Self::Ai]
    }
}

/// The full state of the AI Command Bar at a single
/// moment in time: the active mode + the current text.
/// Mutated through `apply` with a `CommandAction`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandState {
    /// The active mode.
    pub mode: CommandMode,
    /// The current text in the prompt field. Empty
    /// string = no prompt.
    pub text: String,
}

impl CommandState {
    /// Construct a state in the AI mode with an empty
    /// prompt — the bar's startup state.
    #[must_use]
    pub fn new() -> Self {
        Self { mode: CommandMode::Ai, text: String::new() }
    }

    /// Construct a state in the given mode.
    #[must_use]
    pub fn with_mode(mode: CommandMode) -> Self {
        Self { mode, text: String::new() }
    }

    /// Construct a state with the given text.
    #[must_use]
    pub fn with_text(mode: CommandMode, text: impl Into<String>) -> Self {
        Self { mode, text: text.into() }
    }

    /// Whether the prompt is empty. The send button is
    /// disabled when this is true.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The number of characters in the prompt.
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len()
    }
}

impl Default for CommandState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_ai_mode() {
        let s = CommandState::new();
        assert_eq!(s.mode, CommandMode::Ai);
    }

    #[test]
    fn default_state_is_empty() {
        let s = CommandState::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn with_text_carries_text() {
        let s = CommandState::with_text(CommandMode::Ai, "hello");
        assert_eq!(s.text, "hello");
        assert_eq!(s.len(), 5);
        assert!(!s.is_empty());
    }

    #[test]
    fn with_mode_sets_mode() {
        let s = CommandState::with_mode(CommandMode::Files);
        assert_eq!(s.mode, CommandMode::Files);
    }

    #[test]
    fn all_returns_three_modes() {
        let m = CommandMode::all();
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn apps_label() {
        assert_eq!(CommandMode::Apps.label(), "Apps");
    }

    #[test]
    fn files_label() {
        assert_eq!(CommandMode::Files.label(), "Files");
    }

    #[test]
    fn ai_label() {
        assert_eq!(CommandMode::Ai.label(), "AI");
    }

    #[test]
    fn apps_placeholder() {
        assert_eq!(CommandMode::Apps.search_placeholder(), "Open an app or type a command");
    }

    #[test]
    fn files_placeholder() {
        assert_eq!(CommandMode::Files.search_placeholder(), "Find a file");
    }

    #[test]
    fn ai_placeholder() {
        assert_eq!(CommandMode::Ai.search_placeholder(), "Ask Aether anything");
    }

    #[test]
    fn apps_accent_is_blue() {
        assert_eq!(CommandMode::Apps.accent(), Color::PASTEL_BLUE);
    }

    #[test]
    fn files_accent_is_mint() {
        assert_eq!(CommandMode::Files.accent(), Color::PASTEL_MINT);
    }

    #[test]
    fn ai_accent_is_lavender() {
        assert_eq!(CommandMode::Ai.accent(), Color::PASTEL_LAVENDER);
    }

    #[test]
    fn mode_equality() {
        assert_eq!(CommandMode::Apps, CommandMode::Apps);
        assert_ne!(CommandMode::Apps, CommandMode::Files);
    }
}
