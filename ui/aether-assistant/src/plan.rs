//! Workspace plan + plan steps.
//!
//! An agent plan is an ordered list of `PlanStep`s,
//! each with its own `PlanStepState`. Steps are the
//! fine-grained unit the agent reports ("opened
//! calculator", "typed 1+1", "read result") — the
//! `Agent Workspace` renders them as a vertical
//! timeline.

use aether_design_tokens::AiVisualState;

use alloc::string::String;
use alloc::vec::Vec;

/// What kind of work a step is. Renderers use this to
/// pick the leading icon (file / network / system /
/// app / agent reasoning).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PlanStepKind {
    /// The agent is reasoning (no I/O yet).
    Reasoning,
    /// The agent is reading or writing a file.
    File,
    /// The agent is talking to the network.
    Network,
    /// The agent is talking to another app.
    App,
    /// The agent is doing a system / IPC action.
    System,
    /// The agent is waiting for user permission.
    Permission,
}

impl PlanStepKind {
    /// A short glyph / icon name for the step. The
    /// renderer uses this as the leading icon. For
    /// 6.7 the icon system will resolve these to
    /// real SVG paths; today the renderer treats
    /// them as opaque strings.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Reasoning => "spark",
            Self::File => "file",
            Self::Network => "globe",
            Self::App => "app",
            Self::System => "gear",
            Self::Permission => "key",
        }
    }
}

/// The state of a single plan step. Mirrors the
/// 9-state AI vocabulary but with a "Pending" branch
/// for steps the agent hasn't started yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PlanStepState {
    /// The step is queued but not started.
    Pending,
    /// The step is running, with an `AiVisualState`
    /// sub-state (e.g. Thinking, Working, Listening).
    Running(AiVisualState),
    /// The step finished successfully.
    Done,
    /// The step failed.
    Failed,
    /// The step was skipped (e.g. the agent decided
    /// it wasn't needed).
    Skipped,
}

impl PlanStepState {
    /// The `AiVisualState` to use for styling. Pending
    /// and Skipped collapse to Idle; Failed to Error.
    #[must_use]
    pub const fn visual(self) -> AiVisualState {
        match self {
            Self::Pending => AiVisualState::Idle,
            Self::Running(s) => s,
            Self::Done => AiVisualState::Completed,
            Self::Failed => AiVisualState::Error,
            Self::Skipped => AiVisualState::Idle,
        }
    }

    /// A short label for the step's status chip.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Running(s) => match s {
                AiVisualState::Idle => "Idle",
                AiVisualState::Listening => "Listening",
                AiVisualState::Thinking => "Thinking",
                AiVisualState::Planning => "Planning",
                AiVisualState::Working => "Working",
                AiVisualState::WaitingForPermission => "Permission",
                AiVisualState::Completed => "Working",
                AiVisualState::Error => "Working",
                AiVisualState::Recovering => "Recovering",
                _ => "Working",
            },
            Self::Done => "Done",
            Self::Failed => "Failed",
            Self::Skipped => "Skipped",
        }
    }
}

/// A single step in the agent's plan. The agent
/// produces a list of these; the workspace renders
/// them as a vertical timeline.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlanStep {
    /// What kind of work this step is.
    pub kind: PlanStepKind,
    /// A short title for the step. The workspace
    /// shows this as the leading text.
    pub title: String,
    /// The step's state. Pending by default.
    pub state: PlanStepState,
}

impl PlanStep {
    /// Construct a pending step.
    #[must_use]
    pub fn new(kind: PlanStepKind, title: impl Into<String>) -> Self {
        Self { kind, title: title.into(), state: PlanStepState::Pending }
    }

    /// Mark this step as running with the given
    /// `AiVisualState` sub-state.
    #[must_use]
    pub fn running(mut self, s: AiVisualState) -> Self {
        self.state = PlanStepState::Running(s);
        self
    }

    /// Mark this step as done.
    #[must_use]
    pub fn done(mut self) -> Self {
        self.state = PlanStepState::Done;
        self
    }

    /// Mark this step as failed.
    #[must_use]
    pub fn failed(mut self) -> Self {
        self.state = PlanStepState::Failed;
        self
    }

    /// Mark this step as skipped.
    #[must_use]
    pub fn skipped(mut self) -> Self {
        self.state = PlanStepState::Skipped;
        self
    }
}

/// The agent's full plan: a goal + an ordered list of
/// steps. The workspace renders the goal as a header
/// and the steps as a timeline.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspacePlan {
    /// A short title for the plan (e.g. "Open the
    /// meeting notes and summarize them").
    pub goal: String,
    /// The ordered list of steps.
    pub steps: Vec<PlanStep>,
}

impl WorkspacePlan {
    /// Construct an empty plan with the given goal.
    #[must_use]
    pub fn new(goal: impl Into<String>) -> Self {
        Self { goal: goal.into(), steps: Vec::new() }
    }

    /// Append a step to the plan.
    #[must_use]
    pub fn with_step(mut self, step: PlanStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Append many steps at once.
    #[must_use]
    pub fn with_steps(mut self, steps: Vec<PlanStep>) -> Self {
        self.steps.extend(steps);
        self
    }

    /// The number of completed steps.
    #[must_use]
    pub fn completed_count(&self) -> usize {
        self.steps.iter().filter(|s| s.state == PlanStepState::Done).count()
    }

    /// The total number of steps.
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.steps.len()
    }

    /// Progress as a fraction in `[0.0, 1.0]`. Returns
    /// 0.0 for an empty plan.
    #[must_use]
    pub fn progress(&self) -> f32 {
        if self.steps.is_empty() {
            return 0.0;
        }
        self.completed_count() as f32 / self.steps.len() as f32
    }

    /// Whether the plan is fully complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.steps.is_empty()
            && self.steps.iter().all(|s| {
                matches!(s.state, PlanStepState::Done | PlanStepState::Skipped)
            })
    }

    /// The index of the currently running step, if
    /// any. Used by the workspace to highlight the
    /// active row.
    #[must_use]
    pub fn running_index(&self) -> Option<usize> {
        self.steps.iter().position(|s| matches!(s.state, PlanStepState::Running(_)))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn kind_glyph_is_non_empty() {
        let kinds = [
            PlanStepKind::Reasoning,
            PlanStepKind::File,
            PlanStepKind::Network,
            PlanStepKind::App,
            PlanStepKind::System,
            PlanStepKind::Permission,
        ];
        for k in kinds {
            assert!(!k.glyph().is_empty());
        }
    }

    #[test]
    fn visual_collapses_pending_to_idle() {
        assert_eq!(PlanStepState::Pending.visual(), AiVisualState::Idle);
    }

    #[test]
    fn visual_collapses_skipped_to_idle() {
        assert_eq!(PlanStepState::Skipped.visual(), AiVisualState::Idle);
    }

    #[test]
    fn visual_collapses_failed_to_error() {
        assert_eq!(PlanStepState::Failed.visual(), AiVisualState::Error);
    }

    #[test]
    fn visual_uses_running_substate() {
        assert_eq!(
            PlanStepState::Running(AiVisualState::Working).visual(),
            AiVisualState::Working
        );
    }

    #[test]
    fn label_for_done() {
        assert_eq!(PlanStepState::Done.label(), "Done");
    }

    #[test]
    fn label_for_failed() {
        assert_eq!(PlanStepState::Failed.label(), "Failed");
    }

    #[test]
    fn step_starts_pending() {
        let s = PlanStep::new(PlanStepKind::File, "open readme");
        assert_eq!(s.state, PlanStepState::Pending);
        assert_eq!(s.title, "open readme");
    }

    #[test]
    fn step_running_builder() {
        let s = PlanStep::new(PlanStepKind::File, "open readme").running(AiVisualState::Working);
        assert_eq!(s.state, PlanStepState::Running(AiVisualState::Working));
    }

    #[test]
    fn step_done_builder() {
        let s = PlanStep::new(PlanStepKind::File, "x").done();
        assert_eq!(s.state, PlanStepState::Done);
    }

    #[test]
    fn step_failed_builder() {
        let s = PlanStep::new(PlanStepKind::Network, "x").failed();
        assert_eq!(s.state, PlanStepState::Failed);
    }

    #[test]
    fn step_skipped_builder() {
        let s = PlanStep::new(PlanStepKind::File, "x").skipped();
        assert_eq!(s.state, PlanStepState::Skipped);
    }

    #[test]
    fn plan_starts_empty() {
        let p = WorkspacePlan::new("do the thing");
        assert_eq!(p.goal, "do the thing");
        assert_eq!(p.total_count(), 0);
        assert_eq!(p.completed_count(), 0);
    }

    #[test]
    fn plan_progress_empty_is_zero() {
        let p = WorkspacePlan::new("empty");
        assert_eq!(p.progress(), 0.0);
    }

    #[test]
    fn plan_progress_halves_when_half_done() {
        let p = WorkspacePlan::new("p")
            .with_step(PlanStep::new(PlanStepKind::File, "a").done())
            .with_step(PlanStep::new(PlanStepKind::File, "b"));
        assert_eq!(p.completed_count(), 1);
        assert_eq!(p.total_count(), 2);
        assert!((p.progress() - 0.5).abs() < 0.001);
    }

    #[test]
    fn plan_is_complete_when_all_done() {
        let p = WorkspacePlan::new("p")
            .with_step(PlanStep::new(PlanStepKind::File, "a").done())
            .with_step(PlanStep::new(PlanStepKind::File, "b").done());
        assert!(p.is_complete());
    }

    #[test]
    fn plan_is_complete_with_skipped() {
        let p = WorkspacePlan::new("p")
            .with_step(PlanStep::new(PlanStepKind::File, "a").done())
            .with_step(PlanStep::new(PlanStepKind::File, "b").skipped());
        assert!(p.is_complete());
    }

    #[test]
    fn plan_is_not_complete_with_pending() {
        let p = WorkspacePlan::new("p").with_step(PlanStep::new(PlanStepKind::File, "a"));
        assert!(!p.is_complete());
    }

    #[test]
    fn plan_running_index_finds_running_step() {
        let p = WorkspacePlan::new("p")
            .with_step(PlanStep::new(PlanStepKind::File, "a").done())
            .with_step(PlanStep::new(PlanStepKind::File, "b").running(AiVisualState::Working))
            .with_step(PlanStep::new(PlanStepKind::File, "c"));
        assert_eq!(p.running_index(), Some(1));
    }

    #[test]
    fn plan_running_index_none_when_no_running() {
        let p = WorkspacePlan::new("p").with_step(PlanStep::new(PlanStepKind::File, "a").done());
        assert_eq!(p.running_index(), None);
    }
}
