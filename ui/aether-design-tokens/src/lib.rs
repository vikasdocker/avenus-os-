//! Aether design system — tokens.
//!
//! The design direction in §12 of the ROADMAP ("Premium Pastel —
//! Apple × Windows") is a single visual identity shared by every
//! Aether UI surface. This crate is the typed, code-readable
//! version of that identity. Every screen in Aether OS pulls its
//! colors, type, spacing, radius, motion, and AI-state colors
//! from here — never from inline magic numbers.
//
// The crate is split into:
//
//   * [`color`]      — the pastel palette + the `Color` and
//                       `Role` types. The shell, the
//                       application chrome, the launcher's
//                       tiles, the AI assistant panel — all of
//                       them ask for `Color::role(Role::BgBase)`
//                       rather than `Rgb(14, 17, 22)`.
//
//   * [`spacing`]    — a 4 px-base scale (xs..3xl). Buttons,
//                       cards, panels, and section padding all
//                       step off this scale.
//
//   * [`radius`]     — corner radius (sm=8, md=12, lg=18,
//                       xl=24, pill=9999). Pastel = soft = large
//                       rounded corners, per §12.
//
//   * [`type_scale`] — a 6-step type ramp. Headings (xl, lg,
//                       md) for the AI surfaces; body (sm) for
//                       chrome labels; caption (xs) for the
//                       taskbar clock. The actual font
//                       family is platform-specific; the size
//                       and weight are not.
//
//   * [`motion`]     — the 150–300 ms window from §12, plus
//                       the two longer easings (400 ms for
//                       window state, 600 ms for cross-fade).
//                       `ease_standard` is the "smooth, fast,
//                       natural" curve; `ease_emphasized` is for
//                       the AI state transitions.
//
//   * [`ai_state`]   — the 9 AI visual states (IDLE,
//                       LISTENING, THINKING, etc.) and the
//                       color each one maps to. The
//                       assistant panel and the launcher both
//                       read this; an AI surface that rolls
//                       its own state colors is a bug.
//
// `Color::role(Role::...)` is the only public entry point for
// consumers. The raw `Color` constants are exposed too, but
// the role indirection lets us re-skin the system by editing
// a single function rather than grep-and-replace across every
// surface.
//
// `Role` is `Copy + Eq + Hash` so it can be used as a map key
// (e.g. a stylesheet cache).

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod ai_state;
pub mod color;
pub mod motion;
pub mod radius;
pub mod spacing;
pub mod type_scale;

pub use ai_state::{AiVisualState, AiVisualStateColors};
pub use color::{Color, Palette, Role, Theme};
pub use motion::{DurationMs, Easing};
pub use radius::Radius;
pub use spacing::Spacing;
pub use type_scale::{TypeScale, TypeStyle};
