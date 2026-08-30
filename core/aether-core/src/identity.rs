// Aether Identity types
use crate::error::AetherError;
use serde::{Deserialize, Serialize};

/// A validated identifier for a system component (service, app, tool).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ComponentId(String);

impl ComponentId {
    /// Creates a validated component id.
    ///
    /// Valid ids are non-empty and contain only lowercase ASCII letters,
    /// digits, hyphens, underscores, and dots.
    pub fn new(raw: impl Into<String>) -> Result<Self, AetherError> {
        let raw = raw.into();
        if raw.is_empty() {
            return Err(AetherError::invalid_input("component id must not be empty"));
        }
        if !raw
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.'))
        {
            return Err(AetherError::invalid_input(format!(
                "component id '{raw}' contains invalid characters; allowed: [a-z0-9._-]"
            )));
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents an Aether OS identity boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AetherIdentity {
    pub name: String,
    pub scope: String,
    pub permissions: Vec<String>,
    pub is_root: bool,
}

impl AetherIdentity {
    pub fn new(name: impl Into<String>, scope: impl Into<String>) -> Self {
        Self { name: name.into(), scope: scope.into(), permissions: Vec::new(), is_root: false }
    }

    pub fn with_permission(mut self, perm: impl Into<String>) -> Self {
        self.permissions.push(perm.into());
        self
    }

    pub fn with_root(mut self) -> Self {
        self.is_root = true;
        self
    }

    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.iter().any(|p| p == perm)
    }

    pub fn verify(&self, required_perm: &str) -> Result<(), AetherError> {
        if !self.has_permission(required_perm) {
            return Err(AetherError::unauthorized(format!(
                "Identity '{}' lacks permission '{}'",
                self.name, required_perm
            )));
        }
        Ok(())
    }

    /// Returns a display representation of the identity.
    pub fn display_string(&self) -> String {
        let mut s = format!("{}:{}", self.name, self.scope);
        if self.is_root {
            s.push_str("(root)");
        }
        s
    }
}

impl std::fmt::Display for AetherIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_string())
    }
}
