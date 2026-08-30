//! Type scale.
//!
//! §12 calls for a "Modern, readable sans-serif. Clear
//! hierarchy, excellent readability, comfortable spacing,
//! consistent weights, large headings, compact readable
//! system information."
//!
//! The actual font family is platform-specific
//! (system-ui on the host); the size, weight, and line-
//! height are part of the design system and live here.
//!
//! Six steps: `Display` (32), `Heading` (24), `Subhead` (18),
//! `Body` (14), `Caption` (12), `Micro` (10).
//
// §12 calls for a "Modern, readable sans-serif. Clear
// hierarchy, excellent readability, comfortable spacing,
// consistent weights, large headings, compact readable
// system information."
//
// The actual font family is platform-specific
// (system-ui on the host); the size, weight, and line-
// height are part of the design system and live here.
//
// Six steps:
//
//   * `Display` (32 px) — the AI launcher's open state,
//                         the assistant's first-paint
//                         greeting.
//   * `Heading`  (24 px) — section headings.
//   * `Subhead`  (18 px) — card titles, dialog titles.
//   * `Body`     (14 px) — the default body size. Chrome
//                         labels, button text, table cells.
//   * `Caption`  (12 px) — taskbar clock, system tray.
//   * `Micro`    (10 px) — hotkey hints, very small
//                         metadata.

/// A single type style — size, weight, and the
/// line-height that goes with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeStyle {
    /// Pixel size of the font.
    pub size_px: u32,
    /// Weight: 400 = regular, 500 = medium, 600 =
    /// semibold, 700 = bold.
    pub weight: u16,
    /// Line height in pixels.
    pub line_height_px: u32,
}

/// The type ramp. Use the variants for the named step;
/// `step()` returns the matching `TypeStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TypeScale {
    /// 32 px / weight 700 / line 40 — large greeting.
    Display,
    /// 24 px / weight 700 / line 32 — section heading.
    Heading,
    /// 18 px / weight 600 / line 24 — card title.
    Subhead,
    /// 14 px / weight 400 / line 20 — body, button
    /// label, chrome default.
    Body,
    /// 12 px / weight 500 / line 16 — taskbar clock,
    /// small status.
    Caption,
    /// 10 px / weight 500 / line 14 — hotkey hint,
    /// micro-meta.
    Micro,
}

impl TypeScale {
    /// Resolve a scale step to its concrete `TypeStyle`.
    #[must_use]
    pub const fn step(self) -> TypeStyle {
        match self {
            Self::Display => TypeStyle { size_px: 32, weight: 700, line_height_px: 40 },
            Self::Heading => TypeStyle { size_px: 24, weight: 700, line_height_px: 32 },
            Self::Subhead => TypeStyle { size_px: 18, weight: 600, line_height_px: 24 },
            Self::Body => TypeStyle { size_px: 14, weight: 400, line_height_px: 20 },
            Self::Caption => TypeStyle { size_px: 12, weight: 500, line_height_px: 16 },
            Self::Micro => TypeStyle { size_px: 10, weight: 500, line_height_px: 14 },
        }
    }

    /// Shortcut for `step().size_px`.
    #[must_use]
    pub const fn size_px(self) -> u32 {
        self.step().size_px
    }

    /// Shortcut for `step().weight`.
    #[must_use]
    pub const fn weight(self) -> u16 {
        self.step().weight
    }

    /// Shortcut for `step().line_height_px`.
    #[must_use]
    pub const fn line_height_px(self) -> u32 {
        self.step().line_height_px
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn display_is_largest() {
        let s = TypeScale::Display.step();
        assert!(s.size_px >= 24);
    }

    #[test]
    fn micro_is_smallest() {
        let s = TypeScale::Micro.step();
        assert!(s.size_px <= 12);
    }

    #[test]
    fn display_uses_bold_weight() {
        let s = TypeScale::Display.step();
        assert!(s.weight >= 600);
    }

    #[test]
    fn body_uses_regular_weight() {
        let s = TypeScale::Body.step();
        assert_eq!(s.weight, 400);
    }

    #[test]
    fn line_height_exceeds_size_for_every_step() {
        for step in [
            TypeScale::Display,
            TypeScale::Heading,
            TypeScale::Subhead,
            TypeScale::Body,
            TypeScale::Caption,
            TypeScale::Micro,
        ] {
            let s = step.step();
            assert!(
                s.line_height_px >= s.size_px,
                "{step:?} line height {} < size {}",
                s.line_height_px,
                s.size_px
            );
        }
    }

    #[test]
    fn sizes_are_monotonic_decreasing() {
        let steps = [
            TypeScale::Display,
            TypeScale::Heading,
            TypeScale::Subhead,
            TypeScale::Body,
            TypeScale::Caption,
            TypeScale::Micro,
        ];
        let mut prev = u32::MAX;
        for s in steps {
            let size = s.size_px();
            assert!(size < prev, "{s:?} {size} not less than previous {prev}");
            prev = size;
        }
    }
}
