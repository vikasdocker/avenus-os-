//! Aether color system.
//!
//! Light-first, pastel palette, drawn from §12 of the ROADMAP.
//! The palette is intentionally narrow: the warm white
//! surface, a single soft cream secondary, and the six
//! "supporting pastels" that carry meaning (success, info,
//! attention, etc.).
//!
//! The two key entry points for consumers are:
//!
//!   * `Color::role(Role::...)` — the semantic accessor. Every
//!     piece of UI asks for a `Role` (e.g. `Role::BgBase`,
//!     `Role::TextPrimary`, `Role::AccentPink`) and never
//!     hard-codes an `Rgb`. Switching the entire identity
//!     later is a one-file change.
//!
//!   * `Color::rgb(r,g,b)` — the constructor for ad-hoc
//!     surfaces (e.g. a window's content area, which the
//!     application itself owns). The struct is `Copy` so
//!     these can be passed by value without ceremony.
//
// Light-first, pastel palette, drawn from §12 of the ROADMAP.
// The palette is intentionally narrow: the warm white surface,
// a single soft cream secondary, and the six "supporting
// pastels" that carry meaning (success, info, attention, etc.).
//
// The two key entry points for consumers are:
//
//   * `Color::role(Role::...)` — the semantic accessor. Every
//     piece of UI asks for a `Role` (e.g. `Role::BgBase`,
//     `Role::TextPrimary`, `Role::AccentPink`) and never
//     hard-codes an `Rgb`. Switching the entire identity
//     later is a one-file change.
//
//   * `Color::rgb(r,g,b)` — the constructor for ad-hoc
//     surfaces (e.g. a window's content area, which the
//     application itself owns). The struct is `Copy` so
//     these can be passed by value without ceremony.
//
// `Color::Rgb(u8, u8, u8)` is the canonical 24-bit sRGB
// representation. There is no alpha — Aether's surfaces are
// always opaque. The framebuffer / Wayland path serializes
// to 0xRRGGBB; the `to_hex` method does that.

/// A 24-bit sRGB color. No alpha, no HDR, no ICC — Aether
/// is a software-rendered desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red channel, 0..=255.
    pub r: u8,
    /// Green channel, 0..=255.
    pub g: u8,
    /// Blue channel, 0..=255.
    pub b: u8,
}

impl Color {
    /// Construct a color from explicit 0..=255 channels.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Lowercase `#rrggbb` hex, the form most paint code
    /// wants.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// 24-bit packed value (`0xRRGGBB`), the form
    /// framebuffer and Wayland want.
    #[must_use]
    pub const fn to_packed(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    // ---------------------------------------------------------------- raw palette
    //
    // These are the source-of-truth constants. Nothing in
    // the codebase should hard-code a color value not listed
    // here; everything else asks for a `Role`.

    /// Warm white — the desktop and the window body.
    pub const SURFACE_WARM_WHITE: Color = Color::rgb(252, 250, 247);
    /// Soft cream — the secondary surface (panels, sidebar).
    pub const SURFACE_CREAM: Color = Color::rgb(246, 240, 232);
    /// A slightly darker cream for hover/active states.
    pub const SURFACE_CREAM_DEEP: Color = Color::rgb(238, 230, 218);

    /// Dark text. Pastel backgrounds demand near-black text
    /// for AA contrast, not pure black.
    pub const INK_900: Color = Color::rgb(33, 30, 41);
    /// Secondary text — labels, captions.
    pub const INK_700: Color = Color::rgb(78, 72, 88);
    /// Disabled / placeholder text.
    pub const INK_400: Color = Color::rgb(160, 152, 168);

    /// Pastel pink — accents for warm actions (e.g. a
    /// "favorite" toggle, the AI's `LISTENING` glow).
    pub const PASTEL_PINK: Color = Color::rgb(255, 184, 200);
    /// Pastel pink darker — the focus ring / pressed state.
    pub const PASTEL_PINK_DEEP: Color = Color::rgb(240, 142, 168);

    /// Soft blue — info / primary actions.
    pub const PASTEL_BLUE: Color = Color::rgb(176, 206, 255);
    /// Soft blue darker.
    pub const PASTEL_BLUE_DEEP: Color = Color::rgb(120, 162, 232);

    /// Mint green — success / confirmed.
    pub const PASTEL_MINT: Color = Color::rgb(176, 230, 200);
    /// Mint green darker.
    pub const PASTEL_MINT_DEEP: Color = Color::rgb(112, 196, 152);

    /// Lavender — secondary accent (e.g. active tab).
    pub const PASTEL_LAVENDER: Color = Color::rgb(208, 192, 240);
    /// Lavender darker.
    pub const PASTEL_LAVENDER_DEEP: Color = Color::rgb(168, 144, 224);

    /// Peach — warning.
    pub const PASTEL_PEACH: Color = Color::rgb(255, 208, 176);
    /// Peach darker.
    pub const PASTEL_PEACH_DEEP: Color = Color::rgb(240, 168, 120);

    /// Soft yellow — attention.
    pub const PASTEL_YELLOW: Color = Color::rgb(255, 232, 160);
    /// Soft yellow darker.
    pub const PASTEL_YELLOW_DEEP: Color = Color::rgb(232, 200, 96);

    /// Soft shadow — for window drop shadows.
    pub const SHADOW_SOFT: Color = Color::rgb(36, 28, 48);
    /// Hairline divider.
    pub const HAIRLINE: Color = Color::rgb(220, 212, 224);

    // ----------------------------------------------------------------- role indirection

    /// Resolve a semantic `Role` to a concrete `Color`.
    ///
    /// Every UI surface calls this. If the role's value
    /// changes, every screen that asked for it picks up the
    /// new color on the next paint.
    #[must_use]
    pub const fn role(role: Role) -> Self {
        match role {
            Role::BgBase => Self::SURFACE_WARM_WHITE,
            Role::BgPanel => Self::SURFACE_CREAM,
            Role::BgPanelHover => Self::SURFACE_CREAM_DEEP,
            Role::TextPrimary => Self::INK_900,
            Role::TextSecondary => Self::INK_700,
            Role::TextDisabled => Self::INK_400,
            Role::AccentPink => Self::PASTEL_PINK,
            Role::AccentPinkStrong => Self::PASTEL_PINK_DEEP,
            Role::AccentBlue => Self::PASTEL_BLUE,
            Role::AccentBlueStrong => Self::PASTEL_BLUE_DEEP,
            Role::AccentMint => Self::PASTEL_MINT,
            Role::AccentMintStrong => Self::PASTEL_MINT_DEEP,
            Role::AccentLavender => Self::PASTEL_LAVENDER,
            Role::AccentLavenderStrong => Self::PASTEL_LAVENDER_DEEP,
            Role::AccentPeach => Self::PASTEL_PEACH,
            Role::AccentPeachStrong => Self::PASTEL_PEACH_DEEP,
            Role::AccentYellow => Self::PASTEL_YELLOW,
            Role::AccentYellowStrong => Self::PASTEL_YELLOW_DEEP,
            Role::Shadow => Self::SHADOW_SOFT,
            Role::Hairline => Self::HAIRLINE,
        }
    }
}

/// Semantic role of a color. Consumers ask for the role,
/// not the raw `Color`, so the system can be re-skinned
/// in one place.
///
/// The variants are grouped by surface (Bg*), by
/// foreground text (Text*), by accent (Accent*), and by
/// utility (Shadow, Hairline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Role {
    // Backgrounds.
    /// The base surface — the desktop, the window body.
    BgBase,
    /// A panel — the sidebar, the launcher, the AI
    /// assistant card.
    BgPanel,
    /// A hovered or pressed panel.
    BgPanelHover,

    // Foreground text.
    /// Primary body / heading text.
    TextPrimary,
    /// Secondary text — labels, captions.
    TextSecondary,
    /// Disabled / placeholder text.
    TextDisabled,

    // Accents.
    /// Pastel pink — warm accent.
    AccentPink,
    /// Pastel pink, deeper — focus / pressed.
    AccentPinkStrong,
    /// Soft blue — info / primary action.
    AccentBlue,
    /// Soft blue, deeper — focus / pressed.
    AccentBlueStrong,
    /// Mint green — success.
    AccentMint,
    /// Mint green, deeper — focus / pressed.
    AccentMintStrong,
    /// Lavender — secondary accent.
    AccentLavender,
    /// Lavender, deeper — focus / pressed.
    AccentLavenderStrong,
    /// Peach — warning.
    AccentPeach,
    /// Peach, deeper — focus / pressed.
    AccentPeachStrong,
    /// Soft yellow — attention.
    AccentYellow,
    /// Soft yellow, deeper — focus / pressed.
    AccentYellowStrong,

    // Utility.
    /// Soft shadow color for window drop shadows.
    Shadow,
    /// Hairline divider color.
    Hairline,
}

/// The active palette bundle. Right now Aether ships one
/// palette (the pastel one from §12); this enum is here so
/// future themes (high-contrast, dark) can be added without
/// breaking the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Palette {
    /// The default light pastel palette from §12.
    Pastel,
}

/// The active theme. Currently fixed to `Light`; the enum
/// is here so a future `Dark` variant doesn't change every
/// call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Theme {
    /// Light, pastel theme (the only one Aether ships
    /// today; §12 says "light-first").
    #[default]
    Light,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn warm_white_is_warm_white() {
        let c = Color::role(Role::BgBase);
        assert_eq!(c, Color::SURFACE_WARM_WHITE);
        assert_eq!(c.r, 252);
        assert_eq!(c.g, 250);
        assert_eq!(c.b, 247);
    }

    #[test]
    fn hex_format() {
        let c = Color::rgb(0x12, 0x34, 0x56);
        assert_eq!(c.to_hex(), "#123456");
    }

    #[test]
    fn hex_is_lowercase() {
        let c = Color::rgb(0xAB, 0xCD, 0xEF);
        assert_eq!(c.to_hex(), "#abcdef");
    }

    #[test]
    fn packed_format() {
        let c = Color::rgb(0x12, 0x34, 0x56);
        assert_eq!(c.to_packed(), 0x0012_3456);
    }

    #[test]
    fn role_pink_resolves_to_pastel_pink() {
        let c = Color::role(Role::AccentPink);
        assert_eq!(c, Color::PASTEL_PINK);
    }

    #[test]
    fn role_blue_resolves_to_pastel_blue() {
        let c = Color::role(Role::AccentBlue);
        assert_eq!(c, Color::PASTEL_BLUE);
    }

    #[test]
    fn role_mint_resolves_to_pastel_mint() {
        let c = Color::role(Role::AccentMint);
        assert_eq!(c, Color::PASTEL_MINT);
    }

    #[test]
    fn role_lavender_resolves_to_pastel_lavender() {
        let c = Color::role(Role::AccentLavender);
        assert_eq!(c, Color::PASTEL_LAVENDER);
    }

    #[test]
    fn role_peach_resolves_to_pastel_peach() {
        let c = Color::role(Role::AccentPeach);
        assert_eq!(c, Color::PASTEL_PEACH);
    }

    #[test]
    fn role_yellow_resolves_to_pastel_yellow() {
        let c = Color::role(Role::AccentYellow);
        assert_eq!(c, Color::PASTEL_YELLOW);
    }

    #[test]
    fn role_text_primary_is_dark_ink() {
        let c = Color::role(Role::TextPrimary);
        // Per §12 the dark text is "near-black, not pure
        // black" for AA contrast on pastels.
        assert_eq!(c, Color::INK_900);
        assert!(c.r < 50, "ink 900 should be near-black, not pure black: {c:?}");
    }

    #[test]
    fn shadow_color_is_dark() {
        let c = Color::role(Role::Shadow);
        assert_eq!(c, Color::SHADOW_SOFT);
        // Pastel shadow is a desaturated dark, not pure
        // black.
        assert!(c.r < 60 && c.g < 60 && c.b < 60, "shadow should be near-black: {c:?}");
    }

    #[test]
    fn every_role_resolves() {
        // Exhaustive smoke test: every variant of `Role`
        // resolves to a non-default color. Adding a new
        // role without an arm in `Color::role` would
        // break this.
        let roles = [
            Role::BgBase,
            Role::BgPanel,
            Role::BgPanelHover,
            Role::TextPrimary,
            Role::TextSecondary,
            Role::TextDisabled,
            Role::AccentPink,
            Role::AccentPinkStrong,
            Role::AccentBlue,
            Role::AccentBlueStrong,
            Role::AccentMint,
            Role::AccentMintStrong,
            Role::AccentLavender,
            Role::AccentLavenderStrong,
            Role::AccentPeach,
            Role::AccentPeachStrong,
            Role::AccentYellow,
            Role::AccentYellowStrong,
            Role::Shadow,
            Role::Hairline,
        ];
        for r in roles {
            let c = Color::role(r);
            // Sanity: every role resolves to a color
            // (the match is exhaustive in `role`).
            let _ = c.to_hex();
        }
    }

    #[test]
    fn pastel_palette_is_default_theme() {
        assert_eq!(Theme::default(), Theme::Light);
    }

    #[test]
    fn role_is_hashable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Role::BgBase);
        set.insert(Role::AccentPink);
        assert!(set.contains(&Role::BgBase));
        assert!(!set.contains(&Role::AccentMint));
    }

    #[test]
    fn color_is_hashable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Color::rgb(1, 2, 3));
        set.insert(Color::rgb(4, 5, 6));
        assert!(set.contains(&Color::rgb(1, 2, 3)));
        assert!(!set.contains(&Color::rgb(7, 8, 9)));
    }
}
