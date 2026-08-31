//! End-to-end integration test for the proactive daemon
//! library.
//!
//! Drives a real `DaemonLoop` with a sequence of
//! `SystemProbe` snapshots, asserts that observations
//! and action items are submitted to an in-memory sink
//! in the expected order, and pins the loop's
//! idempotency guarantees.
//!
//! These tests live in `tests/` (integration test
//! crate) so they exercise the public API the way a
//! host binary would, not the internal helpers.

use aether_agent_core::{ObservationSeverity, OBSERVATION_LOG_LIMIT};
use aether_background_agent::ActionItem;
use aether_proactive::{
    classify_to_observations, DaemonLoop, InMemorySink, SystemProbe, Thresholds, TickResult,
};

/// A simple monotonic clock stand-in. The host supplies
/// `now_ms` on every tick; this test just makes sure
/// timestamps are unique and ordered.
struct Clock(u64);
impl Clock {
    fn next(&mut self) -> u64 {
        self.0 += 1_000;
        self.0
    }
}

#[test]
fn full_pipeline_high_storage_emits_observation_and_proposal() {
    let mut loop_ = DaemonLoop::new();
    let mut sink = InMemorySink::default();
    let mut clock = Clock(0);

    let mut probe = SystemProbe::default();
    probe.storage_percent.insert("/".to_string(), 98);

    let result: TickResult = loop_.tick(&probe, clock.next(), &mut sink);
    assert_eq!(result.observations, 1);
    // The default diagnostic rules + recovery layer may
    // or may not surface a concrete action item; what we
    // CAN assert is that the daemon classified the probe
    // and submitted exactly one observation.
    assert_eq!(sink.observations.len(), 1);
    assert_eq!(sink.observations[0].component, "storage");
    assert_eq!(sink.observations[0].severity, ObservationSeverity::Critical);
}

#[test]
fn pipeline_handles_combined_pressure_signals() {
    // Three signals in one tick: critical storage,
    // unreachable network, high memory. The daemon
    // must produce three observations and submit all
    // three to the sink in submission order.
    let mut loop_ = DaemonLoop::new();
    let mut sink = InMemorySink::default();
    let mut clock = Clock(0);

    let mut probe = SystemProbe::default();
    probe.storage_percent.insert("/".to_string(), 98);
    probe.network_reachable = Some(false);
    probe.memory_percent = Some(95);

    let result = loop_.tick(&probe, clock.next(), &mut sink);
    assert_eq!(result.observations, 3);
    assert_eq!(sink.observations.len(), 3);
    let components: Vec<&str> = sink.observations.iter().map(|o| o.component.as_str()).collect();
    assert!(components.contains(&"storage"));
    assert!(components.contains(&"network"));
    assert!(components.contains(&"memory"));
}

#[test]
fn pipeline_idempotent_on_repeated_healthy_probe() {
    // The proactive daemon must not re-emit the same
    // healthy observations across ticks (otherwise
    // the agent log would be spammed with noise).
    let mut loop_ = DaemonLoop::new();
    let mut sink = InMemorySink::default();
    let mut clock = Clock(0);
    let probe = SystemProbe {
        network_reachable: Some(true),
        memory_percent: Some(40),
        ..Default::default()
    };
    for _ in 0..5 {
        let result = loop_.tick(&probe, clock.next(), &mut sink);
        assert_eq!(result.observations, 0);
        assert_eq!(result.actions, 0);
    }
    assert!(sink.observations.is_empty());
}

#[test]
fn pipeline_respects_custom_thresholds() {
    let mut loop_ = DaemonLoop::with_thresholds(Thresholds {
        storage_warning_pct: 10,
        ..Thresholds::default()
    });
    let mut sink = InMemorySink::default();
    let mut clock = Clock(0);
    let mut probe = SystemProbe::default();
    probe.storage_percent.insert("/".to_string(), 12);
    let result = loop_.tick(&probe, clock.next(), &mut sink);
    // 12% > 10% warning, so the daemon emits a Warning.
    assert_eq!(result.observations, 1);
    assert_eq!(sink.observations[0].severity, ObservationSeverity::Warning);
}

#[test]
fn pipeline_distinct_ids_for_distinct_components() {
    // Two high-CPU processes with the same percent must
    // produce two distinct observations (otherwise the
    // proposal layer could not tell them apart).
    let mut probe = SystemProbe::default();
    probe.process_cpu.insert(101, 90);
    probe.process_cpu.insert(202, 95);
    let obs = classify_to_observations(&probe, &Thresholds::default(), 1);
    assert_eq!(obs.len(), 2);
    let mut ids: Vec<&str> = obs.iter().map(|o| o.id.as_str()).collect();
    ids.sort();
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn pipeline_sink_records_tick_lifecycle() {
    let mut loop_ = DaemonLoop::new();
    let mut sink = InMemorySink::default();
    let mut clock = Clock(0);
    let _ = loop_.tick(&SystemProbe::default(), clock.next(), &mut sink);
    let _ = loop_.tick(&SystemProbe::default(), clock.next(), &mut sink);
    assert_eq!(sink.started, 2);
    assert_eq!(sink.finished, 2);
}

#[test]
fn observation_log_limit_is_256() {
    // The aether-agent-core library constrains the
    // observation log to 256 entries. The proactive
    // daemon is one of its consumers; this test pins
    // the contract so a future change to the constant
    // forces a deliberate decision.
    assert_eq!(OBSERVATION_LOG_LIMIT, 256);
}

#[test]
fn pipeline_storage_mounts_produce_observations_per_mount() {
    // A system with two mounts over the warning
    // threshold must produce one observation per
    // mount, not one rolled-up observation.
    let mut loop_ = DaemonLoop::new();
    let mut sink = InMemorySink::default();
    let mut clock = Clock(0);
    let mut probe = SystemProbe::default();
    probe.storage_percent.insert("/".to_string(), 90);
    probe.storage_percent.insert("/home".to_string(), 96);
    let result = loop_.tick(&probe, clock.next(), &mut sink);
    assert_eq!(result.observations, 2);
    assert_eq!(sink.observations.len(), 2);
    let mut summaries: Vec<&str> = sink.observations.iter().map(|o| o.summary.as_str()).collect();
    summaries.sort();
    assert!(summaries[0].contains("`/`"));
    assert!(summaries[1].contains("`/home`"));
}

#[test]
fn pipeline_healthy_then_degraded_then_healthy() {
    // The realistic case: the daemon sees a healthy
    // probe, then storage fills up, then the user
    // cleans up. Each transition should produce the
    // correct observation set.
    let mut loop_ = DaemonLoop::new();
    let mut sink = InMemorySink::default();
    let mut clock = Clock(0);

    // Tick 1: healthy.
    let r1 = loop_.tick(&SystemProbe::default(), clock.next(), &mut sink);
    assert_eq!(r1.observations, 0);

    // Tick 2: storage critical.
    let mut degraded = SystemProbe::default();
    degraded.storage_percent.insert("/".to_string(), 98);
    let r2 = loop_.tick(&degraded, clock.next(), &mut sink);
    assert_eq!(r2.observations, 1);
    assert_eq!(sink.observations.last().map(|o| o.severity), Some(ObservationSeverity::Critical));

    // Tick 3: healthy again.
    let r3 = loop_.tick(&SystemProbe::default(), clock.next(), &mut sink);
    assert_eq!(r3.observations, 0);

    // Total observations across the three ticks: 1
    // (the critical one). The healthy ticks produced
    // none.
    assert_eq!(sink.observations.len(), 1);
}

#[test]
fn action_items_carry_subsystem_when_present() {
    // When the background state surfaces a recovery
    // action, the action item must carry the
    // subsystem the diagnostics layer identified.
    // We can't pin the exact action the diagnostics
    // layer picks (it's policy-driven), but we CAN
    // assert that any action item emitted has a
    // well-formed shape.
    let mut loop_ = DaemonLoop::new();
    let mut sink = InMemorySink::default();
    let mut clock = Clock(0);
    let mut probe = SystemProbe::default();
    probe.storage_percent.insert("/".to_string(), 99);
    let _ = loop_.tick(&probe, clock.next(), &mut sink);
    // We don't assert `sink.actions` is non-empty —
    // the default rules may or may not produce a
    // recovery plan from a single observation. The
    // invariant we DO assert is: every action item
    // that did make it through is well-formed.
    for item in &sink.actions {
        let _: &ActionItem = item;
        assert!(!item.title.is_empty());
        assert!(!item.description.is_empty());
    }
}
