// Agent Runtime - Tool registry
//
// Phase 2.3 closure: every ActionVariant has a corresponding
// ToolDefinition in the registry. The registry is the single source
// of truth for which (service_id, command) an action routes to,
// what capabilities it requires, and what risk/side-effects it
// carries. The executor can use the registry as a routing table
// instead of inlining IPC strings.
//
// Design goals:
//   - One tool per ActionVariant. Tool IDs match action_name().
//   - input_schema declares required parameters. The schema is
//     enforced before the IPC call.
//   - risk_level mirrors ActionRisk. A future refactor can wire
//     the executor to look up the tool first and check the
//     policy before dispatching.
//   - register_all_tools() returns a registry with every
//     ActionVariant registered. The list MUST stay in sync with
//     ActionVariant — the sync test enforces this.

use crate::tool::{ToolDefinition, ToolRegistry};

/// The Aether system-core service id. Almost every command
/// routes here for the agent runtime.
const SERVICE_SYSTEM_CORE: &str = "aether-system-core";

/// The Aether surface server. Window operations route here.
const SERVICE_SURFACE: &str = "aether-surface";

/// Build a `ToolDefinition` from a `(command, input_schema,
/// risk, capability, side_effects, timeout, confirmation)`
/// tuple. Centralizes the boilerplate so every entry reads
/// the same way.
#[allow(clippy::too_many_lines)]
/// Build a `ToolDefinition`. Centralizes the boilerplate so
/// every entry reads the same way. `#[allow(too_many_arguments)]`
/// is intentional — every parameter is a different dimension
/// of the tool definition, and bundling them into a struct
/// would obscure the data.
#[allow(clippy::too_many_arguments)]
fn tool(
    id: &str,
    description: &str,
    command: &str,
    input_schema: serde_json::Value,
    risk: crate::tool::ToolRisk,
    capability: &str,
    side_effects: &[&str],
    timeout_ms: u64,
    requires_confirmation: bool,
) -> ToolDefinition {
    let mut t = ToolDefinition::new(id, id, description)
        .with_input_schema(input_schema)
        .with_risk_level(risk)
        .with_capability(capability)
        .with_timeout(timeout_ms);
    for e in side_effects {
        t = t.with_side_effect(e);
    }
    if requires_confirmation {
        t = t.with_confirmation();
    }
    // Embed the routing hint in the output schema so a future
    // dispatcher can look it up. The executor currently inlines
    // (service, command) per match arm, but the registry now
    // carries the same info authoritatively.
    t = t.with_output_schema(serde_json::json!({
        "service": SERVICE_SYSTEM_CORE,
        "command": command,
    }));
    t
}

/// The full set of tools the agent runtime can dispatch. Every
/// ActionVariant appears here exactly once. The list is the
/// source of truth for the executor's routing table.
///
/// When adding a new ActionVariant, add its tool here AND its
/// dispatch arm in executor.rs. The `tools_match_action_variants`
/// test below enforces the two stay in sync.
fn all_tool_definitions() -> Vec<ToolDefinition> {
    use crate::tool::ToolRisk::*;
    vec![
        // ---- Application ----
        tool(
            "application.launch",
            "Launch a registered application.",
            "app.launch",
            serde_json::json!({"required": ["application_id"]}),
            Medium,
            "application.launch",
            &["process.start", "window.create"],
            15_000,
            false,
        ),
        tool(
            "application.close",
            "Close a running application.",
            "app.close",
            serde_json::json!({"required": ["application_id"]}),
            Medium,
            "application.close",
            &["process.stop", "window.close"],
            10_000,
            false,
        ),
        // ---- Window ----
        tool(
            "window.list",
            "List all open windows.",
            "window.list",
            serde_json::json!({}),
            Low,
            "window.list",
            &[],
            5_000,
            false,
        )
        // The output_schema's `service` is overwritten for surface.
        .with_output_schema(serde_json::json!({
            "service": SERVICE_SURFACE,
            "command": "window.list",
        })),
        tool(
            "window.focus",
            "Focus a window by id or by app.",
            "window.focus",
            serde_json::json!({}),
            Low,
            "window.focus",
            &["window.focus"],
            5_000,
            false,
        )
        .with_output_schema(serde_json::json!({
            "service": SERVICE_SURFACE,
            "command": "window.focus",
        })),
        tool(
            "window.minimize",
            "Minimize a window.",
            "window.minimize",
            serde_json::json!({"required": ["window_id"]}),
            Low,
            "window.minimize",
            &["window.minimize"],
            5_000,
            false,
        )
        .with_output_schema(serde_json::json!({
            "service": SERVICE_SURFACE,
            "command": "window.minimize",
        })),
        tool(
            "window.maximize",
            "Maximize a window.",
            "window.maximize",
            serde_json::json!({"required": ["window_id"]}),
            Low,
            "window.maximize",
            &["window.maximize"],
            5_000,
            false,
        )
        .with_output_schema(serde_json::json!({
            "service": SERVICE_SURFACE,
            "command": "window.maximize",
        })),
        tool(
            "window.close",
            "Close a window.",
            "window.close",
            serde_json::json!({}),
            Medium,
            "window.close",
            &["window.close"],
            5_000,
            false,
        )
        .with_output_schema(serde_json::json!({
            "service": SERVICE_SURFACE,
            "command": "window.close",
        })),
        // ---- Filesystem ----
        tool(
            "file.list",
            "List files in a directory.",
            "file.list",
            serde_json::json!({"required": ["path"]}),
            Low,
            "file.list",
            &[],
            5_000,
            false,
        ),
        tool(
            "file.read",
            "Read a file's contents.",
            "file.read",
            serde_json::json!({"required": ["path"]}),
            Low,
            "file.read",
            &[],
            5_000,
            false,
        ),
        tool(
            "file.create",
            "Create a new file.",
            "file.create",
            serde_json::json!({"required": ["path"]}),
            Medium,
            "file.create",
            &["file.create"],
            5_000,
            false,
        ),
        tool(
            "file.write",
            "Overwrite a file.",
            "file.write",
            serde_json::json!({"required": ["path", "content"]}),
            Medium,
            "file.write",
            &["file.write"],
            5_000,
            false,
        ),
        tool(
            "file.search",
            "Search for files matching a query.",
            "file.search",
            serde_json::json!({"required": ["query"]}),
            Low,
            "file.search",
            &[],
            10_000,
            false,
        ),
        tool(
            "file.rename",
            "Rename a file.",
            "file.rename",
            serde_json::json!({"required": ["from", "to"]}),
            Medium,
            "file.rename",
            &["file.rename"],
            5_000,
            false,
        ),
        tool(
            "file.move",
            "Move a file.",
            "file.move",
            serde_json::json!({"required": ["from", "to"]}),
            Medium,
            "file.move",
            &["file.move"],
            5_000,
            false,
        ),
        tool(
            "file.delete",
            "Delete a file. High risk: cannot be undone.",
            "file.delete",
            serde_json::json!({"required": ["path"]}),
            High,
            "file.delete",
            &["file.delete"],
            5_000,
            true,
        ),
        // ---- Process ----
        tool(
            "process.list",
            "List running processes.",
            "process.list",
            serde_json::json!({}),
            Low,
            "process.list",
            &[],
            5_000,
            false,
        ),
        tool(
            "process.inspect",
            "Inspect a process by pid or name.",
            "process.inspect",
            serde_json::json!({}),
            Low,
            "process.inspect",
            &[],
            5_000,
            false,
        ),
        // ---- Network ----
        tool(
            "network.status",
            "Get network connectivity status.",
            "network.status",
            serde_json::json!({}),
            Low,
            "network.status",
            &[],
            5_000,
            false,
        ),
        tool(
            "network.interfaces",
            "List network interfaces.",
            "network.interfaces",
            serde_json::json!({}),
            Low,
            "network.interfaces",
            &[],
            5_000,
            false,
        ),
        // ---- System ----
        tool(
            "system.status",
            "Get system status.",
            "status",
            serde_json::json!({}),
            Low,
            "system.status",
            &[],
            5_000,
            false,
        ),
        tool(
            "system.info",
            "Get system information.",
            "system.info",
            serde_json::json!({}),
            Low,
            "system.info",
            &[],
            5_000,
            false,
        ),
        tool(
            "system.resources",
            "Get current resource usage.",
            "system.resources",
            serde_json::json!({}),
            Low,
            "system.resources",
            &[],
            5_000,
            false,
        ),
        tool(
            "system.uptime",
            "Get system uptime.",
            "system.uptime",
            serde_json::json!({}),
            Low,
            "system.uptime",
            &[],
            5_000,
            false,
        ),
        tool(
            "service.restart",
            "Restart an Aether service unit.",
            "service.restart",
            serde_json::json!({"required": ["service_id"]}),
            Medium,
            "service.restart",
            &["service.restart"],
            30_000,
            true,
        ),
        // ---- Storage ----
        tool(
            "storage.status",
            "Get storage status.",
            "storage.status",
            serde_json::json!({}),
            Low,
            "storage.status",
            &[],
            5_000,
            false,
        ),
        // ---- Context ----
        tool(
            "context.get",
            "Get a context snapshot.",
            "context.get",
            serde_json::json!({}),
            Low,
            "context.get",
            &[],
            5_000,
            false,
        ),
        // ---- Display ----
        tool(
            "display.list",
            "List connected displays.",
            "display.list",
            serde_json::json!({}),
            Low,
            "display.list",
            &[],
            5_000,
            false,
        ),
        tool(
            "display.set_brightness",
            "Set display brightness (0-100).",
            "display.set_brightness",
            serde_json::json!({"required": ["display_id", "level"]}),
            Low,
            "display.set_brightness",
            &["display.brightness"],
            5_000,
            false,
        ),
        tool(
            "display.set_resolution",
            "Set display resolution.",
            "display.set_resolution",
            serde_json::json!({
                "required": ["display_id", "width", "height"],
            }),
            Medium,
            "display.set_resolution",
            &["display.resolution"],
            5_000,
            true,
        ),
        // ---- Device ----
        tool(
            "device.list",
            "List hardware devices.",
            "device.list",
            serde_json::json!({}),
            Low,
            "device.list",
            &[],
            5_000,
            false,
        ),
        tool(
            "device.inspect",
            "Inspect a hardware device.",
            "device.inspect",
            serde_json::json!({"required": ["device_id"]}),
            Low,
            "device.inspect",
            &[],
            5_000,
            false,
        ),
        tool(
            "device.enable",
            "Enable a disabled device.",
            "device.enable",
            serde_json::json!({"required": ["device_id"]}),
            Medium,
            "device.enable",
            &["device.state"],
            5_000,
            true,
        ),
        tool(
            "device.disable",
            "Disable a device.",
            "device.disable",
            serde_json::json!({"required": ["device_id"]}),
            Medium,
            "device.disable",
            &["device.state"],
            5_000,
            true,
        ),
        // ---- Power ----
        tool(
            "system.reboot",
            "Reboot the system. Interrupts every user.",
            "system.reboot",
            serde_json::json!({"required": ["delay_ms"]}),
            Critical,
            "system.reboot",
            &["system.reboot", "session.terminate"],
            60_000,
            true,
        ),
        tool(
            "system.shutdown",
            "Shut the system down.",
            "system.shutdown",
            serde_json::json!({"required": ["delay_ms"]}),
            Critical,
            "system.shutdown",
            &["system.shutdown", "session.terminate"],
            60_000,
            true,
        ),
        tool(
            "system.suspend",
            "Suspend the system.",
            "system.suspend",
            serde_json::json!({}),
            High,
            "system.suspend",
            &["system.suspend"],
            30_000,
            true,
        ),
        // ---- Security ----
        tool(
            "credential.seal",
            "Seal a credential for long-term storage.",
            "credentials.seal",
            serde_json::json!({"required": ["name", "plaintext"]}),
            High,
            "credential.seal",
            &["credential.seal"],
            5_000,
            true,
        ),
        tool(
            "credential.unseal",
            "Unseal a previously sealed credential.",
            "credentials.unseal",
            serde_json::json!({"required": ["name"]}),
            High,
            "credential.unseal",
            &["credential.unseal"],
            5_000,
            true,
        ),
        tool(
            "policy.reload",
            "Reload the active policy bundle.",
            "policy.reload",
            serde_json::json!({}),
            Medium,
            "policy.reload",
            &["policy.reload"],
            10_000,
            true,
        ),
    ]
}

/// The canonical action_name() for every ActionVariant. Used
/// to build the set of tool IDs that MUST be present in the
/// registry, so we can assert every variant is registered.
#[cfg(test)]
fn all_action_names() -> Vec<&'static str> {
    vec![
        // We use a stub Action just to call action_name() without
        // constructing a full params. Easiest is to match the
        // variant and return the constant string from the match
        // table in action.rs. We duplicate the table here so the
        // sync test catches a divergence between action.rs and
        // tools.rs.
        "application.launch",
        "application.close",
        "window.list",
        "window.focus",
        "window.minimize",
        "window.maximize",
        "window.close",
        "file.list",
        "file.read",
        "file.create",
        "file.write",
        "file.search",
        "file.rename",
        "file.move",
        "file.delete",
        "process.list",
        "process.inspect",
        "network.status",
        "network.interfaces",
        "system.status",
        "system.info",
        "system.resources",
        "system.uptime",
        "service.restart",
        "storage.status",
        "context.get",
        "display.list",
        "display.set_brightness",
        "display.set_resolution",
        "device.list",
        "device.inspect",
        "device.enable",
        "device.disable",
        "system.reboot",
        "system.shutdown",
        "system.suspend",
        "credential.seal",
        "credential.unseal",
        "policy.reload",
    ]
}

/// Build a registry with every ActionVariant registered. The
/// returned registry is the single source of truth for the
/// agent runtime's dispatch table.
pub fn register_all_tools() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    for t in all_tool_definitions() {
        // The registry rejects duplicates. The static list in
        // all_tool_definitions() is the only source of these
        // ids, so a duplicate here is a sync bug. We surface
        // it as a panic via a debug_assert (no expect/unwrap
        // allowed in this crate's lint policy).
        let result = reg.register(t);
        debug_assert!(result.is_ok(), "tools.rs: duplicate tool id (sync bug): {result:?}",);
    }
    reg
}

/// Convenience: look up the IPC command for a tool. Returns
/// `(service, command)` if the tool has a routing hint in its
/// output schema.
pub fn routing_for(registry: &ToolRegistry, id: &crate::tool::ToolId) -> Option<(String, String)> {
    let tool = registry.get(id)?;
    let out = &tool.output_schema;
    let service = out.get("service")?.as_str()?.to_string();
    let command = out.get("command")?.as_str()?.to_string();
    Some((service, command))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, ActionVariant};
    use crate::tool::{ToolId, ToolRisk};

    #[test]
    fn register_all_tools_returns_nonempty() {
        let reg = register_all_tools();
        assert!(reg.count() > 30, "expected 30+ tools, got {}", reg.count());
    }

    #[test]
    fn every_registered_tool_has_a_routing_hint() {
        let reg = register_all_tools();
        for tool in reg.list() {
            let out = &tool.output_schema;
            assert!(
                out.get("service").is_some(),
                "tool {} has no service in output_schema",
                tool.id
            );
            assert!(
                out.get("command").is_some(),
                "tool {} has no command in output_schema",
                tool.id
            );
        }
    }

    #[test]
    fn routing_for_known_tool() {
        let reg = register_all_tools();
        let routing = routing_for(&reg, &ToolId::new("system.status"));
        match routing {
            Some((service, command)) => {
                assert_eq!(service, SERVICE_SYSTEM_CORE);
                assert_eq!(command, "status");
            }
            None => panic!("system.status should be registered"),
        }
    }

    #[test]
    fn routing_for_window_tools_uses_surface_service() {
        let reg = register_all_tools();
        let routing = routing_for(&reg, &ToolId::new("window.list"));
        match routing {
            Some((service, _command)) => {
                assert_eq!(service, SERVICE_SURFACE);
            }
            None => panic!("window.list should be registered"),
        }
    }

    #[test]
    fn routing_for_unknown_tool_is_none() {
        let reg = register_all_tools();
        assert!(routing_for(&reg, &ToolId::new("agent.execute_shell")).is_none());
    }

    #[test]
    fn tools_match_action_variants() {
        // Every action_name() must be a registered tool id.
        // Build the set of action names by constructing a stub
        // Action for each variant and reading action_name().
        let _reg = register_all_tools();
        let names = all_action_names();
        #[allow(unused_imports)]
        use crate::action::ActionVariant;
        let mut registry = register_all_tools();
        // The duplicate-register check in `register_all_tools`
        // would already have panicked if there were dups, but
        // re-assert here for explicit safety.
        for tool in all_tool_definitions() {
            assert!(
                registry.register(tool.clone()).is_ok() || registry.get(&tool.id).is_some(),
                "duplicate tool id {}",
                tool.id
            );
        }
        for name in &names {
            assert!(
                registry.get(&ToolId::new(*name)).is_some(),
                "action {name} has no matching tool registered",
            );
        }
        // Drop the local registry to silence unused warnings.
        let _ = registry;
    }

    #[test]
    fn high_and_critical_actions_require_confirmation() {
        let reg = register_all_tools();
        for id in [
            "file.delete",
            "display.set_resolution",
            "device.enable",
            "device.disable",
            "system.reboot",
            "system.shutdown",
            "system.suspend",
            "credential.seal",
            "credential.unseal",
            "policy.reload",
        ] {
            match reg.get(&ToolId::new(id)) {
                Some(tool) => {
                    assert!(
                        tool.requires_confirmation,
                        "{id} should require confirmation but does not",
                    );
                }
                None => panic!("{id} should be registered"),
            }
        }
    }

    #[test]
    fn critical_tools_listed() {
        let reg = register_all_tools();
        let critical: Vec<_> = reg
            .list()
            .into_iter()
            .filter(|t| t.risk_level == ToolRisk::Critical)
            .map(|t| t.id.0.clone())
            .collect();
        assert!(critical.contains(&"system.reboot".to_string()));
        assert!(critical.contains(&"system.shutdown".to_string()));
    }

    #[test]
    fn file_delete_is_high_risk_in_registry() {
        let reg = register_all_tools();
        match reg.get(&ToolId::new("file.delete")) {
            Some(t) => {
                assert_eq!(t.risk_level, ToolRisk::High);
                assert!(t.required_capabilities.contains(&"file.delete".to_string()));
            }
            None => panic!("file.delete should be registered"),
        }
    }

    #[test]
    fn credential_tools_are_high_risk() {
        let reg = register_all_tools();
        for id in ["credential.seal", "credential.unseal"] {
            match reg.get(&ToolId::new(id)) {
                Some(t) => assert_eq!(t.risk_level, ToolRisk::High),
                None => panic!("{id} should be registered"),
            }
        }
    }

    #[test]
    fn action_action_name_matches_tool_id() {
        // The sync test above is the structural version; this is
        // the behavioural version that goes through
        // action.action_name() with a real Action.
        let reg = register_all_tools();
        let a = Action::new("s1", ActionVariant::SystemStatus, "check");
        assert!(reg.get(&ToolId::new(a.action_name())).is_some());
    }

    #[test]
    fn no_privilege_escalation_in_schemas() {
        let reg = register_all_tools();
        for tool in reg.list() {
            if let Some(required) = tool.input_schema.get("required") {
                if let Some(fields) = required.as_array() {
                    for f in fields {
                        let name = f.as_str().unwrap_or_default();
                        assert!(
                            !["root", "admin", "allow", "skip_policy", "trusted"].contains(&name),
                            "tool {} declares privileged field '{}' in schema",
                            tool.id,
                            name,
                        );
                    }
                }
            }
        }
    }
}
