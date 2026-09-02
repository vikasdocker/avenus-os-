//! Aether self-healing — bounded recovery actions.
//!
//! The diagnostics pipeline (7.1) reports symptoms.
//! The self-healing pipeline consumes those
//! symptoms and produces typed `RecoveryAction`s
//! the agent can execute.
//!
//! The contract is *bounded*: every recovery action
//! is a typed description the agent (and the user)
//! can review. There are no "do whatever it takes"
//! modes. The action is gated by an
//! `Outcome::requires_consent` flag — the agent
//! will only execute non-consent actions
//! automatically; consent-required actions must
//! be approved by the user through the assistant
//! panel's `TaskView` (Phase 6.5).
//!
//! The five recovery families per the ROADMAP:
//!
//! - **Service restart** — bring a crashed
//!   `aether-supervisor` / `aether-init` / network
//!   manager back up.
//! - **Network recovery** — reconnect a dropped
//!   link, cycle the network manager.
//! - **Application recovery** — restart a
//!   crash-looping app, re-launch a hung one.
//! - **Dependency recovery** — re-resolve a broken
//!   package or service dependency.
//! - **Resource recovery** — free disk (clean the
//!   package cache), free memory (drop caches),
//!   kill a runaway process (with consent).
//!
//! The crate is *pure* — it produces actions; it
//! does not execute them. The agent (7.2's next
//! layer) is what runs them.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use aether_diagnostics::{Explanation, Subsystem, Symptom};

use alloc::string::String;
use alloc::vec::Vec;

/// A single, reviewable recovery step. The agent
/// executes these in order; the renderer / assistant
/// panel renders them as `TaskView` rows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RecoveryAction {
    /// Restart a system service (e.g.
    /// `aether-supervisor`).
    RestartService {
        /// The service name (e.g.
        /// `"aether-supervisor"`).
        service: String,
        /// The reason this restart was proposed.
        reason: String,
    },
    /// Restart a user application by its app id.
    RestartApp {
        /// The app id (e.g. `"aether.notes"`).
        app_id: String,
        /// The reason this restart was proposed.
        reason: String,
    },
    /// Reconnect the network (cycle the network
    /// manager, retry the DHCP lease).
    ReconnectNetwork {
        /// The network interface (e.g. `"wlan0"`).
        /// Empty = the default route's interface.
        interface: String,
        /// The reason.
        reason: String,
    },
    /// Re-resolve a service / package dependency.
    ResolveDependency {
        /// The dependency (e.g. `"libssl3"`).
        dependency: String,
        /// The reason.
        reason: String,
    },
    /// Free disk space by cleaning a specific
    /// cache (e.g. the package manager's cache).
    FreeDiskCache {
        /// The cache name (e.g. `"apt"`,
        /// `"pacman"`, `"cargo"`).
        cache: String,
        /// The reason.
        reason: String,
    },
    /// Free memory by dropping the kernel's page
    /// cache (this is safe; it just forces the OS
    /// to re-read from disk).
    DropPageCache {
        /// The reason.
        reason: String,
    },
    /// Kill a runaway process by pid. Always
    /// requires consent.
    KillProcess {
        /// The pid to kill.
        pid: u32,
        /// The reason.
        reason: String,
    },
    /// Surface a generic explanation to the user
    /// (e.g. "the disk is full — please run the
    /// cleanup yourself"). This is the "no
    /// auto-recovery available" action.
    InformUser {
        /// The full explanation to surface.
        explanation: Explanation,
    },
}

impl RecoveryAction {
    /// The action's subsystem (the subsystem the
    /// action is targeting).
    #[must_use]
    pub const fn subsystem(&self) -> Subsystem {
        match self {
            Self::RestartService { .. } => Subsystem::Service,
            Self::RestartApp { .. } => Subsystem::App,
            Self::ReconnectNetwork { .. } => Subsystem::Network,
            Self::ResolveDependency { .. } => Subsystem::Other,
            Self::FreeDiskCache { .. } => Subsystem::Disk,
            Self::DropPageCache { .. } => Subsystem::Memory,
            Self::KillProcess { .. } => Subsystem::Other,
            Self::InformUser { .. } => Subsystem::Other,
        }
    }

    /// A short, single-sentence summary of the
    /// action. The renderer / agent uses this as the
    /// task title.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::RestartService { service, reason } => {
                format!("Restart `{service}`: {reason}")
            }
            Self::RestartApp { app_id, reason } => {
                format!("Restart `{app_id}`: {reason}")
            }
            Self::ReconnectNetwork { interface, reason } => {
                let if_name =
                    if interface.is_empty() { "the default network" } else { interface.as_str() };
                format!("Reconnect {if_name}: {reason}")
            }
            Self::ResolveDependency { dependency, reason } => {
                format!("Resolve `{dependency}`: {reason}")
            }
            Self::FreeDiskCache { cache, reason } => {
                format!("Free `{cache}` cache: {reason}")
            }
            Self::DropPageCache { reason } => {
                format!("Drop page cache: {reason}")
            }
            Self::KillProcess { pid, reason } => {
                format!("Kill pid {pid}: {reason}")
            }
            Self::InformUser { explanation } => explanation.cause.clone(),
        }
    }

    /// Whether this action requires user consent
    /// before the agent can execute it.
    /// `InformUser` and the self-healing actions
    /// (restart / reconnect / resolve / free /
    /// drop-cache) are tagged `false`; `KillProcess`
    /// is always `true` (terminating a process is
    /// destructive).
    #[must_use]
    pub const fn requires_consent(&self) -> bool {
        match self {
            Self::KillProcess { .. } => true,
            Self::InformUser { .. } => false,
            _ => false,
        }
    }
}

/// A recovery plan — an ordered list of
/// `RecoveryAction`s the agent will execute to
/// resolve a single symptom.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecoveryPlan {
    /// The symptom this plan addresses.
    pub symptom_id: String,
    /// The ordered list of actions. The agent
    /// executes them top-to-bottom; the first
    /// action that resolves the symptom stops
    /// the rest.
    pub actions: Vec<RecoveryAction>,
}

impl RecoveryPlan {
    /// Construct an empty plan for the given
    /// symptom.
    #[must_use]
    pub fn new(symptom_id: impl Into<String>) -> Self {
        Self { symptom_id: symptom_id.into(), actions: Vec::new() }
    }

    /// Append an action.
    #[must_use]
    pub fn with_action(mut self, action: RecoveryAction) -> Self {
        self.actions.push(action);
        self
    }

    /// The number of actions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Whether the plan has no actions (i.e. no
    /// automatic recovery is available).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Whether any action in the plan requires
    /// consent. If true, the agent must surface
    /// the plan to the user before executing it.
    #[must_use]
    pub fn needs_consent(&self) -> bool {
        self.actions.iter().any(RecoveryAction::requires_consent)
    }
}

/// The recovery policy: maps symptom ids to
/// action recipes. The default policy handles the
/// §7.1 default symptoms; callers can extend it
/// at runtime with subsystem-specific recipes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct RecoveryPolicy {
    /// The recipes: symptom id → ordered actions.
    pub recipes: Vec<(String, Vec<RecoveryAction>)>,
}

impl RecoveryPolicy {
    /// An empty policy.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a recipe.
    #[must_use]
    pub fn with_recipe(
        mut self,
        symptom_id: impl Into<String>,
        actions: Vec<RecoveryAction>,
    ) -> Self {
        self.recipes.push((symptom_id.into(), actions));
        self
    }

    /// Build a plan for the given symptom, or
    /// `None` if no recipe is registered.
    #[must_use]
    pub fn plan_for(&self, symptom_id: &str) -> Option<RecoveryPlan> {
        self.recipes
            .iter()
            .find(|(id, _)| id == symptom_id)
            .map(|(id, actions)| RecoveryPlan { symptom_id: id.clone(), actions: actions.clone() })
    }
}

/// The default recovery policy. The recipes here
/// handle the symptoms `default_rules()` from
/// `aether-diagnostics` produces.
#[must_use]
pub fn default_policy() -> RecoveryPolicy {
    RecoveryPolicy::new()
        // cpu_overload -> no automatic recovery;
        // the user needs to find the runaway
        // process. We surface an InformUser.
        .with_recipe(
            "cpu_overload",
            alloc::vec![RecoveryAction::InformUser {
                explanation: Explanation::new(
                    "cpu_overload",
                    "A process is using the CPU heavily.",
                    "Open the taskbar's CPU chip to see the top process.",
                )
                .self_healing(),
            }],
        )
        // memory_pressure -> drop the page cache
        // to reclaim kernel-side pages. Safe and
        // doesn't need consent.
        .with_recipe(
            "memory_pressure",
            alloc::vec![RecoveryAction::DropPageCache {
                reason: "Free kernel page cache to relieve memory pressure.".into(),
            }],
        )
        // disk_full -> free the package manager's
        // cache.
        .with_recipe(
            "disk_full",
            alloc::vec![RecoveryAction::FreeDiskCache {
                cache: "package-manager".into(),
                reason: "Clean the package manager's cache to free disk space.".into(),
            }],
        )
        // service_down -> restart the service.
        .with_recipe(
            "service_down",
            alloc::vec![RecoveryAction::RestartService {
                service: "<auto>".into(),
                reason: "Service is down; restart to recover.".into(),
            }],
        )
        // app_crash_loop -> restart the app. The
        // caller fills in the app id.
        .with_recipe(
            "app_crash_loop",
            alloc::vec![RecoveryAction::RestartApp {
                app_id: "<auto>".into(),
                reason: "App is in a crash loop; restart to recover.".into(),
            }],
        )
        // system_unstable -> run a multi-step
        // recovery: drop page cache, then
        // reconnect network, then surface to the
        // user.
        .with_recipe(
            "system_unstable",
            alloc::vec![
                RecoveryAction::DropPageCache {
                    reason: "Free kernel page cache as part of system recovery.".into(),
                },
                RecoveryAction::ReconnectNetwork {
                    interface: String::new(),
                    reason: "Cycle the network link as part of system recovery.".into(),
                },
            ],
        )
}

/// Plan recovery for a list of symptoms. Returns
/// one `RecoveryPlan` per symptom; symptoms with
/// no recipe are dropped.
#[must_use]
pub fn plan_recovery(symptoms: &[Symptom], policy: &RecoveryPolicy) -> Vec<RecoveryPlan> {
    symptoms.iter().filter_map(|s| policy.plan_for(&s.id)).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_agent_core::ObservationSeverity;

    #[test]
    fn restart_service_subsystem() {
        let a = RecoveryAction::RestartService { service: "x".into(), reason: "y".into() };
        assert_eq!(a.subsystem(), Subsystem::Service);
    }

    #[test]
    fn restart_app_subsystem() {
        let a = RecoveryAction::RestartApp { app_id: "x".into(), reason: "y".into() };
        assert_eq!(a.subsystem(), Subsystem::App);
    }

    #[test]
    fn kill_process_requires_consent() {
        let a = RecoveryAction::KillProcess { pid: 1234, reason: "y".into() };
        assert!(a.requires_consent());
    }

    #[test]
    fn drop_page_cache_does_not_require_consent() {
        let a = RecoveryAction::DropPageCache { reason: "y".into() };
        assert!(!a.requires_consent());
    }

    #[test]
    fn restart_service_does_not_require_consent() {
        let a = RecoveryAction::RestartService { service: "x".into(), reason: "y".into() };
        assert!(!a.requires_consent());
    }

    #[test]
    fn summary_includes_reason() {
        let a = RecoveryAction::RestartService {
            service: "aether-supervisor".into(),
            reason: "crashed".into(),
        };
        let s = a.summary();
        assert!(s.contains("aether-supervisor"));
        assert!(s.contains("crashed"));
    }

    #[test]
    fn reconnect_network_with_default_interface() {
        let a =
            RecoveryAction::ReconnectNetwork { interface: String::new(), reason: "down".into() };
        let s = a.summary();
        assert!(s.contains("default network"));
    }

    #[test]
    fn reconnect_network_with_specific_interface() {
        let a =
            RecoveryAction::ReconnectNetwork { interface: "wlan0".into(), reason: "down".into() };
        let s = a.summary();
        assert!(s.contains("wlan0"));
    }

    #[test]
    fn plan_starts_empty() {
        let p = RecoveryPlan::new("x");
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
        assert!(!p.needs_consent());
    }

    #[test]
    fn plan_with_action() {
        let p = RecoveryPlan::new("x")
            .with_action(RecoveryAction::DropPageCache { reason: "y".into() });
        assert_eq!(p.len(), 1);
        assert!(!p.needs_consent());
    }

    #[test]
    fn plan_needs_consent_when_action_does() {
        let p = RecoveryPlan::new("x")
            .with_action(RecoveryAction::KillProcess { pid: 1, reason: "y".into() });
        assert!(p.needs_consent());
    }

    #[test]
    fn plan_needs_consent_when_any_action_does() {
        let p = RecoveryPlan::new("x")
            .with_action(RecoveryAction::DropPageCache { reason: "y".into() })
            .with_action(RecoveryAction::KillProcess { pid: 1, reason: "z".into() });
        assert!(p.needs_consent());
    }

    #[test]
    fn policy_starts_empty() {
        let p = RecoveryPolicy::new();
        assert!(p.plan_for("anything").is_none());
    }

    #[test]
    fn policy_plan_for_known() {
        let p = RecoveryPolicy::new()
            .with_recipe("x", alloc::vec![RecoveryAction::DropPageCache { reason: "y".into() }]);
        let plan = p.plan_for("x");
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert_eq!(plan.symptom_id, "x");
        assert_eq!(plan.len(), 1);
    }

    #[test]
    fn default_policy_handles_known_symptoms() {
        let p = default_policy();
        assert!(p.plan_for("cpu_overload").is_some());
        assert!(p.plan_for("memory_pressure").is_some());
        assert!(p.plan_for("disk_full").is_some());
        assert!(p.plan_for("service_down").is_some());
        assert!(p.plan_for("app_crash_loop").is_some());
        assert!(p.plan_for("system_unstable").is_some());
    }

    #[test]
    fn default_policy_unknown_returns_none() {
        let p = default_policy();
        assert!(p.plan_for("nope").is_none());
    }

    #[test]
    fn plan_recovery_drops_unknown() {
        let policy = default_policy();
        let symptoms = [
            Symptom {
                id: "cpu_overload".into(),
                subsystem: Subsystem::Cpu,
                severity: ObservationSeverity::Warning,
                signals: alloc::vec!["cpu.load".into()],
            },
            Symptom {
                id: "no_recipe".into(),
                subsystem: Subsystem::Other,
                severity: ObservationSeverity::Notice,
                signals: Vec::new(),
            },
        ];
        let plans = plan_recovery(&symptoms, &policy);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].symptom_id, "cpu_overload");
    }

    #[test]
    fn plan_recovery_empty_input() {
        let policy = default_policy();
        let plans = plan_recovery(&[], &policy);
        assert!(plans.is_empty());
    }

    #[test]
    fn system_unstable_plan_has_multiple_actions() {
        let p = default_policy();
        let plan = p.plan_for("system_unstable").unwrap();
        assert!(plan.len() >= 2);
    }
}
