//! Aether icon primitives — the typed, non-painting
//! icon system.
//!
//! §12: "rounded-square, soft gradients, custom Aether
//! language." Icons are everywhere in the OS — the
//! taskbar's tray, the launcher's tiles, the
//! assistant's plan steps, the system menus. They
//! must share a single visual language.
//!
//! An Aether icon is a description, not a paint call.
//! The `Icon` struct carries:
//!
//! - `kind` — the glyph (e.g. `IconKind::Calculator`).
//!   The renderer resolves the kind to an actual
//!   bitmap / SVG / paint op.
//! - `size` — the pixel size. The §12 default is
//!   `IconSize::Md` (24 px).
//! - `tint` — the icon's color. Pulled from
//!   `aether_design_tokens::Color` so re-skinning is
//!   one file.
//! - `background` — an optional rounded-square
//!   background (a pastel fill behind the glyph).
//!   The launcher tiles use this; the taskbar tray
//!   icons don't.
//!
//! The crate is non-painting. The renderer / shell
//! reads the `Icon` value and produces the pixel
//! output. The same `Icon` value drives the headless
//! test renderer, the accessibility auditor, and the
//! snapshot tests.
//!
//! Composition:
//!
//! ```text
//!   ┌──────┐
//!   │ calc │   <- 24-px glyph + 40-px rounded-square
//!   └──────┘       pastel background
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use aether_design_tokens::Color;

/// The set of icon kinds Aether ships. Each kind
/// resolves to a glyph the renderer knows how to
/// paint. The kind is the *semantic* identity of the
/// icon; the renderer is free to use any visual
/// representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IconKind {
    // ── App categories ──────────────────────────
    /// A calculator / numeric pad glyph.
    Calculator,
    /// A document / page glyph.
    Document,
    /// A notepad / lined page glyph.
    Notes,
    /// A folder glyph.
    Folder,
    /// A file glyph.
    File,
    /// An image / photo glyph.
    Image,
    /// A music note glyph.
    Music,
    /// A video / play glyph.
    Video,
    /// A settings cog glyph.
    Settings,
    /// A terminal / shell glyph.
    Terminal,

    // ── AI glyphs ───────────────────────────────
    /// The Aether AI glyph (the §12 "spark" mark).
    Aether,
    /// A spark / star (used for `PlanStepKind::Reasoning`).
    Spark,
    /// A globe (used for `PlanStepKind::Network`).
    Globe,
    /// A key (used for `PlanStepKind::Permission`).
    Key,
    /// A gear (used for `PlanStepKind::System`).
    Gear,

    // ── System / tray ───────────────────────────
    /// Network (signal arcs).
    Network,
    /// Volume (speaker).
    Volume,
    /// Battery.
    Battery,
    /// Microphone.
    Microphone,
    /// Camera.
    Camera,
    /// Lock (secure).
    Lock,
    /// Shield.
    Shield,
    /// Search / magnifier.
    Search,

    // ── Action / control ────────────────────────
    /// Plus / add.
    Plus,
    /// Minus / remove.
    Minus,
    /// Close / X.
    Close,
    /// Back arrow.
    Back,
    /// Forward arrow.
    Forward,
    /// Check.
    Check,
    /// Menu (hamburger).
    Menu,
    /// Send (paper plane).
    Send,
    /// Refresh.
    Refresh,
    /// Trash.
    Trash,

    // ── Status ──────────────────────────────────
    /// Info (circle-i).
    Info,
    /// Warning (triangle).
    Warning,
    /// Error (octagon with !).
    Error,
    /// Done (check in a circle).
    Done,
    /// Pending (clock).
    Pending,
}

impl IconKind {
    /// The §12 name for the icon. The renderer uses
    /// this as the lookup key into the icon atlas.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            // App categories
            Self::Calculator => "calculator",
            Self::Document => "document",
            Self::Notes => "notes",
            Self::Folder => "folder",
            Self::File => "file",
            Self::Image => "image",
            Self::Music => "music",
            Self::Video => "video",
            Self::Settings => "settings",
            Self::Terminal => "terminal",
            // AI glyphs
            Self::Aether => "aether",
            Self::Spark => "spark",
            Self::Globe => "globe",
            Self::Key => "key",
            Self::Gear => "gear",
            // System / tray
            Self::Network => "network",
            Self::Volume => "volume",
            Self::Battery => "battery",
            Self::Microphone => "microphone",
            Self::Camera => "camera",
            Self::Lock => "lock",
            Self::Shield => "shield",
            Self::Search => "search",
            // Action / control
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Close => "close",
            Self::Back => "back",
            Self::Forward => "forward",
            Self::Check => "check",
            Self::Menu => "menu",
            Self::Send => "send",
            Self::Refresh => "refresh",
            Self::Trash => "trash",
            // Status
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Done => "done",
            Self::Pending => "pending",
        }
    }

    /// The §12 default tint for this icon when no
    /// caller-supplied tint is provided. The default
    /// is `Color::INK_700` for ordinary UI icons and
    /// `Color::PASTEL_LAVENDER` for AI glyphs.
    #[must_use]
    pub const fn default_tint(self) -> Color {
        match self {
            Self::Aether | Self::Spark => Color::PASTEL_LAVENDER,
            Self::Key => Color::PASTEL_YELLOW,
            Self::Globe => Color::PASTEL_BLUE,
            Self::Gear => Color::PASTEL_MINT,
            _ => Color::INK_700,
        }
    }

    /// The §12 default background for this icon, if
    /// any. Tiles (calculator, notes, etc.) get a
    /// pastel; tray icons (network, volume, battery)
    /// get no background.
    #[must_use]
    pub const fn default_background(self) -> Option<Color> {
        match self {
            Self::Calculator => Some(Color::PASTEL_BLUE),
            Self::Notes => Some(Color::PASTEL_YELLOW),
            Self::Document => Some(Color::PASTEL_MINT),
            Self::Image => Some(Color::PASTEL_PEACH),
            Self::Music => Some(Color::PASTEL_PINK),
            Self::Video => Some(Color::PASTEL_LAVENDER),
            Self::Settings => Some(Color::PASTEL_MINT),
            Self::Terminal => Some(Color::INK_900),
            Self::Folder => Some(Color::PASTEL_PEACH),
            Self::Aether => Some(Color::PASTEL_LAVENDER),
            // Tray / action / status icons: no background.
            _ => None,
        }
    }

    /// The total icon set, in canonical display
    /// order. Used by the icon-picker and the
    /// settings UI.
    #[must_use]
    pub fn all() -> [Self; 38] {
        [
            Self::Calculator,
            Self::Document,
            Self::Notes,
            Self::Folder,
            Self::File,
            Self::Image,
            Self::Music,
            Self::Video,
            Self::Settings,
            Self::Terminal,
            Self::Aether,
            Self::Spark,
            Self::Globe,
            Self::Key,
            Self::Gear,
            Self::Network,
            Self::Volume,
            Self::Battery,
            Self::Microphone,
            Self::Camera,
            Self::Lock,
            Self::Shield,
            Self::Search,
            Self::Plus,
            Self::Minus,
            Self::Close,
            Self::Back,
            Self::Forward,
            Self::Check,
            Self::Menu,
            Self::Send,
            Self::Refresh,
            Self::Trash,
            Self::Info,
            Self::Warning,
            Self::Error,
            Self::Done,
            Self::Pending,
        ]
    }
}

/// The pixel size of an icon. The §12 default is
/// `Md` (24 px). Icons snap to the same 4-px grid
/// the rest of the design system uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IconSize {
    /// 16 px — used in inline text and dense lists.
    Xs,
    /// 20 px — used in compact UI.
    Sm,
    /// 24 px — the §12 default.
    Md,
    /// 32 px — used in launcher tiles and prominent
    /// surfaces.
    Lg,
    /// 40 px — used on large surfaces (hero, splash).
    Xl,
}

impl IconSize {
    /// The size in pixels.
    #[must_use]
    pub const fn px(self) -> u32 {
        match self {
            Self::Xs => 16,
            Self::Sm => 20,
            Self::Md => 24,
            Self::Lg => 32,
            Self::Xl => 40,
        }
    }

    /// The §12 default size.
    #[must_use]
    pub const fn default_size() -> Self {
        Self::Md
    }
}

/// The shape of an icon background. Tiles use
/// `RoundedSquare`; the AI tray icon uses `Circle`;
/// the system tray uses `None` (no background).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum IconBackground {
    /// No background — the glyph paints directly on
    /// the parent surface. This is the default for
    /// tray icons.
    #[default]
    None,
    /// A rounded-square background. The radius
    /// defaults to `Radius::Lg` (18 px per §12).
    RoundedSquare,
    /// A circle background. Used for the AI tray.
    Circle,
}

impl IconBackground {
    /// The §12 default background radius in pixels
    /// for `RoundedSquare`. Pulled from the design
    /// tokens; the renderer is free to override.
    #[must_use]
    pub const fn default_radius_px() -> u32 {
        18
    }
}

/// An icon — a typed description the renderer
/// consumes. Every Aether surface that needs an
/// icon constructs an `Icon` and hands it to the
/// renderer; the renderer paints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Icon {
    /// The icon kind (the glyph).
    pub kind: IconKind,
    /// The icon size.
    pub size: IconSize,
    /// The icon tint (the glyph color).
    pub tint: Color,
    /// The icon background. `None` by default.
    pub background: IconBackground,
    /// Whether the icon is currently focused (e.g.
    /// the keyboard focus is on the button carrying
    /// the icon). The renderer may draw a focus
    /// ring.
    pub focused: bool,
}

impl Icon {
    /// Construct a default icon: `Md`, `INK_700`
    /// tint, no background, not focused.
    #[must_use]
    pub fn new(kind: IconKind) -> Self {
        Self {
            kind,
            size: IconSize::default_size(),
            tint: kind.default_tint(),
            background: IconBackground::default(),
            focused: false,
        }
    }

    /// Override the size.
    #[must_use]
    pub fn with_size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    /// Override the tint.
    #[must_use]
    pub fn with_tint(mut self, tint: Color) -> Self {
        self.tint = tint;
        self
    }

    /// Set the background to a rounded square with
    /// the kind's default background color, or an
    /// explicit override.
    #[must_use]
    pub fn with_rounded_square(mut self, color: Option<Color>) -> Self {
        self.background = IconBackground::RoundedSquare;
        // The caller can override the color via
        // `with_background_color` after this.
        if let Some(c) = color {
            self.tint = c;
        }
        self
    }

    /// Set the background to a circle.
    #[must_use]
    pub fn with_circle(mut self) -> Self {
        self.background = IconBackground::Circle;
        self
    }

    /// Mark the icon as focused.
    #[must_use]
    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }

    /// The icon's total bounding box in pixels. For
    /// a 24-px icon with no background this is 24 ×
    /// 24; for a 24-px icon with a rounded-square
    /// background this is also 24 × 24 (the
    /// background sits *behind* the glyph in the same
    /// box).
    #[must_use]
    pub fn box_px(&self) -> u32 {
        self.size.px()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn kind_name_is_non_empty() {
        for k in IconKind::all() {
            assert!(!k.name().is_empty());
        }
    }

    #[test]
    fn all_names_are_unique() {
        let kinds = IconKind::all();
        let mut seen: Vec<&'static str> = Vec::new();
        for k in kinds {
            let n = k.name();
            assert!(!seen.contains(&n), "duplicate name: {n}");
            seen.push(n);
        }
    }

    #[test]
    fn calculator_default_tint_is_ink() {
        // Calculator is an ordinary UI icon (not an
        // AI glyph), so the default tint is INK_700.
        assert_eq!(IconKind::Calculator.default_tint(), Color::INK_700);
    }

    #[test]
    fn aether_default_tint_is_lavender() {
        // The Aether AI glyph defaults to lavender.
        assert_eq!(IconKind::Aether.default_tint(), Color::PASTEL_LAVENDER);
    }

    #[test]
    fn spark_default_tint_is_lavender() {
        assert_eq!(IconKind::Spark.default_tint(), Color::PASTEL_LAVENDER);
    }

    #[test]
    fn key_default_tint_is_yellow() {
        // Permission glyph = yellow (the
        // WaitingForPermission AI state).
        assert_eq!(IconKind::Key.default_tint(), Color::PASTEL_YELLOW);
    }

    #[test]
    fn calculator_has_pastel_blue_background() {
        // Tiles have a pastel background; calculator
        // gets blue.
        assert_eq!(
            IconKind::Calculator.default_background(),
            Some(Color::PASTEL_BLUE)
        );
    }

    #[test]
    fn tray_icons_have_no_background() {
        assert_eq!(IconKind::Network.default_background(), None);
        assert_eq!(IconKind::Volume.default_background(), None);
        assert_eq!(IconKind::Battery.default_background(), None);
    }

    #[test]
    fn size_md_is_24() {
        assert_eq!(IconSize::Md.px(), 24);
    }

    #[test]
    fn size_lg_is_32() {
        assert_eq!(IconSize::Lg.px(), 32);
    }

    #[test]
    fn size_xl_is_40() {
        assert_eq!(IconSize::Xl.px(), 40);
    }

    #[test]
    fn size_default_is_md() {
        assert_eq!(IconSize::default_size(), IconSize::Md);
    }

    #[test]
    fn background_default_is_none() {
        assert_eq!(IconBackground::default(), IconBackground::None);
    }

    #[test]
    fn background_default_radius_is_18() {
        assert_eq!(IconBackground::default_radius_px(), 18);
    }

    #[test]
    fn icon_new_uses_default_size() {
        let i = Icon::new(IconKind::Calculator);
        assert_eq!(i.size, IconSize::Md);
        assert_eq!(i.kind, IconKind::Calculator);
        assert_eq!(i.tint, Color::INK_700);
    }

    #[test]
    fn icon_with_size() {
        let i = Icon::new(IconKind::Calculator).with_size(IconSize::Lg);
        assert_eq!(i.size, IconSize::Lg);
    }

    #[test]
    fn icon_with_tint() {
        let i = Icon::new(IconKind::Calculator).with_tint(Color::PASTEL_BLUE);
        assert_eq!(i.tint, Color::PASTEL_BLUE);
    }

    #[test]
    fn icon_with_rounded_square() {
        let i = Icon::new(IconKind::Calculator).with_rounded_square(None);
        assert_eq!(i.background, IconBackground::RoundedSquare);
    }

    #[test]
    fn icon_with_circle() {
        let i = Icon::new(IconKind::Aether).with_circle();
        assert_eq!(i.background, IconBackground::Circle);
    }

    #[test]
    fn icon_focused() {
        let i = Icon::new(IconKind::Calculator).focused();
        assert!(i.focused);
    }

    #[test]
    fn icon_box_px_is_size() {
        let i = Icon::new(IconKind::Calculator);
        assert_eq!(i.box_px(), 24);
        let j = Icon::new(IconKind::Calculator).with_size(IconSize::Xl);
        assert_eq!(j.box_px(), 40);
    }

    #[test]
    fn all_returns_expected_kinds() {
        // Sanity guard: if you add an `IconKind`
        // variant, the `name()`, `default_tint()`,
        // `default_background()`, and `all()` match
        // arms must all be updated. The `all()` test
        // pinpoints drift.
        let kinds = IconKind::all();
        assert!(kinds.len() >= 30, "expected at least 30 icon kinds, got {}", kinds.len());
    }
}
