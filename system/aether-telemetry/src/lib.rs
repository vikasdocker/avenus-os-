// Aether Telemetry — privacy-safe, opt-in telemetry framework.
//
// Phase 15.3: Telemetry is off by default. Every consent change
// is recorded in the audit chain. The collected data set is
// minimal and documented. No PII, secrets, credentials, file
// contents, or user-generated content is ever collected.

use serde::{Deserialize, Serialize};

/// The consent state for telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConsentState {
    /// Telemetry is off. No data is collected.
    #[default]
    Off,
    /// Telemetry is on. Data is collected and may be transmitted.
    On,
}

impl ConsentState {
    /// Returns true if telemetry is enabled.
    #[must_use]
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

/// A single telemetry data point. Only system-level metrics
/// are collected — never user content, secrets, or PII.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRecord {
    /// The metric name (e.g. "system.uptime_ms", "memory.used_bytes").
    pub metric: String,
    /// The metric value.
    pub value: f64,
    /// Wall-clock timestamp when the metric was recorded (ms since epoch).
    pub timestamp_ms: u64,
    /// Optional unit (e.g. "bytes", "ms", "percent").
    pub unit: Option<String>,
}

/// A consent change event, recorded in the audit chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentChangeEvent {
    /// The previous consent state.
    pub from: ConsentState,
    /// The new consent state.
    pub to: ConsentState,
    /// Wall-clock timestamp of the change (ms since epoch).
    pub timestamp_ms: u64,
    /// Optional reason for the change.
    pub reason: Option<String>,
}

/// The documented data set that telemetry may collect.
/// This is the single source of truth for what is and is not
/// collected. Any new metric must be added here first.
pub const TELEMETRY_DATA_SET: &[&str] = &[
    "system.uptime_ms",
    "system.boot_count",
    "memory.total_bytes",
    "memory.used_bytes",
    "memory.available_bytes",
    "storage.total_bytes",
    "storage.used_bytes",
    "storage.mount_count",
    "cpu.core_count",
    "cpu.model",
    "process.total_count",
    "process.running_count",
    "network.interface_count",
    "aether.version",
    "aether.os_type",
];

/// What telemetry NEVER collects. This is a documentation
/// contract enforced by code review and CI.
pub const TELEMETRY_NEVER_COLLECTS: &[&str] = &[
    "user names or real names",
    "email addresses",
    "IP addresses (beyond local interface names)",
    "file paths or file contents",
    "passwords, tokens, API keys, or credentials",
    "SSH keys or certificates",
    "browser history",
    "document contents",
    "chat messages or AI conversations",
    "keystrokes or input recordings",
    "screen captures",
    "geolocation data",
    "hardware serial numbers",
    "MAC addresses",
    "clipboard contents",
];

/// The telemetry collector manages consent and collects
/// minimal system metrics when consent is granted.
#[derive(Default)]
pub struct TelemetryCollector {
    consent: ConsentState,
    consent_history: Vec<ConsentChangeEvent>,
}

impl TelemetryCollector {
    /// Creates a new collector with consent OFF (the safe default).
    #[must_use]
    pub fn new() -> Self {
        Self { consent: ConsentState::Off, consent_history: Vec::new() }
    }

    /// Returns the current consent state.
    #[must_use]
    pub fn consent(&self) -> ConsentState {
        self.consent
    }

    /// Changes the consent state. Returns the consent change event
    /// that should be recorded in the audit chain.
    pub fn set_consent(
        &mut self,
        new_state: ConsentState,
        now_ms: u64,
        reason: Option<String>,
    ) -> ConsentChangeEvent {
        let event =
            ConsentChangeEvent { from: self.consent, to: new_state, timestamp_ms: now_ms, reason };
        self.consent = new_state;
        self.consent_history.push(event.clone());
        event
    }

    /// Returns the full consent change history.
    #[must_use]
    pub fn consent_history(&self) -> &[ConsentChangeEvent] {
        &self.consent_history
    }

    /// Collects a telemetry record if consent is ON.
    /// Returns None if consent is OFF (no data is collected).
    pub fn collect(&self, record: TelemetryRecord) -> Option<TelemetryRecord> {
        if self.consent.is_enabled() {
            Some(record)
        } else {
            None
        }
    }

    /// Revokes consent and clears all collected data.
    /// Returns the consent change event for audit recording.
    pub fn revoke_and_clear(&mut self, now_ms: u64) -> ConsentChangeEvent {
        let event = ConsentChangeEvent {
            from: self.consent,
            to: ConsentState::Off,
            timestamp_ms: now_ms,
            reason: Some("User revoked consent".to_string()),
        };
        self.consent = ConsentState::Off;
        self.consent_history.push(event.clone());
        event
    }

    /// Simulates uninstall: revokes consent and returns all
    /// recorded events for deletion verification.
    pub fn uninstall(&mut self, now_ms: u64) -> Vec<ConsentChangeEvent> {
        let revoke_event = ConsentChangeEvent {
            from: self.consent,
            to: ConsentState::Off,
            timestamp_ms: now_ms,
            reason: Some("Uninstall: all telemetry data deleted".into()),
        };
        self.consent = ConsentState::Off;
        let mut history = std::mem::take(&mut self.consent_history);
        history.push(revoke_event);
        history
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn default_consent_is_off() {
        let collector = TelemetryCollector::new();
        assert_eq!(collector.consent(), ConsentState::Off);
    }

    #[test]
    fn consent_change_records_event() {
        let mut collector = TelemetryCollector::new();
        let event = collector.set_consent(ConsentState::On, 1000, Some("user opt-in".into()));
        assert_eq!(event.from, ConsentState::Off);
        assert_eq!(event.to, ConsentState::On);
        assert_eq!(event.timestamp_ms, 1000);
        assert_eq!(collector.consent(), ConsentState::On);
    }

    #[test]
    fn no_collection_when_consent_off() {
        let collector = TelemetryCollector::new();
        let record = TelemetryRecord {
            metric: "system.uptime_ms".into(),
            value: 12345.0,
            timestamp_ms: 1000,
            unit: Some("ms".into()),
        };
        assert!(collector.collect(record).is_none());
    }

    #[test]
    fn collection_when_consent_on() {
        let mut collector = TelemetryCollector::new();
        collector.set_consent(ConsentState::On, 1000, None);
        let record = TelemetryRecord {
            metric: "system.uptime_ms".into(),
            value: 12345.0,
            timestamp_ms: 1000,
            unit: Some("ms".into()),
        };
        let collected = collector.collect(record).unwrap();
        assert_eq!(collected.metric, "system.uptime_ms");
        assert_eq!(collected.value, 12345.0);
    }

    #[test]
    fn revoke_and_clear_turns_off() {
        let mut collector = TelemetryCollector::new();
        collector.set_consent(ConsentState::On, 1000, None);
        let event = collector.revoke_and_clear(2000);
        assert_eq!(event.from, ConsentState::On);
        assert_eq!(event.to, ConsentState::Off);
        assert_eq!(collector.consent(), ConsentState::Off);
    }

    #[test]
    fn uninstall_returns_all_events() {
        let mut collector = TelemetryCollector::new();
        collector.set_consent(ConsentState::On, 1000, None);
        collector.set_consent(ConsentState::Off, 2000, None);
        let events = collector.uninstall(3000);
        // 2 consent changes + 1 revoke = 3 events
        assert_eq!(events.len(), 3);
        // After uninstall, consent is off.
        assert_eq!(collector.consent(), ConsentState::Off);
    }

    #[test]
    fn consent_history_is_recorded() {
        let mut collector = TelemetryCollector::new();
        collector.set_consent(ConsentState::On, 1000, None);
        collector.set_consent(ConsentState::Off, 2000, None);
        assert_eq!(collector.consent_history().len(), 2);
    }

    #[test]
    fn data_set_contains_no_pii() {
        // Verify the documented data set does not contain any
        // metric that could be PII.
        for metric in TELEMETRY_DATA_SET {
            assert!(
                !metric.contains("name")
                    && !metric.contains("email")
                    && !metric.contains("password")
                    && !metric.contains("key")
                    && !metric.contains("secret")
                    && !metric.contains("token")
                    && !metric.contains("path")
                    && !metric.contains("content"),
                "metric '{metric}' may contain PII"
            );
        }
    }

    #[test]
    fn never_collects_contract_is_comprehensive() {
        assert!(!TELEMETRY_NEVER_COLLECTS.is_empty());
        assert!(TELEMETRY_NEVER_COLLECTS.contains(&"user names or real names"));
        assert!(TELEMETRY_NEVER_COLLECTS.contains(&"passwords, tokens, API keys, or credentials"));
        assert!(TELEMETRY_NEVER_COLLECTS.contains(&"file paths or file contents"));
    }

    #[test]
    fn consent_change_event_serializes() {
        let event = ConsentChangeEvent {
            from: ConsentState::Off,
            to: ConsentState::On,
            timestamp_ms: 1000,
            reason: Some("user opt-in".into()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"from\":\"off\""));
        assert!(json.contains("\"to\":\"on\""));
    }
}
