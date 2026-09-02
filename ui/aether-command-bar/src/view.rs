//! Command bar view — the resolved state of the bar at
//! a single moment in time.
//!
//! The view is what the renderer consumes. It carries
//! the active mode, the current text, which region has
//! focus, and what submitting the prompt would do.

extern crate alloc;

use crate::action::CommandAction;
use crate::state::{CommandMode, CommandState};

/// What region of the command bar has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CommandFocus {
    /// The mode tabs row.
    Tabs,
    /// The prompt field. This is the default focus.
    #[default]
    Prompt,
    /// The send button.
    Send,
}

/// What submitting the current prompt would do. The
/// renderer / app router dispatches on this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SubmitIntent {
    /// The mode + text would launch an app. The
    /// router uses the text as the search query and
    /// opens the top match.
    LaunchApp,
    /// The mode + text would open a file. The router
    /// uses the text as a path or a search query and
    /// opens the match in the default handler.
    OpenFile,
    /// The mode + text would hand the prompt to the
    /// agent. The router forwards the text to
    /// `aether-agentd` over IPC.
    AskAgent,
    /// The prompt is empty. The submit is a no-op
    /// (the send button is disabled, but a programmatic
    /// submit still resolves to this).
    Noop,
}

/// The resolved view of the command bar. Built by
/// `CommandView::build`; mutated through `apply` with
/// `CommandAction`s.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandView {
    /// The current state.
    pub state: CommandState,
    /// Which region has keyboard focus.
    pub focus: CommandFocus,
    /// Whether the bar is currently visible.
    pub visible: bool,
}

impl CommandView {
    /// Build a fresh view from the given state.
    #[must_use]
    pub fn build(state: CommandState) -> Self {
        Self { state, focus: CommandFocus::Prompt, visible: true }
    }

    /// Apply an action. Returns the new view.
    #[must_use]
    pub fn apply(&self, action: CommandAction) -> Self {
        match action {
            CommandAction::TypeChar(c) => self.type_char(c),
            CommandAction::Backspace => self.backspace(),
            CommandAction::DeletePrevWord => self.delete_prev_word(),
            CommandAction::ClearPrompt => self.clear_prompt(),
            CommandAction::SwitchMode(m) => Self {
                state: CommandState { mode: m, text: self.state.text.clone() },
                focus: self.focus,
                visible: self.visible,
            },
            CommandAction::FocusNext => {
                Self { state: self.state.clone(), focus: self.focus.next(), visible: self.visible }
            }
            CommandAction::FocusPrev => {
                Self { state: self.state.clone(), focus: self.focus.prev(), visible: self.visible }
            }
            CommandAction::Submit => self.clone(),
            CommandAction::ShowHelp => self.clone(),
            CommandAction::Close => Self { visible: false, ..self.clone() },
        }
    }

    /// What submitting the current prompt would do.
    #[must_use]
    pub fn submit_intent(&self) -> SubmitIntent {
        if self.state.is_empty() {
            return SubmitIntent::Noop;
        }
        match self.state.mode {
            CommandMode::Apps => SubmitIntent::LaunchApp,
            CommandMode::Files => SubmitIntent::OpenFile,
            CommandMode::Ai => SubmitIntent::AskAgent,
        }
    }

    /// Whether the send button is enabled.
    #[must_use]
    pub fn send_enabled(&self) -> bool {
        !self.state.is_empty()
    }

    /// Convenience constructor for a typed character
    /// event.
    fn type_char(&self, c: char) -> Self {
        let mut text = self.state.text.clone();
        text.push(c);
        Self {
            state: CommandState { mode: self.state.mode, text },
            focus: self.focus,
            visible: self.visible,
        }
    }

    fn backspace(&self) -> Self {
        let mut text = self.state.text.clone();
        text.pop();
        Self {
            state: CommandState { mode: self.state.mode, text },
            focus: self.focus,
            visible: self.visible,
        }
    }

    fn clear_prompt(&self) -> Self {
        Self {
            state: CommandState { mode: self.state.mode, text: String::new() },
            focus: self.focus,
            visible: self.visible,
        }
    }

    fn delete_prev_word(&self) -> Self {
        let mut text = self.state.text.clone();
        // Pop a trailing run of whitespace, then a run
        // of non-whitespace.
        while text.chars().last().is_some_and(char::is_whitespace) {
            text.pop();
        }
        while text.chars().last().is_some_and(|c| !c.is_whitespace()) {
            text.pop();
        }
        Self {
            state: CommandState { mode: self.state.mode, text },
            focus: self.focus,
            visible: self.visible,
        }
    }
}

impl CommandFocus {
    /// The next region in the focus order
    /// (Tabs -> Prompt -> Send -> Tabs).
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Tabs => Self::Prompt,
            Self::Prompt => Self::Send,
            Self::Send => Self::Tabs,
        }
    }

    /// The previous region in the focus order
    /// (Send -> Prompt -> Tabs -> Send).
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::Send => Self::Prompt,
            Self::Prompt => Self::Tabs,
            Self::Tabs => Self::Send,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn default_focus_is_prompt() {
        let v = CommandView::build(CommandState::new());
        assert_eq!(v.focus, CommandFocus::Prompt);
    }

    #[test]
    fn build_carries_state() {
        let v = CommandView::build(CommandState::with_text(CommandMode::Ai, "hello"));
        assert_eq!(v.state.text, "hello");
    }

    #[test]
    fn type_char_appends() {
        let v0 = CommandView::build(CommandState::new());
        let v1 = v0.apply(CommandAction::TypeChar('h'));
        let v2 = v1.apply(CommandAction::TypeChar('i'));
        assert_eq!(v2.state.text, "hi");
    }

    #[test]
    fn backspace_removes_last() {
        let v0 = CommandView::build(CommandState::with_text(CommandMode::Ai, "hello"));
        let v1 = v0.apply(CommandAction::Backspace);
        assert_eq!(v1.state.text, "hell");
    }

    #[test]
    fn clear_prompt_resets_text() {
        let v0 = CommandView::build(CommandState::with_text(CommandMode::Ai, "hello"));
        let v1 = v0.apply(CommandAction::ClearPrompt);
        assert_eq!(v1.state.text, "");
    }

    #[test]
    fn switch_mode_keeps_text() {
        let v0 = CommandView::build(CommandState::with_text(CommandMode::Ai, "hello"));
        let v1 = v0.apply(CommandAction::SwitchMode(CommandMode::Files));
        assert_eq!(v1.state.mode, CommandMode::Files);
        assert_eq!(v1.state.text, "hello");
    }

    #[test]
    fn focus_next_cycles() {
        let v0 = CommandView::build(CommandState::new());
        let v1 = v0.apply(CommandAction::FocusNext);
        assert_eq!(v1.focus, CommandFocus::Send);
        let v2 = v1.apply(CommandAction::FocusNext);
        assert_eq!(v2.focus, CommandFocus::Tabs);
    }

    #[test]
    fn focus_prev_cycles() {
        let v0 = CommandView::build(CommandState::new());
        let v1 = v0.apply(CommandAction::FocusPrev);
        assert_eq!(v1.focus, CommandFocus::Tabs);
    }

    #[test]
    fn close_makes_invisible() {
        let v0 = CommandView::build(CommandState::new());
        let v1 = v0.apply(CommandAction::Close);
        assert!(!v1.visible);
    }

    #[test]
    fn submit_intent_ask_agent() {
        let v = CommandView::build(CommandState::with_text(CommandMode::Ai, "what time?"));
        assert_eq!(v.submit_intent(), SubmitIntent::AskAgent);
    }

    #[test]
    fn submit_intent_launch_app() {
        let v = CommandView::build(CommandState::with_text(CommandMode::Apps, "calc"));
        assert_eq!(v.submit_intent(), SubmitIntent::LaunchApp);
    }

    #[test]
    fn submit_intent_open_file() {
        let v = CommandView::build(CommandState::with_text(CommandMode::Files, "readme"));
        assert_eq!(v.submit_intent(), SubmitIntent::OpenFile);
    }

    #[test]
    fn submit_intent_noop_on_empty() {
        let v = CommandView::build(CommandState::new());
        assert_eq!(v.submit_intent(), SubmitIntent::Noop);
    }

    #[test]
    fn send_disabled_on_empty() {
        let v = CommandView::build(CommandState::new());
        assert!(!v.send_enabled());
    }

    #[test]
    fn send_enabled_with_text() {
        let v = CommandView::build(CommandState::with_text(CommandMode::Ai, "x"));
        assert!(v.send_enabled());
    }

    #[test]
    fn delete_prev_word_removes_one_word() {
        let v0 = CommandView::build(CommandState::with_text(CommandMode::Ai, "hello world"));
        let v1 = v0.apply(CommandAction::DeletePrevWord);
        assert_eq!(v1.state.text, "hello ");
    }

    #[test]
    fn delete_prev_word_handles_multiple_spaces() {
        let v0 = CommandView::build(CommandState::with_text(CommandMode::Ai, "hello   world"));
        let v1 = v0.apply(CommandAction::DeletePrevWord);
        assert_eq!(v1.state.text, "hello   ");
    }
}
