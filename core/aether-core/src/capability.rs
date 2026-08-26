// Aether Capability types

/// Represents a named capability in the Aether capability framework.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Capability {
    pub domain: CapabilityDomain,
    pub name: String,
    pub risk_level: RiskLevel,
}

/// Capability domain namespaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityDomain {
    Filesystem,
    Network,
    Process,
    Storage,
    System,
    Audit,
    Identity,
    Application,
}

impl std::fmt::Display for CapabilityDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Filesystem => write!(f, "filesystem"),
            Self::Network => write!(f, "network"),
            Self::Process => write!(f, "process"),
            Self::Storage => write!(f, "storage"),
            Self::System => write!(f, "system"),
            Self::Audit => write!(f, "audit"),
            Self::Identity => write!(f, "identity"),
            Self::Application => write!(f, "application"),
        }
    }
}

/// Risk levels for capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl Capability {
    pub fn new(domain: CapabilityDomain, name: impl Into<String>, risk: RiskLevel) -> Self {
        Self {
            domain,
            name: name.into(),
            risk_level: risk,
        }
    }

    /// Returns the fully qualified capability string (e.g. "filesystem.read").
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.domain, self.name)
    }

    /// Checks if this capability is required.
    pub fn is_required(&self) -> bool {
        self.risk_level >= RiskLevel::High
    }

    /// Checks if this capability is destructive.
    pub fn is_destructive(&self) -> bool {
        self.risk_level >= RiskLevel::Critical
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.qualified_name())
    }
}

/// Checks if a set of granted capabilities includes a required capability.
pub fn has_capability(granted: &[Capability], required: &Capability) -> bool {
    granted.iter().any(|c| c.qualified_name() == required.qualified_name())
}

/// Checks if granted capabilities include all required capabilities.
pub fn has_all_capabilities(granted: &[Capability], required: &[Capability]) -> bool {
    required.iter().all(|r| has_capability(granted, r))
}