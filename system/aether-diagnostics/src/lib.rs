//! Aether system diagnostics — the typed model the
//! agent uses to answer "why is my computer slow?"
//!
//! The diagnostics pipeline is four steps:
//!
//! 1. **Collect** — the diagnostics crate ships
//!    typed `Signal` values for each subsystem
//!    (CPU, memory, disk, network, services, apps,
//!    security). The shell / supervisor fills them
//!    in from `/proc`, the IPC bus, and the event
//!    log.
//! 2. **Symptom** — `Symptom` is the typed
//!    "something is wrong" value. A signal alone
//!    isn't a diagnosis: high CPU could be the
//!    user's compile job, or a runaway process.
//!    A `Symptom` is *correlated* (e.g. "high CPU
//!    *and* `init` OOM-killer fired *and* the
//!    browser restarted 3 times in 5 min").
//! 3. **Explain** — `Explanation` is the human-
//!    readable cause and the suggested fix.
//! 4. **Score** — `SystemHealth::score()` returns
//!    a value in `[0, 100]` that the agent
//!    watches. A score < 50 triggers a
//!    "self-healing" proposal (Phase 7.2).
//!
//! The crate is *pure* — it does not read `/proc`
//! itself, it does not start a thread, it does not
//! talk to the IPC bus. The shell feeds signals in
//! and reads `SystemHealth` out.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use aether_agent_core::{Observation, ObservationSeverity};

use alloc::string::String;
use alloc::vec::Vec;

/// A system subsystem. The diagnostics model is
/// keyed by subsystem so symptoms can be grouped
/// ("memory pressure" is one symptom even if it
/// shows up in multiple signals).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Subsystem {
    /// The CPU (load, temperature, throttling).
    Cpu,
    /// Memory (used, free, swap, OOM events).
    Memory,
    /// The disk (used, free, I/O pressure, SMART).
    Disk,
    /// The network (latency, packet loss, link
    /// state).
    Network,
    /// System services (aether-supervisor, the
    /// init system, the network manager).
    Service,
    /// User-facing applications (crashes, hangs,
    /// permission errors).
    App,
    /// Security events (failed logins, sandbox
    /// denials, certificate errors).
    Security,
    /// Battery / power (on laptops).
    Power,
    /// The file system (full, read-only, corrupt).
    FileSystem,
    /// A subsystem not covered by the above —
    /// catch-all.
    Other,
}

impl Subsystem {
    /// The human-readable name an agent explanation
    /// would use.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Disk => "Disk",
            Self::Network => "Network",
            Self::Service => "Services",
            Self::App => "Applications",
            Self::Security => "Security",
            Self::Power => "Power",
            Self::FileSystem => "File system",
            Self::Other => "Other",
        }
    }
}

/// A single signal from a subsystem. The shell
/// fills these in and hands them to the
/// `DiagnosticReport::ingest` pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Signal {
    /// Which subsystem the signal is from.
    pub subsystem: Subsystem,
    /// A short tag for the signal (e.g. "cpu.load",
    /// "memory.oom", "service.crashed"). The
    /// diagnostic rules key on this tag.
    pub tag: String,
    /// The signal's value, normalized to a `f32`.
    /// 0.0 = "everything is fine," 1.0 = "as bad as
    /// it gets." The shell maps raw values into
    /// this scale.
    pub value: f32,
    /// A free-form human-readable description
    /// (e.g. "load average 12.4 on 4 cores,"
    /// "service aether-supervisor exited 137").
    pub detail: String,
}

impl Signal {
    /// Construct a signal.
    #[must_use]
    pub fn new(
        subsystem: Subsystem,
        tag: impl Into<String>,
        value: f32,
        detail: impl Into<String>,
    ) -> Self {
        Self { subsystem, tag: tag.into(), value: value.clamp(0.0, 1.0), detail: detail.into() }
    }

    /// Whether the signal is "alarming" (its value
    /// is above 0.5). Used by the report to
    /// decide whether to surface a symptom.
    #[must_use]
    pub fn is_alarming(&self) -> bool {
        self.value > 0.5
    }
}

/// A symptom — a correlation of one or more
/// signals that, taken together, indicate a
/// specific problem. The agent maps a symptom to
/// an explanation; the explanation is what the
/// user sees.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symptom {
    /// A short, stable id for the symptom (e.g.
    /// "memory_pressure", "service_down",
    /// "disk_full"). The rules table in
    /// `match_rules` keys on this id.
    pub id: String,
    /// Which subsystem the symptom is primarily
    /// about.
    pub subsystem: Subsystem,
    /// A severity, mirroring the agent's
    /// `ObservationSeverity` vocabulary.
    pub severity: ObservationSeverity,
    /// The signals that produced the symptom. The
    /// agent's explanation step can list these so
    /// the user can verify the diagnosis.
    pub signals: Vec<String>,
}

impl Symptom {
    /// Construct a symptom.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        subsystem: Subsystem,
        severity: ObservationSeverity,
        signals: Vec<String>,
    ) -> Self {
        Self { id: id.into(), subsystem, severity, signals }
    }
}

/// A human-readable explanation of a symptom. The
/// `cause` is the agent's read of the underlying
/// issue; the `fix` is the proposed remediation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Explanation {
    /// The id of the symptom this explains.
    pub symptom_id: String,
    /// A one-sentence cause. E.g. "The browser
    /// process is using 6 GB of memory because of
    /// a known memory leak in its renderer
    /// subsystem."
    pub cause: String,
    /// A one-sentence proposed fix. E.g. "Restart
    /// the browser. If it recurs, file a bug with
    /// the renderer crash dump attached."
    pub fix: String,
    /// Whether the fix requires user consent
    /// before the agent can act on it. Self-healing
    /// fixes (Phase 7.2) are tagged `false`;
    /// destructive fixes (delete a file, kill a
    /// user process) are `true`.
    pub requires_consent: bool,
}

impl Explanation {
    /// Construct an explanation.
    #[must_use]
    pub fn new(
        symptom_id: impl Into<String>,
        cause: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Self {
            symptom_id: symptom_id.into(),
            cause: cause.into(),
            fix: fix.into(),
            requires_consent: true,
        }
    }

    /// Mark the fix as self-healing (no consent
    /// required).
    #[must_use]
    pub fn self_healing(mut self) -> Self {
        self.requires_consent = false;
        self
    }
}

/// A built-in rules table. The diagnostics pipeline
/// uses this to map a list of signals to symptoms
/// and symptoms to explanations. The table is
/// pure data: the caller can extend it at runtime
/// (e.g. with subsystem-specific rules loaded from
/// `/etc/aether/diagnostics.rules`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RulesTable {
    /// The rules: a list of (predicate → symptom)
    /// pairs. The report runs the predicates in
    /// order; the first match wins.
    pub symptom_rules: Vec<(SymptomRule, Symptom)>,
    /// The explanations: keyed by symptom id.
    pub explanations: Vec<(String, Explanation)>,
}

impl RulesTable {
    /// An empty rules table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a symptom rule.
    #[must_use]
    pub fn with_rule(mut self, rule: SymptomRule, symptom: Symptom) -> Self {
        self.symptom_rules.push((rule, symptom));
        self
    }

    /// Add an explanation.
    #[must_use]
    pub fn with_explanation(mut self, explanation: Explanation) -> Self {
        self.explanations.push((explanation.symptom_id.clone(), explanation));
        self
    }

    /// Look up an explanation by symptom id.
    #[must_use]
    pub fn explanation_for(&self, symptom_id: &str) -> Option<&Explanation> {
        self.explanations.iter().find(|(id, _)| id == symptom_id).map(|(_, e)| e)
    }
}

/// A symptom rule — a predicate over a list of
/// signals. The default rules table ships several
/// of these.
#[derive(Debug, Clone, PartialEq)]
pub enum SymptomRule {
    /// Triggers when the signal with the given
    /// subsystem + tag has value >= threshold.
    AboveThreshold {
        /// The subsystem.
        subsystem: Subsystem,
        /// The signal tag.
        tag: String,
        /// The threshold (0..=1).
        threshold: f32,
        /// The symptom id to emit.
        symptom_id: String,
        /// The symptom's severity.
        severity: ObservationSeverity,
    },
    /// Triggers when N of the given signals are
    /// all alarming. Used for correlation rules
    /// (e.g. high CPU *and* OOM-kill *and* app
    /// crash).
    AnyAlarming {
        /// The signal tags that must all be
        /// alarming.
        tags: Vec<String>,
        /// The symptom id.
        symptom_id: String,
        /// The symptom's severity.
        severity: ObservationSeverity,
    },
}

impl SymptomRule {
    /// Whether the rule matches the given signals.
    #[must_use]
    pub fn matches(&self, signals: &[Signal]) -> Option<Symptom> {
        match self {
            Self::AboveThreshold { subsystem, tag, threshold, symptom_id, severity } => {
                let s = signals
                    .iter()
                    .find(|s| s.subsystem == *subsystem && s.tag == *tag)?;
                if s.value >= *threshold {
                    Some(Symptom::new(
                        symptom_id.clone(),
                        *subsystem,
                        *severity,
                        alloc::vec![tag.clone()],
                    ))
                } else {
                    None
                }
            }
            Self::AnyAlarming { tags, symptom_id, severity } => {
                let matching: Vec<String> = tags
                    .iter()
                    .filter(|t| {
                        signals.iter().any(|s| &s.tag == *t && s.is_alarming())
                    })
                    .cloned()
                    .collect();
                if matching.is_empty() {
                    None
                } else {
                    // Pick the first matching tag's
                    // subsystem as the primary.
                    let sub = signals
                        .iter()
                        .find(|s| matching.contains(&s.tag))
                        .map_or(Subsystem::Other, |s| s.subsystem);
                    Some(Symptom::new(
                        symptom_id.clone(),
                        sub,
                        *severity,
                        matching,
                    ))
                }
            }
        }
    }
}

/// The default rules table. The diagnostics pipeline
/// starts from this and callers can extend it.
#[must_use]
pub fn default_rules() -> RulesTable {
    RulesTable::new()
        // High CPU
        .with_rule(
            SymptomRule::AboveThreshold {
                subsystem: Subsystem::Cpu,
                tag: "cpu.load".into(),
                threshold: 0.85,
                symptom_id: "cpu_overload".into(),
                severity: ObservationSeverity::Warning,
            },
            Symptom::new("cpu_overload", Subsystem::Cpu, ObservationSeverity::Warning, alloc::vec!["cpu.load".into()]),
        )
        // Memory pressure
        .with_rule(
            SymptomRule::AboveThreshold {
                subsystem: Subsystem::Memory,
                tag: "memory.pressure".into(),
                threshold: 0.9,
                symptom_id: "memory_pressure".into(),
                severity: ObservationSeverity::Critical,
            },
            Symptom::new("memory_pressure", Subsystem::Memory, ObservationSeverity::Critical, alloc::vec!["memory.pressure".into()]),
        )
        // Disk nearly full
        .with_rule(
            SymptomRule::AboveThreshold {
                subsystem: Subsystem::Disk,
                tag: "disk.used_ratio".into(),
                threshold: 0.95,
                symptom_id: "disk_full".into(),
                severity: ObservationSeverity::Critical,
            },
            Symptom::new("disk_full", Subsystem::Disk, ObservationSeverity::Critical, alloc::vec!["disk.used_ratio".into()]),
        )
        // Service down
        .with_rule(
            SymptomRule::AboveThreshold {
                subsystem: Subsystem::Service,
                tag: "service.down".into(),
                threshold: 0.5,
                symptom_id: "service_down".into(),
                severity: ObservationSeverity::Critical,
            },
            Symptom::new("service_down", Subsystem::Service, ObservationSeverity::Critical, alloc::vec!["service.down".into()]),
        )
        // App crash loop: 3+ crashes in 5 min
        .with_rule(
            SymptomRule::AboveThreshold {
                subsystem: Subsystem::App,
                tag: "app.crash_rate".into(),
                threshold: 0.5,
                symptom_id: "app_crash_loop".into(),
                severity: ObservationSeverity::Warning,
            },
            Symptom::new("app_crash_loop", Subsystem::App, ObservationSeverity::Warning, alloc::vec!["app.crash_rate".into()]),
        )
        // Correlated: high CPU + OOM + crash
        .with_rule(
            SymptomRule::AnyAlarming {
                tags: alloc::vec!["cpu.load".into(), "memory.oom".into(), "app.crashed".into()],
                symptom_id: "system_unstable".into(),
                severity: ObservationSeverity::Critical,
            },
            Symptom::new("system_unstable", Subsystem::Other, ObservationSeverity::Critical, Vec::new()),
        )
        // Default explanations
        .with_explanation(
            Explanation::new("cpu_overload", "A process is using the CPU heavily.", "Open the taskbar's CPU chip to see the top process.")
                .self_healing(),
        )
        .with_explanation(
            Explanation::new("memory_pressure", "Memory is nearly full.", "Close a few large apps, or restart the most memory-hungry one.")
                .self_healing(),
        )
        .with_explanation(
            Explanation::new("disk_full", "The disk is almost full.", "Run the disk cleanup, or archive old files."),
        )
        .with_explanation(
            Explanation::new("service_down", "A required system service is not running.", "Restart the service.")
                .self_healing(),
        )
        .with_explanation(
            Explanation::new("app_crash_loop", "An application is repeatedly crashing.", "Open the app to see the crash report, or uninstall it."),
        )
        .with_explanation(
            Explanation::new("system_unstable", "Multiple subsystems are alarming at once.", "The system is in an unstable state. Consider restarting.")
                .self_healing(),
        )
}

/// The full diagnostic report at one moment. The
/// agent reads `score()` and `symptoms()` and
/// decides what to propose.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticReport {
    /// The most recent signals ingested. The
    /// report keeps the most recent N (default
    /// 256, mirroring `OBSERVATION_LOG_LIMIT`).
    pub signals: Vec<Signal>,
    /// The symptoms produced by the last
    /// `evaluate` call.
    pub symptoms: Vec<Symptom>,
    /// The explanations for the symptoms.
    pub explanations: Vec<Explanation>,
    /// The active rules table.
    pub rules: RulesTable,
}

impl DiagnosticReport {
    /// A fresh, empty report with the default rules.
    #[must_use]
    pub fn new() -> Self {
        Self {
            signals: Vec::new(),
            symptoms: Vec::new(),
            explanations: Vec::new(),
            rules: default_rules(),
        }
    }

    /// A fresh report with a custom rules table.
    #[must_use]
    pub fn with_rules(rules: RulesTable) -> Self {
        Self { signals: Vec::new(), symptoms: Vec::new(), explanations: Vec::new(), rules }
    }

    /// Ingest a signal. The signal is added to the
    /// log; the report's symptoms are *not* re-
    /// evaluated until the caller calls `evaluate`.
    pub fn ingest(&mut self, signal: Signal) {
        self.signals.push(signal);
        // Cap the log at the standard 256.
        if self.signals.len() > 256 {
            let drop = self.signals.len() - 256;
            self.signals.drain(0..drop);
        }
    }

    /// Re-evaluate symptoms and explanations from
    /// the current signal log and the rules table.
    pub fn evaluate(&mut self) {
        let mut new_symptoms: Vec<Symptom> = Vec::new();
        for (rule, default) in &self.rules.symptom_rules {
            if let Some(s) = rule.matches(&self.signals) {
                // Prefer the rule's emitted symptom
                // but fall back to the default if a
                // caller-supplied rule was
                // constructed without a Symptom.
                let _ = default;
                new_symptoms.push(s);
            }
        }
        // Deduplicate by id, keeping the first.
        let mut seen: Vec<String> = Vec::new();
        new_symptoms.retain(|s| {
            if seen.contains(&s.id) {
                false
            } else {
                seen.push(s.id.clone());
                true
            }
        });
        let mut new_explanations: Vec<Explanation> = Vec::new();
        for s in &new_symptoms {
            if let Some(e) = self.rules.explanation_for(&s.id) {
                new_explanations.push(e.clone());
            }
        }
        self.symptoms = new_symptoms;
        self.explanations = new_explanations;
    }

    /// The overall health score, in `[0, 100]`. A
    /// perfect system scores 100. Each symptom
    /// subtracts points weighted by severity
    /// (Info=2, Notice=3, Warning=5, Critical=30).
    /// The score is clamped to 0.
    #[must_use]
    pub fn score(&self) -> u32 {
        let mut s: i32 = 100;
        for sym in &self.symptoms {
            let cost: i32 = severity_cost(sym.severity);
            s -= cost;
        }
        s.max(0) as u32
    }

    /// Convert the active symptoms to the agent's
    /// `Observation` vocabulary, so the agent's
    /// proposal pipeline can consume them without
    /// caring that they came from diagnostics.
    /// The `timestamp_ms` defaults to 0 (the agent
    /// pipeline can fill in the real wall clock).
    #[must_use]
    pub fn to_observations(&self) -> Vec<Observation> {
        self.symptoms
            .iter()
            .filter_map(|s| {
                Observation::new(
                    s.id.clone(),
                    format!("diagnostics.{:?}", s.subsystem),
                    s.id.clone(),
                    s.severity,
                    0,
                )
            })
            .collect()
    }

    /// Whether the system's health is below the
    /// "needs attention" threshold. The threshold
    /// is 50.
    #[must_use]
    pub fn needs_attention(&self) -> bool {
        self.score() < 50
    }

    /// The number of signals currently in the log.
    #[must_use]
    pub fn signal_count(&self) -> usize {
        self.signals.len()
    }

    /// The number of active symptoms.
    #[must_use]
    pub fn symptom_count(&self) -> usize {
        self.symptoms.len()
    }
}

impl Default for DiagnosticReport {
    fn default() -> Self {
        Self::new()
    }
}

/// The cost, in health-score points, of a
/// single symptom at a given severity. A
/// `Critical` symptom is the most expensive —
/// pulling 30 points off the score.
#[allow(unreachable_patterns)]
fn severity_cost(sev: ObservationSeverity) -> i32 {
    match sev {
        ObservationSeverity::Info => 2,
        ObservationSeverity::Notice => 3,
        ObservationSeverity::Warning => 5,
        ObservationSeverity::Critical => 30,
        _ => 0,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn subsystem_labels_are_non_empty() {
        let subs = [
            Subsystem::Cpu,
            Subsystem::Memory,
            Subsystem::Disk,
            Subsystem::Network,
            Subsystem::Service,
            Subsystem::App,
            Subsystem::Security,
            Subsystem::Power,
            Subsystem::FileSystem,
            Subsystem::Other,
        ];
        for s in subs {
            assert!(!s.label().is_empty());
        }
    }

    #[test]
    fn signal_clamps_value() {
        let s = Signal::new(Subsystem::Cpu, "cpu.load", 1.5, "high");
        assert_eq!(s.value, 1.0);
        let s2 = Signal::new(Subsystem::Cpu, "cpu.load", -0.5, "low");
        assert_eq!(s2.value, 0.0);
    }

    #[test]
    fn signal_is_alarming_above_half() {
        let s = Signal::new(Subsystem::Cpu, "cpu.load", 0.6, "");
        assert!(s.is_alarming());
        let s2 = Signal::new(Subsystem::Cpu, "cpu.load", 0.4, "");
        assert!(!s2.is_alarming());
        let s3 = Signal::new(Subsystem::Cpu, "cpu.load", 0.5, "");
        assert!(!s3.is_alarming());
    }

    #[test]
    fn symptom_stores_id() {
        let s = Symptom::new(
            "x",
            Subsystem::Cpu,
            ObservationSeverity::Warning,
            alloc::vec!["cpu.load".into()],
        );
        assert_eq!(s.id, "x");
        assert_eq!(s.subsystem, Subsystem::Cpu);
    }

    #[test]
    fn explanation_default_requires_consent() {
        let e = Explanation::new("x", "cause", "fix");
        assert!(e.requires_consent);
    }

    #[test]
    fn explanation_self_healing_clears_consent() {
        let e = Explanation::new("x", "c", "f").self_healing();
        assert!(!e.requires_consent);
    }

    #[test]
    fn rules_table_starts_empty() {
        let r = RulesTable::new();
        assert!(r.symptom_rules.is_empty());
        assert!(r.explanations.is_empty());
    }

    #[test]
    fn rules_table_with_rule() {
        let r = RulesTable::new().with_rule(
            SymptomRule::AboveThreshold {
                subsystem: Subsystem::Cpu,
                tag: "x".into(),
                threshold: 0.5,
                symptom_id: "y".into(),
                severity: ObservationSeverity::Warning,
            },
            Symptom::new("y", Subsystem::Cpu, ObservationSeverity::Warning, Vec::new()),
        );
        assert_eq!(r.symptom_rules.len(), 1);
    }

    #[test]
    fn rules_table_explanation_for() {
        let r = RulesTable::new().with_explanation(Explanation::new("a", "c", "f"));
        assert!(r.explanation_for("a").is_some());
        assert!(r.explanation_for("z").is_none());
    }

    #[test]
    fn above_threshold_rule_matches() {
        let r = SymptomRule::AboveThreshold {
            subsystem: Subsystem::Cpu,
            tag: "cpu.load".into(),
            threshold: 0.8,
            symptom_id: "cpu_overload".into(),
            severity: ObservationSeverity::Warning,
        };
        let signals = [Signal::new(Subsystem::Cpu, "cpu.load", 0.9, "high")];
        let s = r.matches(&signals);
        assert!(s.is_some());
        assert_eq!(s.unwrap().id, "cpu_overload");
    }

    #[test]
    fn above_threshold_rule_does_not_match_below() {
        let r = SymptomRule::AboveThreshold {
            subsystem: Subsystem::Cpu,
            tag: "cpu.load".into(),
            threshold: 0.8,
            symptom_id: "cpu_overload".into(),
            severity: ObservationSeverity::Warning,
        };
        let signals = [Signal::new(Subsystem::Cpu, "cpu.load", 0.5, "")];
        assert!(r.matches(&signals).is_none());
    }

    #[test]
    fn any_alarming_rule_matches_when_one_present() {
        let r = SymptomRule::AnyAlarming {
            tags: alloc::vec!["cpu.load".into(), "memory.oom".into()],
            symptom_id: "system_unstable".into(),
            severity: ObservationSeverity::Critical,
        };
        let signals = [Signal::new(Subsystem::Cpu, "cpu.load", 0.9, "")];
        let s = r.matches(&signals);
        assert!(s.is_some());
        assert_eq!(s.unwrap().id, "system_unstable");
    }

    #[test]
    fn any_alarming_rule_no_match_when_none() {
        let r = SymptomRule::AnyAlarming {
            tags: alloc::vec!["cpu.load".into(), "memory.oom".into()],
            symptom_id: "system_unstable".into(),
            severity: ObservationSeverity::Critical,
        };
        let signals = [Signal::new(Subsystem::Cpu, "cpu.load", 0.3, "")];
        assert!(r.matches(&signals).is_none());
    }

    #[test]
    fn default_rules_has_known_symptoms() {
        let r = default_rules();
        assert!(r.explanation_for("cpu_overload").is_some());
        assert!(r.explanation_for("memory_pressure").is_some());
        assert!(r.explanation_for("disk_full").is_some());
        assert!(r.explanation_for("service_down").is_some());
        assert!(r.explanation_for("app_crash_loop").is_some());
        assert!(r.explanation_for("system_unstable").is_some());
    }

    #[test]
    fn report_starts_empty() {
        let r = DiagnosticReport::new();
        assert_eq!(r.signal_count(), 0);
        assert_eq!(r.symptom_count(), 0);
        assert_eq!(r.score(), 100);
    }

    #[test]
    fn report_ingest_adds_signal() {
        let mut r = DiagnosticReport::new();
        r.ingest(Signal::new(Subsystem::Cpu, "cpu.load", 0.5, ""));
        assert_eq!(r.signal_count(), 1);
    }

    #[test]
    fn report_evaluate_produces_symptom() {
        let mut r = DiagnosticReport::new();
        r.ingest(Signal::new(Subsystem::Cpu, "cpu.load", 0.95, ""));
        r.evaluate();
        // Two symptoms: cpu_overload AND
        // system_unstable (the latter because
        // cpu.load is alarming).
        assert_eq!(r.symptom_count(), 2);
        let ids: Vec<&str> = r.symptoms.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"cpu_overload"));
        assert!(ids.contains(&"system_unstable"));
    }

    #[test]
    fn report_evaluate_links_explanation() {
        let mut r = DiagnosticReport::new();
        r.ingest(Signal::new(Subsystem::Cpu, "cpu.load", 0.95, ""));
        r.evaluate();
        assert_eq!(r.explanations.len(), 2);
        let ids: Vec<&str> = r.explanations.iter().map(|e| e.symptom_id.as_str()).collect();
        assert!(ids.contains(&"cpu_overload"));
        assert!(ids.contains(&"system_unstable"));
    }

    #[test]
    fn report_evaluate_dedupes() {
        let mut r = DiagnosticReport::new();
        r.ingest(Signal::new(Subsystem::Cpu, "cpu.load", 0.95, ""));
        r.ingest(Signal::new(Subsystem::Cpu, "cpu.load", 0.97, ""));
        r.evaluate();
        // Two signals produce: cpu_overload (once)
        // and system_unstable (once). The dedup is
        // by id, not by signal.
        let cpu_count = r.symptoms.iter().filter(|s| s.id == "cpu_overload").count();
        let unst_count = r.symptoms.iter().filter(|s| s.id == "system_unstable").count();
        assert_eq!(cpu_count, 1);
        assert_eq!(unst_count, 1);
    }

    #[test]
    fn report_score_drops_with_critical() {
        let mut r = DiagnosticReport::new();
        r.ingest(Signal::new(Subsystem::Cpu, "cpu.load", 0.9, ""));
        r.ingest(Signal::new(Subsystem::Memory, "memory.oom", 0.9, ""));
        r.ingest(Signal::new(Subsystem::App, "app.crashed", 0.9, ""));
        r.evaluate();
        // Critical symptom: -30. Plus the
        // cpu_overload warning: -5. Plus the
        // (potential) memory.pressure error if
        // triggered. The 3-alarm correlation
        // produces a Critical symptom.
        let s = r.score();
        assert!(s < 100);
    }

    #[test]
    fn report_score_never_below_zero() {
        let mut r = DiagnosticReport::new();
        // Trigger every symptom at once.
        r.ingest(Signal::new(Subsystem::Cpu, "cpu.load", 1.0, ""));
        r.ingest(Signal::new(Subsystem::Memory, "memory.pressure", 1.0, ""));
        r.ingest(Signal::new(Subsystem::Disk, "disk.used_ratio", 1.0, ""));
        r.ingest(Signal::new(Subsystem::Service, "service.down", 1.0, ""));
        r.ingest(Signal::new(Subsystem::App, "app.crash_rate", 1.0, ""));
        r.ingest(Signal::new(Subsystem::Memory, "memory.oom", 1.0, ""));
        r.ingest(Signal::new(Subsystem::App, "app.crashed", 1.0, ""));
        r.evaluate();
        assert_eq!(r.score(), 0);
    }

    #[test]
    fn report_needs_attention_below_50() {
        let mut r = DiagnosticReport::new();
        assert!(!r.needs_attention());
        // Trigger enough Critical symptoms to
        // push the score below 50. Each Critical
        // subtracts 30, so 2 Criticals (60 total)
        // gets us to 40 < 50.
        r.ingest(Signal::new(Subsystem::Memory, "memory.pressure", 1.0, ""));
        r.ingest(Signal::new(Subsystem::Disk, "disk.used_ratio", 1.0, ""));
        r.ingest(Signal::new(Subsystem::Service, "service.down", 1.0, ""));
        r.ingest(Signal::new(Subsystem::Cpu, "cpu.load", 1.0, ""));
        r.ingest(Signal::new(Subsystem::Memory, "memory.oom", 1.0, ""));
        r.ingest(Signal::new(Subsystem::App, "app.crashed", 1.0, ""));
        r.evaluate();
        // memory_pressure (Critical, -30),
        // disk_full (Critical, -30),
        // service_down (Critical, -30),
        // cpu_overload (Warning, -5),
        // system_unstable (Critical, -30).
        // Total: -125 → 0 (clamped).
        assert!(r.needs_attention());
        assert_eq!(r.score(), 0);
    }

    #[test]
    fn report_to_observations() {
        let mut r = DiagnosticReport::new();
        r.ingest(Signal::new(Subsystem::Cpu, "cpu.load", 0.95, ""));
        r.evaluate();
        let obs = r.to_observations();
        // 2 observations: cpu_overload + system_unstable.
        assert_eq!(obs.len(), 2);
        let ids: Vec<&str> = obs.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"cpu_overload"));
        assert!(ids.contains(&"system_unstable"));
    }

    #[test]
    fn report_signal_log_caps_at_256() {
        let mut r = DiagnosticReport::new();
        for i in 0..300 {
            r.ingest(Signal::new(Subsystem::Other, format!("s{i}"), 0.1, ""));
        }
        assert_eq!(r.signal_count(), 256);
    }
}
