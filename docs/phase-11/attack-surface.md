# Phase 11 — Attack Surface & Defense-in-Depth

Last reviewed 2026-09-01. This document maps every attack vector
Aether OS faces and the layered defenses that mitigate it.

## Attack Surface Summary

| # | Attack Vector | Primary Defense | Secondary Defense | Test Coverage |
|---|---------------|-----------------|-------------------|---------------|
| 1 | Command injection | Structured IPC (no shell eval) | Policy gate | 7 dispatch_policy_tests |
| 2 | Privilege escalation | Capability + risk gating | Untrusted actor denial | 23 defense_in_depth_tests |
| 3 | Prompt injection | AI defense sanitization | Action ceiling enforcement | 27 aether-ai-defense tests |
| 4 | Credential theft | AES-256-GCM sealed store | Process-lifetime keys | 17 credential tests |
| 5 | Audit tampering | SHA-256 hash chain | Index + link verification | 17 audit chain tests |
| 6 | Manifest tampering | Ed25519 signature | Trust store gating | 13 manifest signing tests |
| 7 | Update tampering | Ed25519 signature | Version policy | 15 signed update tests |
| 8 | Boot integrity | Boot measurement chain | Audit chain binding | 18 boot measure tests |
| 9 | Cross-device abuse | Pairing state machine | Grant-gated delivery | 36 device core tests |
| 10 | Sandbox escape | Declarative sandbox plan | Linux enforcement binary | 9 sandbox tests |

## Detailed Attack Vectors

### 1. Command Injection

**Vector:** An attacker crafts input that causes Aether to execute
arbitrary shell commands.

**Defenses:**
- Layer 1: Commands are never parsed as shell text. The shell parser
  produces structured `CommandRequest` values; `system()` / `popen()`
  are never used for agent-controlled operations.
- Layer 2: The IPC layer validates command strings against a fixed
  whitelist via `command_to_capability()`. Unknown commands return
  `None` and are handled as internal dispatch.
- Layer 3: The policy gate evaluates every command against
  `DefaultPermissionPolicy` before execution.

**Evidence:** `system/aether-system-core/src/policy.rs` (8 unit
tests), `agent/aether-agent-runtime/src/validator.rs` (26 tests
including shell-execution blocking).

### 2. Privilege Escalation

**Vector:** An untrusted actor or low-privilege service attempts
high-risk operations (delete, shutdown, credential access).

**Defenses:**
- Layer 1: `ActorTrust::Untrusted` is denied outright before the
  policy is consulted (`policy.rs:96`).
- Layer 2: High-risk capabilities (`file.delete`, `system.shutdown`,
  `credential.seal`, `policy.reload`) return `RequireConsent` for
  trusted actors.
- Layer 3: The agent runtime validator gates `ActionRisk::High |
  Critical` actions into `pending_approvals` for explicit user
  consent.
- Layer 4: The agentd's `ConfirmationPolicy` independently gates
  high-risk actions.

**Evidence:** `system/aether-system-core/src/policy.rs`,
`agent/aether-agent-runtime/src/validator.rs` (270 tests),
`services/aether-agentd/src/confirmation.rs`.

### 3. Prompt Injection

**Vector:** Malicious content (web pages, peer messages, file
contents) attempts to manipulate the AI agent into executing
unauthorized actions.

**Defenses:**
- Layer 1: `SanitizationPolicy` strips injection patterns, hides
  tool-call syntax, redacts system prompt markers, and truncates
  long content.
- Layer 2: `ActionCeiling` enforces per-actor verb allowlists and
  maximum risk levels.
- Layer 3: `ActionValidator` checks proposed actions against the
  ceiling and revocation set.
- Layer 4: The IPC policy gate independently evaluates the action.

**Evidence:** `security/aether-ai-defense/src/lib.rs` (27 tests).

### 4. Credential Theft

**Vector:** An attacker attempts to read sealed credentials or
extract the sealing key.

**Defenses:**
- Layer 1: AES-256-GCM authenticated encryption with random nonces.
- Layer 2: Sealing key held in process memory only; never logged
  or serialized.
- Layer 3: `Secret<T>` zeroizes on drop; `into_inner()` consumes
  the secret.
- Layer 4: `RandomKeyProvider` produces non-round-trippable keys
  (sealed data is unrecoverable after restart).
- Layer 5: Duplicate seal rejected unless `force` flag is set.

**Evidence:** `security/aether-security/src/credentials.rs` (17
tests including tamper detection, wrong-key rejection, nonce
uniqueness).

### 5. Audit Tampering

**Vector:** An attacker modifies audit log entries to hide
malicious activity.

**Defenses:**
- Layer 1: Each entry's `content_hash` is SHA-256 over
  `prev_hash || timestamp || event || component || detail`.
- Layer 2: `verify_chain()` recomputes hashes in order and rejects
  content mutation, broken links, and index gaps.
- Layer 3: Retention is bounded by `max_entries` and `max_age_ms`.
- Layer 4: The chain is append-only; entries cannot be reordered
  without breaking the hash chain.

**Evidence:** `security/aether-security/src/audit.rs` (17 tests),
`system/aether-system-core/src/main.rs` (5 audit_chain_tests).

### 6. Manifest Tampering

**Vector:** An attacker modifies a service manifest to gain
unauthorized capabilities.

**Defenses:**
- Layer 1: Ed25519 signature over canonical manifest bytes.
- Layer 2: `TrustStore` gates verification to known signers only.
- Layer 3: Tampered manifests, swapped signatures, and untrusted
  signers are all rejected.

**Evidence:** `security/aether-security/src/manifest_signing.rs`
(13 tests), `system/aether-system-core/src/loader.rs` (5
trust-aware loader tests).

### 7. Update Tampering

**Vector:** A malicious update package is presented to the system.

**Defenses:**
- Layer 1: `SignedUpdate` envelope with Ed25519 signature.
- Layer 2: `UpdateTrustList` pins to known signers + fingerprint
  list.
- Layer 3: `VersionPolicy` blocks downgrades and unauthorized
  reinstalls.
- Layer 4: The IPC policy gate requires trusted actor + consent
  for `update.plan`.

**Evidence:** `security/aether-security/src/signed_update.rs`
(15 tests), `system/aether-update-core/src/plan.rs` (32 tests).

### 8. Boot Integrity

**Vector:** An attacker modifies boot artifacts (kernel cmdline,
initramfs, kernel modules) to compromise the system before
userspace starts.

**Defenses:**
- Layer 1: `BootMeasurementChain` records each artifact with
  SHA-256 content hashing.
- Layer 2: `verify_chain()` detects content tampering, broken
  links, and index gaps.
- Layer 3: `BootComplete` marker binds the boot chain to the
  runtime audit chain via genesis hash.
- Layer 4: `kernel_cmdline_digest` canonicalizes argument order
  to prevent reordering attacks.

**Evidence:** `security/aether-security/src/boot_measure.rs` (18
tests).

### 9. Cross-Device Abuse

**Vector:** An unpaired or malicious device attempts to deliver
observations, proposals, or tasks.

**Defenses:**
- Layer 1: `PairingState` must be `Paired` for any delivery.
- Layer 2: Fingerprint must match the registry entry.
- Layer 3: `PairingGrant` gates specific operations
  (`receive_observations`, `receive_proposals`,
  `execute_remote_tasks`).
- Layer 4: Monotonic `seq` counter prevents replay.
- Layer 5: Timestamp skew window prevents stale deliveries.

**Evidence:** `agent/aether-device-core/src/remote.rs` (36 unit
tests), `agent/aether-device-core/src/pairing.rs` tests.

### 10. Sandbox Escape

**Vector:** A sandboxed service attempts to escape its
constraints (namespace, capabilities, seccomp).

**Defenses:**
- Layer 1: Declarative `SandboxPlan` per profile
  (Internal / SystemService / RestrictedService).
- Layer 2: `aether-sandbox` binary applies primitives in
  deterministic order: `prctl(NO_NEW_PRIVS)` → `unshare(CLONE_NEW*)`
  → cgroup v2 write → `capset()` → `execvp()`.
- Layer 3: Forbidden capabilities (`sys_admin`, `sys_module`,
  `sys_rawio`) are never granted.
- Layer 4: The policy audit layer records applied/skipped/failed
  primitives.

**Evidence:** `core/aether-core/src/sandbox.rs` (9 tests),
`system/aether-sandbox/src/main.rs` (10 tests),
`system/aether-sandbox/src/linux.rs` (3 tests),
`system/aether-sandbox-policy/src/lib.rs` (16 tests).

## Defense Layers (Cross-Cutting)

```
Layer 1: Input Validation        (Syntactic security)
   ↓
Layer 2: Session & Identity      (Authentication)
   ↓
Layer 3: Capability Verification (Authorization)
   ↓
Layer 4: Policy & Confirmation   (Policy enforcement)
   ↓
Layer 5: Audit Logging           (Accountability)
```

Every IPC request passes through all five layers. The policy gate
(`Layer 4`) evaluates the request against `DefaultPermissionPolicy`
combined with `ActorTrust` before any capability handler runs.
Denials are recorded in the tamper-evident audit chain (`Layer 5`).

## Test Inventory

| Category | Unit Tests | Integration Tests | Total |
|----------|-----------|-------------------|-------|
| Policy gate | 8 | 7 | 15 |
| Audit chain | 17 | 5 | 22 |
| Credentials | 17 | 8 | 25 |
| Manifest signing | 13 | 5 | 18 |
| Signed updates | 15 | 9 | 24 |
| Boot measurement | 18 | 0 | 18 |
| AI defense | 27 | 0 | 27 |
| Sandbox | 9 | 0 | 9 |
| Device core | 36 | 8 | 44 |
| Defense-in-depth | 0 | 23 | 23 |
| **Total** | **160** | **65** | **225** |

## Known Limitations

1. **No real seccomp-BPF enforcement** — the sandbox plan is
   declarative; actual syscall filtering requires Linux kernel
   support (Phase 10).
2. **No MAC policy** — SELinux/AppArmor integration is deferred.
3. **Loopback-only IPC** — no mutual TLS or authentication at the
   socket layer; security relies on OS-level network isolation.
4. **Process-lifetime keys** — sealing keys are not persisted across
   restarts with `RandomKeyProvider`; `StaticKeyProvider` is for
   testing only.
5. **No active network probing** — `network.connectivity` is derived
   from interface state, not active reachability testing.
