//! Corner-radius scale.
//!
//! §12 is emphatic: "Large rounded corners" and "Windows
//! should feel lightweight and premium." The default for
//! most surfaces is `Lg` (18 px). Buttons and chips can be
//! `Md` (12 px); full-pill toggles use `Pill`.
//
// §12 is emphatic: "Large rounded corners" and "Windows
// should feel lightweight and premium." The default for
// most surfaces is `Lg` (18 px). Buttons and chips can be
// `Md` (12 px); full-pill toggles use `Pill`.

/// Corner-radius step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Radius {
    /// 6 px — small inline elements, busy rows.
    Sm,
    /// 12 px — buttons, chips, small cards.
    Md,
    /// 18 px — panels, the AI assistant card, the
    /// launcher tiles. The default.
    Lg,
    /// 24 px — modal dialogs, the AI command bar in
    /// its expanded state.
    Xl,
    /// 9999 px — full-pill toggles and "Capsule"
    /// buttons.
    Pill,
}

impl Radius {
    /// Pixel value of the radius.
    #[must_use]
    pub const fn px(self) -> i32 {
        match self {
            Self::Sm => 6,
            Self::Md => 12,
            Self::Lg => 18,
            Self::Xl => 24,
            Self::Pill => 9999,
        }
    }

    /// Same as `px` but as `u32` for framebuffer APIs.
    #[must_use]
    pub const fn px_u32(self) -> u32 {
        self.px() as u32
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn sm_is_6() {
        assert_eq!(Radius::Sm.px(), 6);
    }

    #[test]
    fn md_is_12() {
        assert_eq!(Radius::Md.px(), 12);
    }

    #[test]
    fn lg_is_18() {
        assert_eq!(Radius::Lg.px(), 18);
    }

    #[test]
    fn xl_is_24() {
        assert_eq!(Radius::Xl.px(), 24);
    }

    #[test]
    fn pill_is_huge() {
        // Pill rounds to a full half-width; the value
        // is intentionally much larger than any
        // realistic corner so the renderer can treat
        // it as "as round as possible."
        assert!(Radius::Pill.px() > 1000);
    }

    #[test]
    fn px_u32_matches_px() {
        for r in [Radius::Sm, Radius::Md, Radius::Lg, Radius::Xl, Radius::Pill] {
            assert_eq!(r.px(), r.px_u32() as i32);
        }
    }
}
