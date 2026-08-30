//! Command bar actions — user-driven events that
//! mutate the state.

/// A user-driven event on the AI Command Bar. The
/// renderer / input layer produces these from key
/// presses, mouse clicks, and voice events; the
/// `CommandView` consumes them through `apply`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CommandAction {
    /// Append a character to the prompt.
    TypeChar(char),
    /// Remove the last character from the prompt.
    Backspace,
    /// Remove the character before the cursor.
    DeletePrevWord,
    /// Clear the prompt entirely.
    ClearPrompt,
    /// Switch to a different mode.
    SwitchMode(crate::CommandMode),
    /// Move keyboard focus to the next region (tabs ->
    /// prompt -> send -> tabs).
    FocusNext,
    /// Move keyboard focus to the previous region.
    FocusPrev,
    /// Submit the current prompt. The view's
    /// `SubmitIntent` describes what the submit means
    /// given the current mode.
    Submit,
    /// Open the help / shortcut overlay.
    ShowHelp,
    /// Close the command bar (the renderer fades it
    /// out and the desktop / launcher return).
    Close,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::CommandMode;

    #[test]
    fn actions_construct() {
        let a = CommandAction::TypeChar('a');
        let b = CommandAction::Backspace;
        let c = CommandAction::SwitchMode(CommandMode::Files);
        assert_eq!(a, CommandAction::TypeChar('a'));
        assert_eq!(b, CommandAction::Backspace);
        assert_eq!(c, CommandAction::SwitchMode(CommandMode::Files));
    }

    #[test]
    fn actions_differ() {
        assert_ne!(CommandAction::TypeChar('a'), CommandAction::Backspace);
        assert_ne!(
            CommandAction::SwitchMode(CommandMode::Apps),
            CommandAction::SwitchMode(CommandMode::Files)
        );
    }
}
