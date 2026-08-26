// Aether Error types for the Aether OS
use std::fmt;

/// Main error type for all Aether OS operations.
#[derive(Debug, Clone)]
pub struct AetherError {
    pub code: ErrorKind,
    pub message: String,
    pub details: Vec<(String, String)>,
}

/// Categories of errors in Aether OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Io,
    PermissionDenied,
    Unauthorized,
    NotCapable,
    PathTraversal,
    SymlinkEscape,
    ResourceExhausted,
    NotFound,
    InvalidInput,
    ServiceFailed,
    Internal,
    Audit,
    Config,
    Other,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io => write!(f, "IO"),
            Self::PermissionDenied => write!(f, "PERMISSION_DENIED"),
            Self::Unauthorized => write!(f, "UNAUTHORIZED"),
            Self::NotCapable => write!(f, "NOT_CAPABLE"),
            Self::PathTraversal => write!(f, "PATH_TRAVERSAL"),
            Self::SymlinkEscape => write!(f, "SYMLINK_ESCAPE"),
            Self::ResourceExhausted => write!(f, "RESOURCE_EXHAUSTED"),
            Self::NotFound => write!(f, "NOT_FOUND"),
            Self::InvalidInput => write!(f, "INVALID_INPUT"),
            Self::ServiceFailed => write!(f, "SERVICE_FAILED"),
            Self::Internal => write!(f, "INTERNAL"),
            Self::Audit => write!(f, "AUDIT"),
            Self::Config => write!(f, "CONFIG"),
            Self::Other => write!(f, "OTHER"),
        }
    }
}

impl AetherError {
    pub fn new(code: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: Vec::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.push((key.into(), value.into()));
        self
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::PermissionDenied, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unauthorized, message)
    }

    pub fn not_capable(caps: &str, required: &str) -> Self {
        Self::new(
            ErrorKind::NotCapable,
            format!("Missing capability {required}; required: [{caps}]",),
        )
    }

    pub fn path_traversal(path: &str) -> Self {
        Self::new(ErrorKind::PathTraversal, format!("Path traversal detected: {path}"))
    }

    pub fn symlink_escape(path: &str, target: &str) -> Self {
        Self::new(
            ErrorKind::SymlinkEscape,
            format!("Symlink escape detected: {path} -> {target}"),
        )
    }

    pub fn resource_exhausted(kind: &str) -> Self {
        Self::new(ErrorKind::ResourceExhausted, format!("Resource exhausted: {kind}"))
    }

    pub fn not_found(path: &str) -> Self {
        Self::new(ErrorKind::NotFound, format!("Path not found: {path}"))
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, msg)
    }

    pub fn service_failed(service: &str, reason: &str) -> Self {
        Self::new(
            ErrorKind::ServiceFailed,
            format!("Service {service} failed: {reason}"),
        )
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, msg)
    }
}

impl fmt::Display for AetherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        for (key, value) in &self.details {
            write!(f, " | {key}={value}")?;
        }
        Ok(())
    }
}

impl std::error::Error for AetherError {}

impl From<std::io::Error> for AetherError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorKind::Io, err.to_string())
    }
}

impl From<anyhow::Error> for AetherError {
    fn from(err: anyhow::Error) -> Self {
        Self::new(ErrorKind::Internal, err.to_string())
    }
}