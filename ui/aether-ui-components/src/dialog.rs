//! Dialog — a modal with a title, body, and a set of
//! actions.
//!
//! A dialog is a `Card::Overlay` (`Xl` radius, soft
//! shadow) with a centered action row. Renderers
//! additionally paint a scrim behind the dialog; the
//! component layer doesn't own the scrim.
//!
//! The action row is a list of `Button`s laid out
//! right-to-left, primary on the right. The dialog
//! carries the *intent*; the renderer / layout pass
//! translates each `DialogAction` into a `Button`.

extern crate alloc;

use aether_design_tokens::{Color, Radius, Role, Spacing, TypeScale};

use crate::{
    button::{ButtonSize, ButtonVariant},
    Card, Component, ComponentStyle, Insets, LayoutBox,
};

/// A single action in a dialog's action row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialogAction {
    /// The label shown on the button.
    pub label: String,
    /// The button variant.
    pub variant: ButtonVariant,
    /// The action's stable identifier (e.g.
    /// `"consent.allow"`, `"install.cancel"`). The
    /// renderer emits this on click; the component
    /// layer never interprets it.
    pub id: String,
}

impl DialogAction {
    /// Construct a primary action.
    #[must_use]
    pub fn primary(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), variant: ButtonVariant::Primary }
    }

    /// Construct a secondary action.
    #[must_use]
    pub fn secondary(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), variant: ButtonVariant::Secondary }
    }

    /// Construct a ghost action.
    #[must_use]
    pub fn ghost(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), variant: ButtonVariant::Ghost }
    }
}

/// A dialog.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Dialog {
    /// Top-left of the dialog in the parent surface.
    pub origin: (i32, i32),
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels (callers usually set this to
    /// fit the body content + action row + padding).
    pub height: u32,
    /// The dialog's title. Always present.
    pub title: String,
    /// The body text. Multi-line, plain text.
    pub body: String,
    /// The action row, right-to-left.
    pub actions: alloc::vec::Vec<DialogAction>,
}

impl Dialog {
    /// Construct a dialog with the §12 default internal
    /// padding (`Xl` on all sides).
    #[must_use]
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            origin: (0, 0),
            width: 0,
            height: 0,
            title: title.into(),
            body: body.into(),
            actions: alloc::vec::Vec::new(),
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
    pub fn with_size(mut self, w: u32, h: u32) -> Self {
        self.width = w;
        self.height = h;
        self
    }

    /// Add a primary action.
    #[must_use]
    pub fn primary_action(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.actions.push(DialogAction::primary(id, label));
        self
    }

    /// Add a secondary action.
    #[must_use]
    pub fn secondary_action(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.actions.push(DialogAction::secondary(id, label));
        self
    }

    /// Add a ghost action.
    #[must_use]
    pub fn ghost_action(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.actions.push(DialogAction::ghost(id, label));
        self
    }

    /// The action row's height: large button + 2 * Lg padding.
    #[must_use]
    pub fn action_row_height_px() -> u32 {
        ButtonSize::Large.height_px() + 2 * Spacing::Lg.px_u32()
    }
}

impl Component for Dialog {
    fn layout(&self) -> LayoutBox {
        LayoutBox::new(self.origin.0, self.origin.1, self.width, self.height)
    }

    fn style(&self) -> ComponentStyle {
        // Dialogs are overlay cards: BgBase fill, soft
        // shadow border, Xl radius.
        ComponentStyle::from_roles(Role::BgBase, Role::TextPrimary, Role::Shadow, Radius::Xl)
    }

    fn padding(&self) -> Insets {
        Insets::even(Spacing::Xl.px())
    }
}

/// The title's type scale (`Heading`, 24 px). Renderers
/// use this for the dialog's title text.
#[must_use]
pub fn title_type() -> TypeScale {
    TypeScale::Heading
}

/// The body's type scale (`Body`, 14 px). Renderers
/// use this for the dialog's body text.
#[must_use]
pub fn body_type() -> TypeScale {
    TypeScale::Body
}

/// Convert a `Dialog` into a `Card` with the same outer
/// geometry. Some renderers prefer the card abstraction;
/// this keeps the two in sync.
#[must_use]
pub fn dialog_as_card(d: &Dialog) -> Card {
    let mut c = Card::new()
        .at(d.origin.0, d.origin.1)
        .with_size(d.width, d.height)
        .with_padding(Insets::even(Spacing::Xl.px()));
    if !d.title.is_empty() {
        c = c.title(d.title.clone());
    }
    c
}

/// A simple scrim color. Renderers paint a translucent
/// rect of this color behind the dialog.
pub const SCRIM: Color = Color::SHADOW_SOFT;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn new_dialog_has_empty_actions() {
        let d = Dialog::new("Allow camera?", "Sample needs camera access.");
        assert!(d.actions.is_empty());
        assert_eq!(d.title, "Allow camera?");
        assert_eq!(d.body, "Sample needs camera access.");
    }

    #[test]
    fn primary_action_appends() {
        let d = Dialog::new("t", "b").primary_action("consent.allow", "Allow");
        assert_eq!(d.actions.len(), 1);
        assert_eq!(d.actions[0].variant, ButtonVariant::Primary);
    }

    #[test]
    fn secondary_and_ghost_actions_stack() {
        let d = Dialog::new("t", "b")
            .primary_action("a.allow", "Allow")
            .secondary_action("a.deny", "Deny")
            .ghost_action("a.help", "Help");
        assert_eq!(d.actions.len(), 3);
        assert_eq!(d.actions[0].variant, ButtonVariant::Primary);
        assert_eq!(d.actions[1].variant, ButtonVariant::Secondary);
        assert_eq!(d.actions[2].variant, ButtonVariant::Ghost);
    }

    #[test]
    fn dialog_uses_xl_radius() {
        let d = Dialog::new("t", "b");
        let s = d.style();
        assert_eq!(s.radius, Radius::Xl);
    }

    #[test]
    fn dialog_uses_shadow_border() {
        let d = Dialog::new("t", "b");
        let s = d.style();
        assert_eq!(s.border, Color::role(Role::Shadow));
    }

    #[test]
    fn dialog_padding_is_xl() {
        let d = Dialog::new("t", "b");
        let p = d.padding();
        assert_eq!(p.top, Spacing::Xl.px());
        assert_eq!(p.left, Spacing::Xl.px());
    }

    #[test]
    fn action_row_height_is_large_button_plus_padding() {
        let expected = ButtonSize::Large.height_px() + 2 * Spacing::Lg.px_u32();
        assert_eq!(Dialog::action_row_height_px(), expected);
    }

    #[test]
    fn title_and_body_types() {
        assert_eq!(title_type(), TypeScale::Heading);
        assert_eq!(body_type(), TypeScale::Body);
    }

    #[test]
    fn dialog_as_card_carries_origin_and_size() {
        let d = Dialog::new("t", "b").at(40, 50).with_size(300, 200);
        let c = dialog_as_card(&d);
        assert_eq!(c.layout().x, 40);
        assert_eq!(c.layout().y, 50);
        assert_eq!(c.layout().width, 300);
        assert_eq!(c.layout().height, 200);
    }

    #[test]
    fn scrim_is_shadow_color() {
        assert_eq!(SCRIM, Color::role(Role::Shadow));
    }
}
