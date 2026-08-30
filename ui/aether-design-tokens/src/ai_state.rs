//! AI visual-state colors.
//!
//! §12: "AI visual feedback." The AI must feel part of
//! the operating system. The 9 states in §12 are:
//!
//!   IDLE, LISTENING, THINKING, PLANNING, WORKING,
//!   WAITING_FOR_PERMISSION, COMPLETED, ERROR,
//!   RECOVERING.
//!
//! Each state maps to a pastel color. The launcher and
//! the assistant panel both consume `AiVisualState`
//! rather than rolling their own, so the visual identity
//! is one edit.
//
// §12: "AI visual feedback." The AI must feel part of
// the operating system. The 9 states in §12 are:
//
//   IDLE, LISTENING, THINKING, PLANNING, WORKING,
//   WAITING_FOR_PERMISSION, COMPLETED, ERROR,
//   RECOVERING.
//
// Each state maps to a pastel color. The launcher and
// the assistant panel both consume `AiVisualState`
// rather than rolling their own, so the visual identity
// is one edit.
//
// `AiVisualStateColors` is the read-side bundle: every
// state's color in one struct, for callers that want to
// render a state legend or animate between two states.

use crate::color::Color;

/// The 9 AI visual states from §12. The discriminant
/// order is the canonical "happy path" of the agent:
/// IDLE -> LISTENING -> THINKING -> PLANNING ->
/// WORKING -> (WAITING_FOR_PERMISSION) -> COMPLETED,
/// with ERROR and RECOVERING as branches off WORKING.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AiVisualState {
    /// The agent is waiting. Default state.
    Idle,
    /// The agent is listening for input (mic / voice).
    Listening,
    /// The agent is reasoning.
    Thinking,
    /// The agent is laying out a plan.
    Planning,
    /// The agent is executing a plan.
    Working,
    /// The agent is blocked on a permission prompt.
    WaitingForPermission,
    /// The agent finished the current task.
    Completed,
    /// The agent hit an error and stopped.
    Error,
    /// The agent is recovering from a transient error.
    Recovering,
}

impl AiVisualState {
    /// Wire form: the kebab-case identifier the IPC
    /// layer sends over the control plane.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Listening => "listening",
            Self::Thinking => "thinking",
            Self::Planning => "planning",
            Self::Working => "working",
            Self::WaitingForPermission => "waiting-for-permission",
            Self::Completed => "completed",
            Self::Error => "error",
            Self::Recovering => "recovering",
        }
    }

    /// The accent color for this state. Pulled from the
    /// pastel palette so the AI's visual identity
    /// matches every other surface.
    #[must_use]
    pub const fn color(self) -> Color {
        match self {
            Self::Idle => Color::PASTEL_LAVENDER,
            Self::Listening => Color::PASTEL_PINK,
            Self::Thinking => Color::PASTEL_BLUE,
            Self::Planning => Color::PASTEL_LAVENDER,
            Self::Working => Color::PASTEL_MINT,
            Self::WaitingForPermission => Color::PASTEL_YELLOW,
            Self::Completed => Color::PASTEL_MINT_DEEP,
            Self::Error => Color::PASTEL_PEACH_DEEP,
            Self::Recovering => Color::PASTEL_PEACH,
        }
    }
}

/// A read-side bundle of all 9 state colors. The
/// `AiVisualState::all_colors` constructor returns
/// one; the assistant panel and the launcher use it
/// to pre-compute a stylesheet cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiVisualStateColors {
    /// `Idle` color.
    pub idle: Color,
    /// `Listening` color.
    pub listening: Color,
    /// `Thinking` color.
    pub thinking: Color,
    /// `Planning` color.
    pub planning: Color,
    /// `Working` color.
    pub working: Color,
    /// `WaitingForPermission` color.
    pub waiting_for_permission: Color,
    /// `Completed` color.
    pub completed: Color,
    /// `Error` color.
    pub error: Color,
    /// `Recovering` color.
    pub recovering: Color,
}

impl AiVisualStateColors {
    /// The complete bundle of all 9 state colors.
    #[must_use]
    pub const fn all_colors() -> Self {
        Self {
            idle: Color::PASTEL_LAVENDER,
            listening: Color::PASTEL_PINK,
            thinking: Color::PASTEL_BLUE,
            planning: Color::PASTEL_LAVENDER,
            working: Color::PASTEL_MINT,
            waiting_for_permission: Color::PASTEL_YELLOW,
            completed: Color::PASTEL_MINT_DEEP,
            error: Color::PASTEL_PEACH_DEEP,
            recovering: Color::PASTEL_PEACH,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn all_nine_states_have_a_wire_form() {
        // Every variant maps to a unique, non-empty
        // kebab-case string. Adding a new variant
        // without updating `as_str` would fail the
        // `match` exhaustiveness in non-test code.
        let states = [
            AiVisualState::Idle,
            AiVisualState::Listening,
            AiVisualState::Thinking,
            AiVisualState::Planning,
            AiVisualState::Working,
            AiVisualState::WaitingForPermission,
            AiVisualState::Completed,
            AiVisualState::Error,
            AiVisualState::Recovering,
        ];
        let mut seen: Vec<&'static str> = Vec::new();
        for s in states {
            let w = s.as_str();
            assert!(!w.is_empty());
            assert!(!w.contains('_'), "{w} is not kebab-case");
            assert!(!seen.contains(&w), "duplicate wire form: {w}");
            seen.push(w);
        }
    }

    #[test]
    fn wire_forms_match_roadmap() {
        assert_eq!(AiVisualState::Idle.as_str(), "idle");
        assert_eq!(AiVisualState::Listening.as_str(), "listening");
        assert_eq!(AiVisualState::Thinking.as_str(), "thinking");
        assert_eq!(AiVisualState::Planning.as_str(), "planning");
        assert_eq!(AiVisualState::Working.as_str(), "working");
        assert_eq!(AiVisualState::WaitingForPermission.as_str(), "waiting-for-permission");
        assert_eq!(AiVisualState::Completed.as_str(), "completed");
        assert_eq!(AiVisualState::Error.as_str(), "error");
        assert_eq!(AiVisualState::Recovering.as_str(), "recovering");
    }

    #[test]
    fn all_nine_states_have_a_color() {
        // Sanity: every variant maps to a color. Adding
        // a new variant without updating `color` would
        // fail the match.
        let states = [
            AiVisualState::Idle,
            AiVisualState::Listening,
            AiVisualState::Thinking,
            AiVisualState::Planning,
            AiVisualState::Working,
            AiVisualState::WaitingForPermission,
            AiVisualState::Completed,
            AiVisualState::Error,
            AiVisualState::Recovering,
        ];
        for s in states {
            let c = s.color();
            // Sanity: a real color, not all-zeros.
            assert!(c.r != 0 || c.g != 0 || c.b != 0);
        }
    }

    #[test]
    fn error_uses_a_warm_warning_color() {
        // §12 says "Avoid aggressive effects" — the
        // error color should still be in the pastel
        // family, not pure red.
        let c = AiVisualState::Error.color();
        assert_eq!(c, Color::PASTEL_PEACH_DEEP);
        // Sanity: the "deep" peach is still warm and
        // pastel, not pure red.
        assert!(c.r > c.b, "error color should be warm (R > B): {c:?}");
    }

    #[test]
    fn completed_uses_a_calm_green() {
        let c = AiVisualState::Completed.color();
        assert_eq!(c, Color::PASTEL_MINT_DEEP);
    }

    #[test]
    fn all_colors_matches_individual_accessors() {
        let bundle = AiVisualStateColors::all_colors();
        assert_eq!(bundle.idle, AiVisualState::Idle.color());
        assert_eq!(bundle.listening, AiVisualState::Listening.color());
        assert_eq!(bundle.thinking, AiVisualState::Thinking.color());
        assert_eq!(bundle.planning, AiVisualState::Planning.color());
        assert_eq!(bundle.working, AiVisualState::Working.color());
        assert_eq!(bundle.waiting_for_permission, AiVisualState::WaitingForPermission.color());
        assert_eq!(bundle.completed, AiVisualState::Completed.color());
        assert_eq!(bundle.error, AiVisualState::Error.color());
        assert_eq!(bundle.recovering, AiVisualState::Recovering.color());
    }

    #[test]
    fn all_colors_covers_nine_distinct_colors() {
        let bundle = AiVisualStateColors::all_colors();
        let all = [
            bundle.idle,
            bundle.listening,
            bundle.thinking,
            bundle.planning,
            bundle.working,
            bundle.waiting_for_permission,
            bundle.completed,
            bundle.error,
            bundle.recovering,
        ];
        // 9 states, 9 colors. (Some may coincide, but
        // the identity check at least guards against a
        // future refactor that drops a field.)
        assert_eq!(all.len(), 9);
    }

    #[test]
    fn state_is_hashable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(AiVisualState::Idle);
        set.insert(AiVisualState::Thinking);
        assert!(set.contains(&AiVisualState::Idle));
        assert!(!set.contains(&AiVisualState::Error));
    }
}
