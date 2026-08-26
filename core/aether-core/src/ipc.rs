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
        Self {
            ok: true,
            command: command.to_string(),
            result,
            error: None,
        }
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
        Self {
            code: err.code.to_string(),
            message: err.message,
        }
    }
}