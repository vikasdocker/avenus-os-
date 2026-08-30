//! Card — a content surface.
//!
//! A card is a contained surface that holds a unit of
//! information: a launcher tile, a file preview, an AI
//! suggestion. §12 calls for "comfortable spacing" and
//! "soft shadows"; the default card has `Lg` radius and
//! a soft-shadow border color (the renderer paints the
//! actual shadow).
//!
//! Three elevations:
//!   * `Flat`    — no shadow, hairline border. Inline
//!                  cards (a metadata strip).
//!   * `Raised`  — soft shadow, no border. Default for
//!                  launcher tiles.
//!   * `Overlay` — stronger shadow. For cards on top of
//!                  other content (the AI assistant
//!                  panel over the desktop).

use aether_design_tokens::{Color, Radius, Role, Spacing};

use crate::{Component, ComponentStyle, Insets, LayoutBox};

/// Card elevation. Drives the shadow and border.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CardElevation {
    /// No shadow; hairline border.
    Flat,
    /// Soft shadow, no border. Default.
    Raised,
    /// Stronger shadow. For overlays.
    Overlay,
}

impl CardElevation {
    /// Corner radius for this elevation.
    #[must_use]
    pub const fn radius(self) -> Radius {
        match self {
            Self::Flat => Radius::Md,
            Self::Raised => Radius::Lg,
            Self::Overlay => Radius::Xl,
        }
    }
}

/// A card. Width and height are caller-controlled; the
/// card's content area is the layout box minus the
/// internal padding.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Card {
    /// Top-left in the parent surface.
    pub origin: (i32, i32),
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Elevation bucket.
    pub elevation: CardElevation,
    /// Internal padding (top/right/bottom/left).
    pub padding: Insets,
    /// Whether the card is the active / selected card
    /// in its container (the launcher uses this to
    /// highlight the focused tile).
    pub selected: bool,
    /// Optional title shown at the top of the card.
    /// Renderers may or may not display this; the
    /// component layer just carries it.
    pub title: Option<String>,
}

impl Card {
    /// Construct a raised card with the §12 default
    /// padding (`Lg` on all sides).
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: (0, 0),
            width: 0,
            height: 0,
            elevation: CardElevation::Raised,
            padding: Insets::even(Spacing::Lg.px()),
            selected: false,
            title: None,
        }
    }

    /// Set the origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.origin = (x, y);
        self
    }

    /// Set the size.
    #[must_use]
    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Override the elevation.
    #[must_use]
    pub fn elevation(mut self, e: CardElevation) -> Self {
        self.elevation = e;
        self
    }

    /// Override the internal padding.
    #[must_use]
    pub fn with_padding(mut self, p: Insets) -> Self {
        self.padding = p;
        self
    }

    /// Mark the card as selected.
    #[must_use]
    pub fn selected(mut self) -> Self {
        self.selected = true;
        self
    }

    /// Set an optional title.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Card {
    fn layout(&self) -> LayoutBox {
        LayoutBox::new(self.origin.0, self.origin.1, self.width, self.height)
    }

    fn style(&self) -> ComponentStyle {
        let base = match self.elevation {
            CardElevation::Flat => ComponentStyle::from_roles(
                Role::BgBase,
                Role::TextPrimary,
                Role::Hairline,
                Radius::Md,
            ),
            CardElevation::Raised => ComponentStyle::from_roles(
                Role::BgBase,
                Role::TextPrimary,
                Role::Shadow,
                Radius::Lg,
            ),
            CardElevation::Overlay => ComponentStyle::from_roles(
                Role::BgBase,
                Role::TextPrimary,
                Role::Shadow,
                Radius::Xl,
            ),
        };
        if self.selected {
            // Selected cards swap to the panel background
            // and use the lavender accent as the border.
            ComponentStyle {
                fill: Color::role(Role::BgPanel),
                text: base.text,
                border: Color::role(Role::AccentLavenderStrong),
                radius: base.radius,
            }
        } else {
            base
        }
    }

    fn padding(&self) -> Insets {
        self.padding
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn new_card_is_raised() {
        let c = Card::new();
        assert_eq!(c.elevation, CardElevation::Raised);
        assert!(!c.selected);
    }

    #[test]
    fn default_card_matches_new() {
        let c = Card::default();
        assert_eq!(c.elevation, CardElevation::Raised);
    }

    #[test]
    fn flat_card_uses_md_radius() {
        assert_eq!(CardElevation::Flat.radius(), Radius::Md);
    }

    #[test]
    fn raised_card_uses_lg_radius() {
        assert_eq!(CardElevation::Raised.radius(), Radius::Lg);
    }

    #[test]
    fn overlay_card_uses_xl_radius() {
        assert_eq!(CardElevation::Overlay.radius(), Radius::Xl);
    }

    #[test]
    fn raised_card_uses_shadow_border() {
        let c = Card::new();
        let s = c.style();
        assert_eq!(s.border, Color::role(Role::Shadow));
    }

    #[test]
    fn selected_card_uses_panel_and_lavender() {
        let c = Card::new().selected();
        let s = c.style();
        assert_eq!(s.fill, Color::role(Role::BgPanel));
        assert_eq!(s.border, Color::role(Role::AccentLavenderStrong));
    }

    #[test]
    fn layout_returns_origin_and_size() {
        let c = Card::new().at(5, 10).with_size(200, 100);
        let l = c.layout();
        assert_eq!(l.x, 5);
        assert_eq!(l.y, 10);
        assert_eq!(l.width, 200);
        assert_eq!(l.height, 100);
    }

    #[test]
    fn padding_override_is_honored() {
        let pad = Insets::even(8);
        let c = Card::new().with_padding(pad);
        assert_eq!(c.padding(), pad);
    }
}
