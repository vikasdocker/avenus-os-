// Typed IPC commands for app-store interactions.
//
// The Aether Store speaks the same newline-delimited JSON
// IPC protocol as the rest of the control plane (see
// `aether_core::ipc`). App authors that want to install,
// launch, or uninstall their package programmatically —
// e.g. from a CI pipeline, a developer-mode CLI, or a
// self-update helper — can use the builders here instead
// of hand-writing `serde_json::Value` payloads.
//
// The wire format is:
//   * `service_id` is fixed to `aether-store` for all
//     three commands.
//   * `command` is one of `install`, `launch`, `uninstall`.
//   * `parameters` is a `serde_json::Value` matching the
//     store's expected schema for that command.
//   * `actor_trust` is `Trusted` by default — set it
//     explicitly to `Untrusted` for tests that want to
//     confirm the dispatcher rejects store commands
//     from hostile prompts.

use aether_core::ipc::{ActorTrust, IpcRequest};
use serde_json::{json, Value};

/// The service id the store dispatcher listens on.
pub const STORE_SERVICE_ID: &str = "aether-store";

/// Build a `request install <package_path>` command.
///
/// `package_path` is an absolute path on the device. The
/// store reads the file, verifies its signature, and runs
/// the consent flow. Returns the `IpcRequest` that
/// `AetherClient::request` will serialize.
#[must_use]
pub fn install_request(package_path: impl Into<String>, actor_trust: ActorTrust) -> IpcRequest {
    IpcRequest {
        service_id: STORE_SERVICE_ID.to_string(),
        command: "install".to_string(),
        parameters: json!({ "package_path": package_path.into() }),
        actor_trust,
    }
}

/// Build a `request launch <app_id>` command.
///
/// The store looks the app up in its installed-app table,
/// consults the consent record, and — if granted — asks
/// the launcher to spawn it. `instance_label` is an
/// optional developer-supplied string the launcher can
/// surface in logs and audit records.
#[must_use]
pub fn launch_request(
    app_id: impl Into<String>,
    instance_label: Option<String>,
    actor_trust: ActorTrust,
) -> IpcRequest {
    let mut params = json!({ "app_id": app_id.into() });
    if let Some(label) = instance_label {
        params["instance_label"] = Value::String(label);
    }
    IpcRequest {
        service_id: STORE_SERVICE_ID.to_string(),
        command: "launch".to_string(),
        parameters: params,
        actor_trust,
    }
}

/// Build a `request uninstall <app_id>` command.
///
/// The store removes the app's files and revokes any
/// outstanding consent. Installed-at receipts are
/// retained in the audit log; the install directory is
/// gone.
#[must_use]
pub fn uninstall_request(app_id: impl Into<String>, actor_trust: ActorTrust) -> IpcRequest {
    IpcRequest {
        service_id: STORE_SERVICE_ID.to_string(),
        command: "uninstall".to_string(),
        parameters: json!({ "app_id": app_id.into() }),
        actor_trust,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_core::ipc::ActorTrust;

    #[test]
    fn install_request_shape() {
        let req = install_request("/var/cache/apps/calc.aether", ActorTrust::Trusted);
        assert_eq!(req.service_id, "aether-store");
        assert_eq!(req.command, "install");
        assert_eq!(req.parameters["package_path"], "/var/cache/apps/calc.aether");
        assert_eq!(req.actor_trust, ActorTrust::Trusted);
    }

    #[test]
    fn install_request_untrusted() {
        let req = install_request("/tmp/x.aether", ActorTrust::Untrusted);
        assert_eq!(req.actor_trust, ActorTrust::Untrusted);
    }

    #[test]
    fn launch_request_with_label() {
        let req =
            launch_request("com.example.calc", Some("dev-run-1".to_string()), ActorTrust::Trusted);
        assert_eq!(req.command, "launch");
        assert_eq!(req.parameters["app_id"], "com.example.calc");
        assert_eq!(req.parameters["instance_label"], "dev-run-1");
    }

    #[test]
    fn launch_request_without_label_omits_field() {
        let req = launch_request("com.example.calc", None, ActorTrust::Trusted);
        assert!(req.parameters.get("instance_label").is_none());
    }

    #[test]
    fn uninstall_request_shape() {
        let req = uninstall_request("com.example.calc", ActorTrust::Trusted);
        assert_eq!(req.command, "uninstall");
        assert_eq!(req.parameters["app_id"], "com.example.calc");
    }

    #[test]
    fn all_three_commands_use_same_service_id() {
        let install = install_request("/p", ActorTrust::Trusted);
        let launch = launch_request("a", None, ActorTrust::Trusted);
        let uninstall = uninstall_request("a", ActorTrust::Trusted);
        assert_eq!(install.service_id, launch.service_id);
        assert_eq!(launch.service_id, uninstall.service_id);
    }

    #[test]
    fn serialized_install_round_trips() {
        // Smoke test: the IPC wire format is the
        // serde_json representation of `IpcRequest`. A
        // round-trip through `Value` should preserve
        // every field.
        let req = install_request("/p", ActorTrust::Trusted);
        let v = serde_json::to_value(&req).expect("serialize");
        let de: IpcRequest = serde_json::from_value(v).expect("deserialize");
        assert_eq!(de.service_id, req.service_id);
        assert_eq!(de.command, req.command);
        assert_eq!(de.parameters, req.parameters);
        assert_eq!(de.actor_trust, req.actor_trust);
    }
}
