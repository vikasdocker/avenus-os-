//! Agent Workspace — the full work area that shows
//! the agent's current plan and per-step state.
//!
//! The Workspace is the user's "what is the agent
//! doing right now and how does the plan look"
//! surface. It docks to the left of the Assistant
//! Panel (or takes the full screen on tablet form
//! factors) and renders:
//!
//! - A header: the plan's goal + a progress bar.
//! - A vertical timeline: one row per `PlanStep`,
//!   with the step's `PlanStepKind` glyph, title,
//!   and a state chip colored by the step's
//!   `AiVisualState`.
//! - A footer: the agent's overall `AiVisualState`
//!   (which may differ from any individual step's
//!   state — the agent might be in `Planning` while
//!   a step is `Working`).

use aether_design_tokens::{AiVisualState, Spacing};
use aether_ui_components::{Component, ComponentStyle, Insets, LayoutBox, Panel, PanelSide};

use crate::plan::{PlanStep, WorkspacePlan};

use alloc::string::String;

/// The state of the workspace at one moment. The
/// renderer reads this and produces a frame.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceViewState {
    /// The agent's overall `AiVisualState`. This is
    /// the "agent is …" indicator in the footer.
    pub state: AiVisualState,
    /// The plan to render.
    pub plan: WorkspacePlan,
    /// The index of the selected step, if any. The
    /// user selects a step to focus it; selecting a
    /// `Running` step opens its `TaskView`.
    pub selected: Option<usize>,
    /// An optional subtitle for the header (e.g. a
    /// timestamp or a session id). Empty = no
    /// subtitle.
    pub subtitle: String,
}

impl WorkspaceViewState {
    /// Construct a fresh state in `Idle` with an
    /// empty plan and no selection.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AiVisualState::Idle,
            plan: WorkspacePlan::new(""),
            selected: None,
            subtitle: String::new(),
        }
    }

    /// Override the AI state.
    #[must_use]
    pub fn with_state(mut self, state: AiVisualState) -> Self {
        self.state = state;
        self
    }

    /// Override the plan.
    #[must_use]
    pub fn with_plan(mut self, plan: WorkspacePlan) -> Self {
        self.plan = plan;
        self
    }

    /// Override the selected step.
    #[must_use]
    pub fn with_selected(mut self, idx: Option<usize>) -> Self {
        self.selected = idx;
        self
    }

    /// Override the subtitle.
    #[must_use]
    pub fn with_subtitle(mut self, sub: impl Into<String>) -> Self {
        self.subtitle = sub.into();
        self
    }

    /// Select the next step (down). Wraps at the
    /// end. Returns the new state.
    #[must_use]
    pub fn select_next(mut self) -> Self {
        let n = self.plan.steps.len();
        if n == 0 {
            self.selected = None;
            return self;
        }
        self.selected = Some(match self.selected {
            None => 0,
            Some(i) => (i + 1) % n,
        });
        self
    }

    /// Select the previous step (up). Wraps at the
    /// top.
    #[must_use]
    pub fn select_prev(mut self) -> Self {
        let n = self.plan.steps.len();
        if n == 0 {
            self.selected = None;
            return self;
        }
        self.selected = Some(match self.selected {
            None => n - 1,
            Some(0) => n - 1,
            Some(i) => i - 1,
        });
        self
    }

    /// The currently selected step, if any.
    #[must_use]
    pub fn selected_step(&self) -> Option<&PlanStep> {
        self.selected.and_then(|i| self.plan.steps.get(i))
    }
}

impl Default for WorkspaceViewState {
    fn default() -> Self {
        Self::new()
    }
}

/// The workspace surface — a `Panel::Left` (a wide
/// column) that hosts the plan header, the timeline
/// of steps, and the agent's overall state footer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceView {
    /// The backing left-side panel.
    pub panel: Panel,
    /// The resolved state at this frame.
    pub state: WorkspaceViewState,
}

impl WorkspaceView {
    /// The §12 default workspace width in pixels.
    pub const DEFAULT_WIDTH_PX: u32 = 480;

    /// Construct a fresh workspace in `Idle`.
    #[must_use]
    pub fn new() -> Self {
        let panel = Panel::new(PanelSide::Left).with_size(Self::DEFAULT_WIDTH_PX, 720);
        Self { panel, state: WorkspaceViewState::new() }
    }

    /// Override the state.
    #[must_use]
    pub fn with_state(mut self, state: WorkspaceViewState) -> Self {
        self.state = state;
        self
    }

    /// Set the panel's height in pixels.
    #[must_use]
    pub fn with_height(mut self, h: u32) -> Self {
        self.panel = self.panel.with_size(Self::DEFAULT_WIDTH_PX, h);
        self
    }

    /// Set the panel's origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.panel = self.panel.at(x, y);
        self
    }

    /// The header region: goal + progress bar.
    /// 96 px tall.
    #[must_use]
    pub fn header_box(&self) -> LayoutBox {
        LayoutBox::new(self.panel.origin.0, self.panel.origin.1, self.panel.width, 96)
    }

    /// The timeline region: the area between the
    /// header and the footer.
    #[must_use]
    pub fn timeline_box(&self) -> LayoutBox {
        let header = self.header_box();
        let footer = self.footer_box();
        let y = header.bottom() + Spacing::Md.px();
        let height = (footer.y - y).max(0) as u32;
        LayoutBox::new(self.panel.origin.0, y, self.panel.width, height)
    }

    /// The footer region: the agent's overall state
    /// indicator. 48 px tall.
    #[must_use]
    pub fn footer_box(&self) -> LayoutBox {
        let h = 48;
        let y = self.panel.origin.1 + self.panel.height as i32 - h as i32;
        LayoutBox::new(self.panel.origin.0, y, self.panel.width, h)
    }

    /// The layout for the step at the given index,
    /// in the timeline region. Each step is 56 px
    /// tall (Body line + 2 × Lg padding).
    #[must_use]
    pub fn step_box(&self, index: usize) -> LayoutBox {
        let timeline = self.timeline_box();
        let row_h: u32 = 56;
        let y = timeline.y + (row_h as i32 * index as i32);
        LayoutBox::new(timeline.x, y, timeline.width, row_h)
    }
}

impl Default for WorkspaceView {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for WorkspaceView {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::plan::{PlanStep, PlanStepKind};

    #[test]
    fn state_default_is_idle() {
        let s = WorkspaceViewState::new();
        assert_eq!(s.state, AiVisualState::Idle);
        assert_eq!(s.plan.total_count(), 0);
        assert!(s.selected.is_none());
    }

    #[test]
    fn state_select_next_empty_plan() {
        let s = WorkspaceViewState::new();
        let s2 = s.select_next();
        assert!(s2.selected.is_none());
    }

    #[test]
    fn state_select_next_cycles() {
        let s = WorkspaceViewState::new()
            .with_plan(
                WorkspacePlan::new("p")
                    .with_step(PlanStep::new(PlanStepKind::File, "a"))
                    .with_step(PlanStep::new(PlanStepKind::File, "b"))
                    .with_step(PlanStep::new(PlanStepKind::File, "c")),
            )
            .with_selected(Some(0));
        let s2 = s.select_next();
        assert_eq!(s2.selected, Some(1));
        let s3 = s2.select_next();
        assert_eq!(s3.selected, Some(2));
        let s4 = s3.select_next();
        assert_eq!(s4.selected, Some(0));
    }

    #[test]
    fn state_select_prev_wraps() {
        let s = WorkspaceViewState::new()
            .with_plan(WorkspacePlan::new("p").with_step(PlanStep::new(PlanStepKind::File, "a")))
            .with_selected(Some(0));
        let s2 = s.select_prev();
        assert_eq!(s2.selected, Some(0));
    }

    #[test]
    fn state_select_next_from_none() {
        let s = WorkspaceViewState::new()
            .with_plan(WorkspacePlan::new("p").with_step(PlanStep::new(PlanStepKind::File, "a")));
        let s2 = s.select_next();
        assert_eq!(s2.selected, Some(0));
    }

    #[test]
    fn state_selected_step_returns_step() {
        let s = WorkspaceViewState::new()
            .with_plan(
                WorkspacePlan::new("p")
                    .with_step(PlanStep::new(PlanStepKind::File, "a"))
                    .with_step(PlanStep::new(PlanStepKind::File, "b")),
            )
            .with_selected(Some(1));
        let step = s.selected_step().unwrap();
        assert_eq!(step.title, "b");
    }

    #[test]
    fn state_selected_step_none_when_no_selection() {
        let s = WorkspaceViewState::new();
        assert!(s.selected_step().is_none());
    }

    #[test]
    fn workspace_new_is_left_side() {
        let w = WorkspaceView::new();
        assert_eq!(w.panel.side, PanelSide::Left);
    }

    #[test]
    fn workspace_default_width_is_480() {
        let w = WorkspaceView::new();
        assert_eq!(w.panel.width, 480);
    }

    #[test]
    fn workspace_header_is_96() {
        let w = WorkspaceView::new().with_height(800);
        assert_eq!(w.header_box().height, 96);
    }

    #[test]
    fn workspace_footer_is_48() {
        let w = WorkspaceView::new().with_height(800);
        assert_eq!(w.footer_box().height, 48);
    }

    #[test]
    fn workspace_layout_stacks_three_regions() {
        let w = WorkspaceView::new().with_height(800);
        let h = w.header_box();
        let t = w.timeline_box();
        let f = w.footer_box();
        assert!(h.bottom() <= t.y);
        assert!(t.bottom() <= f.y);
    }

    #[test]
    fn workspace_step_box_is_56px() {
        let w = WorkspaceView::new().with_height(800);
        let s = w.step_box(0);
        assert_eq!(s.height, 56);
    }
}
