use aether_core::ComponentId;
use std::fmt::{self, Display};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppManifest {
    id: ComponentId,
    display_name: String,
    command: String,
}

impl AppManifest {
    pub fn new(
        id: ComponentId,
        display_name: impl Into<String>,
        command: impl Into<String>,
    ) -> Result<Self, AppManifestError> {
        let display_name = display_name.into();
        let command = command.into();
        if display_name.trim().is_empty() {
            return Err(AppManifestError::EmptyDisplayName);
        }
        if command.trim().is_empty() {
            return Err(AppManifestError::EmptyCommand);
        }
        Ok(Self { id, display_name, command })
    }

    #[must_use]
    pub fn id(&self) -> &ComponentId {
        &self.id
    }

    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppManifestError {
    EmptyDisplayName,
    EmptyCommand,
}

impl Display for AppManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDisplayName => {
                formatter.write_str("application display name must not be empty")
            }
            Self::EmptyCommand => formatter.write_str("application command must not be empty"),
        }
    }
}

impl std::error::Error for AppManifestError {}

#[cfg(test)]
mod tests {
    use super::AppManifest;
    use aether_core::ComponentId;

    #[test]
    fn creates_app_manifest() {
        let id = ComponentId::new("terminal").unwrap_or_else(|error| panic!("{error}"));
        let manifest = AppManifest::new(id, "Terminal", "/opt/aether/bin/terminal")
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(manifest.display_name(), "Terminal");
    }
}
