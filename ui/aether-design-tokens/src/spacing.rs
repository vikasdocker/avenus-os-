//! Spacing scale.
//!
//! 4 px-base scale, six steps. Buttons, cards, panels, and
//! section padding all step off this — no inline magic
//! numbers in any Aether surface.
//!
//! §12 calls for "comfortable spacing" and §12's visual
//! language is soft / pastel / friendly; a small scale with
//! regular steps reads cleaner than one-off padding values.
//
// 4 px-base scale, six steps. Buttons, cards, panels, and
// section padding all step off this — no inline magic
// numbers in any Aether surface.
//
// §12 calls for "comfortable spacing" and §12's visual
// language is soft / pastel / friendly; a small scale with
// regular steps reads cleaner than one-off padding values.

/// Spacing step. The discriminant is the number of
/// 4-pixel base units the step represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Spacing {
    /// 4 px — icon-to-text, small inset.
    Xs,
    /// 8 px — chip padding, tight stack.
    Sm,
    /// 12 px — button padding (vertical), small card.
    Md,
    /// 16 px — default card padding.
    Lg,
    /// 24 px — section padding, panel inset.
    Xl,
    /// 32 px — large surface inset, between sections.
    Xxl,
    /// 48 px — hero / empty-state padding.
    Xxxl,
}

impl Spacing {
    /// Pixel value of the step.
    #[must_use]
    pub const fn px(self) -> i32 {
        match self {
            Self::Xs => 4,
            Self::Sm => 8,
            Self::Md => 12,
            Self::Lg => 16,
            Self::Xl => 24,
            Self::Xxl => 32,
            Self::Xxxl => 48,
        }
    }

    /// Same as `px` but as `u32` for framebuffer APIs that
    /// don't take signed.
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
    fn xs_is_4px() {
        assert_eq!(Spacing::Xs.px(), 4);
    }

    #[test]
    fn sm_is_8px() {
        assert_eq!(Spacing::Sm.px(), 8);
    }

    #[test]
    fn md_is_12px() {
        assert_eq!(Spacing::Md.px(), 12);
    }

    #[test]
    fn lg_is_16px() {
        assert_eq!(Spacing::Lg.px(), 16);
    }

    #[test]
    fn xl_is_24px() {
        assert_eq!(Spacing::Xl.px(), 24);
    }

    #[test]
    fn xxl_is_32px() {
        assert_eq!(Spacing::Xxl.px(), 32);
    }

    #[test]
    fn xxxl_is_48px() {
        assert_eq!(Spacing::Xxxl.px(), 48);
    }

    #[test]
    fn px_u32_matches_px() {
        for s in [
            Spacing::Xs,
            Spacing::Sm,
            Spacing::Md,
            Spacing::Lg,
            Spacing::Xl,
            Spacing::Xxl,
            Spacing::Xxxl,
        ] {
            assert_eq!(s.px(), s.px_u32() as i32);
        }
    }

    #[test]
    fn scale_is_monotonic() {
        let mut prev = -1;
        for s in [
            Spacing::Xs,
            Spacing::Sm,
            Spacing::Md,
            Spacing::Lg,
            Spacing::Xl,
            Spacing::Xxl,
            Spacing::Xxxl,
        ] {
            let v = s.px();
            assert!(v > prev, "scale not monotonic: {s:?} ({v}) after {prev}");
            prev = v;
        }
    }
}
