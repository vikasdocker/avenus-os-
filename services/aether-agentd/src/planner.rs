// Action Planner - deterministic sequential execution of safe capabilities.
//
// Takes a natural-language prompt, asks the Intent engine for all intents
// (including multi-step), validates each via the capability policy, checks
// confirmation requirements, executes in order, and returns structured
// per-step feedback. Stops on first failure unless the step is non-critical.

use crate::confirmation::ConfirmationPolicy;
use crate::context::SystemContext;
use crate::intent::{self, CapabilityId, Intent, Rejection};
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
    pub fn plan(text: &str, ctx: &SystemContext, convo_last_app: Option<&str>) -> Option<Vec<Intent>> {
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
        let mut intents = intent::parse_intents_with_file(text, ctx, convo_last_app, convo_last_file);
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

            // 4) Execute via intent layer with both ports.
            let client = intent::control_client(control_port);
            let surface_client = intent::SurfaceClient::new(surface_port);
            let result = intent::execute_extended(&intent, &client, &surface_client);

            match result {
                Ok(value) => {
                    let msg = intent::format_result(cap, &value);
                    actions.push(ActionResult {
                        capability: cap,
                        arguments: args,
                        status: ActionStatus::Success,
                        message: msg,
                        raw_result: Some(value),
                    });
                }
                Err(e) => {
                    overall_ok = false;
                    // Map known errors to user-friendly messages.
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
        let ok = overall_ok && !actions.is_empty() && actions.iter().all(|a| a.status == ActionStatus::Success);
        PlanResult { actions, summary, ok }
    }

    /// Lightweight pre-check using context to fail fast with friendly message.
    /// Returns None to proceed, Some((false, msg)) to reject.
    fn precheck(intent: &Intent, ctx: &SystemContext) -> Option<(bool, String)> {
        let app = intent.arguments.get("app").and_then(|v| v.as_str()).unwrap_or_default();
        match intent.capability {
            CapabilityId::WindowFocus | CapabilityId::WindowMinimize | CapabilityId::WindowMaximize | CapabilityId::WindowClose | CapabilityId::WindowRestore => {
                if !app.is_empty() && ctx.window_for_app(app).is_none() && !ctx.is_running(app) {
                    // If no window exists but app is installed, focus/minimize could mean launch+focus? For now report not found.
                    // Allow focus to auto-create? No - spec says window not found error.
                    return Some((false, format!("WINDOW NOT FOUND - no window for '{app}'")));
                }
                None
            }
            CapabilityId::AppLaunch => {
                if !app.is_empty() && !ctx.is_installed(app) {
                    return Some((false, format!("APPLICATION NOT FOUND - '{app}' is not installed")));
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
        if lower.contains("not_found") || lower.contains("unknown") || lower.contains("no such window") || lower.contains("not found") {
            format!("NOT FOUND - {raw}")
        } else if lower.contains("already running") || lower.contains("already open") {
            format!("ALREADY RUNNING - {raw}")
        } else if lower.contains("unknown_capability") {
            format!("UNKNOWN CAPABILITY - {raw}")
        } else if lower.contains("malformed") {
            format!("INVALID REQUEST - {raw}")
        } else if lower.contains("approval_required") || lower.contains("requires confirmation") {
            format!("REQUIRES CONFIRMATION - {raw}")
        } else if lower.contains("connect") || lower.contains("timeout") || lower.contains("service unavailable") {
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
            ctx.app_states.insert(id.to_string(), if running.contains(id) { "RUNNING".to_string() } else { "INSTALLED".to_string() });
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
        assert!(Planner::friendly_error("unknown application 'ghost'", CapabilityId::AppLaunch).contains("NOT FOUND"));
        assert!(Planner::friendly_error("already running", CapabilityId::AppLaunch).contains("ALREADY RUNNING"));
    }
}
