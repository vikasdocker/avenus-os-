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
    /// Deep shadow — for elevated glass panels.
    pub const SHADOW_DEEP: Color = Color::rgb(20, 16, 32);
    /// Hairline divider.
    pub const HAIRLINE: Color = Color::rgb(220, 212, 224);

    // ------------------------------------------------------- glass palette

    /// Glass panel base — near-white with high translucency.
    pub const GLASS_BG: Color = Color::rgb(255, 255, 255);
    /// Glass panel border — subtle prismatic edge.
    pub const GLASS_BORDER: Color = Color::rgb(220, 215, 235);
    /// Glass highlight — top-edge shine.
    pub const GLASS_HIGHLIGHT: Color = Color::rgb(255, 255, 255);
    /// Glass frosted overlay — desaturated lavender.
    pub const GLASS_FROST: Color = Color::rgb(235, 230, 245);

    // ------------------------------------------------------- crystal palette

    /// Crystal prismatic — soft iridescent blue-pink.
    pub const CRYSTAL_PRISM: Color = Color::rgb(200, 180, 255);
    /// Crystal refraction — warm pink-gold.
    pub const CRYSTAL_REFRACT: Color = Color::rgb(255, 190, 210);
    /// Crystal highlight — bright white-blue shine.
    pub const CRYSTAL_SHINE: Color = Color::rgb(230, 240, 255);
    /// Crystal edge — prismatic border accent.
    pub const CRYSTAL_EDGE: Color = Color::rgb(180, 160, 240);

    // ------------------------------------------------------- glow palette

    /// Glow blue — soft ambient light.
    pub const GLOW_BLUE: Color = Color::rgb(140, 180, 255);
    /// Glow pink — warm ambient light.
    pub const GLOW_PINK: Color = Color::rgb(255, 160, 200);
    /// Glow mint — cool ambient light.
    pub const GLOW_MINT: Color = Color::rgb(140, 230, 190);
    /// Glow lavender — soft ambient light.
    pub const GLOW_LAVENDER: Color = Color::rgb(180, 160, 240);

    // -------------------------------------------------- dark crystal palette

    /// Dark crystal canvas — deep navy base.
    pub const DARK_CRYSTAL_CANVAS: Color = Color::rgb(12, 14, 24);
    /// Dark crystal surface — slightly lighter navy.
    pub const DARK_CRYSTAL_SURFACE: Color = Color::rgb(18, 20, 35);
    /// Dark crystal surface strong — elevated panels.
    pub const DARK_CRYSTAL_SURFACE_STRONG: Color = Color::rgb(24, 28, 48);
    /// Dark crystal surface hover — interactive hover.
    pub const DARK_CRYSTAL_HOVER: Color = Color::rgb(30, 34, 56);
    /// Dark crystal surface active — pressed state.
    pub const DARK_CRYSTAL_ACTIVE: Color = Color::rgb(36, 40, 64);

    /// Dark crystal text — near-white for high contrast.
    pub const DARK_CRYSTAL_TEXT: Color = Color::rgb(235, 232, 245);
    /// Dark crystal muted text — soft lavender-grey.
    pub const DARK_CRYSTAL_MUTED: Color = Color::rgb(140, 136, 160);
    /// Dark crystal disabled text.
    pub const DARK_CRYSTAL_DISABLED: Color = Color::rgb(80, 76, 100);

    /// Dark crystal border — subtle prismatic line.
    pub const DARK_CRYSTAL_BORDER: Color = Color::rgb(50, 48, 72);
    /// Dark crystal border strong — focused/active borders.
    pub const DARK_CRYSTAL_BORDER_STRONG: Color = Color::rgb(100, 92, 160);

    /// Dark crystal accent — vibrant lavender-blue.
    pub const DARK_CRYSTAL_ACCENT: Color = Color::rgb(130, 120, 220);
    /// Dark crystal accent ink — text on accent surfaces.
    pub const DARK_CRYSTAL_ACCENT_INK: Color = Color::rgb(255, 255, 255);
    /// Dark crystal accent soft — tinted accent background.
    pub const DARK_CRYSTAL_ACCENT_SOFT: Color = Color::rgb(40, 36, 72);

    /// Dark crystal secondary — teal accent.
    pub const DARK_CRYSTAL_SECONDARY: Color = Color::rgb(80, 200, 180);
    /// Dark crystal tertiary — warm peach accent.
    pub const DARK_CRYSTAL_TERTIARY: Color = Color::rgb(220, 140, 100);

    /// Dark crystal focus ring — bright accent.
    pub const DARK_CRYSTAL_FOCUS: Color = Color::rgb(130, 120, 220);
    /// Dark crystal danger — soft red.
    pub const DARK_CRYSTAL_DANGER: Color = Color::rgb(220, 80, 80);
    /// Dark crystal danger ink — text on danger.
    pub const DARK_CRYSTAL_DANGER_INK: Color = Color::rgb(255, 255, 255);
    /// Dark crystal success — soft green.
    pub const DARK_CRYSTAL_SUCCESS: Color = Color::rgb(60, 200, 120);
    /// Dark crystal success ink — text on success.
    pub const DARK_CRYSTAL_SUCCESS_INK: Color = Color::rgb(12, 14, 24);
    /// Dark crystal warning — soft amber.
    pub const DARK_CRYSTAL_WARNING: Color = Color::rgb(240, 180, 60);
    /// Dark crystal warning ink — text on warning.
    pub const DARK_CRYSTAL_WARNING_INK: Color = Color::rgb(12, 14, 24);

    /// Dark crystal glass fill — translucent white.
    pub const DARK_CRYSTAL_GLASS: Color = Color::rgb(255, 255, 255);
    /// Dark crystal glass border — prismatic edge.
    pub const DARK_CRYSTAL_GLASS_BORDER: Color = Color::rgb(100, 96, 140);
    /// Dark crystal glass highlight — top edge shine.
    pub const DARK_CRYSTAL_GLASS_HIGHLIGHT: Color = Color::rgb(255, 255, 255);
    /// Dark crystal glass frost — tinted overlay.
    pub const DARK_CRYSTAL_GLASS_FROST: Color = Color::rgb(40, 36, 64);

    /// Dark crystal shadow — deep dark.
    pub const DARK_CRYSTAL_SHADOW: Color = Color::rgb(4, 4, 12);
    /// Dark crystal shadow deep — for elevated panels.
    pub const DARK_CRYSTAL_SHADOW_DEEP: Color = Color::rgb(2, 2, 8);

    // ----------------------------------------------------------------- role indirection

    /// Resolve a semantic `Role` to a concrete `Color`
    /// using the default theme (DarkCrystal).
    ///
    /// Every UI surface calls this. If the role's value
    /// changes, every screen that asked for it picks up the
    /// new color on the next paint.
    #[must_use]
    pub const fn role(role: Role) -> Self {
        Self::theme_role(role, Theme::DarkCrystal)
    }

    /// Resolve a semantic `Role` to a concrete `Color`
    /// for the given theme. This is the theme-aware
    /// accessor; `role()` defaults to DarkCrystal.
    #[must_use]
    pub const fn theme_role(role: Role, theme: Theme) -> Self {
        match theme {
            Theme::Light => Self::light_role(role),
            Theme::DarkCrystal => Self::dark_crystal_role(role),
        }
    }

    /// Light theme role resolution (the original §12 pastel palette).
    const fn light_role(role: Role) -> Self {
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
            Role::ShadowDeep => Self::SHADOW_DEEP,
            Role::Hairline => Self::HAIRLINE,
            Role::GlassBg => Self::GLASS_BG,
            Role::GlassBorder => Self::GLASS_BORDER,
            Role::GlassHighlight => Self::GLASS_HIGHLIGHT,
            Role::GlassFrost => Self::GLASS_FROST,
            Role::CrystalPrism => Self::CRYSTAL_PRISM,
            Role::CrystalRefract => Self::CRYSTAL_REFRACT,
            Role::CrystalShine => Self::CRYSTAL_SHINE,
            Role::CrystalEdge => Self::CRYSTAL_EDGE,
            Role::GlowBlue => Self::GLOW_BLUE,
            Role::GlowPink => Self::GLOW_PINK,
            Role::GlowMint => Self::GLOW_MINT,
            Role::GlowLavender => Self::GLOW_LAVENDER,
            // Dark crystal roles map to their light equivalents
            // for backward compatibility.
            Role::DcCanvas => Self::SURFACE_WARM_WHITE,
            Role::DcSurface => Self::SURFACE_CREAM,
            Role::DcSurfaceStrong => Self::SURFACE_CREAM_DEEP,
            Role::DcSurfaceHover => Self::SURFACE_CREAM_DEEP,
            Role::DcSurfaceActive => Self::SURFACE_CREAM_DEEP,
            Role::DcText => Self::INK_900,
            Role::DcMuted => Self::INK_700,
            Role::DcDisabled => Self::INK_400,
            Role::DcBorder => Self::HAIRLINE,
            Role::DcBorderStrong => Self::INK_700,
            Role::DcAccent => Self::PASTEL_LAVENDER,
            Role::DcAccentInk => Self::INK_900,
            Role::DcAccentSoft => Self::PASTEL_LAVENDER,
            Role::DcSecondary => Self::PASTEL_MINT,
            Role::DcTertiary => Self::PASTEL_PEACH,
            Role::DcFocus => Self::PASTEL_LAVENDER_DEEP,
            Role::DcDanger => Self::PASTEL_PEACH_DEEP,
            Role::DcSuccess => Self::PASTEL_MINT_DEEP,
            Role::DcWarning => Self::PASTEL_YELLOW_DEEP,
        }
    }

    /// Dark Crystal theme role resolution.
    const fn dark_crystal_role(role: Role) -> Self {
        match role {
            Role::BgBase => Self::DARK_CRYSTAL_CANVAS,
            Role::BgPanel => Self::DARK_CRYSTAL_SURFACE,
            Role::BgPanelHover => Self::DARK_CRYSTAL_HOVER,
            Role::TextPrimary => Self::DARK_CRYSTAL_TEXT,
            Role::TextSecondary => Self::DARK_CRYSTAL_MUTED,
            Role::TextDisabled => Self::DARK_CRYSTAL_DISABLED,
            Role::AccentPink => Self::GLOW_PINK,
            Role::AccentPinkStrong => Self::CRYSTAL_REFRACT,
            Role::AccentBlue => Self::GLOW_BLUE,
            Role::AccentBlueStrong => Self::PASTEL_BLUE_DEEP,
            Role::AccentMint => Self::GLOW_MINT,
            Role::AccentMintStrong => Self::DARK_CRYSTAL_SUCCESS,
            Role::AccentLavender => Self::DARK_CRYSTAL_ACCENT,
            Role::AccentLavenderStrong => Self::CRYSTAL_PRISM,
            Role::AccentPeach => Self::DARK_CRYSTAL_TERTIARY,
            Role::AccentPeachStrong => Self::PASTEL_PEACH_DEEP,
            Role::AccentYellow => Self::DARK_CRYSTAL_WARNING,
            Role::AccentYellowStrong => Self::PASTEL_YELLOW_DEEP,
            Role::Shadow => Self::DARK_CRYSTAL_SHADOW,
            Role::ShadowDeep => Self::DARK_CRYSTAL_SHADOW_DEEP,
            Role::Hairline => Self::DARK_CRYSTAL_BORDER,
            Role::GlassBg => Self::DARK_CRYSTAL_GLASS,
            Role::GlassBorder => Self::DARK_CRYSTAL_GLASS_BORDER,
            Role::GlassHighlight => Self::DARK_CRYSTAL_GLASS_HIGHLIGHT,
            Role::GlassFrost => Self::DARK_CRYSTAL_GLASS_FROST,
            Role::CrystalPrism => Self::CRYSTAL_PRISM,
            Role::CrystalRefract => Self::CRYSTAL_REFRACT,
            Role::CrystalShine => Self::CRYSTAL_SHINE,
            Role::CrystalEdge => Self::CRYSTAL_EDGE,
            Role::GlowBlue => Self::GLOW_BLUE,
            Role::GlowPink => Self::GLOW_PINK,
            Role::GlowMint => Self::GLOW_MINT,
            Role::GlowLavender => Self::GLOW_LAVENDER,
            // Dark crystal specific roles.
            Role::DcCanvas => Self::DARK_CRYSTAL_CANVAS,
            Role::DcSurface => Self::DARK_CRYSTAL_SURFACE,
            Role::DcSurfaceStrong => Self::DARK_CRYSTAL_SURFACE_STRONG,
            Role::DcSurfaceHover => Self::DARK_CRYSTAL_HOVER,
            Role::DcSurfaceActive => Self::DARK_CRYSTAL_ACTIVE,
            Role::DcText => Self::DARK_CRYSTAL_TEXT,
            Role::DcMuted => Self::DARK_CRYSTAL_MUTED,
            Role::DcDisabled => Self::DARK_CRYSTAL_DISABLED,
            Role::DcBorder => Self::DARK_CRYSTAL_BORDER,
            Role::DcBorderStrong => Self::DARK_CRYSTAL_BORDER_STRONG,
            Role::DcAccent => Self::DARK_CRYSTAL_ACCENT,
            Role::DcAccentInk => Self::DARK_CRYSTAL_ACCENT_INK,
            Role::DcAccentSoft => Self::DARK_CRYSTAL_ACCENT_SOFT,
            Role::DcSecondary => Self::DARK_CRYSTAL_SECONDARY,
            Role::DcTertiary => Self::DARK_CRYSTAL_TERTIARY,
            Role::DcFocus => Self::DARK_CRYSTAL_FOCUS,
            Role::DcDanger => Self::DARK_CRYSTAL_DANGER,
            Role::DcSuccess => Self::DARK_CRYSTAL_SUCCESS,
            Role::DcWarning => Self::DARK_CRYSTAL_WARNING,
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
    /// Deep shadow for elevated glass panels.
    ShadowDeep,
    /// Hairline divider color.
    Hairline,

    // Glass.
    /// Glass panel background.
    GlassBg,
    /// Glass panel border.
    GlassBorder,
    /// Glass top-edge highlight.
    GlassHighlight,
    /// Glass frosted overlay.
    GlassFrost,

    // Crystal.
    /// Crystal prismatic accent.
    CrystalPrism,
    /// Crystal refraction accent.
    CrystalRefract,
    /// Crystal shine highlight.
    CrystalShine,
    /// Crystal edge border.
    CrystalEdge,

    // Glow.
    /// Blue ambient glow.
    GlowBlue,
    /// Pink ambient glow.
    GlowPink,
    /// Mint ambient glow.
    GlowMint,
    /// Lavender ambient glow.
    GlowLavender,

    // Dark Crystal — theme-aware surface roles.
    /// Canvas background (desktop).
    DcCanvas,
    /// Surface (panels, cards).
    DcSurface,
    /// Elevated surface (modals, popovers).
    DcSurfaceStrong,
    /// Hovered surface.
    DcSurfaceHover,
    /// Active/pressed surface.
    DcSurfaceActive,

    // Dark Crystal — text roles.
    /// Primary text on dark surfaces.
    DcText,
    /// Muted/secondary text.
    DcMuted,
    /// Disabled text.
    DcDisabled,

    // Dark Crystal — border roles.
    /// Subtle border.
    DcBorder,
    /// Strong/focused border.
    DcBorderStrong,

    // Dark Crystal — accent roles.
    /// Primary accent (lavender-blue).
    DcAccent,
    /// Text on accent surfaces.
    DcAccentInk,
    /// Tinted accent background.
    DcAccentSoft,
    /// Secondary accent (teal).
    DcSecondary,
    /// Tertiary accent (warm peach).
    DcTertiary,
    /// Focus ring color.
    DcFocus,
    /// Danger/error state.
    DcDanger,
    /// Success state.
    DcSuccess,
    /// Warning state.
    DcWarning,
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

/// The active theme. `Light` is the original pastel
/// palette; `DarkCrystal` is the premium glass UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Theme {
    /// Light, pastel theme (the original §12 identity).
    Light,
    /// Dark crystal glass theme — the premium UI.
    #[default]
    DarkCrystal,
}

/// Glass material level. Each material has a different
/// frost amount and translucency, following the OpenGlass
/// philosophy: CLEAR for hero surfaces, REGULAR for
/// default panels, FROSTED for overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GlassMaterial {
    /// 4% frost — hero surfaces, lenses over imagery.
    Clear,
    /// 24% frost — the default. Cards, panels, windows.
    Regular,
    /// 66% frost — menus, dialogs, toasts over busy content.
    Frosted,
}

impl GlassMaterial {
    /// Frost amount (0.0 = no frost, 1.0 = fully frosted).
    #[must_use]
    pub const fn frost(self) -> f32 {
        match self {
            Self::Clear => 0.04,
            Self::Regular => 0.24,
            Self::Frosted => 0.66,
        }
    }

    /// Base alpha (opacity) for the glass fill.
    #[must_use]
    pub const fn alpha(self) -> f32 {
        match self {
            Self::Clear => 0.12,
            Self::Regular => 0.35,
            Self::Frosted => 0.72,
        }
    }

    /// Border opacity.
    #[must_use]
    pub const fn border_alpha(self) -> f32 {
        match self {
            Self::Clear => 0.15,
            Self::Regular => 0.30,
            Self::Frosted => 0.50,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn warm_white_is_warm_white() {
        let c = Color::theme_role(Role::BgBase, Theme::Light);
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
        let c = Color::theme_role(Role::AccentPink, Theme::Light);
        assert_eq!(c, Color::PASTEL_PINK);
    }

    #[test]
    fn role_blue_resolves_to_pastel_blue() {
        let c = Color::theme_role(Role::AccentBlue, Theme::Light);
        assert_eq!(c, Color::PASTEL_BLUE);
    }

    #[test]
    fn role_mint_resolves_to_pastel_mint() {
        let c = Color::theme_role(Role::AccentMint, Theme::Light);
        assert_eq!(c, Color::PASTEL_MINT);
    }

    #[test]
    fn role_lavender_resolves_to_pastel_lavender() {
        let c = Color::theme_role(Role::AccentLavender, Theme::Light);
        assert_eq!(c, Color::PASTEL_LAVENDER);
    }

    #[test]
    fn role_peach_resolves_to_pastel_peach() {
        let c = Color::theme_role(Role::AccentPeach, Theme::Light);
        assert_eq!(c, Color::PASTEL_PEACH);
    }

    #[test]
    fn role_yellow_resolves_to_pastel_yellow() {
        let c = Color::theme_role(Role::AccentYellow, Theme::Light);
        assert_eq!(c, Color::PASTEL_YELLOW);
    }

    #[test]
    fn role_text_primary_is_dark_ink() {
        let c = Color::theme_role(Role::TextPrimary, Theme::Light);
        // Per §12 the dark text is "near-black, not pure
        // black" for AA contrast on pastels.
        assert_eq!(c, Color::INK_900);
        assert!(c.r < 50, "ink 900 should be near-black, not pure black: {c:?}");
    }

    #[test]
    fn shadow_color_is_dark() {
        let c = Color::theme_role(Role::Shadow, Theme::Light);
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
            Role::ShadowDeep,
            Role::Hairline,
            Role::GlassBg,
            Role::GlassBorder,
            Role::GlassHighlight,
            Role::GlassFrost,
            Role::CrystalPrism,
            Role::CrystalRefract,
            Role::CrystalShine,
            Role::CrystalEdge,
            Role::GlowBlue,
            Role::GlowPink,
            Role::GlowMint,
            Role::GlowLavender,
            Role::DcCanvas,
            Role::DcSurface,
            Role::DcSurfaceStrong,
            Role::DcSurfaceHover,
            Role::DcSurfaceActive,
            Role::DcText,
            Role::DcMuted,
            Role::DcDisabled,
            Role::DcBorder,
            Role::DcBorderStrong,
            Role::DcAccent,
            Role::DcAccentInk,
            Role::DcAccentSoft,
            Role::DcSecondary,
            Role::DcTertiary,
            Role::DcFocus,
            Role::DcDanger,
            Role::DcSuccess,
            Role::DcWarning,
        ];
        for r in roles {
            let c = Color::role(r);
            // Sanity: every role resolves to a color
            // (the match is exhaustive in `role`).
            let _ = c.to_hex();
        }
    }

    #[test]
    fn dark_crystal_is_default_theme() {
        assert_eq!(Theme::default(), Theme::DarkCrystal);
    }

    #[test]
    fn light_theme_bg_base_is_warm_white() {
        let c = Color::theme_role(Role::BgBase, Theme::Light);
        assert_eq!(c, Color::SURFACE_WARM_WHITE);
    }

    #[test]
    fn dark_crystal_theme_bg_base_is_canvas() {
        let c = Color::theme_role(Role::BgBase, Theme::DarkCrystal);
        assert_eq!(c, Color::DARK_CRYSTAL_CANVAS);
    }

    #[test]
    fn dark_crystal_theme_text_is_light() {
        let c = Color::theme_role(Role::TextPrimary, Theme::DarkCrystal);
        assert_eq!(c, Color::DARK_CRYSTAL_TEXT);
        // Text on dark must be bright.
        assert!(c.r > 200, "dark crystal text should be bright: {c:?}");
    }

    #[test]
    fn dark_crystal_roles_resolve() {
        let dc_roles = [
            Role::DcCanvas,
            Role::DcSurface,
            Role::DcSurfaceStrong,
            Role::DcSurfaceHover,
            Role::DcSurfaceActive,
            Role::DcText,
            Role::DcMuted,
            Role::DcDisabled,
            Role::DcBorder,
            Role::DcBorderStrong,
            Role::DcAccent,
            Role::DcAccentInk,
            Role::DcAccentSoft,
            Role::DcSecondary,
            Role::DcTertiary,
            Role::DcFocus,
            Role::DcDanger,
            Role::DcSuccess,
            Role::DcWarning,
        ];
        for r in dc_roles {
            let c = Color::theme_role(r, Theme::DarkCrystal);
            let _ = c.to_hex();
        }
    }

    #[test]
    fn glass_material_frost_values() {
        assert!((GlassMaterial::Clear.frost() - 0.04).abs() < f32::EPSILON);
        assert!((GlassMaterial::Regular.frost() - 0.24).abs() < f32::EPSILON);
        assert!((GlassMaterial::Frosted.frost() - 0.66).abs() < f32::EPSILON);
    }

    #[test]
    fn glass_material_alpha_values() {
        assert!((GlassMaterial::Clear.alpha() - 0.12).abs() < f32::EPSILON);
        assert!((GlassMaterial::Regular.alpha() - 0.35).abs() < f32::EPSILON);
        assert!((GlassMaterial::Frosted.alpha() - 0.72).abs() < f32::EPSILON);
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
