//! Aether accessibility — the focus model, the
//! keyboard navigation, the scaling, the
//! reduced-motion / contrast preferences, and the
//! accessibility roles.
//!
//! §12 calls for the OS to "feel right" for everyone.
//! Concretely, that means:
//!
//! - **Focus rings** — every interactive surface
//!   draws a clear focus ring when the keyboard
//!   focus is on it. The renderer reads the
//!   `FocusRing` and paints a 2-px stroke around
//!   the focused region.
//! - **Keyboard navigation** — `Tab` / `Shift+Tab`
//!   cycle focus; `Enter` / `Space` activate;
//!   `Arrow` keys navigate within a region. The
//!   `KeyboardNav` value describes the current
//!   chain of focusable regions.
//! - **Scaling** — the `Scale` token (1..=4) scales
//!   all `Spacing` and `TypeScale` values
//!   proportionally. The renderer reads the
//!   active scale.
//! - **Reduced motion** — a `MotionPreference`
//!   that, when set to `Reduced`, swaps every
//!   animation for an instant transition. The
//!   renderer / shell respects this by routing
//!   through `apply_motion_preference`.
//! - **Contrast** — a `ContrastPreference` that,
//!   when set to `High`, swaps the active palette
//!   for a high-contrast equivalent.
//! - **Accessibility roles** — every Aether
//!   surface carries a `Role` (Button, Slider,
//!   List, Nav, Dialog, Status, Tab) that the
//!   AT-SPI / screen-reader bridge uses.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use aether_design_tokens::motion::DurationMs;
use alloc::string::String;
use alloc::vec::Vec;

/// An accessibility role. Every interactive Aether
/// surface carries one; the AT-SPI / screen-reader
/// bridge uses this to announce the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Role {
    /// A button (e.g. the command bar's send button).
    Button,
    /// A text input field (e.g. the command bar's
    /// prompt field).
    TextInput,
    /// A list (e.g. the launcher's tile grid, the
    /// assistant panel's history list).
    List,
    /// A list item (one row in a list).
    ListItem,
    /// A navigation rail (e.g. the launcher's mode
    /// rail, the command bar's mode tabs).
    Nav,
    /// A tab in a nav rail.
    Tab,
    /// A dialog (e.g. the permission prompt).
    Dialog,
    /// A status indicator (e.g. the AI tray on the
    /// taskbar).
    Status,
    /// A progress bar (e.g. the workspace's plan
    /// progress).
    ProgressBar,
    /// A tile (e.g. a launcher tile).
    Tile,
    /// A heading (e.g. the workspace's plan goal).
    Heading,
    /// A generic region / group.
    Region,
}

impl Role {
    /// The human-readable label an AT-SPI bridge
    /// would announce for this role.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::TextInput => "text input",
            Self::List => "list",
            Self::ListItem => "list item",
            Self::Nav => "navigation",
            Self::Tab => "tab",
            Self::Dialog => "dialog",
            Self::Status => "status",
            Self::ProgressBar => "progress bar",
            Self::Tile => "tile",
            Self::Heading => "heading",
            Self::Region => "region",
        }
    }
}

/// A description for a surface, used by the
/// screen-reader bridge. Multiple descriptions can
/// be supplied (e.g. one for the surface itself and
/// one for its current state).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Description {
    /// The short, single-sentence description of
    /// the surface (e.g. "Send prompt").
    pub label: String,
    /// The longer, paragraph-form description
    /// (e.g. the full prompt the user typed).
    pub detail: String,
    /// The current state of the surface, if any
    /// (e.g. "Working", "Done", "Error"). Empty =
    /// no state to announce.
    pub state: String,
    /// The keyboard shortcut that activates this
    /// surface, if any (e.g. "Ctrl+Enter" for the
    /// send button). Empty = no shortcut.
    pub shortcut: String,
}

impl Description {
    /// A description with just a label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), detail: String::new(), state: String::new(), shortcut: String::new() }
    }

    /// Add a detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// Add a state.
    #[must_use]
    pub fn with_state(mut self, state: impl Into<String>) -> Self {
        self.state = state.into();
        self
    }

    /// Add a keyboard shortcut.
    #[must_use]
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = shortcut.into();
        self
    }

    /// Whether the description is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.label.is_empty()
            && self.detail.is_empty()
            && self.state.is_empty()
            && self.shortcut.is_empty()
    }
}

/// A single focusable region on a surface. The
/// keyboard navigation chain is a list of these.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Focusable {
    /// A unique id (the surface name + a region
    /// index, e.g. `"launcher.tiles.0"`).
    pub id: String,
    /// The accessibility role of this region.
    pub role: Role,
    /// The screen-reader description.
    pub description: Description,
    /// Whether this region is currently disabled.
    /// Disabled regions are skipped by keyboard
    /// navigation.
    pub disabled: bool,
}

impl Focusable {
    /// Construct a focusable with the given id, role,
    /// and label.
    #[must_use]
    pub fn new(id: impl Into<String>, role: Role, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role,
            description: Description::new(label),
            disabled: false,
        }
    }

    /// Override the description.
    #[must_use]
    pub fn with_description(mut self, d: Description) -> Self {
        self.description = d;
        self
    }

    /// Mark the region as disabled.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// A keyboard navigation chain — the ordered list of
/// focusable regions on the current surface. The
/// `Tab` / `Shift+Tab` keys cycle through this list
/// (skipping disabled regions).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct KeyboardNav {
    /// The ordered list of focusable regions.
    pub chain: Vec<Focusable>,
    /// The index of the currently focused region in
    /// `chain`. `None` = no focus.
    pub focused: Option<usize>,
}

impl KeyboardNav {
    /// An empty chain.
    #[must_use]
    pub fn new() -> Self {
        Self { chain: Vec::new(), focused: None }
    }

    /// Append a focusable to the chain.
    #[must_use]
    pub fn push(mut self, f: Focusable) -> Self {
        self.chain.push(f);
        self
    }

    /// Override the focused index.
    #[must_use]
    pub fn with_focused(mut self, idx: Option<usize>) -> Self {
        self.focused = idx;
        self
    }

    /// Move focus to the next focusable region
    /// (skipping disabled ones). Wraps at the end.
    /// Returns the new `KeyboardNav`. If the chain
    /// is empty, returns the original value.
    #[must_use]
    pub fn focus_next(mut self) -> Self {
        if self.chain.is_empty() {
            return self;
        }
        let n = self.chain.len();
        let mut i = match self.focused {
            None => 0,
            Some(j) => (j + 1) % n,
        };
        // Walk forward to find the first non-disabled
        // focusable.
        for _ in 0..n {
            if !self.chain[i].disabled {
                self.focused = Some(i);
                return self;
            }
            i = (i + 1) % n;
        }
        // All focusables are disabled.
        self.focused = None;
        self
    }

    /// Move focus to the previous focusable region.
    /// Wraps at the top.
    #[must_use]
    pub fn focus_prev(mut self) -> Self {
        if self.chain.is_empty() {
            return self;
        }
        let n = self.chain.len();
        let mut i = match self.focused {
            None => n - 1,
            Some(0) => n - 1,
            Some(j) => j - 1,
        };
        for _ in 0..n {
            if !self.chain[i].disabled {
                self.focused = Some(i);
                return self;
            }
            i = if i == 0 { n - 1 } else { i - 1 };
        }
        self.focused = None;
        self
    }

    /// The currently focused focusable, if any.
    #[must_use]
    pub fn focused_focusable(&self) -> Option<&Focusable> {
        self.focused.and_then(|i| self.chain.get(i))
    }

    /// The id of the currently focused region, if
    /// any.
    #[must_use]
    pub fn focused_id(&self) -> Option<&str> {
        self.focused_focusable().map(|f| f.id.as_str())
    }

    /// The number of non-disabled focusables.
    #[must_use]
    pub fn enabled_count(&self) -> usize {
        self.chain.iter().filter(|f| !f.disabled).count()
    }
}

/// The user's motion preference. The renderer /
/// shell reads this and decides whether to run an
/// animation or to do an instant transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MotionPreference {
    /// The default. Animations run as designed.
    #[default]
    Standard,
    /// The user has requested reduced motion
    /// (the OS-level "Reduce motion" toggle).
    /// Animations are skipped or shortened to
    /// < 100 ms.
    Reduced,
}

impl MotionPreference {
    /// Whether the current preference is "Reduced."
    #[must_use]
    pub const fn is_reduced(self) -> bool {
        matches!(self, Self::Reduced)
    }
}

/// The user's contrast preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ContrastPreference {
    /// The default. Standard pastel contrast.
    #[default]
    Standard,
    /// The user has requested higher contrast (the
    /// OS-level "Increase contrast" toggle). The
    /// renderer / shell swaps the active palette
    /// for a high-contrast equivalent (deeper
    /// INK_900 text, stronger pastels, thicker
    /// borders).
    High,
}

impl ContrastPreference {
    /// Whether the current preference is "High."
    #[must_use]
    pub const fn is_high(self) -> bool {
        matches!(self, Self::High)
    }
}

/// The display scale. The renderer multiplies every
/// `Spacing` and `TypeScale` value by the active
/// scale. `1` is the default; `2` doubles every
/// dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scale(pub u8);

impl Scale {
    /// The default scale (1×).
    pub const DEFAULT: Self = Self(1);
    /// The maximum supported scale (4×).
    pub const MAX: Self = Self(4);
    /// The minimum supported scale (1×).
    pub const MIN: Self = Self(1);

    /// The default scale (1×). This is the canonical
    /// "fresh state" constructor; prefer this over
    /// `Default::default()` (which would yield 0).
    #[must_use]
    pub const fn new() -> Self {
        Self(1)
    }

    /// The scale factor. The renderer multiplies
    /// every dimension by this.
    #[must_use]
    pub const fn factor(self) -> u32 {
        // u8 → u32 widening; `Self::DEFAULT.factor() == 1`.
        self.0 as u32
    }

    /// Whether the scale is the default.
    #[must_use]
    pub const fn is_default(self) -> bool {
        self.0 == 1
    }
}

impl Default for Scale {
    fn default() -> Self {
        Self::new()
    }
}

/// Adjust a `DurationMs` for the user's motion
/// preference. Under `Reduced`, the duration is
/// shortened to 0 (instant transition) for any
/// value > 100 ms; values ≤ 100 ms are preserved.
#[must_use]
pub fn apply_motion_preference(d: DurationMs, pref: MotionPreference) -> DurationMs {
    match pref {
        MotionPreference::Standard => d,
        MotionPreference::Reduced => {
            if d.as_ms() > 100 {
                DurationMs::from_ms(0)
            } else {
                d
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn role_label_is_non_empty() {
        let roles = [
            Role::Button,
            Role::TextInput,
            Role::List,
            Role::ListItem,
            Role::Nav,
            Role::Tab,
            Role::Dialog,
            Role::Status,
            Role::ProgressBar,
            Role::Tile,
            Role::Heading,
            Role::Region,
        ];
        for r in roles {
            assert!(!r.label().is_empty());
        }
    }

    #[test]
    fn description_starts_with_label() {
        let d = Description::new("Send");
        assert_eq!(d.label, "Send");
        assert!(d.detail.is_empty());
        assert!(d.state.is_empty());
        assert!(d.shortcut.is_empty());
        assert!(!d.is_empty());
    }

    #[test]
    fn description_empty_when_all_empty() {
        let d = Description {
            label: String::new(),
            detail: String::new(),
            state: String::new(),
            shortcut: String::new(),
        };
        assert!(d.is_empty());
    }

    #[test]
    fn description_with_detail() {
        let d = Description::new("Send").with_detail("Sends the prompt to Aether");
        assert_eq!(d.detail, "Sends the prompt to Aether");
    }

    #[test]
    fn description_with_state() {
        let d = Description::new("AI").with_state("Working");
        assert_eq!(d.state, "Working");
    }

    #[test]
    fn description_with_shortcut() {
        let d = Description::new("Send").with_shortcut("Ctrl+Enter");
        assert_eq!(d.shortcut, "Ctrl+Enter");
    }

    #[test]
    fn focusable_starts_enabled() {
        let f = Focusable::new("send", Role::Button, "Send");
        assert!(!f.disabled);
        assert_eq!(f.id, "send");
        assert_eq!(f.role, Role::Button);
        assert_eq!(f.description.label, "Send");
    }

    #[test]
    fn focusable_disabled() {
        let f = Focusable::new("send", Role::Button, "Send").disabled();
        assert!(f.disabled);
    }

    #[test]
    fn keyboard_nav_starts_empty() {
        let k = KeyboardNav::new();
        assert!(k.chain.is_empty());
        assert!(k.focused.is_none());
        assert_eq!(k.enabled_count(), 0);
    }

    #[test]
    fn keyboard_nav_focus_next_empty() {
        let k = KeyboardNav::new();
        let k2 = k.focus_next();
        assert!(k2.focused.is_none());
    }

    #[test]
    fn keyboard_nav_focus_next_cycles() {
        let k = KeyboardNav::new()
            .push(Focusable::new("a", Role::Button, "A"))
            .push(Focusable::new("b", Role::Button, "B"))
            .push(Focusable::new("c", Role::Button, "C"));
        let k2 = k.focus_next();
        assert_eq!(k2.focused, Some(0));
        let k3 = k2.focus_next();
        assert_eq!(k3.focused, Some(1));
        let k4 = k3.focus_next();
        assert_eq!(k4.focused, Some(2));
        let k5 = k4.focus_next();
        assert_eq!(k5.focused, Some(0));
    }

    #[test]
    fn keyboard_nav_focus_prev_wraps() {
        let k = KeyboardNav::new()
            .push(Focusable::new("a", Role::Button, "A"))
            .push(Focusable::new("b", Role::Button, "B"))
            .with_focused(Some(0));
        let k2 = k.focus_prev();
        assert_eq!(k2.focused, Some(1));
    }

    #[test]
    fn keyboard_nav_skips_disabled() {
        // Start focused on `a`; press Tab — the next
        // non-disabled focusable is `c` (skipping
        // the disabled `b`).
        let k = KeyboardNav::new()
            .push(Focusable::new("a", Role::Button, "A"))
            .push(Focusable::new("b", Role::Button, "B").disabled())
            .push(Focusable::new("c", Role::Button, "C"))
            .with_focused(Some(0));
        let k2 = k.focus_next();
        // a (0) -> c (2), skipping b.
        assert_eq!(k2.focused, Some(2));
    }

    #[test]
    fn keyboard_nav_focus_next_from_none_starts_at_zero() {
        let k = KeyboardNav::new()
            .push(Focusable::new("a", Role::Button, "A"))
            .push(Focusable::new("b", Role::Button, "B"));
        let k2 = k.focus_next();
        assert_eq!(k2.focused, Some(0));
    }

    #[test]
    fn keyboard_nav_focused_focusable_returns_some() {
        let k = KeyboardNav::new()
            .push(Focusable::new("a", Role::Button, "A"))
            .push(Focusable::new("b", Role::Button, "B"))
            .with_focused(Some(1));
        let f = k.focused_focusable().unwrap();
        assert_eq!(f.id, "b");
    }

    #[test]
    fn keyboard_nav_focused_focusable_returns_none() {
        let k = KeyboardNav::new();
        assert!(k.focused_focusable().is_none());
    }

    #[test]
    fn keyboard_nav_focused_id() {
        let k = KeyboardNav::new()
            .push(Focusable::new("send", Role::Button, "Send"))
            .with_focused(Some(0));
        assert_eq!(k.focused_id(), Some("send"));
    }

    #[test]
    fn keyboard_nav_enabled_count_skips_disabled() {
        let k = KeyboardNav::new()
            .push(Focusable::new("a", Role::Button, "A"))
            .push(Focusable::new("b", Role::Button, "B").disabled())
            .push(Focusable::new("c", Role::Button, "C"));
        assert_eq!(k.enabled_count(), 2);
    }

    #[test]
    fn motion_preference_default_is_standard() {
        assert_eq!(MotionPreference::default(), MotionPreference::Standard);
        assert!(!MotionPreference::Standard.is_reduced());
        assert!(MotionPreference::Reduced.is_reduced());
    }

    #[test]
    fn contrast_preference_default_is_standard() {
        assert_eq!(ContrastPreference::default(), ContrastPreference::Standard);
        assert!(!ContrastPreference::Standard.is_high());
        assert!(ContrastPreference::High.is_high());
    }

    #[test]
    fn scale_default_is_one() {
        assert_eq!(Scale::new(), Scale(1));
        assert!(Scale::DEFAULT.is_default());
    }

    #[test]
    fn scale_factor_is_u8_as_u32() {
        assert_eq!(Scale(1).factor(), 1);
        assert_eq!(Scale(2).factor(), 2);
        assert_eq!(Scale(4).factor(), 4);
    }

    #[test]
    fn scale_min_max() {
        assert_eq!(Scale::MIN, Scale(1));
        assert_eq!(Scale::MAX, Scale(4));
    }

    #[test]
    fn apply_motion_preference_preserves_under_standard() {
        let d = DurationMs::from_ms(240);
        assert_eq!(apply_motion_preference(d, MotionPreference::Standard), d);
    }

    #[test]
    fn apply_motion_preference_zeroes_long_under_reduced() {
        let d = DurationMs::from_ms(240);
        assert_eq!(apply_motion_preference(d, MotionPreference::Reduced), DurationMs::from_ms(0));
    }

    #[test]
    fn apply_motion_preference_preserves_short_under_reduced() {
        let d = DurationMs::from_ms(50);
        assert_eq!(apply_motion_preference(d, MotionPreference::Reduced), d);
    }
}
