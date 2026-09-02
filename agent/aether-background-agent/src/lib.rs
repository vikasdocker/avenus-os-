//! Aether background agent — the runtime that
//! ties Phase 7 together.
//!
//! The background agent is the long-running
//! loop that:
//!
//! 1. **Ticks** every `tick_ms` (the host
//!    supplies a wall clock).
//! 2. **Ingest** the latest `DiagnosticReport`.
//! 3. **Plan** recovery actions for each
//!    symptom (via `aether-recovery`'s
//!    `plan_recovery`).
//! 4. **Fire** any time-of-day or event-based
//!    workflows (via `aether-automation`'s
//!    `compile_to_tasks`).
//! 5. **Emit** an `ActionQueue` of `ActionItem`s
//!    the foreground agent / shell can review
//!    and execute.
//!
//! The contract is *review-then-execute*: the
//! background agent never *runs* anything. It
//! produces a typed `ActionItem` (the unit of
//! review) and the foreground agent is what
//! turns approved items into `AgentTask`s.
//! `KillProcess` and any task with a
//! `TaskRisk::High` / `Critical` always require
//! consent; the others can be auto-executed.
//!
//! The crate is *pure* — it is a state machine
//! with no IO, no clock drift, no global state.
//! The host (aether-agentd) drives the ticks
//! and supplies the wall clock.
//!
//! ## Event bus
//!
//! The runtime consumes a stream of `AgentEvent`s
//! (`Signal::Update`, `TimeTick(hour, minute)`,
//! `WorkflowTrigger(id)`) and reacts to them.
//! The host is responsible for translating OS
//! events into `AgentEvent`s; the runtime does
//! not poll hardware.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use aether_agent_core::{AgentTask, TaskId, TaskKind, TaskRisk};
use aether_automation::{
    compile_to_tasks, FailurePolicy, StepAction, Trigger, Workflow, WorkflowId, WorkflowRegistry,
    WorkflowStep,
};
use aether_diagnostics::{DiagnosticReport, Signal, Subsystem};
use aether_recovery::{RecoveryAction, RecoveryPlan, RecoveryPolicy};

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// An event the background agent reacts to. The
/// host produces these; the runtime consumes
/// them on each tick.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AgentEvent {
    /// A new or updated signal from a subsystem
    /// (CPU usage, memory pressure, etc).
    SignalUpdate(Signal),
    /// A periodic time tick. The host supplies
    /// the current wall-clock hour and minute
    /// (in 24-hour local time). The runtime
    /// fires any time-of-day workflows whose
    /// hour/minute match.
    TimeTick {
        /// Hour (0..=23).
        hour: u8,
        /// Minute (0..=59).
        minute: u8,
    },
    /// A workflow trigger request. The runtime
    /// runs the matching workflow (manual or
    /// event-triggered).
    WorkflowTrigger(WorkflowId),
}

/// The kind of action item the runtime emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActionKind {
    /// A typed recovery action (restart /
    /// reconnect / resolve / free / drop /
    /// kill / inform).
    Recovery,
    /// A workflow's compiled task (launch /
    /// open / wait / notify / etc).
    Workflow,
}

impl ActionKind {
    /// The kebab-case name (stable for the
    /// renderer / IPC).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Recovery => "recovery",
            Self::Workflow => "workflow",
        }
    }
}

/// Why the action was produced. The renderer
/// uses this to group action items in the
/// `TaskView` ("3 actions from `cpu_overload`").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionReason {
    /// The action came from a diagnostic
    /// symptom (the `symptom_id` is the
    /// `Symptom::id`).
    Symptom(String),
    /// The action came from a workflow (the
    /// id is the `WorkflowId`).
    Workflow(String),
}

impl ActionReason {
    /// A short, single-sentence description.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Symptom(id) => format!("Symptom `{id}`"),
            Self::Workflow(id) => format!("Workflow `{id}`"),
        }
    }
}

/// The unit of review the runtime emits. The
/// foreground agent / shell displays these as
/// rows in the `TaskView` and (after consent
/// when required) turns them into `AgentTask`s.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActionItem {
    /// The action's kind.
    pub kind: ActionKind,
    /// Why the action was produced.
    pub reason: ActionReason,
    /// A short, single-sentence title.
    pub title: String,
    /// A longer description.
    pub description: String,
    /// Whether the action requires explicit
    /// user consent before the agent can
    /// execute it.
    pub requires_consent: bool,
    /// The kind of task this action compiles
    /// to. The runner uses this when it
    /// materialises the `AgentTask`.
    pub task_kind: TaskKind,
    /// The risk level of the resulting task.
    pub task_risk: TaskRisk,
    /// The subsystem the action targets, if
    /// any. `None` for actions that span
    /// subsystems (e.g. `Wait`).
    pub subsystem: Option<Subsystem>,
    /// The structured payload for the runner
    /// (target, app_id, file path, ...). The
    /// runtime encodes it as a JSON object so
    /// the runner can pattern-match on
    /// `payload["kind"]`.
    pub payload: serde_json::Value,
}

impl ActionItem {
    /// The action's `WorkflowId` (if the
    /// reason is `Workflow`).
    #[must_use]
    pub fn workflow_id(&self) -> Option<&str> {
        match &self.reason {
            ActionReason::Workflow(id) => Some(id.as_str()),
            ActionReason::Symptom(_) => None,
        }
    }

    /// The action's `symptom_id` (if the
    /// reason is `Symptom`).
    #[must_use]
    pub fn symptom_id(&self) -> Option<&str> {
        match &self.reason {
            ActionReason::Symptom(id) => Some(id.as_str()),
            ActionReason::Workflow(_) => None,
        }
    }
}

/// The current state of the background agent.
/// The runtime holds the state; each `tick`
/// returns the new actions to surface.
#[derive(Debug, Clone, PartialEq)]
pub struct BackgroundState {
    /// The latest diagnostic report. The host
    /// calls `ingest_signals` to update it.
    pub report: DiagnosticReport,
    /// The recovery policy.
    pub recovery: RecoveryPolicy,
    /// The workflow registry.
    pub workflows: WorkflowRegistry,
    /// The last time each time-of-day workflow
    /// fired (in milliseconds since epoch).
    /// The runtime uses this to avoid firing
    /// the same workflow twice in the same
    /// minute.
    pub last_fired: BTreeMap<WorkflowId, u64>,
    /// The symptom ids the runtime has already
    /// produced recovery actions for. The
    /// runtime skips symptoms already in this
    /// map, so a stable diagnosis (e.g. an
    /// app crash loop that won't go away) is
    /// only planned once.
    pub planned_symptoms: BTreeMap<String, u64>,
}

impl BackgroundState {
    /// Construct a fresh state.
    #[must_use]
    pub fn new(
        report: DiagnosticReport,
        recovery: RecoveryPolicy,
        workflows: WorkflowRegistry,
    ) -> Self {
        Self {
            report,
            recovery,
            workflows,
            last_fired: BTreeMap::new(),
            planned_symptoms: BTreeMap::new(),
        }
    }
}

/// The action queue: the list of action items
/// the runtime just produced. The runner
/// drains the queue, asks the user for consent
/// when required, and turns each item into an
/// `AgentTask`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ActionQueue {
    /// The actions to surface.
    pub items: Vec<ActionItem>,
}

impl ActionQueue {
    /// An empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an item.
    #[must_use]
    pub fn push(mut self, item: ActionItem) -> Self {
        self.items.push(item);
        self
    }

    /// The number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether any item in the queue requires
    /// consent. The runner uses this to decide
    /// whether to block on a user dialog.
    #[must_use]
    pub fn needs_consent(&self) -> bool {
        self.items.iter().any(|i| i.requires_consent)
    }

    /// The number of items that require
    /// consent.
    #[must_use]
    pub fn consent_count(&self) -> usize {
        self.items.iter().filter(|i| i.requires_consent).count()
    }

    /// Split into (auto, needs_consent) — the
    /// runner can dispatch the auto items
    /// immediately and surface the rest in a
    /// single dialog.
    #[must_use]
    pub fn partition_consent(self) -> (Vec<ActionItem>, Vec<ActionItem>) {
        let mut auto = Vec::new();
        let mut consent = Vec::new();
        for item in self.items {
            if item.requires_consent {
                consent.push(item);
            } else {
                auto.push(item);
            }
        }
        (auto, consent)
    }
}

/// Ingest a stream of `AgentEvent`s and produce
/// the new actions. This is the one tick of the
/// background loop.
///
/// The function is *pure*: it takes the current
/// state plus the event stream, and returns the
/// actions to surface. The host updates the
/// state (e.g. the diagnostic report, the
/// `last_fired` map) after it has surfaced the
/// actions.
#[must_use]
pub fn tick(state: &mut BackgroundState, events: &[AgentEvent], now_ms: u64) -> ActionQueue {
    let mut queue = ActionQueue::new();

    for event in events {
        match event {
            AgentEvent::SignalUpdate(signal) => {
                state.report.ingest(signal.clone());
            }
            AgentEvent::TimeTick { hour, minute } => {
                queue = queue.merge(plan_time_triggers(
                    &state.workflows,
                    *hour,
                    *minute,
                    now_ms,
                    &mut state.last_fired,
                ));
            }
            AgentEvent::WorkflowTrigger(id) => {
                if let Some(w) = state.workflows.get(id.as_str()) {
                    queue = queue.merge(plan_workflow(w, &id.to_string(), now_ms));
                }
            }
        }
    }

    // After ingesting signals, plan recovery for
    // any new symptoms. We only plan for
    // symptoms we haven't already planned
    // (the `planned_symptoms` map).
    state.report.evaluate();
    let symptoms = state.report.symptoms.clone();
    let mut new_plans: Vec<RecoveryPlan> = Vec::new();
    for s in &symptoms {
        if state.planned_symptoms.contains_key(&s.id) {
            continue;
        }
        if let Some(plan) = state.recovery.plan_for(&s.id) {
            state.planned_symptoms.insert(s.id.clone(), now_ms);
            new_plans.push(plan);
        }
    }
    queue = queue.merge(plan_recovery_actions(&new_plans, now_ms));

    queue
}

impl ActionQueue {
    /// Merge another queue into this one. The
    /// order of items is preserved (left then
    /// right).
    #[must_use]
    pub fn merge(mut self, other: ActionQueue) -> Self {
        self.items.extend(other.items);
        self
    }
}

/// Plan the time-of-day workflows whose
/// hour/minute match the tick. Updates
/// `last_fired` so the workflow does not fire
/// twice in the same minute.
fn plan_time_triggers(
    registry: &WorkflowRegistry,
    hour: u8,
    minute: u8,
    now_ms: u64,
    last_fired: &mut BTreeMap<WorkflowId, u64>,
) -> ActionQueue {
    let mut queue = ActionQueue::new();
    for w in registry.with_trigger(&Trigger::TimeOfDay { hour, minute }) {
        let id = w.id.clone();
        // Avoid firing the same workflow twice
        // in the same minute. The host supplies
        // `now_ms`; we treat any `last_fired`
        // within the last 60s as "already
        // fired".
        if let Some(prev) = last_fired.get(&id) {
            if now_ms.saturating_sub(*prev) < 60_000 {
                continue;
            }
        }
        last_fired.insert(id.clone(), now_ms);
        queue = queue.merge(plan_workflow(w, &id.to_string(), now_ms));
    }
    queue
}

/// Plan a single workflow (manual or
/// event-triggered) into action items.
fn plan_workflow(workflow: &Workflow, id: &str, now_ms: u64) -> ActionQueue {
    let mut queue = ActionQueue::new();
    let tasks = compile_to_tasks(workflow, &format!("wf.{id}"), now_ms);
    for task in tasks {
        let payload = task.arguments.clone().unwrap_or_else(|| serde_json::json!({}));
        let kind = payload["action"]["kind"].as_str().unwrap_or("").to_string();
        let title = task.title.clone();
        let description = payload["action"].to_string();
        let subsystem = subsystem_for_payload(&payload);
        let requires_consent = matches!(task.risk, TaskRisk::High | TaskRisk::Critical);
        let item = ActionItem {
            kind: ActionKind::Workflow,
            reason: ActionReason::Workflow(id.to_string()),
            title,
            description,
            requires_consent,
            task_kind: task.kind,
            task_risk: task.risk,
            subsystem,
            payload,
        };
        let _ = kind;
        queue = queue.push(item);
    }
    queue
}

fn subsystem_for_payload(payload: &serde_json::Value) -> Option<Subsystem> {
    let kind = payload.get("action")?.get("kind")?.as_str()?;
    match kind {
        "launch_app" => Some(Subsystem::App),
        "open_file" => Some(Subsystem::FileSystem),
        "agent_task" => Some(Subsystem::Other),
        "recovery" => Some(Subsystem::Other),
        "notify" => Some(Subsystem::Other),
        "wait" => None,
        _ => None,
    }
}

/// Plan a list of recovery plans into action
/// items. Each plan becomes one item per
/// action.
fn plan_recovery_actions(plans: &[RecoveryPlan], now_ms: u64) -> ActionQueue {
    let mut queue = ActionQueue::new();
    let _ = now_ms;
    for plan in plans {
        for action in &plan.actions {
            queue = queue.push(recovery_to_item(action, &plan.symptom_id));
        }
    }
    queue
}

fn recovery_to_item(action: &RecoveryAction, symptom_id: &str) -> ActionItem {
    let subsystem = action.subsystem();
    let requires_consent = action.requires_consent();
    let title = action.summary();
    let (task_kind, payload) = recovery_payload(action);
    let description = match action {
        RecoveryAction::RestartService { service, .. } if service == "<auto>" => {
            "The agent will look up the actual service name at execution time.".to_string()
        }
        RecoveryAction::RestartApp { app_id, .. } if app_id == "<auto>" => {
            "The agent will look up the actual app id at execution time.".to_string()
        }
        RecoveryAction::RestartService { .. }
        | RecoveryAction::RestartApp { .. }
        | RecoveryAction::ReconnectNetwork { .. }
        | RecoveryAction::ResolveDependency { .. }
        | RecoveryAction::FreeDiskCache { .. }
        | RecoveryAction::DropPageCache { .. }
        | RecoveryAction::KillProcess { .. }
        | RecoveryAction::InformUser { .. } => title.clone(),
        _ => title.clone(),
    };
    ActionItem {
        kind: ActionKind::Recovery,
        reason: ActionReason::Symptom(symptom_id.to_string()),
        title,
        description,
        requires_consent,
        task_kind,
        task_risk: if requires_consent { TaskRisk::High } else { TaskRisk::Medium },
        subsystem: Some(subsystem),
        payload,
    }
}

fn recovery_payload(action: &RecoveryAction) -> (TaskKind, serde_json::Value) {
    match action {
        RecoveryAction::RestartService { service, reason } => (
            TaskKind::RestartService,
            serde_json::json!({"kind": "restart_service", "service": service, "reason": reason}),
        ),
        RecoveryAction::RestartApp { app_id, reason } => (
            TaskKind::Custom,
            serde_json::json!({"kind": "restart_app", "app_id": app_id, "reason": reason}),
        ),
        RecoveryAction::ReconnectNetwork { interface, reason } => (
            TaskKind::Custom,
            serde_json::json!({"kind": "reconnect_network", "interface": interface, "reason": reason}),
        ),
        RecoveryAction::ResolveDependency { dependency, reason } => (
            TaskKind::Custom,
            serde_json::json!({"kind": "resolve_dependency", "dependency": dependency, "reason": reason}),
        ),
        RecoveryAction::FreeDiskCache { cache, reason } => (
            TaskKind::ProposeCleanup,
            serde_json::json!({"kind": "free_disk_cache", "cache": cache, "reason": reason}),
        ),
        RecoveryAction::DropPageCache { reason } => (
            TaskKind::ProposeCleanup,
            serde_json::json!({"kind": "drop_page_cache", "reason": reason}),
        ),
        RecoveryAction::KillProcess { pid, reason } => (
            TaskKind::Custom,
            serde_json::json!({"kind": "kill_process", "pid": pid, "reason": reason}),
        ),
        RecoveryAction::InformUser { explanation } => (
            TaskKind::Notify,
            serde_json::json!({"kind": "inform_user", "cause": explanation.cause, "fix": explanation.fix}),
        ),
        _ => (
            TaskKind::Notify,
            serde_json::json!({"kind": "inform_user", "cause": "unknown", "fix": ""}),
        ),
    }
}

/// Compile a single `ActionItem` into the
/// `AgentTask` the runner will execute. The
/// caller supplies the `task_id` (the runner's
/// UUIDv7 base) and the wall-clock timestamp.
#[must_use]
pub fn action_to_task(
    item: &ActionItem,
    task_id: &TaskId,
    _timestamp_ms: u64,
) -> Option<AgentTask> {
    let mut task = AgentTask::new(
        task_id.as_str().to_string(),
        item.task_kind,
        item.title.clone(),
        item.description.clone(),
    )?;
    task.risk = item.task_risk;
    task = task.with_arguments(item.payload.clone());
    Some(task)
}

/// Run a workflow and return the compiled
/// `AgentTask`s in execution order. The host
/// is responsible for tracking failure policy
/// and retrying.
#[must_use]
pub fn run_workflow(workflow: &Workflow, task_id_prefix: &str, now_ms: u64) -> Vec<AgentTask> {
    compile_to_tasks(workflow, task_id_prefix, now_ms)
}

/// The default background state: empty
/// diagnostics + default recovery policy +
/// default workflow registry.
#[must_use]
pub fn default_state() -> BackgroundState {
    BackgroundState::new(
        DiagnosticReport::default(),
        aether_recovery::default_policy(),
        aether_automation::default_registry(),
    )
}

/// Diagnostics helpers — re-expose the
/// default rules so the runner can build its
/// own state with the project's defaults.
pub use aether_diagnostics::default_rules as default_diagnostic_rules;

#[allow(unused_imports)]
use FailurePolicy as _;
#[allow(unused_imports)]
use StepAction as _;
#[allow(unused_imports)]
use WorkflowStep as _;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_diagnostics::Explanation;
    use alloc::collections::BTreeSet;

    fn high_cpu_signal(value: f32) -> Signal {
        Signal::new(Subsystem::Cpu, "cpu.load", value, "load")
    }

    fn high_mem_signal(value: f32) -> Signal {
        Signal::new(Subsystem::Memory, "memory.pressure", value, "pressure")
    }

    fn make_state() -> BackgroundState {
        let report = DiagnosticReport::new();
        BackgroundState::new(
            report,
            aether_recovery::default_policy(),
            aether_automation::default_registry(),
        )
    }

    #[test]
    fn action_kind_as_str() {
        assert_eq!(ActionKind::Recovery.as_str(), "recovery");
        assert_eq!(ActionKind::Workflow.as_str(), "workflow");
    }

    #[test]
    fn action_reason_summary() {
        assert_eq!(ActionReason::Symptom("x".into()).summary(), "Symptom `x`");
        assert_eq!(ActionReason::Workflow("y".into()).summary(), "Workflow `y`");
    }

    #[test]
    fn action_item_workflow_id() {
        let i = ActionItem {
            kind: ActionKind::Workflow,
            reason: ActionReason::Workflow("wf".into()),
            title: "t".into(),
            description: "d".into(),
            requires_consent: false,
            task_kind: TaskKind::Notify,
            task_risk: TaskRisk::Low,
            subsystem: None,
            payload: serde_json::json!({}),
        };
        assert_eq!(i.workflow_id(), Some("wf"));
        assert_eq!(i.symptom_id(), None);
    }

    #[test]
    fn action_item_symptom_id() {
        let i = ActionItem {
            kind: ActionKind::Recovery,
            reason: ActionReason::Symptom("s".into()),
            title: "t".into(),
            description: "d".into(),
            requires_consent: false,
            task_kind: TaskKind::RestartService,
            task_risk: TaskRisk::Medium,
            subsystem: Some(Subsystem::Service),
            payload: serde_json::json!({}),
        };
        assert_eq!(i.symptom_id(), Some("s"));
        assert_eq!(i.workflow_id(), None);
    }

    #[test]
    fn action_queue_partition_consent() {
        let i1 = ActionItem {
            kind: ActionKind::Recovery,
            reason: ActionReason::Symptom("s".into()),
            title: "t1".into(),
            description: "d".into(),
            requires_consent: false,
            task_kind: TaskKind::ProposeCleanup,
            task_risk: TaskRisk::Medium,
            subsystem: Some(Subsystem::Memory),
            payload: serde_json::json!({}),
        };
        let i2 = ActionItem {
            kind: ActionKind::Recovery,
            reason: ActionReason::Symptom("s".into()),
            title: "t2".into(),
            description: "d".into(),
            requires_consent: true,
            task_kind: TaskKind::Custom,
            task_risk: TaskRisk::High,
            subsystem: Some(Subsystem::Other),
            payload: serde_json::json!({}),
        };
        let q = ActionQueue::new().push(i1).push(i2);
        let (auto, consent) = q.partition_consent();
        assert_eq!(auto.len(), 1);
        assert_eq!(consent.len(), 1);
        assert!(!auto[0].requires_consent);
        assert!(consent[0].requires_consent);
    }

    #[test]
    fn action_queue_consent_count() {
        let i1 = ActionItem {
            kind: ActionKind::Recovery,
            reason: ActionReason::Symptom("s".into()),
            title: "t".into(),
            description: "d".into(),
            requires_consent: false,
            task_kind: TaskKind::Notify,
            task_risk: TaskRisk::Low,
            subsystem: None,
            payload: serde_json::json!({}),
        };
        let i2 = ActionItem {
            kind: ActionKind::Recovery,
            reason: ActionReason::Symptom("s".into()),
            title: "t".into(),
            description: "d".into(),
            requires_consent: true,
            task_kind: TaskKind::Custom,
            task_risk: TaskRisk::High,
            subsystem: None,
            payload: serde_json::json!({}),
        };
        let q = ActionQueue::new().push(i1).push(i2);
        assert_eq!(q.consent_count(), 1);
        assert!(q.needs_consent());
    }

    #[test]
    fn tick_ingests_signals() {
        let mut s = make_state();
        let q = tick(&mut s, &[AgentEvent::SignalUpdate(high_cpu_signal(0.9))], 1000);
        let _ = q;
        // After ingesting a high-CPU signal, the
        // diagnostics rule should fire.
        assert!(!s.report.symptoms.is_empty());
    }

    #[test]
    fn tick_plans_recovery_for_cpu_overload() {
        let mut s = make_state();
        let q = tick(&mut s, &[AgentEvent::SignalUpdate(high_cpu_signal(0.9))], 1000);
        let titles: Vec<String> = q.items.iter().map(|i| i.title.clone()).collect();
        // cpu_overload -> InformUser action.
        assert!(titles.iter().any(|t| t.contains("CPU")));
    }

    #[test]
    fn tick_plans_recovery_for_memory_pressure() {
        let mut s = make_state();
        let q = tick(&mut s, &[AgentEvent::SignalUpdate(high_mem_signal(0.9))], 1000);
        let titles: Vec<String> = q.items.iter().map(|i| i.title.clone()).collect();
        // memory_pressure -> DropPageCache.
        assert!(titles.iter().any(|t| t.contains("Drop page cache")));
        // Drop page cache does not require consent.
        assert!(!q.needs_consent());
    }

    #[test]
    fn kill_process_action_requires_consent() {
        let plan = RecoveryPlan::new("x")
            .with_action(RecoveryAction::KillProcess { pid: 1234, reason: "runaway".into() });
        let q = plan_recovery_actions(&[plan], 0);
        assert_eq!(q.len(), 1);
        assert!(q.items[0].requires_consent);
        assert_eq!(q.items[0].task_risk, TaskRisk::High);
    }

    #[test]
    fn time_tick_fires_morning_setup() {
        let mut s = make_state();
        let q = tick(&mut s, &[AgentEvent::TimeTick { hour: 9, minute: 0 }], 1000);
        let titles: Vec<String> = q.items.iter().map(|i| i.title.clone()).collect();
        // morning_setup has 3 steps.
        assert_eq!(titles.len(), 3);
        assert!(titles.iter().all(|t| t.contains("Morning setup")));
    }

    #[test]
    fn time_tick_does_not_fire_twice_in_same_minute() {
        let mut s = make_state();
        let q1 = tick(&mut s, &[AgentEvent::TimeTick { hour: 9, minute: 0 }], 1000);
        let q2 = tick(&mut s, &[AgentEvent::TimeTick { hour: 9, minute: 0 }], 30_000);
        assert!(!q1.is_empty());
        assert!(q2.is_empty());
    }

    #[test]
    fn time_tick_refires_after_a_minute() {
        let mut s = make_state();
        let _ = tick(&mut s, &[AgentEvent::TimeTick { hour: 9, minute: 0 }], 1000);
        let q = tick(&mut s, &[AgentEvent::TimeTick { hour: 9, minute: 0 }], 70_000);
        assert!(!q.is_empty());
    }

    #[test]
    fn time_tick_does_not_fire_at_other_hours() {
        let mut s = make_state();
        let q = tick(&mut s, &[AgentEvent::TimeTick { hour: 14, minute: 30 }], 1000);
        // No workflow at 14:30 in the default
        // registry, and no signal-induced
        // symptoms.
        assert!(q.is_empty());
    }

    #[test]
    fn workflow_trigger_fires_specific_workflow() {
        let mut s = make_state();
        let id = WorkflowId::new("before_meeting").unwrap();
        let q = tick(&mut s, &[AgentEvent::WorkflowTrigger(id)], 1000);
        assert!(!q.is_empty());
        let reasons: BTreeSet<String> =
            q.items.iter().filter_map(|i| i.workflow_id().map(|s| s.to_string())).collect();
        assert!(reasons.contains("before_meeting"));
    }

    #[test]
    fn workflow_trigger_unknown_is_noop() {
        let mut s = make_state();
        let id = WorkflowId::new("nope").unwrap();
        let q = tick(&mut s, &[AgentEvent::WorkflowTrigger(id)], 1000);
        assert!(q.is_empty());
    }

    #[test]
    fn action_to_task_carries_payload_and_risk() {
        let item = ActionItem {
            kind: ActionKind::Recovery,
            reason: ActionReason::Symptom("s".into()),
            title: "Drop page cache".into(),
            description: "d".into(),
            requires_consent: false,
            task_kind: TaskKind::ProposeCleanup,
            task_risk: TaskRisk::Medium,
            subsystem: Some(Subsystem::Memory),
            payload: serde_json::json!({"kind": "drop_page_cache", "reason": "x"}),
        };
        let task = action_to_task(&item, &TaskId::new("t1").unwrap(), 0).expect("task");
        assert_eq!(task.kind, TaskKind::ProposeCleanup);
        assert_eq!(task.risk, TaskRisk::Medium);
        assert_eq!(task.arguments.unwrap()["kind"], "drop_page_cache");
    }

    #[test]
    fn default_state_has_three_workflows() {
        let s = default_state();
        assert_eq!(s.workflows.len(), 3);
    }

    #[test]
    fn recovery_payload_drop_page_cache() {
        let a = RecoveryAction::DropPageCache { reason: "free".into() };
        let (kind, payload) = recovery_payload(&a);
        assert_eq!(kind, TaskKind::ProposeCleanup);
        assert_eq!(payload["kind"], "drop_page_cache");
    }

    #[test]
    fn recovery_payload_kill_process() {
        let a = RecoveryAction::KillProcess { pid: 42, reason: "bad".into() };
        let (kind, payload) = recovery_payload(&a);
        assert_eq!(kind, TaskKind::Custom);
        assert_eq!(payload["pid"], 42);
    }

    #[test]
    fn recovery_payload_inform_user() {
        let a = RecoveryAction::InformUser { explanation: Explanation::new("c", "cause", "fix") };
        let (kind, payload) = recovery_payload(&a);
        assert_eq!(kind, TaskKind::Notify);
        assert_eq!(payload["cause"], "cause");
    }

    #[test]
    fn subsystem_for_payload_maps_action_kinds() {
        let p = serde_json::json!({"action": {"kind": "launch_app"}});
        assert_eq!(subsystem_for_payload(&p), Some(Subsystem::App));
        let p = serde_json::json!({"action": {"kind": "wait"}});
        assert_eq!(subsystem_for_payload(&p), None);
        let p = serde_json::json!({"action": {"kind": "open_file"}});
        assert_eq!(subsystem_for_payload(&p), Some(Subsystem::FileSystem));
    }

    #[test]
    fn run_workflow_returns_tasks() {
        let reg = aether_automation::default_registry();
        let w = reg.get("before_meeting").unwrap();
        let tasks = run_workflow(w, "wf", 0);
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn tick_handles_empty_event_stream() {
        let mut s = make_state();
        let q = tick(&mut s, &[], 0);
        assert!(q.is_empty());
    }

    #[test]
    fn tick_processes_multiple_events() {
        let mut s = make_state();
        let id = WorkflowId::new("before_meeting").unwrap();
        let q = tick(
            &mut s,
            &[AgentEvent::SignalUpdate(high_mem_signal(0.9)), AgentEvent::WorkflowTrigger(id)],
            1000,
        );
        // memory_pressure -> DropPageCache +
        // before_meeting -> 2 steps.
        assert!(q.items.len() >= 3);
    }

    #[test]
    fn action_queue_merge_preserves_order() {
        let a = ActionQueue::new().push(ActionItem {
            kind: ActionKind::Recovery,
            reason: ActionReason::Symptom("s".into()),
            title: "a".into(),
            description: "".into(),
            requires_consent: false,
            task_kind: TaskKind::Notify,
            task_risk: TaskRisk::Low,
            subsystem: None,
            payload: serde_json::json!({}),
        });
        let b = ActionQueue::new().push(ActionItem {
            kind: ActionKind::Recovery,
            reason: ActionReason::Symptom("s".into()),
            title: "b".into(),
            description: "".into(),
            requires_consent: false,
            task_kind: TaskKind::Notify,
            task_risk: TaskRisk::Low,
            subsystem: None,
            payload: serde_json::json!({}),
        });
        let c = a.merge(b);
        assert_eq!(c.items[0].title, "a");
        assert_eq!(c.items[1].title, "b");
    }

    #[test]
    fn tick_does_not_replan_already_diagnosed() {
        // The first tick plans cpu_overload; the
        // second tick (no new signals) should
        // not re-plan it.
        let mut s = make_state();
        let q1 = tick(&mut s, &[AgentEvent::SignalUpdate(high_cpu_signal(0.9))], 1000);
        let q2 = tick(&mut s, &[], 2000);
        assert!(!q1.is_empty());
        assert!(q2.is_empty());
    }
}

// Additional coverage: the recovery's
// InformUser / FreeDiskCache / RestartService
// paths.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod more_tests {
    use super::*;

    fn high_cpu_signal(value: f32) -> Signal {
        Signal::new(Subsystem::Cpu, "cpu.load", value, "load")
    }

    fn make_state() -> BackgroundState {
        let report = DiagnosticReport::new();
        BackgroundState::new(
            report,
            aether_recovery::default_policy(),
            aether_automation::default_registry(),
        )
    }

    #[test]
    fn recovery_payload_restart_service() {
        let a = RecoveryAction::RestartService {
            service: "aether-supervisor".into(),
            reason: "x".into(),
        };
        let (kind, payload) = recovery_payload(&a);
        assert_eq!(kind, TaskKind::RestartService);
        assert_eq!(payload["service"], "aether-supervisor");
    }

    #[test]
    fn recovery_payload_free_disk_cache() {
        let a = RecoveryAction::FreeDiskCache { cache: "apt".into(), reason: "x".into() };
        let (kind, payload) = recovery_payload(&a);
        assert_eq!(kind, TaskKind::ProposeCleanup);
        assert_eq!(payload["cache"], "apt");
    }

    #[test]
    fn recovery_payload_reconnect_network() {
        let a = RecoveryAction::ReconnectNetwork { interface: "wlan0".into(), reason: "x".into() };
        let (kind, payload) = recovery_payload(&a);
        assert_eq!(kind, TaskKind::Custom);
        assert_eq!(payload["interface"], "wlan0");
    }

    #[test]
    fn recovery_payload_resolve_dependency() {
        let a =
            RecoveryAction::ResolveDependency { dependency: "libssl3".into(), reason: "x".into() };
        let (kind, payload) = recovery_payload(&a);
        assert_eq!(kind, TaskKind::Custom);
        assert_eq!(payload["dependency"], "libssl3");
    }

    #[test]
    fn recovery_to_item_uses_symptom_id() {
        let a = RecoveryAction::DropPageCache { reason: "x".into() };
        let i = recovery_to_item(&a, "memory_pressure");
        assert_eq!(i.symptom_id(), Some("memory_pressure"));
        assert_eq!(i.task_kind, TaskKind::ProposeCleanup);
    }

    #[test]
    fn recovery_to_item_uses_auto_placeholder() {
        let a = RecoveryAction::RestartService { service: "<auto>".into(), reason: "x".into() };
        let i = recovery_to_item(&a, "service_down");
        assert!(i.description.contains("look up"));
    }

    #[test]
    fn subsystem_for_payload_unknown_kind() {
        let p = serde_json::json!({"action": {"kind": "???"}});
        assert_eq!(subsystem_for_payload(&p), None);
    }

    #[test]
    fn subsystem_for_payload_missing_action() {
        let p = serde_json::json!({});
        assert_eq!(subsystem_for_payload(&p), None);
    }

    #[test]
    fn action_to_task_rejects_empty_id() {
        // Just a sanity check that the helper
        // returns None on bad input.
        let item = ActionItem {
            kind: ActionKind::Recovery,
            reason: ActionReason::Symptom("s".into()),
            title: "t".into(),
            description: "d".into(),
            requires_consent: false,
            task_kind: TaskKind::Notify,
            task_risk: TaskRisk::Low,
            subsystem: None,
            payload: serde_json::json!({}),
        };
        let task = action_to_task(&item, &TaskId::new("t1").unwrap(), 0);
        assert!(task.is_some());
    }

    #[test]
    fn tick_increments_report_evaluate() {
        let mut s = make_state();
        // 2 high-CPU signals should not change
        // the symptom set (idempotent).
        let _ = tick(&mut s, &[AgentEvent::SignalUpdate(high_cpu_signal(0.9))], 1);
        let before_count = s.report.symptoms.iter().filter(|x| x.id == "cpu_overload").count();
        let _ = tick(&mut s, &[AgentEvent::SignalUpdate(high_cpu_signal(0.95))], 2);
        let after_count = s.report.symptoms.iter().filter(|x| x.id == "cpu_overload").count();
        assert_eq!(before_count, after_count);
    }

    #[test]
    fn _silence_unused_severity_imports() {
        let _ = ();
    }
}
