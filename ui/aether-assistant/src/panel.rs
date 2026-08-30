//! Assistant Panel — the always-on sidebar that shows
//! the agent's current state, a short history, and a
//! quick prompt.
//!
//! §12: the AI must feel like part of the OS. The
//! Assistant Panel is the user's persistent
//! awareness of the agent. It docks to the right
//! edge of the screen (Panel::Right at the §12
//! default of 360 px) and reads its colors from
//! `AiVisualState` so the agent's visual identity
//! stays consistent across surfaces.
//!
//! The panel carries three layers:
//!
//! - `AssistantPanelState` — the resolved state at
//!   one moment in time (current `AiVisualState`,
//!   recent messages, draft prompt).
//! - `AssistantMessage` — one entry in the recent
//!   history (a `Role` + text + timestamp).
//! - `AssistantPanel` — the composing surface that
//!   the renderer reads. Layout: header (state +
//!   accent), message list (last N), quick-prompt
//!   input at the bottom.

use aether_design_tokens::{AiVisualState, Color, Spacing};
use aether_ui_components::{
    Component, ComponentStyle, Insets, LayoutBox, Panel, PanelSide,
};

use alloc::string::String;
use alloc::vec::Vec;

/// Who produced an `AssistantMessage` — user or AI.
/// The renderer colors the two roles differently
/// (user = right-aligned neutral, AI = left-aligned
/// with the current state accent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AssistantRole {
    /// A user turn.
    User,
    /// An AI turn.
    Assistant,
}

impl AssistantRole {
    /// Whether this role is right-aligned in the
    /// history list. The user role aligns right;
    /// the assistant role aligns left.
    #[must_use]
    pub const fn right_aligned(self) -> bool {
        matches!(self, Self::User)
    }
}

/// A single turn in the assistant's recent history.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssistantMessage {
    /// Who said it.
    pub role: AssistantRole,
    /// The message text.
    pub text: String,
    /// A monotonically increasing turn index. The
    /// renderer orders by this; the panel itself
    /// doesn't re-sort.
    pub index: u32,
}

impl AssistantMessage {
    /// Construct a user turn at the given index.
    #[must_use]
    pub fn user(text: impl Into<String>, index: u32) -> Self {
        Self { role: AssistantRole::User, text: text.into(), index }
    }

    /// Construct an assistant turn at the given index.
    #[must_use]
    pub fn assistant(text: impl Into<String>, index: u32) -> Self {
        Self { role: AssistantRole::Assistant, text: text.into(), index }
    }
}

/// The full state of the assistant panel at one
/// moment in time. The renderer reads this and
/// produces a frame.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssistantPanelState {
    /// The agent's current visual state. Drives
    /// the panel's accent color and the header
    /// label.
    pub state: AiVisualState,
    /// The recent history, in index order (oldest
    /// first). The renderer is free to clip to
    /// the most recent N.
    pub history: Vec<AssistantMessage>,
    /// The current draft in the quick-prompt input.
    /// Empty string = empty.
    pub draft: String,
    /// The next message index to assign. Increment
    /// when adding a new turn.
    pub next_index: u32,
}

impl AssistantPanelState {
    /// Construct a fresh state in `AiVisualState::Idle`
    /// with an empty history and an empty draft.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AiVisualState::Idle,
            history: Vec::new(),
            draft: String::new(),
            next_index: 0,
        }
    }

    /// Override the AI state.
    #[must_use]
    pub fn with_state(mut self, state: AiVisualState) -> Self {
        self.state = state;
        self
    }

    /// Override the history.
    #[must_use]
    pub fn with_history(mut self, history: Vec<AssistantMessage>) -> Self {
        self.history = history;
        self
    }

    /// Override the draft.
    #[must_use]
    pub fn with_draft(mut self, draft: impl Into<String>) -> Self {
        self.draft = draft.into();
        self
    }

    /// Set the next message index.
    #[must_use]
    pub fn with_next_index(mut self, idx: u32) -> Self {
        self.next_index = idx;
        self
    }

    /// Append a user turn, incrementing `next_index`.
    #[must_use]
    pub fn push_user(mut self, text: impl Into<String>) -> Self {
        let i = self.next_index;
        self.next_index = i.wrapping_add(1);
        self.history.push(AssistantMessage::user(text, i));
        self
    }

    /// Append an assistant turn, incrementing
    /// `next_index`.
    #[must_use]
    pub fn push_assistant(mut self, text: impl Into<String>) -> Self {
        let i = self.next_index;
        self.next_index = i.wrapping_add(1);
        self.history.push(AssistantMessage::assistant(text, i));
        self
    }

    /// Set the AI state in place.
    pub fn set_state(&mut self, state: AiVisualState) {
        self.state = state;
    }

    /// Set the draft in place.
    pub fn set_draft(&mut self, draft: impl Into<String>) {
        self.draft = draft.into();
    }

    /// The accent color the panel should use, given
    /// the current AI state.
    #[must_use]
    pub fn accent(&self) -> Color {
        self.state.color()
    }
}

impl Default for AssistantPanelState {
    fn default() -> Self {
        Self::new()
    }
}

/// The assistant panel surface — the composing type
/// the renderer reads. A `Panel::Right` sidebar
/// carrying the AI state, recent history, and
/// quick-prompt input.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssistantPanel {
    /// The backing right-side panel.
    pub panel: Panel,
    /// The resolved state at this frame.
    pub state: AssistantPanelState,
}

impl AssistantPanel {
    /// The §12 default panel width in pixels.
    pub const DEFAULT_WIDTH_PX: u32 = 360;

    /// Construct a fresh assistant panel in
    /// `AiVisualState::Idle`.
    #[must_use]
    pub fn new() -> Self {
        let panel = Panel::new(PanelSide::Right).with_size(Self::DEFAULT_WIDTH_PX, 720);
        Self { panel, state: AssistantPanelState::new() }
    }

    /// Override the state.
    #[must_use]
    pub fn with_state(mut self, state: AssistantPanelState) -> Self {
        self.state = state;
        self
    }

    /// Set the panel's height in pixels.
    #[must_use]
    pub fn with_height(mut self, h: u32) -> Self {
        self.panel = self.panel.with_size(Self::DEFAULT_WIDTH_PX, h);
        self
    }

    /// Set the panel's origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.panel = self.panel.at(x, y);
        self
    }

    /// The header region: a 48-px-tall strip carrying
    /// the state label and the state accent dot.
    #[must_use]
    pub fn header_box(&self) -> LayoutBox {
        LayoutBox::new(
            self.panel.origin.0,
            self.panel.origin.1,
            self.panel.width,
            48,
        )
    }

    /// The history region: the area between the
    /// header and the quick-prompt input. Sized to
    /// whatever's left.
    #[must_use]
    pub fn history_box(&self) -> LayoutBox {
        let header = self.header_box();
        let prompt = self.prompt_box();
        let y = header.bottom();
        let height = (prompt.y - y).max(0) as u32;
        LayoutBox::new(self.panel.origin.0, y, self.panel.width, height)
    }

    /// The quick-prompt region: a 56-px-tall strip at
    /// the bottom of the panel, with a mode-style
    /// input field and a send button.
    #[must_use]
    pub fn prompt_box(&self) -> LayoutBox {
        let pad = Spacing::Md.px_u32();
        let h = 56;
        let y = self.panel.origin.1 + self.panel.height as i32 - h as i32 - pad as i32;
        LayoutBox::new(self.panel.origin.0, y, self.panel.width, h)
    }
}

impl Default for AssistantPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AssistantPanel {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn role_right_aligned() {
        assert!(AssistantRole::User.right_aligned());
        assert!(!AssistantRole::Assistant.right_aligned());
    }

    #[test]
    fn message_user_constructor() {
        let m = AssistantMessage::user("hi", 0);
        assert_eq!(m.role, AssistantRole::User);
        assert_eq!(m.text, "hi");
        assert_eq!(m.index, 0);
    }

    #[test]
    fn message_assistant_constructor() {
        let m = AssistantMessage::assistant("hello", 3);
        assert_eq!(m.role, AssistantRole::Assistant);
        assert_eq!(m.text, "hello");
        assert_eq!(m.index, 3);
    }

    #[test]
    fn state_default_is_idle() {
        let s = AssistantPanelState::new();
        assert_eq!(s.state, AiVisualState::Idle);
        assert!(s.history.is_empty());
        assert!(s.draft.is_empty());
        assert_eq!(s.next_index, 0);
    }

    #[test]
    fn state_push_user_increments_index() {
        let s = AssistantPanelState::new().push_user("hi");
        assert_eq!(s.history.len(), 1);
        assert_eq!(s.next_index, 1);
        assert_eq!(s.history[0].role, AssistantRole::User);
    }

    #[test]
    fn state_push_assistant_increments_index() {
        let s = AssistantPanelState::new()
            .push_user("hi")
            .push_assistant("hello");
        assert_eq!(s.history.len(), 2);
        assert_eq!(s.next_index, 2);
        assert_eq!(s.history[0].role, AssistantRole::User);
        assert_eq!(s.history[1].role, AssistantRole::Assistant);
    }

    #[test]
    fn state_accent_matches_visual_state_color() {
        let s = AssistantPanelState::new().with_state(AiVisualState::Listening);
        assert_eq!(s.accent(), AiVisualState::Listening.color());
    }

    #[test]
    fn state_set_state_mutates() {
        let mut s = AssistantPanelState::new();
        s.set_state(AiVisualState::Working);
        assert_eq!(s.state, AiVisualState::Working);
    }

    #[test]
    fn state_set_draft_mutates() {
        let mut s = AssistantPanelState::new();
        s.set_draft("hello world");
        assert_eq!(s.draft, "hello world");
    }

    #[test]
    fn panel_new_is_right_side() {
        let p = AssistantPanel::new();
        assert_eq!(p.panel.side, PanelSide::Right);
    }

    #[test]
    fn panel_default_width_is_360() {
        let p = AssistantPanel::new();
        assert_eq!(p.panel.width, 360);
    }

    #[test]
    fn panel_header_is_48px() {
        let p = AssistantPanel::new().with_height(800);
        let h = p.header_box();
        assert_eq!(h.height, 48);
    }

    #[test]
    fn panel_prompt_is_56px() {
        let p = AssistantPanel::new().with_height(800);
        let q = p.prompt_box();
        assert_eq!(q.height, 56);
    }

    #[test]
    fn panel_layout_lays_three_regions() {
        let p = AssistantPanel::new().with_height(800);
        let header = p.header_box();
        let history = p.history_box();
        let prompt = p.prompt_box();
        assert!(header.bottom() <= history.y);
        assert!(history.bottom() <= prompt.y);
    }
}
