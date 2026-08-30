//! Aether system automation — user-defined
//! workflows.
//!
//! Phase 7.3 of the ROADMAP. A *workflow* is a
//! named, ordered list of steps the user (or
//! another agent) authors once and runs many
//! times. A typical workflow is "morning setup":
//! open the email app, dim the lights, queue the
//! focus playlist, surface today's calendar.
//!
//! The contract is *typed review*: a `Workflow`
//! is a pure value (no IO, no running side
//! effects), and the `compile_to_tasks` helper
//! turns it into an ordered list of `AgentTask`s
//! the agent runtime can execute. The renderer
//! / assistant panel surfaces the workflow as a
//! `TaskView` row so the user can read every
//! step before it runs.
//!
//! The model has four pieces:
//!
//! 1. **Step** — a single, atomic action. The
//!    action is expressed as a `StepAction` enum
//!    (the typed "what to do"). Each step has a
//!    `retry` policy and an `on_failure`
//!    continuation.
//! 2. **Workflow** — a named, ordered list of
//!    steps. Workflows have a unique id and a
//!    human-readable description.
//! 3. **Trigger** — *when* the workflow runs.
//!    Manual (the user invokes it), time-of-day
//!    (cron-like), or event-based (when a
//!    `TriggerEvent` arrives).
//! 4. **Registry** — the named collection of
//!    workflows. The runtime boots with a
//!    default registry (`morning_setup`,
//!    `end_of_day`, `before_meeting`) and the
//!    user can add their own at runtime.
//!
//! The crate is *pure* — it produces tasks; it
//! does not run them. The agent runtime (7.4's
//! background agent) is what executes the
//! compiled tasks.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use aether_agent_core::{AgentTask, TaskId, TaskKind, TaskRisk};

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// What to do for a single step. The action is
/// expressed as a typed enum so the renderer can
/// describe it (and the future runtime can
/// dispatch on it) without parsing free-form
/// strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StepAction {
    /// Launch an application by app id.
    LaunchApp {
        /// The app id (e.g. `"aether.notes"`).
        app_id: String,
    },
    /// Open a file or URL.
    OpenFile {
        /// The path or URL.
        target: String,
    },
    /// Run a typed agent task (a proposal-to-task
    /// compile target, e.g. "drop the page
    /// cache" or "rotate the log file").
    AgentTask {
        /// A short human title.
        title: String,
        /// A long description of the task.
        description: String,
    },
    /// Run a recovery action from
    /// `aether-recovery`. Useful for the
    /// "before-meeting" workflow that drops the
    /// page cache first.
    RecoveryAction {
        /// The action's summary (e.g.
        /// "Drop page cache: free memory").
        /// The runtime looks up the action in
        /// its recovery registry.
        action_summary: String,
    },
    /// Surface a notification to the user.
    Notify {
        /// The notification body.
        body: String,
    },
    /// Wait a fixed duration (in seconds) before
    /// continuing. Useful for "wait for the
    /// network to come back" steps.
    Wait {
        /// The duration in seconds.
        seconds: u32,
    },
}

impl StepAction {
    /// The action's task kind, for `AgentTask`
    /// construction. Steps that are not
    /// directly agent tasks (`Wait`,
    /// `RecoveryAction`) map to the closest
    /// task kind; the runtime dispatches on
    /// the action's own kind at execution time.
    #[must_use]
    pub const fn task_kind(&self) -> TaskKind {
        match self {
            Self::LaunchApp { .. } | Self::OpenFile { .. } => TaskKind::Custom,
            Self::AgentTask { .. } | Self::RecoveryAction { .. } | Self::Notify { .. } => {
                TaskKind::Notify
            }
            Self::Wait { .. } => TaskKind::Custom,
        }
    }

    /// A short, single-sentence description of
    /// the action.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::LaunchApp { app_id } => format!("Launch `{app_id}`"),
            Self::OpenFile { target } => format!("Open `{target}`"),
            Self::AgentTask { title, .. } => title.clone(),
            Self::RecoveryAction { action_summary } => action_summary.clone(),
            Self::Notify { body } => format!("Notify: {body}"),
            Self::Wait { seconds } => format!("Wait {seconds}s"),
        }
    }
}

/// What to do when a step fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FailurePolicy {
    /// Stop the workflow and surface a
    /// notification to the user.
    Abort,
    /// Log the failure, skip the rest of the
    /// workflow silently.
    Skip,
    /// Continue with the next step despite
    /// the failure. The failed step's effects
    /// (if any) are still applied.
    Continue,
    /// Retry the step a fixed number of times
    /// (up to 3) before applying the inner
    /// policy on the final failure.
    RetryThenAbort,
    /// Retry, then skip.
    RetryThenSkip,
}

impl FailurePolicy {
    /// The maximum number of retries. The
    /// `RetryThen*` variants retry; the others
    /// do not.
    #[must_use]
    pub const fn max_retries(&self) -> u32 {
        match self {
            Self::RetryThenAbort | Self::RetryThenSkip => 3,
            Self::Abort | Self::Skip | Self::Continue => 0,
        }
    }
}

/// A single step in a workflow.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowStep {
    /// A short label for the step (e.g.
    /// "Open the email app").
    pub label: String,
    /// The action to take.
    pub action: StepAction,
    /// What to do on failure.
    pub on_failure: FailurePolicy,
}

impl WorkflowStep {
    /// A new step with the default failure
    /// policy (`Abort`).
    #[must_use]
    pub fn new(label: impl Into<String>, action: StepAction) -> Self {
        Self { label: label.into(), action, on_failure: FailurePolicy::Abort }
    }

    /// Override the failure policy.
    #[must_use]
    pub const fn with_failure_policy(mut self, policy: FailurePolicy) -> Self {
        self.on_failure = policy;
        self
    }
}

/// A unique identifier for a workflow. The
/// registry uses it as a key; the user can also
/// reference it from the command bar (e.g.
/// "run morning setup").
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkflowId(String);

impl WorkflowId {
    /// Creates a new `WorkflowId` from a
    /// non-empty string.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let s: String = value.into();
        if s.is_empty() {
            return None;
        }
        Some(Self(s))
    }

    /// Returns the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// When to run a workflow.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Trigger {
    /// The user invokes the workflow manually
    /// (e.g. from the command bar).
    Manual,
    /// A time-of-day trigger. The fields are
    /// hour and minute in 24-hour local time.
    TimeOfDay {
        /// Hour (0..=23).
        hour: u8,
        /// Minute (0..=59).
        minute: u8,
    },
    /// An event-based trigger. The workflow
    /// runs when the matching event arrives
    /// from the agent's event bus.
    OnEvent {
        /// The event id (e.g. `"network.up"`,
        /// `"battery.low"`).
        event_id: String,
    },
}

impl Trigger {
    /// A short, single-sentence description.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Manual => "Manual".to_string(),
            Self::TimeOfDay { hour, minute } => format!("Daily at {hour:02}:{minute:02}"),
            Self::OnEvent { event_id } => format!("On event `{event_id}`"),
        }
    }
}

/// A user-defined workflow: a named, ordered
/// list of steps plus a trigger that says when
/// to run it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Workflow {
    /// A unique id (e.g. `"morning_setup"`).
    pub id: WorkflowId,
    /// A human-readable name (e.g.
    /// "Morning setup").
    pub name: String,
    /// A longer description of the workflow.
    pub description: String,
    /// When the workflow runs.
    pub trigger: Trigger,
    /// The ordered list of steps.
    pub steps: Vec<WorkflowStep>,
}

impl Workflow {
    /// A new workflow with no steps. Use the
    /// `with_step` builder to add steps.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        trigger: Trigger,
    ) -> Option<Self> {
        let id = WorkflowId::new(id)?;
        let name: String = name.into();
        if name.is_empty() {
            return None;
        }
        Some(Self {
            id,
            name,
            description: description.into(),
            trigger,
            steps: Vec::new(),
        })
    }

    /// Append a step.
    #[must_use]
    pub fn with_step(mut self, step: WorkflowStep) -> Self {
        self.steps.push(step);
        self
    }

    /// The number of steps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the workflow has no steps.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

/// A named collection of workflows. The agent
/// runtime boots with a default registry; users
/// can register their own at runtime.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct WorkflowRegistry {
    /// The registered workflows.
    pub workflows: Vec<Workflow>,
}

impl WorkflowRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a workflow. Returns `false` if a
    /// workflow with the same id is already
    /// registered (no overwrite).
    #[must_use]
    pub fn register(&mut self, workflow: Workflow) -> bool {
        if self.workflows.iter().any(|w| w.id == workflow.id) {
            return false;
        }
        self.workflows.push(workflow);
        true
    }

    /// Look up a workflow by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Workflow> {
        self.workflows.iter().find(|w| w.id.as_str() == id)
    }

    /// List all workflows that have the given
    /// trigger. Useful for the runtime: "give me
    /// everything that should run at 09:00".
    #[must_use]
    pub fn with_trigger(&self, trigger: &Trigger) -> Vec<&Workflow> {
        self.workflows
            .iter()
            .filter(|w| &w.trigger == trigger)
            .collect()
    }

    /// The number of registered workflows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.workflows.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.workflows.is_empty()
    }
}

/// Compile a workflow's steps into a list of
/// `AgentTask`s the agent runtime can execute.
/// Each step produces one `AgentTask`. The
/// caller supplies a `task_id_prefix` (the
/// runtime's UUIDv7 base) and a
/// `timestamp_ms`.
///
/// The tasks are returned in execution order.
/// The runtime is responsible for honoring each
/// task's `risk` and the workflow's failure
/// policy; this helper just translates steps
/// to tasks.
#[allow(unused_variables)]
#[must_use]
pub fn compile_to_tasks(
    workflow: &Workflow,
    task_id_prefix: &str,
    timestamp_ms: u64,
) -> Vec<AgentTask> {
    let mut tasks = Vec::new();
    for (i, step) in workflow.steps.iter().enumerate() {
        let id = format!("{task_id_prefix}.{i}");
        let task_id = match TaskId::new(id) {
            Some(t) => t,
            None => continue,
        };
        let title = format!("{} — {}", workflow.name, step.label);
        let description = step.action.summary();
        if let Some(mut t) = AgentTask::new(
            task_id.as_str().to_string(),
            step.action.task_kind(),
            title,
            description,
        ) {
            t.risk = TaskRisk::Low;
            // Encode the failure policy as a JSON
            // argument so the runtime can read it
            // back at execution time.
            let policy = match step.on_failure {
                FailurePolicy::Abort => "abort",
                FailurePolicy::Skip => "skip",
                FailurePolicy::Continue => "continue",
                FailurePolicy::RetryThenAbort => "retry-then-abort",
                FailurePolicy::RetryThenSkip => "retry-then-skip",
            };
            t = t.with_arguments(serde_json::json!({
                "policy": policy,
                "max_retries": step.on_failure.max_retries(),
                "step_index": i,
                "action": match &step.action {
                    StepAction::LaunchApp { app_id } => serde_json::json!({"kind": "launch_app", "app_id": app_id}),
                    StepAction::OpenFile { target } => serde_json::json!({"kind": "open_file", "target": target}),
                    StepAction::AgentTask { title, description } => serde_json::json!({"kind": "agent_task", "title": title, "description": description}),
                    StepAction::RecoveryAction { action_summary } => serde_json::json!({"kind": "recovery", "summary": action_summary}),
                    StepAction::Notify { body } => serde_json::json!({"kind": "notify", "body": body}),
                    StepAction::Wait { seconds } => serde_json::json!({"kind": "wait", "seconds": seconds}),
                }
            }));
            tasks.push(t);
        }
    }
    tasks
}

/// The default workflow registry. It ships a
/// few example workflows so the runtime has
/// something to run on a fresh boot.
#[must_use]
pub fn default_registry() -> WorkflowRegistry {
    let mut reg = WorkflowRegistry::new();

    // morning_setup: open email, drop page cache,
    // notify today's agenda. Runs at 09:00.
    if let Some(w) = Workflow::new(
        "morning_setup",
        "Morning setup",
        "Open email, free memory, surface today's agenda.",
        Trigger::TimeOfDay { hour: 9, minute: 0 },
    ) {
        let _ = reg.register(
            w.with_step(WorkflowStep::new(
                "Open the email app",
                StepAction::LaunchApp { app_id: "aether.mail".into() },
            ))
            .with_step(WorkflowStep::new(
                "Drop the page cache to free memory",
                StepAction::RecoveryAction {
                    action_summary: "Drop page cache: free kernel page cache for a fresh day.".into(),
                },
            ).with_failure_policy(FailurePolicy::Skip))
            .with_step(WorkflowStep::new(
                "Surface the day's agenda",
                StepAction::Notify {
                    body: "Good morning — your first meeting is at 10:00.".into(),
                },
            )),
        );
    }

    // end_of_day: drop the page cache, surface a
    // summary. Runs at 18:00.
    if let Some(w) = Workflow::new(
        "end_of_day",
        "End of day",
        "Free memory, close the day.",
        Trigger::TimeOfDay { hour: 18, minute: 0 },
    ) {
        let _ = reg.register(
            w.with_step(WorkflowStep::new(
                "Drop the page cache",
                StepAction::RecoveryAction {
                    action_summary: "Drop page cache: end of day cleanup.".into(),
                },
            ))
            .with_step(WorkflowStep::new(
                "Notify the user",
                StepAction::Notify {
                    body: "Wrapping up — see you tomorrow.".into(),
                },
            )),
        );
    }

    // before_meeting: drop cache, close
    // notifications, open notes. Manual trigger.
    if let Some(w) = Workflow::new(
        "before_meeting",
        "Before a meeting",
        "Quiet the desktop, open the notes app.",
        Trigger::Manual,
    ) {
        let _ = reg.register(
            w.with_step(WorkflowStep::new(
                "Drop the page cache",
                StepAction::RecoveryAction {
                    action_summary: "Drop page cache: pre-meeting cleanup.".into(),
                },
            ).with_failure_policy(FailurePolicy::Skip))
            .with_step(WorkflowStep::new(
                "Open the notes app",
                StepAction::LaunchApp { app_id: "aether.notes".into() },
            )),
        );
    }

    reg
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn step_action_summary() {
        assert_eq!(StepAction::LaunchApp { app_id: "aether.notes".into() }.summary(), "Launch `aether.notes`");
        assert_eq!(StepAction::OpenFile { target: "/etc/hosts".into() }.summary(), "Open `/etc/hosts`");
        assert_eq!(StepAction::Notify { body: "hi".into() }.summary(), "Notify: hi");
        assert_eq!(StepAction::Wait { seconds: 5 }.summary(), "Wait 5s");
    }

    #[test]
    fn step_action_task_kind() {
        assert_eq!(StepAction::LaunchApp { app_id: "x".into() }.task_kind(), TaskKind::Custom);
        assert_eq!(StepAction::Notify { body: "x".into() }.task_kind(), TaskKind::Notify);
        assert_eq!(StepAction::Wait { seconds: 1 }.task_kind(), TaskKind::Custom);
    }

    #[test]
    fn failure_policy_max_retries() {
        assert_eq!(FailurePolicy::Abort.max_retries(), 0);
        assert_eq!(FailurePolicy::Skip.max_retries(), 0);
        assert_eq!(FailurePolicy::Continue.max_retries(), 0);
        assert_eq!(FailurePolicy::RetryThenAbort.max_retries(), 3);
        assert_eq!(FailurePolicy::RetryThenSkip.max_retries(), 3);
    }

    #[test]
    fn step_with_default_policy_is_abort() {
        let s = WorkflowStep::new("x", StepAction::Notify { body: "y".into() });
        assert_eq!(s.on_failure, FailurePolicy::Abort);
    }

    #[test]
    fn step_with_failure_policy() {
        let s = WorkflowStep::new("x", StepAction::Notify { body: "y".into() })
            .with_failure_policy(FailurePolicy::Continue);
        assert_eq!(s.on_failure, FailurePolicy::Continue);
    }

    #[test]
    fn workflow_id_rejects_empty() {
        assert!(WorkflowId::new("").is_none());
        assert!(WorkflowId::new("morning_setup").is_some());
    }

    #[test]
    fn workflow_new_rejects_empty_name() {
        let w = Workflow::new("id", "", "d", Trigger::Manual);
        assert!(w.is_none());
    }

    #[test]
    fn workflow_with_step() {
        let w = Workflow::new("id", "name", "desc", Trigger::Manual)
            .unwrap()
            .with_step(WorkflowStep::new("s1", StepAction::Notify { body: "x".into() }));
        assert_eq!(w.len(), 1);
        assert!(!w.is_empty());
    }

    #[test]
    fn registry_register_and_get() {
        let mut reg = WorkflowRegistry::new();
        let w = Workflow::new("id1", "name", "desc", Trigger::Manual).unwrap();
        assert!(reg.register(w));
        assert!(!reg.register(Workflow::new("id1", "other", "desc", Trigger::Manual).unwrap()));
        assert!(reg.get("id1").is_some());
        assert!(reg.get("nope").is_none());
    }

    #[test]
    fn registry_with_trigger() {
        let mut reg = WorkflowRegistry::new();
        let _ = reg.register(Workflow::new("a", "A", "", Trigger::Manual).unwrap());
        let _ = reg.register(Workflow::new("b", "B", "", Trigger::TimeOfDay { hour: 9, minute: 0 }).unwrap());
        let _ = reg.register(Workflow::new("c", "C", "", Trigger::TimeOfDay { hour: 9, minute: 0 }).unwrap());
        let m = reg.with_trigger(&Trigger::Manual);
        assert_eq!(m.len(), 1);
        let t = reg.with_trigger(&Trigger::TimeOfDay { hour: 9, minute: 0 });
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn compile_to_tasks_preserves_order() {
        let w = Workflow::new("w", "W", "D", Trigger::Manual)
            .unwrap()
            .with_step(WorkflowStep::new("s1", StepAction::LaunchApp { app_id: "a".into() }))
            .with_step(WorkflowStep::new("s2", StepAction::Notify { body: "b".into() }));
        let tasks = compile_to_tasks(&w, "wf", 1000);
        assert_eq!(tasks.len(), 2);
        assert!(tasks[0].title.contains("s1"));
        assert!(tasks[1].title.contains("s2"));
        assert_eq!(tasks[0].id.as_str(), "wf.0");
        assert_eq!(tasks[1].id.as_str(), "wf.1");
        assert_eq!(tasks[0].risk, TaskRisk::Low);
    }

    #[test]
    fn compile_to_tasks_encodes_failure_policy() {
        let w = Workflow::new("w", "W", "D", Trigger::Manual)
            .unwrap()
            .with_step(WorkflowStep::new("s1", StepAction::Notify { body: "x".into() })
                .with_failure_policy(FailurePolicy::RetryThenSkip));
        let tasks = compile_to_tasks(&w, "wf", 0);
        assert_eq!(tasks.len(), 1);
        let args = tasks[0].arguments.as_ref().expect("args");
        assert_eq!(args["policy"], "retry-then-skip");
        assert_eq!(args["max_retries"], 3);
        assert_eq!(args["step_index"], 0);
        assert_eq!(args["action"]["kind"], "notify");
        assert_eq!(args["action"]["body"], "x");
    }

    #[test]
    fn compile_to_tasks_encodes_action_payloads() {
        let w = Workflow::new("w", "W", "D", Trigger::Manual)
            .unwrap()
            .with_step(WorkflowStep::new("a", StepAction::LaunchApp { app_id: "x".into() }))
            .with_step(WorkflowStep::new("b", StepAction::OpenFile { target: "/y".into() }))
            .with_step(WorkflowStep::new("c", StepAction::Wait { seconds: 4 }));
        let tasks = compile_to_tasks(&w, "wf", 0);
        assert_eq!(tasks[0].arguments.as_ref().unwrap()["action"]["kind"], "launch_app");
        assert_eq!(tasks[1].arguments.as_ref().unwrap()["action"]["kind"], "open_file");
        assert_eq!(tasks[2].arguments.as_ref().unwrap()["action"]["kind"], "wait");
    }

    #[test]
    fn compile_to_tasks_empty_workflow() {
        let w = Workflow::new("w", "W", "D", Trigger::Manual).unwrap();
        let tasks = compile_to_tasks(&w, "wf", 0);
        assert!(tasks.is_empty());
    }

    #[test]
    fn default_registry_has_three_workflows() {
        let reg = default_registry();
        assert!(reg.get("morning_setup").is_some());
        assert!(reg.get("end_of_day").is_some());
        assert!(reg.get("before_meeting").is_some());
        assert_eq!(reg.len(), 3);
    }

    #[test]
    fn default_registry_morning_runs_at_09_00() {
        let reg = default_registry();
        let m = reg.get("morning_setup").unwrap();
        assert!(matches!(m.trigger, Trigger::TimeOfDay { hour: 9, minute: 0 }));
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn default_registry_before_meeting_is_manual() {
        let reg = default_registry();
        let m = reg.get("before_meeting").unwrap();
        assert!(matches!(m.trigger, Trigger::Manual));
        let skip_steps = m.steps.iter().filter(|s| s.on_failure == FailurePolicy::Skip).count();
        assert!(skip_steps >= 1);
    }

    #[test]
    fn default_registry_with_trigger_time() {
        let reg = default_registry();
        let at_9 = reg.with_trigger(&Trigger::TimeOfDay { hour: 9, minute: 0 });
        assert_eq!(at_9.len(), 1);
        let at_18 = reg.with_trigger(&Trigger::TimeOfDay { hour: 18, minute: 0 });
        assert_eq!(at_18.len(), 1);
    }

    #[test]
    fn trigger_summary() {
        assert_eq!(Trigger::Manual.summary(), "Manual");
        assert_eq!(Trigger::TimeOfDay { hour: 9, minute: 0 }.summary(), "Daily at 09:00");
        assert_eq!(Trigger::TimeOfDay { hour: 18, minute: 30 }.summary(), "Daily at 18:30");
        assert_eq!(Trigger::OnEvent { event_id: "battery.low".into() }.summary(), "On event `battery.low`");
    }
}
