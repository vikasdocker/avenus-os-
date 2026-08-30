// Aether-bench: micro-benchmark harness for Aether OS hot paths.
//
// `expect` / `unwrap` are appropriate inside a smoke benchmark —
// a failed assertion is the correct outcome, and the operator is
// reading the output directly.
#![allow(clippy::unwrap_used, clippy::expect_used)]
//
// Measures end-to-end throughput of the operations that the
// system-core dispatch loop hits on every IPC request:
//   * Audit chain record + verify (security::audit)
//   * SealedStore seal + unseal (security::credentials)
//   * Pairing code validation (device-core::pairing)
//   * Fingerprint SHA-256 (device-core::fingerprint)
//   * IPC request/response round trip (in-memory)
//
// The output is human-readable so a CI run can print it
// directly. There is no external reporting; this is not
// a regression gate, it is a smoke measurement.

use std::time::Instant;

use aether_agent_core::{Observation, ObservationSeverity, Proposal, ProposalRisk, TaskKind};
use aether_device_core::fingerprint::DeviceFingerprint;
use aether_device_core::pairing::{
    validate_acceptance, PairingAcceptance, PairingCode, PairingGrant, PairingRequest,
};
use aether_device_core::{DeviceClass, DeviceId, DeviceRegistry};
use aether_security::audit::{AuditChain, RetentionPolicy};
use aether_security::credentials::{SealedStore, StaticKeyProvider};
use sha2::{Digest, Sha256};

const ITERATIONS: usize = 5_000;

fn header(name: &str) {
    println!("\n== {name} ==");
}

fn report(name: &str, iters: usize, elapsed_ns: u128) {
    let ns_per_op = elapsed_ns as f64 / iters as f64;
    let ops_per_sec = 1e9 / ns_per_op;
    println!("  {name:<32} {iters:>8} iters  {ns_per_op:>10.1} ns/op  {ops_per_sec:>14.0} op/s");
}

fn bench_audit_chain() {
    header("audit chain (record + verify)");
    let mut chain = AuditChain::new(RetentionPolicy::last_n(ITERATIONS + 16));
    let started = Instant::now();
    for i in 0..ITERATIONS {
        chain.record(
            1_700_000_000_000 + i as u64,
            "bench.append",
            "bench",
            &format!("{{\"i\":{i}}}"),
        );
    }
    chain.verify_chain().expect("audit verify must pass for an untampered chain");
    let elapsed = started.elapsed().as_nanos();
    report("record+verify", ITERATIONS, elapsed);
}

fn bench_seal_unseal() {
    header("sealed store (seal + unseal)");
    let mut store = SealedStore::new(StaticKeyProvider::new([0x99u8; 32]));
    let plaintext = "super-secret-value-for-benchmarking-throughput";
    let started = Instant::now();
    let mut names: Vec<String> = Vec::with_capacity(ITERATIONS);
    for i in 0..ITERATIONS {
        let name = format!("name-{i}");
        store
            .seal(&name, plaintext, Some(&format!("label-{i}")), true)
            .expect("seal must succeed with a valid key");
        names.push(name);
    }
    for name in &names {
        let _ = store.unseal(name).expect("unseal must round-trip a freshly sealed envelope");
    }
    let elapsed = started.elapsed().as_nanos();
    report("seal+unseal", ITERATIONS, elapsed);
}

fn bench_fingerprint_sha256() {
    header("fingerprint (SHA-256 over a 32-byte public key)");
    let started = Instant::now();
    let mut hasher = Sha256::new();
    for i in 0..ITERATIONS {
        hasher.update([i as u8; 32]);
        let _digest: [u8; 32] = hasher.finalize_reset().into();
    }
    let elapsed = started.elapsed().as_nanos();
    report("sha256-32B", ITERATIONS, elapsed);

    // Also time the public-key → fingerprint helper.
    let started = Instant::now();
    for i in 0..ITERATIONS {
        let _ = DeviceFingerprint::from_public_key(&[i as u8; 32]);
    }
    let elapsed = started.elapsed().as_nanos();
    report("from_public_key", ITERATIONS, elapsed);
}

fn bench_pairing_validate() {
    header("pairing acceptance (validate_acceptance)");
    let req = PairingRequest {
        device_id: DeviceId::new("dev-bench").expect("valid id"),
        device_class: DeviceClass::Laptop,
        fingerprint: DeviceFingerprint::from_bytes([0x33u8; 32]),
        code: PairingCode::new("123456").expect("valid code"),
        grant: PairingGrant::default(),
        timestamp_ms: 1_700_000_000_000,
    };
    let acceptance = PairingAcceptance {
        device_id: req.device_id.clone(),
        device_class: req.device_class,
        fingerprint: req.fingerprint,
        code: req.code,
        grant: req.grant.clone(),
        timestamp_ms: 1_700_000_000_000,
    };
    let started = Instant::now();
    let mut ok = 0u64;
    for i in 0..ITERATIONS {
        if validate_acceptance(&req, &acceptance, 60_000, 1_700_000_000_000 + i as u64).is_ok() {
            ok += 1;
        }
    }
    let elapsed = started.elapsed().as_nanos();
    report("validate_acceptance", ITERATIONS, elapsed);
    assert_eq!(ok, ITERATIONS as u64, "all validations should pass");
}

fn bench_device_registry() {
    header("device registry (register + get)");
    let mut registry = DeviceRegistry::new();
    let started = Instant::now();
    // The registry is bounded at 256; insert up to the cap and
    // then exercise get() on the stored set.
    let n = 256.min(ITERATIONS);
    let mut ids: Vec<DeviceId> = Vec::with_capacity(n);
    for i in 0..n {
        let id = DeviceId::new(format!("dev-bench-{i}")).expect("valid id");
        registry
            .register(
                id.clone(),
                DeviceClass::Phone,
                DeviceFingerprint::from_bytes([(i & 0xff) as u8; 32]),
                PairingGrant::default(),
                1_700_000_000_000,
            )
            .expect("register within 256-entry cap is fine");
        ids.push(id);
    }
    for id in &ids {
        let _ = registry.get(id);
    }
    let elapsed = started.elapsed().as_nanos();
    report("register+get", n, elapsed);
}

fn bench_ipc_round_trip() {
    header("ipc request encode/decode (serde_json)");
    use aether_core::ipc::{ActorTrust, IpcRequest};
    use serde_json::json;
    let obs = Observation::new(
        "obs-bench",
        "storage",
        "disk is full",
        ObservationSeverity::Warning,
        1_700_000_000_000,
    )
    .expect("valid observation");
    let req = IpcRequest {
        service_id: "aether-system-core".to_string(),
        command: "agent.observe".to_string(),
        parameters: json!({ "observation": obs }),
        actor_trust: ActorTrust::Trusted,
    };
    let started = Instant::now();
    for _ in 0..ITERATIONS {
        let buf = serde_json::to_string(&req).expect("encode");
        let _: IpcRequest = serde_json::from_str(&buf).expect("decode");
    }
    let elapsed = started.elapsed().as_nanos();
    report("encode+decode", ITERATIONS, elapsed);
    // Reference Proposal so the linter doesn't complain.
    let _ = Proposal::new(
        "prop-bench",
        TaskKind::ProposeCleanup,
        "free up space",
        "delete cached files",
        "disk is full",
        ProposalRisk::Medium,
        1_700_000_000_000,
    );
}

fn main() {
    println!("aether-bench: ITERATIONS = {ITERATIONS}");
    bench_audit_chain();
    bench_seal_unseal();
    bench_fingerprint_sha256();
    bench_pairing_validate();
    bench_device_registry();
    bench_ipc_round_trip();
    println!("\naether-bench: OK");
}
