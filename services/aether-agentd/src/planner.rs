// Action Planner - deterministic sequential execution of safe capabilities.
//
// Takes a natural-language prompt, asks the Intent engine for all intents
// (including multi-step), validates each via the capability policy, checks
// confirmation requirements, executes in order, and returns structured
// per-step feedback. Stops on first failure unless the step is non-critical.
//
// Bounded recovery semantics (Phase 2.7):
//   * Read-only capabilities (window.list, file.read, system.status, …)
//     are auto-retried on transient IPC errors up to 3 times with
//     capped exponential backoff.
//   * Mutating capabilities (app.launch, file.delete, …) are retried at
//     most once, and only on transient errors.
//   * Permanent failures (capability denied, approval required, not
//     found) are never retried.
//   * The runtime's `decide_recovery` is the source of truth. We
//     import it from `aether-agent-runtime::recovery` and feed it the
//     same `FailureKind` classifier.

use crate::confirmation::ConfirmationPolicy;
use crate::context::SystemContext;
use crate::intent::{self, CapabilityId, Intent, Rejection};
use aether_agent_runtime::recovery::{
    backoff_delay, decide_recovery, FailureKind, RecoveryAction, RecoveryPolicy,
};
use serde_json::Value;

/// Result of a single planned action.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionResult {
    pub capability: CapabilityId,
    pub arguments: Value,
    pub status: ActionStatus,
    pub message: String,
    /// Raw result from the capability execution, if any.
    pub raw_result: Option<Value>,
}

/// Discrete status for one action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionStatus {
    Success,
    Failed,
    Rejected,
    RequiresConsent,
}

/// The outcome of a full plan (one or more actions for one user prompt).
#[derive(Debug, Clone, PartialEq)]
pub struct PlanResult {
    pub actions: Vec<ActionResult>,
    /// Human-readable aggregate summary for the UI / chat response.
    pub summary: String,
    /// Whether the whole plan succeeded (all steps succeeded or no-ops).
    pub ok: bool,
}

impl PlanResult {
    pub fn single_rejected(capability: CapabilityId, rejection: &Rejection, args: Value) -> Self {
        let msg = format!("REQUEST REJECTED - {rejection}");
        Self {
            actions: vec![ActionResult {
                capability,
                arguments: args,
                status: ActionStatus::Rejected,
                message: msg.clone(),
                raw_result: None,
            }],
            summary: msg,
            ok: false,
        }
    }
}

/// Deterministic planner.
pub struct Planner;

impl Planner {
    /// Build a plan from raw text + current context.
    /// Returns None when no intent is detected (plain chat).
    pub fn plan(
        text: &str,
        ctx: &SystemContext,
        convo_last_app: Option<&str>,
    ) -> Option<Vec<Intent>> {
        Self::plan_with_file(text, ctx, convo_last_app, None)
    }

    /// Extended plan that also handles file pronoun resolution.
    pub fn plan_with_file(
        text: &str,
        ctx: &SystemContext,
        convo_last_app: Option<&str>,
        convo_last_file: Option<&str>,
    ) -> Option<Vec<Intent>> {
        // Ask intent engine for all intents in the text (multi-step).
        let mut intents =
            intent::parse_intents_with_file(text, ctx, convo_last_app, convo_last_file);
        if intents.is_empty() {
            return None;
        }
        // Attach confirmation info later in execution; keep plan deterministic order.
        // Deduplicate consecutive identical intents? Keep as-is for now (e.g., "open calc and notes" yields two distinct).
        // Filter no-ops? Not needed.
        if intents.len() > 8 {
            intents.truncate(8); // bounded plan size.
        }
        Some(intents)
    }

    /// Execute a plan sequentially, returning structured per-step results.
    /// Each intent is validated and checked for confirmation before execution.
    pub fn execute(
        intents: Vec<Intent>,
        control_port: u16,
        surface_port: u16,
        ctx: &SystemContext,
    ) -> PlanResult {
        let mut actions = Vec::with_capacity(intents.len());
        let mut overall_ok = true;

        for intent in intents {
            let cap = intent.capability;
            let args = intent.arguments.clone();

            // 1) Validate against capability policy.
            if let Err(rejection) = intent::validate(&intent) {
                overall_ok = false;
                let msg = format!("REQUEST REJECTED - {rejection}");
                actions.push(ActionResult {
                    capability: cap,
                    arguments: args,
                    status: ActionStatus::Rejected,
                    message: msg,
                    raw_result: None,
                });
                break; // stop on rejection
            }

            // 2) Confirmation policy.
            let capability = cap.capability();
            let decision = ConfirmationPolicy::decide(&capability);
            match decision {
                crate::confirmation::Confirmation::Denied { reason } => {
                    overall_ok = false;
                    actions.push(ActionResult {
                        capability: cap,
                        arguments: args,
                        status: ActionStatus::Rejected,
                        message: format!("DENIED - {reason}"),
                        raw_result: None,
                    });
                    break;
                }
                crate::confirmation::Confirmation::RequiresConsent { reason } => {
                    overall_ok = false;
                    actions.push(ActionResult {
                        capability: cap,
                        arguments: args,
                        status: ActionStatus::RequiresConsent,
                        message: format!("REQUIRES CONFIRMATION - {reason}"),
                        raw_result: None,
                    });
                    break;
                }
                crate::confirmation::Confirmation::AutoExecute => {}
            }

            // 3) Context-aware pre-checks (e.g., app not found).
            if let Some(pre) = Self::precheck(&intent, ctx) {
                if !pre.0 {
                    overall_ok = false;
                    actions.push(ActionResult {
                        capability: cap,
                        arguments: args,
                        status: ActionStatus::Failed,
                        message: pre.1.clone(),
                        raw_result: None,
                    });
                    // For non-critical read-only like window.list, we treat single failure as stop.
                    // For multi-launch, continue? Spec says stop or recover safely. Choose stop on failure.
                    break;
                }
            }

            // 4) Execute via intent layer with bounded retry.
            let client = intent::control_client(control_port);
            let surface_client = intent::SurfaceClient::new(surface_port);
            let policy = Self::recovery_policy_for(cap);
            let mut attempt = 0u32;
            let mut last_err: Option<String> = None;
            let mut last_value: Option<Value> = None;
            let final_outcome = loop {
                attempt += 1;
                match intent::execute_extended(&intent, &client, &surface_client) {
                    Ok(value) => {
                        last_value = Some(value);
                        break Ok(());
                    }
                    Err(e) => {
                        let kind = Self::classify_failure(&e);
                        let action = decide_recovery(&policy, attempt, kind, false);
                        match action {
                            RecoveryAction::Retry => {
                                let wait = backoff_delay(&policy, attempt);
                                // Honour the backoff unless the
                                // test environment asked us not to.
                                if std::env::var("AETHER_FAST_RETRY")
                                    .ok()
                                    .filter(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                                    .is_none()
                                {
                                    std::thread::sleep(wait);
                                }
                                last_err = Some(e);
                                continue;
                            }
                            RecoveryAction::Abort => {
                                last_err = Some(e);
                                break Err(());
                            }
                            RecoveryAction::Skip => {
                                // We never mark daemon steps as
                                // optional in this surface. Treat
                                // Skip as Abort.
                                last_err = Some(e);
                                break Err(());
                            }
                        }
                    }
                }
            };

            match final_outcome {
                Ok(()) => {
                    let value = last_value.unwrap_or(Value::Null);
                    let msg = intent::format_result(cap, &value);
                    actions.push(ActionResult {
                        capability: cap,
                        arguments: args,
                        status: ActionStatus::Success,
                        message: msg,
                        raw_result: Some(value),
                    });
                }
                Err(()) => {
                    overall_ok = false;
                    let e = last_err.unwrap_or_else(|| "unknown failure".to_string());
                    let friendly = Self::friendly_error(&e, cap);
                    actions.push(ActionResult {
                        capability: cap,
                        arguments: args,
                        status: ActionStatus::Failed,
                        message: friendly,
                        raw_result: None,
                    });
                    break;
                }
            }
        }

        let summary = Self::build_summary(&actions);
        let ok = overall_ok
            && !actions.is_empty()
            && actions.iter().all(|a| a.status == ActionStatus::Success);
        PlanResult { actions, summary, ok }
    }

    /// Per-capability recovery policy. Read-only capabilities retry
    /// up to 3 times (transient IPC faults are common on a busy
    /// system). Mutating capabilities retry at most once — the
    /// cost of a second attempt is real, and the failure is more
    /// likely to be permanent.
    fn recovery_policy_for(cap: CapabilityId) -> RecoveryPolicy {
        match cap {
            CapabilityId::WindowList
            | CapabilityId::WindowFocus
            | CapabilityId::WindowMinimize
            | CapabilityId::WindowMaximize
            | CapabilityId::WindowClose
            | CapabilityId::WindowRestore
            | CapabilityId::AppList
            | CapabilityId::AppStatus
            | CapabilityId::SystemStatus
            | CapabilityId::SystemInfo
            | CapabilityId::SystemResources
            | CapabilityId::SystemUptime
            | CapabilityId::ContextGet
            | CapabilityId::FileList
            | CapabilityId::FileSearch
            | CapabilityId::FileRead => RecoveryPolicy::transient_default(),
            _ => RecoveryPolicy {
                max_retries: 1,
                backoff_base_ms: 50,
                backoff_max_ms: 1_000,
                timeout_ms: None,
            },
        }
    }

    /// Classify a raw IPC error string into a `FailureKind` for
    /// the recovery decision. The classification is intentionally
    /// conservative: only clear transient signals retry, everything
    /// else aborts.
    fn classify_failure(err: &str) -> FailureKind {
        let lower = err.to_ascii_lowercase();
        if lower.contains("connect")
            || lower.contains("timeout")
            || lower.contains("timed out")
            || lower.contains("temporarily")
            || lower.contains("unavailable")
            || lower.contains("service unavailable")
        {
            FailureKind::Transient
        } else if lower.contains("not_found")
            || lower.contains("not found")
            || lower.contains("unknown")
            || lower.contains("denied")
            || lower.contains("rejected")
            || lower.contains("approval")
            || lower.contains("requires consent")
        {
            FailureKind::Permanent
        } else {
            FailureKind::Unknown
        }
    }

    /// Lightweight pre-check using context to fail fast with friendly message.
    /// Returns None to proceed, Some((false, msg)) to reject.
    fn precheck(intent: &Intent, ctx: &SystemContext) -> Option<(bool, String)> {
        let app = intent.arguments.get("app").and_then(|v| v.as_str()).unwrap_or_default();
        match intent.capability {
            CapabilityId::WindowFocus
            | CapabilityId::WindowMinimize
            | CapabilityId::WindowMaximize
            | CapabilityId::WindowClose
            | CapabilityId::WindowRestore => {
                if !app.is_empty() && ctx.window_for_app(app).is_none() && !ctx.is_running(app) {
                    // If no window exists but app is installed, focus/minimize could mean launch+focus? For now report not found.
                    // Allow focus to auto-create? No - spec says window not found error.
                    return Some((false, format!("WINDOW NOT FOUND - no window for '{app}'")));
                }
                None
            }
            CapabilityId::AppLaunch => {
                if !app.is_empty() && !ctx.is_installed(app) {
                    return Some((
                        false,
                        format!("APPLICATION NOT FOUND - '{app}' is not installed"),
                    ));
                }
                if ctx.is_running(app) {
                    return Some((false, format!("ALREADY RUNNING - '{app}' is already open")));
                }
                None
            }
            CapabilityId::AppClose => {
                if !app.is_empty() && !ctx.is_running(app) {
                    // Still attempt; close may fail gracefully. Let execution decide.
                    // But provide hint.
                }
                None
            }
            _ => None,
        }
    }

    fn friendly_error(raw: &str, _cap: CapabilityId) -> String {
        let lower = raw.to_ascii_lowercase();
        if lower.contains("not_found")
            || lower.contains("unknown")
            || lower.contains("no such window")
            || lower.contains("not found")
        {
            format!("NOT FOUND - {raw}")
        } else if lower.contains("already running") || lower.contains("already open") {
            format!("ALREADY RUNNING - {raw}")
        } else if lower.contains("unknown_capability") {
            format!("UNKNOWN CAPABILITY - {raw}")
        } else if lower.contains("malformed") {
            format!("INVALID REQUEST - {raw}")
        } else if lower.contains("approval_required") || lower.contains("requires confirmation") {
            format!("REQUIRES CONFIRMATION - {raw}")
        } else if lower.contains("connect")
            || lower.contains("timeout")
            || lower.contains("service unavailable")
        {
            format!("SERVICE UNAVAILABLE - {raw}")
        } else {
            format!("ACTION FAILED - {raw}")
        }
    }

    fn build_summary(actions: &[ActionResult]) -> String {
        if actions.is_empty() {
            return "NO ACTIONS PERFORMED".to_string();
        }
        if actions.len() == 1 {
            return actions[0].message.clone();
        }
        // Multi-step: join each message with newline style for UI.
        let mut lines = Vec::new();
        for a in actions {
            let cap_str = a.capability.as_str().to_ascii_uppercase();
            let status = match a.status {
                ActionStatus::Success => "✓",
                ActionStatus::Failed => "✗",
                ActionStatus::Rejected => "⛔",
                ActionStatus::RequiresConsent => "⏳",
            };
            lines.push(format!("{status} {cap_str}: {}", a.message));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SystemContext;

    fn ctx_with_apps(installed: &[&str], running: &[&str]) -> SystemContext {
        let mut ctx = SystemContext::empty();
        ctx.installed_apps = installed.iter().map(|s| s.to_string()).collect();
        ctx.running_apps = running.iter().map(|s| s.to_string()).collect();
        for id in installed {
            ctx.app_states.insert(
                id.to_string(),
                if running.contains(id) { "RUNNING".to_string() } else { "INSTALLED".to_string() },
            );
        }
        ctx
    }

    #[test]
    fn plan_is_bounded() {
        let ctx = ctx_with_apps(&["calculator", "notes"], &[]);
        let intents = match Planner::plan("open calculator and notes and calculator and notes and calculator and notes and calculator and notes and calculator", &ctx, None) {
            Some(v) => v,
            None => panic!("plan returned None"),
        };
        assert!(intents.len() <= 8);
    }

    #[test]
    fn precheck_rejects_unknown_app_launch() {
        let ctx = ctx_with_apps(&["calculator"], &[]);
        let intent = crate::intent::Intent {
            capability: CapabilityId::AppLaunch,
            arguments: serde_json::json!({ "app": "ghost" }),
        };
        let result = match Planner::precheck(&intent, &ctx) {
            Some(v) => v,
            None => panic!("precheck returned None"),
        };
        assert!(!result.0);
        assert!(result.1.contains("NOT FOUND"));
    }

    #[test]
    fn friendly_error_mapping() {
        assert!(Planner::friendly_error("unknown application 'ghost'", CapabilityId::AppLaunch)
            .contains("NOT FOUND"));
        assert!(Planner::friendly_error("already running", CapabilityId::AppLaunch)
            .contains("ALREADY RUNNING"));
    }

    // ---- Phase 2.7 bounded-recovery tests ----

    #[test]
    fn recovery_policy_for_readonly_caps_is_transient_default() {
        let p = Planner::recovery_policy_for(CapabilityId::SystemStatus);
        assert_eq!(p.max_retries, 3);
        assert_eq!(p.backoff_max_ms, 5_000);
        let p = Planner::recovery_policy_for(CapabilityId::WindowList);
        assert_eq!(p.max_retries, 3);
        let p = Planner::recovery_policy_for(CapabilityId::FileRead);
        assert_eq!(p.max_retries, 3);
    }

    #[test]
    fn recovery_policy_for_mutating_caps_is_single_retry() {
        let p = Planner::recovery_policy_for(CapabilityId::AppLaunch);
        assert_eq!(p.max_retries, 1);
        let p = Planner::recovery_policy_for(CapabilityId::FileDelete);
        assert_eq!(p.max_retries, 1);
        let p = Planner::recovery_policy_for(CapabilityId::FileWrite);
        assert_eq!(p.max_retries, 1);
    }

    #[test]
    fn classify_failure_recognises_transient_signals() {
        assert!(matches!(Planner::classify_failure("connection refused"), FailureKind::Transient));
        assert!(matches!(
            Planner::classify_failure("operation timed out after 2s"),
            FailureKind::Transient
        ));
        assert!(matches!(Planner::classify_failure("service unavailable"), FailureKind::Transient));
        assert!(matches!(
            Planner::classify_failure("temporarily unavailable, try again"),
            FailureKind::Transient
        ));
    }

    #[test]
    fn classify_failure_recognises_permanent_signals() {
        assert!(matches!(
            Planner::classify_failure("unknown application 'ghost'"),
            FailureKind::Permanent
        ));
        assert!(matches!(Planner::classify_failure("not found: file"), FailureKind::Permanent));
        assert!(matches!(Planner::classify_failure("capability denied"), FailureKind::Permanent));
        assert!(matches!(
            Planner::classify_failure("policy rejected: forbidden"),
            FailureKind::Permanent
        ));
    }

    #[test]
    fn classify_failure_defaults_to_unknown() {
        assert!(matches!(Planner::classify_failure("weird internal error"), FailureKind::Unknown));
    }

    /// End-to-end bounded-retry path: simulate an `execute_extended`
    /// that fails twice with a transient error, then succeeds. The
    /// `recovery_policy_for(readonly)` must be `transient_default`,
    /// and `decide_recovery` must report `Retry` for the first two
    /// failures and stop after success — so the call count must
    /// land at exactly 3.
    #[test]
    fn bounded_retry_exhausts_then_succeeds_against_runtime_decide_recovery() {
        let policy = Planner::recovery_policy_for(CapabilityId::SystemStatus);
        assert_eq!(policy.max_retries, 3);

        // Simulate three attempts: 1 = transient fail, 2 = transient
        // fail, 3 = success.
        let mut attempt = 0u32;
        let mut last_err = None;
        loop {
            attempt += 1;
            let outcome: Result<(), &str> =
                if attempt < 3 { Err("connection refused") } else { Ok(()) };
            match outcome {
                Ok(()) => break,
                Err(e) => {
                    let kind = Planner::classify_failure(e);
                    let action = decide_recovery(&policy, attempt, kind, false);
                    match action {
                        aether_agent_runtime::recovery::RecoveryAction::Retry => {
                            last_err = Some(e);
                            continue;
                        }
                        _ => panic!("unexpected recovery action at attempt {attempt}"),
                    }
                }
            }
        }
        assert_eq!(attempt, 3);
        assert!(last_err.is_some());
    }

    /// Permanent failure must NOT be retried by the runtime
    /// `decide_recovery`. Verify the wiring: classify + decide
    /// yields Abort on attempt 1 for a permanent signal.
    #[test]
    fn bounded_retry_does_not_retry_permanent() {
        let policy = Planner::recovery_policy_for(CapabilityId::AppLaunch);
        let err = "unknown application 'ghost'";
        let kind = Planner::classify_failure(err);
        let action = decide_recovery(&policy, 1, kind, false);
        assert_eq!(action, aether_agent_runtime::recovery::RecoveryAction::Abort);
    }
}
