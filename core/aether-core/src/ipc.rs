// Aether IPC types and traits for the Aether OS
use crate::error::AetherError;

/// Represents an Aether OS IPC endpoint.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IpcEndpoint {
    pub name: String,
    pub socket_path: Option<String>,
    pub transport: String,
}

/// An IPC request message.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IpcRequest {
    pub service_id: String,
    pub command: String,
    pub parameters: serde_json::Value,
    /// Trust level of the actor that originated the request.
    /// Defaults to `Trusted` for backwards compatibility (existing
    /// callers don't have to set it). The system-core dispatcher
    /// combines this with the `DefaultPermissionPolicy` to decide
    /// whether a capability is allowed, denied, or requires user
    /// consent.
    #[serde(default = "default_actor_trust")]
    pub actor_trust: ActorTrust,
}

/// Trust level of the actor that originated an IPC request.
///
/// The system-core dispatcher reads this when it consults the
/// permission policy. `Trusted` matches the historic behaviour
/// (capability risk is the only gate). `Untrusted` is the
/// defence-in-depth knob: every request from an untrusted actor
/// is denied regardless of risk.
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorTrust {
    /// The actor is known and trusted (default for local IPC).
    Trusted,
    /// The actor is unknown / unverified. The dispatcher denies
    /// every capability. Used by tests and by the red-team suite
    /// to verify that hostile prompts cannot run shell or
    /// shutdown commands.
    Untrusted,
}

fn default_actor_trust() -> ActorTrust {
    ActorTrust::Trusted
}

impl Default for ActorTrust {
    fn default() -> Self {
        ActorTrust::Trusted
    }
}

/// An IPC response message.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    pub command: String,
    pub result: serde_json::Value,
    pub error: Option<IpcError>,
}

/// An error in an IPC response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IpcError {
    pub code: String,
    pub message: String,
}

impl IpcResponse {
    pub fn ok(command: &str, result: serde_json::Value) -> Self {
        Self { ok: true, command: command.to_string(), result, error: None }
    }

    pub fn err(command: &str, error: IpcError) -> Self {
        Self {
            ok: false,
            command: command.to_string(),
            result: serde_json::Value::Null,
            error: Some(error),
        }
    }
}

impl From<AetherError> for IpcError {
    fn from(err: AetherError) -> Self {
        Self { code: err.code.to_string(), message: err.message }
    }
}
