//! Button — the primary interactive control.
//!
//! The §12 button family has three variants:
//!   * `Primary`   — the "go" action. Filled with the
//!                    `AccentBlue` pastel, white text.
//!   * `Secondary` — a non-default action. Filled with the
//!                    `BgPanel` cream surface, primary
//!                    text.
//!   * `Ghost`     — a tertiary action (e.g. "Cancel").
//!                    Transparent fill, hairline border,
//!                    primary text.
//!
//! And two sizes:
//!   * `Medium` — 36 px tall, body type. Default for chrome.
//!   * `Large`  — 44 px tall, subhead type. Default for the
//!                 AI command bar's primary action.

use aether_design_tokens::{Color, Radius, Role, Spacing, TypeScale};

use crate::{Component, ComponentStyle, Insets, LayoutBox};

/// Which button family a button belongs to. Drives the fill
/// color and the border.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ButtonVariant {
    /// Filled, primary action. `AccentBlue` fill.
    Primary,
    /// Filled, secondary action. `BgPanel` fill.
    Secondary,
    /// Outlined, tertiary action. Transparent fill,
    /// hairline border.
    Ghost,
}

/// Button size. Affects height, padding, and type scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ButtonSize {
    /// 36 px tall, body type. Chrome default.
    Medium,
    /// 44 px tall, subhead type. AI command bar default.
    Large,
}

impl ButtonSize {
    /// Height in pixels.
    #[must_use]
    pub const fn height_px(self) -> u32 {
        match self {
            Self::Medium => 36,
            Self::Large => 44,
        }
    }

    /// Vertical padding inside the button.
    #[must_use]
    pub const fn vertical_padding_px(self) -> i32 {
        match self {
            Self::Medium => Spacing::Sm.px(),
            Self::Large => Spacing::Md.px(),
        }
    }

    /// Horizontal padding inside the button.
    #[must_use]
    pub const fn horizontal_padding_px(self) -> i32 {
        match self {
            Self::Medium => Spacing::Lg.px(),
            Self::Large => Spacing::Xl.px(),
        }
    }

    /// The type scale the button's label uses.
    #[must_use]
    pub const fn type_scale(self) -> TypeScale {
        match self {
            Self::Medium => TypeScale::Body,
            Self::Large => TypeScale::Subhead,
        }
    }
}

/// A button. Width is the caller's choice (set by the
/// layout pass); height is fixed by the size.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Button {
    /// Top-left of the button in the parent surface.
    pub origin: (i32, i32),
    /// Width in pixels (height is set by `size`).
    pub width: u32,
    /// Visual + semantic family.
    pub variant: ButtonVariant,
    /// Height bucket.
    pub size: ButtonSize,
    /// Whether the button is currently the focused
    /// control (draws a focus ring).
    pub focused: bool,
    /// Whether the button is currently pressed (held
    /// under the pointer / a tap).
    pub pressed: bool,
    /// Whether the button is disabled (low-opacity label,
    /// `cursor: not-allowed`).
    pub disabled: bool,
    /// The label. The button grows to fit; callers that
    /// want a fixed width set `width` explicitly.
    pub label: String,
}

impl Button {
    /// Construct a primary, medium, non-focused button.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            origin: (0, 0),
            width: 0,
            variant: ButtonVariant::Primary,
            size: ButtonSize::Medium,
            focused: false,
            pressed: false,
            disabled: false,
            label: label.into(),
        }
    }

    /// Override the origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.origin = (x, y);
        self
    }

    /// Override the width (height is fixed by the size).
    #[must_use]
    pub fn with_width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    /// Override the variant.
    #[must_use]
    pub fn variant(mut self, v: ButtonVariant) -> Self {
        self.variant = v;
        self
    }

    /// Override the size.
    #[must_use]
    pub fn size(mut self, s: ButtonSize) -> Self {
        self.size = s;
        self
    }

    /// Mark the button as focused.
    #[must_use]
    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }

    /// Mark the button as pressed.
    #[must_use]
    pub fn pressed(mut self) -> Self {
        self.pressed = true;
        self
    }

    /// Mark the button as disabled.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

impl Component for Button {
    fn layout(&self) -> LayoutBox {
        LayoutBox::new(self.origin.0, self.origin.1, self.width, self.size.height_px())
    }

    fn style(&self) -> ComponentStyle {
        // The fill / text / border resolve through
        // `Color::role` so a re-skin picks up the new
        // identity. The disabled / pressed / focused
        // modifiers are layered on top.
        let base = match self.variant {
            ButtonVariant::Primary => {
                ComponentStyle::from_roles(
                    Role::AccentBlue,
                    Role::TextPrimary, // text on the deep accent
                    Role::AccentBlueStrong,
                    Radius::Md,
                )
            }
            ButtonVariant::Secondary => ComponentStyle::from_roles(
                Role::BgPanel,
                Role::TextPrimary,
                Role::Hairline,
                Radius::Md,
            ),
            ButtonVariant::Ghost => ComponentStyle::from_roles(
                Role::BgBase,
                Role::TextPrimary,
                Role::Hairline,
                Radius::Md,
            ),
        };
        // Pressed -> deeper accent.
        if self.pressed {
            let strong = match self.variant {
                ButtonVariant::Primary => Color::role(Role::AccentBlueStrong),
                ButtonVariant::Secondary => Color::role(Role::BgPanelHover),
                ButtonVariant::Ghost => Color::role(Role::BgPanelHover),
            };
            return ComponentStyle {
                fill: strong,
                text: if matches!(self.variant, ButtonVariant::Primary) {
                    Color::SURFACE_WARM_WHITE
                } else {
                    base.text
                },
                border: base.border,
                radius: base.radius,
            };
        }
        // Focused -> swap text to white on the
        // primary variant so the label stays legible.
        if self.focused && matches!(self.variant, ButtonVariant::Primary) {
            return ComponentStyle {
                fill: base.fill,
                text: Color::SURFACE_WARM_WHITE,
                border: Color::role(Role::AccentBlueStrong),
                radius: base.radius,
            };
        }
        base
    }

    fn padding(&self) -> Insets {
        Insets::symmetric(self.size.vertical_padding_px(), self.size.horizontal_padding_px())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn medium_height_is_36() {
        assert_eq!(ButtonSize::Medium.height_px(), 36);
    }

    #[test]
    fn large_height_is_44() {
        assert_eq!(ButtonSize::Large.height_px(), 44);
    }

    #[test]
    fn medium_uses_body_type() {
        assert_eq!(ButtonSize::Medium.type_scale(), TypeScale::Body);
    }

    #[test]
    fn large_uses_subhead_type() {
        assert_eq!(ButtonSize::Large.type_scale(), TypeScale::Subhead);
    }

    #[test]
    fn primary_button_uses_accent_blue() {
        let b = Button::new("OK");
        let s = b.style();
        assert_eq!(s.fill, Color::role(Role::AccentBlue));
    }

    #[test]
    fn secondary_button_uses_panel_background() {
        let b = Button::new("Cancel").variant(ButtonVariant::Secondary);
        let s = b.style();
        assert_eq!(s.fill, Color::role(Role::BgPanel));
    }

    #[test]
    fn ghost_button_uses_base_background() {
        let b = Button::new("More").variant(ButtonVariant::Ghost);
        let s = b.style();
        assert_eq!(s.fill, Color::role(Role::BgBase));
    }

    #[test]
    fn focused_primary_has_white_label() {
        let b = Button::new("OK").focused();
        let s = b.style();
        assert_eq!(s.text, Color::SURFACE_WARM_WHITE);
    }

    #[test]
    fn pressed_primary_uses_deep_accent() {
        let b = Button::new("OK").pressed();
        let s = b.style();
        assert_eq!(s.fill, Color::role(Role::AccentBlueStrong));
    }

    #[test]
    fn layout_returns_origin_and_width() {
        let b = Button::new("OK").at(10, 20).with_width(100);
        let l = b.layout();
        assert_eq!(l.x, 10);
        assert_eq!(l.y, 20);
        assert_eq!(l.width, 100);
        assert_eq!(l.height, ButtonSize::Medium.height_px());
    }

    #[test]
    fn padding_uses_size_tokens() {
        let m = Button::new("OK").style(); // force style compile
        let _ = m;
        let b = Button::new("OK");
        let p = b.padding();
        assert_eq!(p.top, ButtonSize::Medium.vertical_padding_px());
        assert_eq!(p.left, ButtonSize::Medium.horizontal_padding_px());
    }

    #[test]
    fn large_padding_uses_md_xl() {
        let b = Button::new("OK").size(ButtonSize::Large);
        let p = b.padding();
        assert_eq!(p.top, Spacing::Md.px());
        assert_eq!(p.left, Spacing::Xl.px());
    }
}
