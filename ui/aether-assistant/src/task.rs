//! Task View — a focused view of a single in-progress
//! task: its inputs, outputs, the permission prompt
//! if it's blocked, and the accept / reject controls.
//!
//! §12 says the user must always be able to see what
//! the agent is doing, intervene, and reject. The
//! Task View is the focused manifestation of that:
//! when the user opens a single step from the
//! Agent Workspace, the Task View shows the step's
//! inputs (files read, network calls, prompts sent)
//! and outputs (results, errors), with explicit
//! accept / reject controls for permission-blocked
//! steps.

use aether_design_tokens::{AiVisualState, Color};
use aether_ui_components::{
    Component, ComponentStyle, Insets, LayoutBox, Panel, PanelSide,
};

use alloc::string::String;
use alloc::vec::Vec;

/// The inputs of a task — what the agent read or
/// sent. Each entry is a `kind` label and a
/// human-readable summary. The renderer renders
/// these as a vertical list of "input" rows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskViewInputs {
    /// A short title for the inputs block
    /// (e.g. "Read 2 files, sent 1 prompt").
    pub summary: String,
    /// The individual input rows.
    pub rows: Vec<String>,
}

impl TaskViewInputs {
    /// Construct inputs with a summary and rows.
    #[must_use]
    pub fn new(summary: impl Into<String>, rows: Vec<String>) -> Self {
        Self { summary: summary.into(), rows }
    }

    /// An empty inputs block.
    #[must_use]
    pub fn empty() -> Self {
        Self { summary: String::new(), rows: Vec::new() }
    }

    /// Whether the inputs block is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The number of input rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }
}

/// The outputs of a task — what the agent produced.
/// Same shape as `TaskViewInputs`, but rendered
/// below the inputs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskViewOutputs {
    /// A short title for the outputs block
    /// (e.g. "Result", "Wrote file", "Error").
    pub summary: String,
    /// The individual output rows.
    pub rows: Vec<String>,
}

impl TaskViewOutputs {
    /// Construct outputs with a summary and rows.
    #[must_use]
    pub fn new(summary: impl Into<String>, rows: Vec<String>) -> Self {
        Self { summary: summary.into(), rows }
    }

    /// An empty outputs block.
    #[must_use]
    pub fn empty() -> Self {
        Self { summary: String::new(), rows: Vec::new() }
    }

    /// Whether the outputs block is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The number of output rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }
}

/// A user's decision on a permission-blocked task.
/// The renderer / router dispatches on this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TaskDecision {
    /// Accept the proposed action and let the agent
    /// continue.
    Accept,
    /// Reject the proposed action; the agent will
    /// skip this step.
    Reject,
    /// Pause: defer the decision; the agent stays
    /// blocked.
    Pause,
}

/// The state of a single task view at one moment.
/// This is what the renderer reads; the `TaskView`
/// composing type is a thin wrapper around a
/// `Panel::Center` carrying this state.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskViewState {
    /// A short title for the task (e.g. "Open
    /// `~/notes/q3.md`").
    pub title: String,
    /// The agent's current visual state for this
    /// task.
    pub state: AiVisualState,
    /// The task's inputs.
    pub inputs: TaskViewInputs,
    /// The task's outputs.
    pub outputs: TaskViewOutputs,
    /// The pending permission prompt, if any. Only
    /// set when `state == WaitingForPermission`.
    pub permission: Option<String>,
}

impl TaskViewState {
    /// Construct a fresh task view in `Idle` with
    /// empty inputs / outputs and no permission.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            state: AiVisualState::Idle,
            inputs: TaskViewInputs::empty(),
            outputs: TaskViewOutputs::empty(),
            permission: None,
        }
    }

    /// Override the state.
    #[must_use]
    pub fn with_state(mut self, state: AiVisualState) -> Self {
        self.state = state;
        self
    }

    /// Override the inputs.
    #[must_use]
    pub fn with_inputs(mut self, inputs: TaskViewInputs) -> Self {
        self.inputs = inputs;
        self
    }

    /// Override the outputs.
    #[must_use]
    pub fn with_outputs(mut self, outputs: TaskViewOutputs) -> Self {
        self.outputs = outputs;
        self
    }

    /// Set a permission prompt. Pass `None` to
    /// clear.
    #[must_use]
    pub fn with_permission(mut self, p: Option<String>) -> Self {
        self.permission = p;
        self
    }

    /// The accent color the task view should use.
    #[must_use]
    pub fn accent(&self) -> Color {
        self.state.color()
    }

    /// Whether the task is currently waiting on a
    /// permission decision. Mirrors
    /// `state == WaitingForPermission` and a non-
    /// `None` permission prompt.
    #[must_use]
    pub fn needs_decision(&self) -> bool {
        self.state == AiVisualState::WaitingForPermission && self.permission.is_some()
    }
}

/// The task view surface — a `Panel::Center` (a
/// floating card) carrying a `TaskViewState`. The
/// renderer reads the state and the layout helpers
/// to draw the inputs / outputs / permission regions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskView {
    /// The backing centered panel.
    pub panel: Panel,
    /// The resolved state at this frame.
    pub state: TaskViewState,
}

impl TaskView {
    /// The §12 default task view width in pixels.
    pub const DEFAULT_WIDTH_PX: u32 = 560;
    /// The §12 default task view height in pixels.
    pub const DEFAULT_HEIGHT_PX: u32 = 480;

    /// Construct a fresh task view with the given
    /// title. The Task View is a floating card; we
    /// model it as a `Panel::Bottom` because it has
    /// a finite size and isn't anchored to a screen
    /// edge, but the renderer treats it as a centered
    /// modal.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        let panel = Panel::new(PanelSide::Bottom)
            .with_size(Self::DEFAULT_WIDTH_PX, Self::DEFAULT_HEIGHT_PX);
        Self { panel, state: TaskViewState::new(title) }
    }

    /// Override the state.
    #[must_use]
    pub fn with_state(mut self, state: TaskViewState) -> Self {
        self.state = state;
        self
    }

    /// Set the panel's size.
    #[must_use]
    pub fn with_size(mut self, w: u32, h: u32) -> Self {
        self.panel = self.panel.with_size(w, h);
        self
    }

    /// Set the panel's origin.
    #[must_use]
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.panel = self.panel.at(x, y);
        self
    }

    /// The header region: a 56-px-tall strip with
    /// the title and the state accent dot.
    #[must_use]
    pub fn header_box(&self) -> LayoutBox {
        LayoutBox::new(
            self.panel.origin.0,
            self.panel.origin.1,
            self.panel.width,
            56,
        )
    }

    /// The inputs region: a card that lists the
    /// task's inputs. Sized based on the number of
    /// rows.
    #[must_use]
    pub fn inputs_box(&self) -> LayoutBox {
        let header = self.header_box();
        let row_h = 28;
        let h = 48 + (row_h * self.state.inputs.len() as u32);
        LayoutBox::new(
            self.panel.origin.0 + 16,
            header.bottom() + 16,
            self.panel.width.saturating_sub(32),
            h,
        )
    }

    /// The outputs region: a card that lists the
    /// task's outputs. Sized based on the number of
    /// rows.
    #[must_use]
    pub fn outputs_box(&self) -> LayoutBox {
        let inputs = self.inputs_box();
        let row_h = 28;
        let h = 48 + (row_h * self.state.outputs.len() as u32);
        LayoutBox::new(
            self.panel.origin.0 + 16,
            inputs.bottom() + 16,
            self.panel.width.saturating_sub(32),
            h,
        )
    }

    /// The permission region: only meaningful when
    /// `state.needs_decision()` is true. Renders the
    /// permission prompt and the accept / reject
    /// buttons.
    #[must_use]
    pub fn permission_box(&self) -> LayoutBox {
        let outputs = self.outputs_box();
        LayoutBox::new(
            self.panel.origin.0 + 16,
            outputs.bottom() + 16,
            self.panel.width.saturating_sub(32),
            80,
        )
    }
}

impl Component for TaskView {
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

    #[test]
    fn inputs_empty() {
        let i = TaskViewInputs::empty();
        assert!(i.is_empty());
        assert_eq!(i.len(), 0);
    }

    #[test]
    fn inputs_with_rows() {
        let i = TaskViewInputs::new("Read 2 files", alloc::vec!["a.rs".into(), "b.rs".into()]);
        assert!(!i.is_empty());
        assert_eq!(i.len(), 2);
        assert_eq!(i.summary, "Read 2 files");
    }

    #[test]
    fn outputs_empty() {
        let o = TaskViewOutputs::empty();
        assert!(o.is_empty());
        assert_eq!(o.len(), 0);
    }

    #[test]
    fn decision_is_copy() {
        // TaskDecision is a small enum; the renderer
        // needs to be able to dispatch on it freely.
        let d = TaskDecision::Accept;
        let d2 = d;
        assert_eq!(d, d2);
    }

    #[test]
    fn state_default_is_idle() {
        let s = TaskViewState::new("task");
        assert_eq!(s.state, AiVisualState::Idle);
        assert!(s.inputs.is_empty());
        assert!(s.outputs.is_empty());
        assert!(s.permission.is_none());
    }

    #[test]
    fn state_with_permission() {
        let s = TaskViewState::new("task")
            .with_state(AiVisualState::WaitingForPermission)
            .with_permission(Some("Allow file write?".into()));
        assert!(s.needs_decision());
    }

    #[test]
    fn state_needs_decision_false_when_no_permission() {
        let s = TaskViewState::new("task")
            .with_state(AiVisualState::WaitingForPermission);
        assert!(!s.needs_decision());
    }

    #[test]
    fn state_needs_decision_false_when_not_blocked() {
        let s = TaskViewState::new("task")
            .with_state(AiVisualState::Working)
            .with_permission(Some("unused".into()));
        assert!(!s.needs_decision());
    }

    #[test]
    fn state_accent_matches_visual() {
        let s = TaskViewState::new("t").with_state(AiVisualState::Working);
        assert_eq!(s.accent(), AiVisualState::Working.color());
    }

    #[test]
    fn task_view_new_is_bottom_panel() {
        // The Task View is a floating card. The panel
        // is technically a `Panel::Bottom` because the
        // panel system has 4 edges; the renderer treats
        // it as a centered modal.
        let v = TaskView::new("task");
        assert_eq!(v.panel.side, PanelSide::Bottom);
    }

    #[test]
    fn task_view_default_size() {
        let v = TaskView::new("task");
        assert_eq!(v.panel.width, 560);
        assert_eq!(v.panel.height, 480);
    }

    #[test]
    fn task_view_header_is_56px() {
        let v = TaskView::new("task");
        let h = v.header_box();
        assert_eq!(h.height, 56);
    }

    #[test]
    fn task_view_layout_stacks_regions() {
        let v = TaskView::new("task").with_size(560, 480);
        let h = v.header_box();
        let i = v.inputs_box();
        let o = v.outputs_box();
        let p = v.permission_box();
        assert!(h.bottom() < i.y);
        assert!(i.bottom() < o.y);
        assert!(o.bottom() < p.y);
    }
}
