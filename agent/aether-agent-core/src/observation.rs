// Observation: a fact the agent has surfaced about
// the system's state.
//
// The agent watches the system (storage use,
// service restart counts, network connectivity,
// etc) and emits `Observation`s into a bounded
// log. The future runtime pairs observations
// with proposals: "storage is 95% full" is
// evidence for "delete cached files" or "move
// data to the cloud".
//
// Observations are passive — they do not
// represent an action, only a fact. The future
// agentd code is the only thing allowed to
// create them. The shell stores them and
// exposes them via the IPC layer.

use serde::{Deserialize, Serialize};

/// The maximum number of observations kept in
/// memory. Older observations are dropped on
/// overflow; the agentd may also persist them
/// to disk.
pub const OBSERVATION_LOG_LIMIT: usize = 256;

/// How serious an observation is. Used by the
/// proposal layer to decide which observations
/// warrant a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationSeverity {
    /// A fact the agent noticed but does not
    /// consider a problem.
    Info,
    /// A fact the agent wants the user to know
    /// about, but no action is required.
    Notice,
    /// A fact that warrants an automated
    /// proposal.
    Warning,
    /// A fact that warrants immediate user
    /// attention.
    Critical,
}

impl ObservationSeverity {
    /// Returns the canonical kebab-case name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Notice => "notice",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

impl std::fmt::Display for ObservationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    /// A unique id. The future agentd generates
    /// UUIDv7-style ids; the shell accepts any
    /// non-empty string.
    pub id: String,
    /// The component the observation is about
    /// (e.g. `"storage"`, `"aether-agentd"`,
    /// `"network"`).
    pub component: String,
    /// A short summary of the fact.
    pub summary: String,
    /// Optional longer description.
    pub detail: Option<String>,
    /// How serious the observation is.
    pub severity: ObservationSeverity,
    /// Wall-clock timestamp of the observation.
    pub timestamp_ms: u64,
    /// Optional structured data (a number, a
    /// percentage, a count, etc). The shell
    /// does not interpret this; the proposal
    /// layer reads it to build evidence.
    pub data: Option<serde_json::Value>,
}

impl Observation {
    /// Creates a new observation. `id`, `component`,
    /// and `summary` must be non-empty.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        component: impl Into<String>,
        summary: impl Into<String>,
        severity: ObservationSeverity,
        timestamp_ms: u64,
    ) -> Option<Self> {
        let id: String = id.into();
        let component: String = component.into();
        let summary: String = summary.into();
        if id.is_empty() || component.is_empty() || summary.is_empty() {
            return None;
        }
        Some(Self {
            id,
            component,
            summary,
            detail: None,
            severity,
            timestamp_ms,
            data: None,
        })
    }

    /// Attaches a longer description.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Attaches structured data.
    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn observation_new_rejects_empty_fields() {
        assert!(Observation::new("o1", "", "summary", ObservationSeverity::Info, 1).is_none());
        assert!(Observation::new("o1", "comp", "", ObservationSeverity::Info, 1).is_none());
        assert!(Observation::new("", "comp", "summary", ObservationSeverity::Info, 1).is_none());
    }

    #[test]
    fn observation_new_accepts_minimal_valid_input() {
        let o = Observation::new("o1", "storage", "disk full", ObservationSeverity::Warning, 1);
        assert!(o.is_some());
    }

    #[test]
    fn severity_as_str_is_stable() {
        assert_eq!(ObservationSeverity::Info.as_str(), "info");
        assert_eq!(ObservationSeverity::Notice.as_str(), "notice");
        assert_eq!(ObservationSeverity::Warning.as_str(), "warning");
        assert_eq!(ObservationSeverity::Critical.as_str(), "critical");
    }

    #[test]
    fn builder_chain_attaches_detail_and_data() {
        let o = Observation::new("o1", "storage", "disk full", ObservationSeverity::Warning, 1)
            .expect("valid")
            .with_detail("the user is at 95% of the 500 GB quota")
            .with_data(serde_json::json!({"percent": 95}));
        assert_eq!(o.detail.as_deref(), Some("the user is at 95% of the 500 GB quota"));
        assert_eq!(o.data, Some(serde_json::json!({"percent": 95})));
    }

    #[test]
    fn severity_ordering_is_info_lt_notice_lt_warning_lt_critical() {
        assert!(ObservationSeverity::Info < ObservationSeverity::Notice);
        assert!(ObservationSeverity::Notice < ObservationSeverity::Warning);
        assert!(ObservationSeverity::Warning < ObservationSeverity::Critical);
    }
}
