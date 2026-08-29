// Aether Agent Daemon - convert a structured intent (the LLM's
// proposal, in JSON) into a typed `Action` for the runtime.
//
// This module is the only place where LLM-style JSON intent text is
// turned into runtime types. There is no `agent.execute_shell` and
// there is no path from a string action name to a raw shell command;
// every action must be a typed `ActionVariant` and is validated
// before execution.

use aether_agent_runtime::action::{
    Action, ActionVariant, ApplicationCloseParams, ApplicationLaunchParams, FileCreateParams,
    FileDeleteParams, FileListParams, FileMoveParams, FileReadParams, FileRenameParams,
    FileSearchParams, FileWriteParams, ProcessInspectParams, WindowCloseParams, WindowFocusParams,
    WindowMaximizeParams, WindowMinimizeParams,
};
use serde_json::Value;

pub fn intent_to_action(
    session_id: &str,
    capability: &str,
    args: &Value,
) -> Result<Action, String> {
    if session_id.trim().is_empty() {
        return Err("session_id is required".to_string());
    }
    let variant = match capability {
        // Application
        "application.launch" => ActionVariant::ApplicationLaunch(ApplicationLaunchParams {
            application_id: string_arg_with_alias(args, &["application_id", "app"])?,
        }),
        "application.close" => ActionVariant::ApplicationClose(ApplicationCloseParams {
            application_id: string_arg_with_alias(args, &["application_id", "app"])?,
        }),
        // Window
        "window.list" => ActionVariant::WindowList,
        "window.focus" => {
            let window_id = args.get("window_id").and_then(|v| v.as_u64());
            let application_id = args
                .get("application_id")
                .or_else(|| args.get("app"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if window_id.is_none() && application_id.is_none() {
                return Err("window.focus requires 'window_id' or 'application_id'".to_string());
            }
            ActionVariant::WindowFocus(WindowFocusParams { window_id, application_id })
        }
        "window.minimize" => ActionVariant::WindowMinimize(WindowMinimizeParams {
            window_id: args
                .get("window_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "window.minimize requires 'window_id'".to_string())?,
        }),
        "window.maximize" => ActionVariant::WindowMaximize(WindowMaximizeParams {
            window_id: args
                .get("window_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "window.maximize requires 'window_id'".to_string())?,
        }),
        "window.close" => {
            let window_id = args.get("window_id").and_then(|v| v.as_u64());
            let application_id = args
                .get("application_id")
                .or_else(|| args.get("app"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if window_id.is_none() && application_id.is_none() {
                return Err("window.close requires 'window_id' or 'application_id'".to_string());
            }
            ActionVariant::WindowClose(WindowCloseParams { window_id, application_id })
        }
        // File system
        "file.list" => ActionVariant::FileList(FileListParams {
            path: args.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        }),
        "file.read" => {
            ActionVariant::FileRead(FileReadParams { path: string_arg(args, "path", "path")? })
        }
        "file.create" => ActionVariant::FileCreate(FileCreateParams {
            path: string_arg(args, "path", "path")?,
            content: args.get("content").and_then(|v| v.as_str()).map(str::to_string),
        }),
        "file.write" => ActionVariant::FileWrite(FileWriteParams {
            path: string_arg(args, "path", "path")?,
            content: args
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "file.write requires 'content' string".to_string())?
                .to_string(),
        }),
        "file.search" => ActionVariant::FileSearch(FileSearchParams {
            query: string_arg(args, "query", "query")?,
            path: args.get("path").and_then(|v| v.as_str()).map(str::to_string),
        }),
        "file.rename" => ActionVariant::FileRename(FileRenameParams {
            from: string_arg(args, "from", "from")?,
            to: string_arg(args, "to", "to")?,
        }),
        "file.move" => ActionVariant::FileMove(FileMoveParams {
            from: string_arg(args, "from", "from")?,
            to: string_arg(args, "to", "to")?,
        }),
        "file.delete" => {
            ActionVariant::FileDelete(FileDeleteParams { path: string_arg(args, "path", "path")? })
        }
        // Process
        "process.list" => ActionVariant::ProcessList,
        "process.inspect" => {
            let pid = args.get("pid").and_then(|v| v.as_u64()).and_then(|n| u32::try_from(n).ok());
            let name = args.get("name").and_then(|v| v.as_str()).map(str::to_string);
            if pid.is_none() && name.is_none() {
                return Err("process.inspect requires 'pid' or 'name'".to_string());
            }
            ActionVariant::ProcessInspect(ProcessInspectParams { pid, name })
        }
        // Network
        "network.status" => ActionVariant::NetworkStatus,
        "network.interfaces" => ActionVariant::NetworkInterfaces,
        // System
        "system.status" => ActionVariant::SystemStatus,
        "system.info" => ActionVariant::SystemInfo,
        "system.resources" => ActionVariant::SystemResources,
        "system.uptime" => ActionVariant::SystemUptime,
        // Storage
        "storage.status" => ActionVariant::StorageStatus,
        unknown => {
            return Err(format!("unknown capability: '{unknown}' (or not exposed by this surface)"))
        }
    };
    let action = Action::new(session_id, variant, "structured intent via agent.intent");
    Ok(action)
}

fn string_arg(args: &Value, key: &str, display: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| format!("missing or non-string '{display}' argument"))
}

/// Returns the first matching string-typed argument under any of the
/// given keys. Used so capability callers can use either the canonical
/// parameter name (e.g. `application_id`) or the LLM-friendly alias
/// (e.g. `app`).
fn string_arg_with_alias(args: &Value, keys: &[&str]) -> Result<String, String> {
    for key in keys {
        if let Some(v) = args.get(*key).and_then(|v| v.as_str()) {
            return Ok(v.to_string());
        }
    }
    Err(format!("missing or non-string '{}' argument", keys.first().unwrap_or(&"argument")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_agent_runtime::action::ActionVariant;

    fn args(pairs: &[(&str, &str)]) -> Value {
        let mut map = serde_json::Map::new();
        for (k, v) in pairs {
            map.insert((*k).to_string(), Value::String((*v).to_string()));
        }
        Value::Object(map)
    }

    #[test]
    fn app_launch_with_id() {
        let a = intent_to_action("s1", "application.launch", &args(&[("application_id", "calc")]))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(a.variant, ActionVariant::ApplicationLaunch(_)));
        assert_eq!(a.action_name(), "application.launch");
    }

    #[test]
    fn app_launch_accepts_app_alias() {
        let a = intent_to_action("s1", "application.launch", &args(&[("app", "calc")]))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(a.variant, ActionVariant::ApplicationLaunch(_)));
    }

    #[test]
    fn unknown_capability_rejected() {
        let res = intent_to_action("s1", "agent.execute_shell", &Value::Null);
        match res {
            Ok(_) => panic!("expected error, got Ok"),
            Err(e) => assert!(e.contains("unknown capability"), "got: {e}"),
        }
    }

    #[test]
    fn shell_like_capability_rejected() {
        // Defense in depth: any capability name containing "shell"
        // or "exec" must be rejected at the type boundary.
        assert!(intent_to_action("s1", "shell.exec", &Value::Null).is_err());
        assert!(intent_to_action("s1", "execute_command", &Value::Null).is_err());
    }

    #[test]
    fn window_focus_requires_target() {
        let res = intent_to_action("s1", "window.focus", &Value::Null);
        match res {
            Ok(_) => panic!("expected error, got Ok"),
            Err(e) => assert!(e.contains("window_id") || e.contains("application_id"), "got: {e}"),
        }
    }

    #[test]
    fn window_minimize_requires_id() {
        let res = intent_to_action("s1", "window.minimize", &Value::Null);
        match res {
            Ok(_) => panic!("expected error, got Ok"),
            Err(e) => assert!(e.contains("window_id"), "got: {e}"),
        }
    }

    #[test]
    fn file_write_requires_content() {
        let res = intent_to_action("s1", "file.write", &args(&[("path", "/tmp/x")]));
        match res {
            Ok(_) => panic!("expected error, got Ok"),
            Err(e) => assert!(e.contains("content"), "got: {e}"),
        }
    }

    #[test]
    fn file_read_constructs_correctly() {
        let a = intent_to_action("s1", "file.read", &args(&[("path", "/tmp/x")]))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(a.variant, ActionVariant::FileRead(_)));
    }

    #[test]
    fn process_inspect_with_name() {
        let a = intent_to_action("s1", "process.inspect", &args(&[("name", "bash")]))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(a.variant, ActionVariant::ProcessInspect(_)));
    }

    #[test]
    fn process_inspect_without_target_rejected() {
        assert!(intent_to_action("s1", "process.inspect", &Value::Null).is_err());
    }

    #[test]
    fn system_status_zero_args() {
        let a =
            intent_to_action("s1", "system.status", &Value::Null).unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(a.variant, ActionVariant::SystemStatus));
    }

    #[test]
    fn network_status_zero_args() {
        let a = intent_to_action("s1", "network.status", &Value::Null)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(a.variant, ActionVariant::NetworkStatus));
    }

    #[test]
    fn storage_status_zero_args() {
        let a = intent_to_action("s1", "storage.status", &Value::Null)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(a.variant, ActionVariant::StorageStatus));
    }

    #[test]
    fn empty_session_id_rejected() {
        assert!(intent_to_action("", "system.status", &Value::Null).is_err());
    }

    #[test]
    fn file_list_defaults_to_root() {
        let a = intent_to_action("s1", "file.list", &Value::Null).unwrap_or_else(|e| panic!("{e}"));
        if let ActionVariant::FileList(p) = &a.variant {
            assert_eq!(p.path, "");
        } else {
            panic!("wrong variant");
        }
    }
}
