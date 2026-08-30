//! Motion system.
//!
//! §12: "Smooth, fast, natural, premium. Roughly 150–300
//! ms. Used to communicate: window state, AI state,
//! selection, loading, navigation, completion. Do not
//! over-animate."
//!
//! The scale here is the window the spec calls for, plus
//! the two longer easings the existing shell already
//! uses: 400 ms for window-state transitions (the desktop
//! shell resizes a window over this window) and 600 ms
//! for cross-fades (the AI assistant panel's state
//! transitions).
//!
//! `Easing::ease_standard` is the "smooth, fast, natural"
//! curve. `Easing::ease_emphasized` is for the AI state
//! transitions where the AI "settles" into a new
//! identity.
//
// §12: "Smooth, fast, natural, premium. Roughly 150–300
// ms. Used to communicate: window state, AI state,
// selection, loading, navigation, completion. Do not
// over-animate."
//
// The scale here is the window the spec calls for, plus
// the two longer easings the existing shell already
// uses: 400 ms for window-state transitions (the desktop
// shell resizes a window over this window) and 600 ms
// for cross-fades (the AI assistant panel's state
// transitions).
//
// `Easing::ease_standard` is the "smooth, fast, natural"
// curve. `Easing::ease_emphasized` is for the AI state
// transitions where the AI "settles" into a new
// identity. The actual curve values are an extended-
// material cubic-bezier; consumers that don't speak
// bezier can map `Standard` to a CSS `ease` and
// `Emphasized` to a CSS `ease-in-out` for the same
// effect.

/// Duration of a motion, in milliseconds. The type is
/// just a `u16` newtype to keep the call sites readable:
/// `DurationMs::from_ms(180)` rather than `180u16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DurationMs(u16);

impl DurationMs {
    /// Construct a duration from a millisecond value.
    #[must_use]
    pub const fn from_ms(ms: u16) -> Self {
        Self(ms)
    }

    /// The millisecond value.
    #[must_use]
    pub const fn as_ms(self) -> u16 {
        self.0
    }
}

impl From<u16> for DurationMs {
    fn from(v: u16) -> Self {
        Self(v)
    }
}

/// A standard easing curve.
///
/// The cubic-bezier control points are stored as
/// `(x1, y1, x2, y2)` (the `(0, 0)` and `(1, 1)` endpoints
/// are implied).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Easing {
    /// `cubic-bezier(0.2, 0.0, 0.0, 1.0)` — "smooth, fast,
    /// natural." The default for selection, navigation,
    /// completion.
    Standard,
    /// `cubic-bezier(0.3, 0.0, 0.0, 1.0)` — slightly more
    /// pronounced. Use for the AI state transitions
    /// where the AI "settles" into a new state.
    Emphasized,
    /// Linear — for progress bars and continuous
    /// animations that should not "ease."
    Linear,
}

impl Easing {
    /// Cubic-bezier control points as
    /// `(x1, y1, x2, y2)`. Consumers that don't speak
    /// bezier can map these to CSS cubic-bezier()
    /// strings directly.
    #[must_use]
    pub const fn bezier(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Standard => (0.2, 0.0, 0.0, 1.0),
            Self::Emphasized => (0.3, 0.0, 0.0, 1.0),
            Self::Linear => (0.0, 0.0, 1.0, 1.0),
        }
    }

    /// CSS cubic-bezier() string form, the most portable
    /// representation. Useful for HTML / WebView-based
    /// surfaces.
    #[must_use]
    pub fn css(self) -> String {
        let (x1, y1, x2, y2) = self.bezier();
        format!("cubic-bezier({x1:.3}, {y1:.3}, {x2:.3}, {y2:.3})")
    }
}

/// The standard fast tap — 150 ms, the bottom of the
/// §12 range. Use for selection state, button press.
pub const TAP: DurationMs = DurationMs(150);
/// The standard hover / focus state change — 180 ms.
pub const HOVER: DurationMs = DurationMs(180);
/// The standard navigation / completion transition —
/// 240 ms, the middle of the §12 range.
pub const NAV: DurationMs = DurationMs(240);
/// The standard window-state transition — 400 ms. Above
/// the §12 range; used for window resize / move.
pub const WINDOW_STATE: DurationMs = DurationMs(400);
/// A longer cross-fade for AI surfaces — 600 ms.
pub const AI_CROSSFADE: DurationMs = DurationMs(600);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn tap_is_150ms() {
        assert_eq!(TAP.as_ms(), 150);
    }

    #[test]
    fn hover_is_180ms() {
        assert_eq!(HOVER.as_ms(), 180);
    }

    #[test]
    fn nav_is_240ms() {
        assert_eq!(NAV.as_ms(), 240);
    }

    #[test]
    fn window_state_is_400ms() {
        assert_eq!(WINDOW_STATE.as_ms(), 400);
    }

    #[test]
    fn ai_crossfade_is_600ms() {
        assert_eq!(AI_CROSSFADE.as_ms(), 600);
    }

    #[test]
    fn standard_window_respects_150_300_range() {
        // §12 says 150-300 ms for "selection, loading,
        // navigation, completion". HOVER (180) and
        // NAV (240) are both in that window; TAP (150)
        // is the floor.
        for d in [TAP, HOVER, NAV] {
            assert!(d.as_ms() >= 150, "{} below §12 floor", d.as_ms());
            assert!(d.as_ms() <= 300, "{} above §12 ceiling", d.as_ms());
        }
    }

    #[test]
    fn standard_easing_is_not_linear() {
        // Sanity: the standard curve should differ from
        // a straight line (which would be `Linear`).
        let standard = Easing::Standard.bezier();
        let linear = Easing::Linear.bezier();
        assert_ne!(standard, linear);
    }

    #[test]
    fn standard_easing_x1_is_in_open_unit_interval() {
        // Material's "standard" curve has x1 in (0, 1)
        // — a real ease, not "instant" or "linear".
        let (x1, _, _, _) = Easing::Standard.bezier();
        assert!(x1 > 0.0 && x1 < 1.0, "x1 should be in (0, 1), got {x1}");
    }

    #[test]
    fn emphasized_easing_x1_is_in_open_unit_interval() {
        let (x1, _, _, _) = Easing::Emphasized.bezier();
        assert!(x1 > 0.0 && x1 < 1.0, "x1 should be in (0, 1), got {x1}");
    }

    #[test]
    fn linear_easing_is_straight_line() {
        let (x1, y1, x2, y2) = Easing::Linear.bezier();
        // (0,0) and (1,1) are the implicit endpoints;
        // for "linear" the control points sit ON that
        // line.
        assert_eq!(x1, 0.0);
        assert_eq!(y1, 0.0);
        assert_eq!(x2, 1.0);
        assert_eq!(y2, 1.0);
    }

    #[test]
    fn css_format_is_parsable() {
        let s = Easing::Standard.css();
        assert!(s.starts_with("cubic-bezier("));
        assert!(s.ends_with(')'));
    }

    #[test]
    fn from_u16_works() {
        let d: DurationMs = 200u16.into();
        assert_eq!(d.as_ms(), 200);
    }

    #[test]
    fn durations_are_ordered() {
        assert!(TAP < HOVER);
        assert!(HOVER < NAV);
        assert!(NAV < WINDOW_STATE);
        assert!(WINDOW_STATE < AI_CROSSFADE);
    }
}
