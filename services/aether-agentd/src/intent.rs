// Aether Agent - intent parsing and capability policy.
//
// Translates user text into a STRUCTURED intent, validates it against the
// capability policy (reuse of aether-core Capability/RiskLevel), and hands
// approved intents to the control plane. The AI provider never sees raw
// system access; it can only trigger capabilities defined here.

use aether_core::capability::{Capability, CapabilityDomain, RiskLevel};
use serde_json::Value;
use std::time::Duration;

/// Capabilities this phase exposes. Read-only ones execute directly;
/// state-changing ones are marked for future approval flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityId {
    SystemStatus,
    AppStatus,
    AppList,
    AppLaunch,
    AppClose,
}

impl CapabilityId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SystemStatus => "system.status",
            Self::AppStatus => "app.status",
            Self::AppList => "app.list",
            Self::AppLaunch => "app.launch",
            Self::AppClose => "app.close",
        }
    }

    /// The aether-core capability this id maps to (domain + risk).
    pub fn capability(&self) -> Capability {
        match self {
            Self::SystemStatus => {
                Capability::new(CapabilityDomain::System, "status", RiskLevel::Low)
            }
            Self::AppStatus => {
                Capability::new(CapabilityDomain::Application, "status", RiskLevel::Low)
            }
            Self::AppList => {
                Capability::new(CapabilityDomain::Application, "list", RiskLevel::Low)
            }
            Self::AppLaunch => {
                Capability::new(CapabilityDomain::Application, "launch", RiskLevel::Medium)
            }
            Self::AppClose => {
                Capability::new(CapabilityDomain::Application, "close", RiskLevel::Medium)
            }
        }
    }

    /// Whether this phase executes without an approval dialog.
    pub fn auto_execute(&self) -> bool {
        // Launch/close operate on registered apps only and stay inside the
        // application manager; approval hooks arrive in a later phase.
        true
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(raw: &str) -> Option<Self> {
        match raw {
            "system.status" => Some(Self::SystemStatus),
            "app.status" => Some(Self::AppStatus),
            "app.list" => Some(Self::AppList),
            "app.launch" => Some(Self::AppLaunch),
            "app.close" => Some(Self::AppClose),
            _ => None,
        }
    }
}

/// A structured intent extracted from user text.
#[derive(Debug, Clone, PartialEq)]
pub struct Intent {
    pub capability: CapabilityId,
    /// Arguments per capability: app.launch/app.close require their target.
    pub arguments: Value,
}

/// Why an intent was rejected by validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection(pub String);

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Keyword-based intent classifier (deterministic stand-in for model-based
/// intent detection; the capability layer below is unchanged either way).
pub fn parse_intent(text: &str) -> Option<Intent> {
    let upper = text.to_uppercase();
    let words: Vec<&str> = upper.split_whitespace().collect();

    // app.close: CLOSE <target>
    for (idx, verb) in words.iter().enumerate() {
        if *verb == "CLOSE" {
            let target = words[idx + 1..]
                .iter()
                .find(|w| !matches!(**w, "THE" | "MY"))
                .map(|w| {
                    w.trim_end_matches(|c: char| !c.is_ascii_alphanumeric())
                        .to_ascii_lowercase()
                })
                .and_then(|w| w.non_empty());
            return Some(Intent {
                capability: CapabilityId::AppClose,
                arguments: serde_json::json!({ "app": target }),
            });
        }
    }

    // app.launch: OPEN/LAUNCH/START <target>
    for (idx, verb) in words.iter().enumerate() {
        if matches!(*verb, "OPEN" | "LAUNCH" | "START") {
            // Skip filler words like THE/MY; normalize to registry id casing.
            let target = words[idx + 1..]
                .iter()
                .find(|w| !matches!(**w, "THE" | "MY"))
                .map(|w| {
                    w.trim_end_matches(|c: char| !c.is_ascii_alphanumeric())
                        .to_ascii_lowercase()
                })
                .and_then(|w| w.non_empty());
            return Some(Intent {
                capability: CapabilityId::AppLaunch,
                arguments: serde_json::json!({ "app": target }),
            });
        }
    }

    // app.list
    if words.contains(&"APPS") || upper.contains("APPLICATIONS") {
        return Some(Intent {
            capability: CapabilityId::AppList,
            arguments: serde_json::json!({}),
        });
    }

    // app.status: "<APP> RUNNING?" / "IS <APP> RUNNING"
    if upper.contains("RUNNING") {
        // Walk backwards from the word RUNNING to the app name.
        let running_idx = words.iter().position(|w| w.contains("RUNNING"));
        let target = running_idx.and_then(|ri| {
            words[..ri]
                .iter()
                .rev()
                .find(|w| !matches!(**w, "IS" | "THE" | "MY" | "STILL"))
                .map(|w| {
                    w.trim_end_matches(|c: char| !c.is_ascii_alphanumeric())
                        .to_ascii_lowercase()
                })
                .and_then(|w| w.non_empty())
        });
        return Some(Intent {
            capability: CapabilityId::AppStatus,
            arguments: serde_json::json!({ "app": target }),
        });
    }

    // system.status
    if upper.contains("STATUS") || upper.contains("HEALTH") {
        return Some(Intent {
            capability: CapabilityId::SystemStatus,
            arguments: serde_json::json!({}),
        });
    }

    None
}

trait NonEmptyOption {
    fn non_empty(self) -> Option<String>;
}
impl NonEmptyOption for String {
    fn non_empty(self) -> Option<String> {
        if self.is_empty() { None } else { Some(self) }
    }
}

/// Validates an intent against the capability policy. Unknown capabilities
/// and malformed arguments are rejected here, before anything executes.
pub fn validate(intent: &Intent) -> Result<(), Rejection> {
    // Unknown capability names can only arrive via forged requests; the
    // enum makes them unrepresentable, but keep an explicit guard anyway.
    if CapabilityId::from_str(intent.capability.as_str()).is_none() {
        return Err(Rejection("UNKNOWN_CAPABILITY".to_string()));
    }

    let needs_target = matches!(
        intent.capability,
        CapabilityId::AppLaunch | CapabilityId::AppClose | CapabilityId::AppStatus
    );

    if needs_target {
        let app_value = intent.arguments.get("app").cloned().unwrap_or(Value::Null);
        let ok = match app_value {
            Value::String(s) => {
                !s.trim().is_empty()
                    && s.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_')
                    })
            }
            _ => false,
        };
        if !ok {
            return Err(Rejection(
                "MALFORMED_ARGUMENTS: 'app' must be a registered app id".to_string(),
            ));
        }
    }

    if !intent.capability.auto_execute() {
        return Err(Rejection("APPROVAL_REQUIRED".to_string()));
    }
    Ok(())
}

/// Executes a validated intent against the control plane via the SDK client.
pub fn execute(intent: &Intent, client: &aether_sdk::AetherClient) -> Result<Value, String> {
    let response = match intent.capability {
        CapabilityId::SystemStatus => client.status()?,
        CapabilityId::AppStatus => client.request(&aether_sdk::IpcRequest {
            service_id: "aether-system-core".to_string(),
            command: "app.status".to_string(),
            parameters: serde_json::json!({ "app": intent.arguments["app"] }),
        })?,
        CapabilityId::AppList => client.request(&aether_sdk::IpcRequest {
            service_id: "aether-system-core".to_string(),
            command: "app.list".to_string(),
            parameters: serde_json::json!({}),
        })?,
        CapabilityId::AppLaunch => client.request(&aether_sdk::IpcRequest {
            service_id: "aether-system-core".to_string(),
            command: "app.launch".to_string(),
            parameters: serde_json::json!({ "app": intent.arguments["app"] }),
        })?,
        CapabilityId::AppClose => client.request(&aether_sdk::IpcRequest {
            service_id: "aether-system-core".to_string(),
            command: "app.close".to_string(),
            parameters: serde_json::json!({ "app": intent.arguments["app"] }),
        })?,
    };

    if response.ok {
        Ok(response.result)
    } else {
        Err(response
            .error
            .map(|e| format!("{}: {}", e.code, e.message))
            .unwrap_or_else(|| "unknown error".to_string()))
    }
}

/// Client used by the agent's capability executor.
pub fn control_client(port: u16) -> aether_sdk::AetherClient {
    aether_sdk::AetherClient::new(format!("127.0.0.1:{port}"), Duration::from_secs(5))
}

/// Human-readable (uppercase-rendered later) summary of a capability result.
pub fn format_result(capability: CapabilityId, result: &Value) -> String {
    match capability {
        CapabilityId::SystemStatus => {
            let health = result["overall_health"].as_str().unwrap_or("UNKNOWN");
            let count = result["services"].as_array().map(|a| a.len()).unwrap_or(0);
            format!("SYSTEM {health} - {count} SERVICES REGISTERED")
        }
        CapabilityId::AppList => {
            let names: Vec<String> = result["apps"]
                .as_array()
                .map(|apps| {
                    apps.iter()
                        .map(|a| a["id"].as_str().unwrap_or("?").to_uppercase())
                        .collect()
                })
                .unwrap_or_default();
            if names.is_empty() {
                "NO APPLICATIONS INSTALLED".to_string()
            } else {
                format!("INSTALLED APPS: {}", names.join(", "))
            }
        }
        CapabilityId::AppLaunch => {
            let app = result["app"].as_str().unwrap_or("APP").to_uppercase();
            let pid = result["instance"]["pid"].as_u64();
            match pid {
                Some(p) => format!("LAUNCHED {app} (PID {p})"),
                None => format!("LAUNCHED {app}"),
            }
        }
        CapabilityId::AppClose => "APPLICATION CLOSED".to_string(),
        CapabilityId::AppStatus => {
            let app = result["report"]["app"].as_str().unwrap_or("APP").to_uppercase();
            let state = result["report"]["state"].as_str().unwrap_or("UNKNOWN");
            format!("{app} STATE: {state}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_sentence_maps_to_app_launch() {
        let intent = parse_intent("Open the calculator.")
            .unwrap_or_else(|| panic!("expected launch intent"));
        assert_eq!(intent.capability, CapabilityId::AppLaunch);
        assert_eq!(intent.arguments["app"], "calculator");
    }

    #[test]
    fn apps_list_sentence_maps_to_app_list() {
        let intent = parse_intent("Show me what apps are installed.")
            .unwrap_or_else(|| panic!("expected list intent"));
        assert_eq!(intent.capability, CapabilityId::AppList);
    }

    #[test]
    fn status_sentence_maps_to_system_status() {
        let intent = parse_intent("what is the system health?")
            .unwrap_or_else(|| panic!("expected status intent"));
        assert_eq!(intent.capability, CapabilityId::SystemStatus);
    }

    #[test]
    fn plain_chat_has_no_intent() {
        assert!(parse_intent("tell me a joke").is_none());
        assert!(parse_intent("").is_none());
    }

    #[test]
    fn validate_rejects_malformed_launch_arguments() {
        let intent = Intent {
            capability: CapabilityId::AppLaunch,
            arguments: serde_json::json!({}),
        };
        assert_eq!(
            validate(&intent),
            Err(Rejection(
                "MALFORMED_ARGUMENTS: 'app' must be a registered app id".to_string()
            ))
        );
    }

    #[test]
    fn validate_accepts_wellformed_launch() {
        let intent =
            parse_intent("open calculator").unwrap_or_else(|| panic!("expected intent"));
        assert!(validate(&intent).is_ok());
    }

    #[test]
    fn close_sentence_maps_to_app_close() {
        let intent =
            parse_intent("Close Calculator.").unwrap_or_else(|| panic!("expected close intent"));
        assert_eq!(intent.capability, CapabilityId::AppClose);
        assert_eq!(intent.arguments["app"], "calculator");
    }

    #[test]
    fn running_question_maps_to_app_status() {
        let intent = parse_intent("Is Calculator running?")
            .unwrap_or_else(|| panic!("expected status intent"));
        assert_eq!(intent.capability, CapabilityId::AppStatus);
        assert_eq!(intent.arguments["app"], "calculator");
        assert!(validate(&intent).is_ok());
    }

    #[test]
    fn unknown_capability_names_are_unrepresentable_but_guarded() {
        assert!(CapabilityId::from_str("shell.exec").is_none());
    }
}
