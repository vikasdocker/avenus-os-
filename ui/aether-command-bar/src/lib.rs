//! Aether AI Command Bar — the prompt surface.
//!
//! The AI Command Bar is the *type-to-AI* surface. It is
//! distinct from the launcher: the launcher answers
//! "where do I go next?", the command bar answers
//! "what do I want Aether to do?". They share a visual
//! language (the same three-mode rail, the same
//! pastels) but the command bar is always-on, always
//! focused, and the AI is its primary mode (Apps and
//! Files are secondary paths into the launcher).
//!
//! Composition:
//!
//! ```text
//!   +---------------------------------------------+
//!   |  Apps  Files |  Ask Aether ......... [Send] |
//!   +---------------------------------------------+
//! ```
//!
//! The bar is a single horizontal surface: a `Nav` of
//! mode tabs on the left, a `SearchBox`-style text
//! input in the middle, and a `Button` (primary) on the
//! right. The bar carries a `CommandState` and a
//! `CommandAction` enum that maps user input into
//! `CommandView` transitions.
//!
//! The crate is *non-painting*: it produces a
//! `CommandView` value the renderer consumes. The same
//! value drives the headless test renderer, the
//! accessibility auditor, and the snapshot tests.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

pub mod action;
pub mod state;
pub mod view;

pub use action::CommandAction;
pub use state::{CommandMode, CommandState};
pub use view::{CommandView, SubmitIntent};

use aether_design_tokens::{Color, Spacing};
use aether_ui_components::{
    ButtonSize, Component, ComponentStyle, Insets, LayoutBox, Nav, NavItem, NavOrientation, Panel,
    PanelSide,
};

use alloc::string::String;
use alloc::vec::Vec;

/// The AI Command Bar's mode tabs. Three tabs (Apps,
/// Files, AI) — the same canonical order as the
/// launcher's mode rail, but the AI is the default
/// active tab because the command bar is the
/// type-to-AI surface.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandTabs {
    /// The horizontal `Nav` of mode tabs.
    pub nav: Nav,
}

impl CommandTabs {
    /// Construct a tab row for the given active mode.
    #[must_use]
    pub fn new(active: CommandMode) -> Self {
        let mut apps = NavItem::new("Apps");
        let mut files = NavItem::new("Files");
        let mut ai = NavItem::new("AI");
        match active {
            CommandMode::Apps => apps = apps.active(),
            CommandMode::Files => files = files.active(),
            CommandMode::Ai => ai = ai.active(),
        }
        let items = alloc::vec![apps, files, ai];
        let nav = Nav::new(NavOrientation::Horizontal).with_length(36).items(items);
        Self { nav }
    }

    /// Set the origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.nav = self.nav.at(x, y);
        self
    }

    /// The tabs' total width in pixels.
    #[must_use]
    pub fn width_px() -> u32 {
        // 3 tabs * (Body line + 2 * Lg padding) + 2 * Md gap.
        let tab = Nav::item_length_px();
        3 * tab + 2 * Spacing::Md.px_u32()
    }
}

impl Component for CommandTabs {
    fn layout(&self) -> LayoutBox {
        self.nav.layout()
    }

    fn style(&self) -> ComponentStyle {
        self.nav.style()
    }

    fn padding(&self) -> Insets {
        self.nav.padding()
    }
}

/// The text input that lives in the middle of the
/// command bar. A flat surface with a mode-aware
/// placeholder. The renderer paints the placeholder
/// when the field is empty and shows the blinking
/// cursor when the field is focused.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PromptField {
    /// Top-left in the parent surface.
    pub origin: (i32, i32),
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels. The §12 default is 40 px.
    pub height: u32,
    /// Current text. Empty string = no prompt.
    pub text: String,
    /// Whether the field has keyboard focus.
    pub focused: bool,
    /// Placeholder shown when `text` is empty.
    pub placeholder: String,
    /// Whether the field is currently in the
    /// `multiline` state (e.g. Shift+Enter inserted a
    /// newline). Renderers expand the height when
    /// multiline.
    pub multiline: bool,
}

impl PromptField {
    /// Construct a focused prompt field with the §12
    /// default height and an empty query.
    #[must_use]
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            origin: (0, 0),
            width: 0,
            height: Self::default_height_px(),
            text: String::new(),
            focused: true,
            placeholder: placeholder.into(),
            multiline: false,
        }
    }

    /// Set the origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.origin = (x, y);
        self
    }

    /// Set the width.
    #[must_use]
    pub fn with_width(mut self, w: u32) -> Self {
        self.width = w;
        self
    }

    /// Override the current text.
    #[must_use]
    pub fn text(mut self, t: impl Into<String>) -> Self {
        self.text = t.into();
        self
    }

    /// Mark the field as not focused.
    #[must_use]
    pub fn blurred(mut self) -> Self {
        self.focused = false;
        self
    }

    /// Mark the field as multiline.
    #[must_use]
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    /// §12 default prompt-field height.
    #[must_use]
    pub fn default_height_px() -> u32 {
        40
    }
}

impl Component for PromptField {
    fn layout(&self) -> LayoutBox {
        LayoutBox::new(self.origin.0, self.origin.1, self.width, self.height)
    }

    fn style(&self) -> ComponentStyle {
        use aether_design_tokens::Role;
        if self.focused {
            ComponentStyle::from_roles(
                Role::BgBase,
                Role::TextPrimary,
                Role::AccentLavenderStrong,
                aether_design_tokens::Radius::Lg,
            )
        } else {
            ComponentStyle::from_roles(
                Role::BgBase,
                Role::TextPrimary,
                Role::Hairline,
                aether_design_tokens::Radius::Lg,
            )
        }
    }

    fn padding(&self) -> Insets {
        Insets::symmetric(Spacing::Sm.px(), Spacing::Lg.px())
    }
}

/// The submit button on the right edge of the command
/// bar. A `Button::Primary` of `ButtonSize::Large` so
/// it visually anchors the bar.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SendButton {
    /// Top-left in the parent surface.
    pub origin: (i32, i32),
    /// Width in pixels.
    pub width: u32,
    /// Whether the button is enabled. Disabled when the
    /// prompt field is empty.
    pub enabled: bool,
    /// Whether the button is currently focused.
    pub focused: bool,
}

impl SendButton {
    /// Construct a default, enabled button.
    #[must_use]
    pub fn new() -> Self {
        Self { origin: (0, 0), width: 0, enabled: true, focused: false }
    }

    /// Set the origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.origin = (x, y);
        self
    }

    /// Set the width.
    #[must_use]
    pub fn with_width(mut self, w: u32) -> Self {
        self.width = w;
        self
    }

    /// Disable the button.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Mark the button as focused.
    #[must_use]
    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }

    /// The §12 default send-button width. Sized so
    /// "Send" fits comfortably in body type.
    #[must_use]
    pub fn default_width_px() -> u32 {
        96
    }
}

impl Default for SendButton {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SendButton {
    fn layout(&self) -> LayoutBox {
        LayoutBox::new(
            self.origin.0,
            self.origin.1,
            self.width,
            ButtonSize::Large.height_px(),
        )
    }

    fn style(&self) -> ComponentStyle {
        use aether_design_tokens::Role;
        if !self.enabled {
            return ComponentStyle::from_roles(
                Role::BgPanel,
                Role::TextDisabled,
                Role::Hairline,
                aether_design_tokens::Radius::Md,
            );
        }
        if self.focused {
            return ComponentStyle::from_roles(
                Role::AccentBlue,
                Role::TextPrimary,
                Role::AccentBlueStrong,
                aether_design_tokens::Radius::Md,
            );
        }
        ComponentStyle::from_roles(
            Role::AccentBlue,
            Role::TextPrimary,
            Role::AccentBlueStrong,
            aether_design_tokens::Radius::Md,
        )
    }

    fn padding(&self) -> Insets {
        Insets::symmetric(
            ButtonSize::Large.vertical_padding_px(),
            ButtonSize::Large.horizontal_padding_px(),
        )
    }
}

/// The AI Command Bar surface. A `Panel::Top` (a thin
/// horizontal strip) carrying the mode tabs, the prompt
/// field, and the send button. The renderer reads the
/// resolved `CommandView` and paints each region.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandBarSurface {
    /// The backing panel (a thin top strip).
    pub panel: Panel,
    /// The mode tabs.
    pub tabs: CommandTabs,
    /// The prompt field.
    pub prompt: PromptField,
    /// The send button.
    pub send: SendButton,
    /// The current mode.
    pub mode: CommandMode,
    /// The current text.
    pub text: String,
}

impl CommandBarSurface {
    /// Construct the §12 default AI command bar in the
    /// given mode, with the mode-appropriate placeholder
    /// on the prompt field.
    #[must_use]
    pub fn new(mode: CommandMode) -> Self {
        let panel = Panel::new(PanelSide::Top);
        let tabs = CommandTabs::new(mode);
        let prompt = PromptField::new(mode.search_placeholder());
        let send = SendButton::new();
        Self { panel, tabs, prompt, send, mode, text: String::new() }
    }

    /// Set the origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.panel = self.panel.at(x, y);
        self
    }

    /// Set the size.
    #[must_use]
    pub fn with_size(mut self, w: u32, h: u32) -> Self {
        self.panel = self.panel.with_size(w, h);
        self
    }

    /// The tabs' `LayoutBox`, given the bar origin and
    /// total height.
    #[must_use]
    pub fn tabs_box(&self) -> LayoutBox {
        LayoutBox::new(
            self.panel.origin.0 + Spacing::Md.px(),
            self.panel.origin.1 + (self.panel.height as i32 - CommandTabs::width_px() as i32 / 8) / 2,
            CommandTabs::width_px(),
            self.panel.height.saturating_sub(Spacing::Md.px_u32() * 2),
        )
    }

    /// The prompt field's `LayoutBox`. Sits between the
    /// tabs and the send button.
    #[must_use]
    pub fn prompt_box(&self) -> LayoutBox {
        let tabs = self.tabs_box();
        let send = self.send_box();
        let x = tabs.right() + Spacing::Lg.px();
        let width = (send.x - x - Spacing::Lg.px()).max(0) as u32;
        LayoutBox::new(x, self.panel.origin.1 + Spacing::Md.px(), width, PromptField::default_height_px())
    }

    /// The send button's `LayoutBox`. Anchored to the
    /// right edge of the bar.
    #[must_use]
    pub fn send_box(&self) -> LayoutBox {
        let right_pad = Spacing::Md.px();
        let x = self.panel.origin.0 + self.panel.width as i32 - SendButton::default_width_px() as i32 - right_pad;
        LayoutBox::new(
            x,
            self.panel.origin.1 + Spacing::Md.px(),
            SendButton::default_width_px(),
            PromptField::default_height_px(),
        )
    }
}

// No helper needed; layout helpers are self-contained.

impl Component for CommandBarSurface {
    fn layout(&self) -> LayoutBox {
        self.panel.layout()
    }

    fn style(&self) -> ComponentStyle {
        self.panel.style()
    }

    fn padding(&self) -> Insets {
        self.panel.padding
    }
}

/// The color used to tint the bar's active-mode
/// indicator. Renderers may use this to draw a 1-px
/// hairline under the active tab. Pulled from the
/// design tokens.
#[must_use]
pub fn mode_indicator_color(mode: CommandMode) -> Color {
    use crate::state::CommandMode;
    match mode {
        CommandMode::Apps => Color::PASTEL_BLUE,
        CommandMode::Files => Color::PASTEL_MINT,
        CommandMode::Ai => Color::PASTEL_LAVENDER,
    }
}

/// The canonical mode order for the command bar's tab
/// row.
#[must_use]
pub fn default_mode_order() -> Vec<CommandMode> {
    alloc::vec![CommandMode::Apps, CommandMode::Files, CommandMode::Ai]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn new_command_bar_uses_top_panel() {
        let b = CommandBarSurface::new(CommandMode::Ai);
        assert_eq!(b.panel.side, PanelSide::Top);
    }

    #[test]
    fn new_command_bar_starts_with_ai_mode() {
        let b = CommandBarSurface::new(CommandMode::Ai);
        assert_eq!(b.mode, CommandMode::Ai);
    }

    #[test]
    fn new_command_bar_starts_with_empty_text() {
        let b = CommandBarSurface::new(CommandMode::Ai);
        assert!(b.text.is_empty());
        assert!(b.prompt.text.is_empty());
    }

    #[test]
    fn new_command_bar_prompt_is_focused() {
        let b = CommandBarSurface::new(CommandMode::Ai);
        assert!(b.prompt.focused);
    }

    #[test]
    fn command_tabs_have_three_items() {
        let t = CommandTabs::new(CommandMode::Files);
        assert_eq!(t.nav.items.len(), 3);
        assert_eq!(t.nav.items[0].label, "Apps");
        assert_eq!(t.nav.items[1].label, "Files");
        assert_eq!(t.nav.items[2].label, "AI");
    }

    #[test]
    fn active_mode_tab_is_marked() {
        let t = CommandTabs::new(CommandMode::Ai);
        assert!(t.nav.items[2].active);
        assert!(!t.nav.items[0].active);
    }

    #[test]
    fn command_tabs_are_horizontal() {
        let t = CommandTabs::new(CommandMode::Apps);
        assert_eq!(t.nav.orientation, NavOrientation::Horizontal);
    }

    #[test]
    fn prompt_field_default_height_is_40() {
        assert_eq!(PromptField::default_height_px(), 40);
    }

    #[test]
    fn prompt_focused_uses_lavender_border() {
        let p = PromptField::new("Ask Aether");
        let s = p.style();
        assert_eq!(
            s.border,
            Color::role(aether_design_tokens::Role::AccentLavenderStrong)
        );
    }

    #[test]
    fn prompt_unfocused_uses_hairline() {
        let p = PromptField::new("Ask Aether").blurred();
        let s = p.style();
        assert_eq!(s.border, Color::role(aether_design_tokens::Role::Hairline));
    }

    #[test]
    fn send_button_default_is_enabled() {
        let s = SendButton::new();
        assert!(s.enabled);
    }

    #[test]
    fn send_button_disabled_uses_panel() {
        let s = SendButton::new().disabled();
        let sty = s.style();
        assert_eq!(sty.fill, Color::role(aether_design_tokens::Role::BgPanel));
    }

    #[test]
    fn send_button_enabled_uses_accent_blue() {
        let s = SendButton::new();
        let sty = s.style();
        assert_eq!(sty.fill, Color::role(aether_design_tokens::Role::AccentBlue));
    }

    #[test]
    fn send_button_default_width_is_96() {
        assert_eq!(SendButton::default_width_px(), 96);
    }

    #[test]
    fn surface_layout_lays_three_regions() {
        let b = CommandBarSurface::new(CommandMode::Ai)
            .at(0, 0)
            .with_size(800, 56);
        let tabs = b.tabs_box();
        let prompt = b.prompt_box();
        let send = b.send_box();
        // Tabs < prompt < send in x.
        assert!(tabs.right() < prompt.x);
        assert!(prompt.right() <= send.x);
    }

    #[test]
    fn mode_indicator_color_uses_mode_pastel() {
        assert_eq!(mode_indicator_color(CommandMode::Apps), Color::PASTEL_BLUE);
        assert_eq!(mode_indicator_color(CommandMode::Files), Color::PASTEL_MINT);
        assert_eq!(mode_indicator_color(CommandMode::Ai), Color::PASTEL_LAVENDER);
    }

    #[test]
    fn default_mode_order_is_apps_files_ai() {
        let o = default_mode_order();
        assert_eq!(o.len(), 3);
        assert_eq!(o[0], CommandMode::Apps);
        assert_eq!(o[1], CommandMode::Files);
        assert_eq!(o[2], CommandMode::Ai);
    }
}
