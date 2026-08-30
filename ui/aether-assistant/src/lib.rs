//! Aether AI Assistant surfaces — the agent's three
//! canonical UIs.
//!
//! §12 calls for the AI to feel part of the operating
//! system. Concretely, that means three surfaces, each
//! with a distinct focus:
//!
//! 1. **Assistant Panel** — a docked sidebar (Panel::Right
//!    at §12's default 360 px) that always shows the
//!    agent's current state (via `AiVisualState`), a
//!    short recent history, and a quick prompt field.
//!    The user's "always-on awareness" of the agent.
//! 2. **Agent Workspace** — a full work area that shows
//!    the current plan as a step list, with each step's
//!    own `AiVisualState`. The user's "what is the agent
//!    doing right now and how does the plan look"
//!    surface.
//! 3. **Task View** — a focused modal-like view for a
//!    single in-progress task: its inputs, outputs, the
//!    permission prompt if it's blocked, and the
//!    accept / reject controls.
//!
//! All three consume `AiVisualState` from
//! `aether-design-tokens` so the agent's visual identity
//! is one edit. All three are non-painting: the renderer
//! reads the resolved structs and applies its paint logic.
//!
//! Composition:
//!
//! ```text
//!   ┌──────────────────┬────────────────────────────┐
//!   │ Assistant Panel  │  Agent Workspace           │
//!   │ (state, history, │  (plan, steps, status)     │
//!   │  quick prompt)   │                            │
//!   ├──────────────────┴────────────────────────────┤
//!   │  Task View (when a single task is focused)    │
//!   └───────────────────────────────────────────────┘
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

pub mod panel;
pub mod plan;
pub mod task;
pub mod workspace;

pub use panel::{AssistantMessage, AssistantPanel, AssistantPanelState, AssistantRole};
pub use plan::{PlanStep, PlanStepKind, PlanStepState, WorkspacePlan};
pub use task::{TaskDecision, TaskView, TaskViewInputs, TaskViewOutputs, TaskViewState};
pub use workspace::{WorkspaceView, WorkspaceViewState};

use aether_design_tokens::{AiVisualState, Color};

/// The accent color an AI surface should use, given
/// the agent's current `AiVisualState`. All three
/// surfaces read through this so the visual identity
/// is consistent.
#[must_use]
pub const fn assistant_accent(state: AiVisualState) -> Color {
    state.color()
}

/// The human-readable label for an `AiVisualState`,
/// suitable for the "Listening…" / "Working…" /
/// "Recovering…" header on each surface.
#[must_use]
pub const fn assistant_state_label(state: AiVisualState) -> &'static str {
    match state {
        AiVisualState::Idle => "Idle",
        AiVisualState::Listening => "Listening…",
        AiVisualState::Thinking => "Thinking…",
        AiVisualState::Planning => "Planning…",
        AiVisualState::Working => "Working…",
        AiVisualState::WaitingForPermission => "Waiting for permission…",
        AiVisualState::Completed => "Done",
        AiVisualState::Error => "Error",
        AiVisualState::Recovering => "Recovering…",
        _ => "Idle",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn accent_matches_state_color() {
        assert_eq!(assistant_accent(AiVisualState::Idle), AiVisualState::Idle.color());
        assert_eq!(assistant_accent(AiVisualState::Listening), AiVisualState::Listening.color());
        assert_eq!(assistant_accent(AiVisualState::Error), AiVisualState::Error.color());
    }

    #[test]
    fn state_label_for_idle() {
        assert_eq!(assistant_state_label(AiVisualState::Idle), "Idle");
    }

    #[test]
    fn state_label_for_listening() {
        assert_eq!(assistant_state_label(AiVisualState::Listening), "Listening…");
    }

    #[test]
    fn state_label_for_thinking() {
        assert_eq!(assistant_state_label(AiVisualState::Thinking), "Thinking…");
    }

    #[test]
    fn state_label_for_working() {
        assert_eq!(assistant_state_label(AiVisualState::Working), "Working…");
    }

    #[test]
    fn state_label_for_completed() {
        assert_eq!(assistant_state_label(AiVisualState::Completed), "Done");
    }

    #[test]
    fn state_label_for_error() {
        assert_eq!(assistant_state_label(AiVisualState::Error), "Error");
    }
}
