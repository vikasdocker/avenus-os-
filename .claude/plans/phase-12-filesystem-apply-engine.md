# Phase 12.7 — `FilesystemApplyEngine` (in-memory, deterministic)

## Context

`aether-update-agent` ships an `ApplyEngine` trait that the runtime
plugs in. The `NullApplyEngine` returns `Ok(())` for every step and
is used for tests. There is no second engine yet — the live
"filesystem" backend (the one that the future `aether-update-agent`
daemon will use to write to `/var/lib/aether/...`, swap symlinks,
and trigger the supervisor) is still out of scope.

This change adds a **second engine** in the same crate:
`FilesystemApplyEngine`. It operates on an in-memory model of the
filesystem (`BTreeMap<PathKey, Vec<u8>>`) so the contract is fully
tested without touching real disk. The real disk-backed engine
("`RealFsApplyEngine`") can be added later, mechanically, by
swapping the storage type.

The engine demonstrates the five-step pipeline end-to-end with
real audit-shaped outputs:

1. **Download** — copies the `payload` (a `&[u8]` the caller hands
   in) into the staging area.
2. **Verify** — recomputes a SHA-256 over the staged bytes and
   compares it to the `expected_sha256` the caller registered.
3. **Stage** — renames `staging/<filename>` to
   `staging/<filename>.staged` (the marker the future atomic-swap
   will look for).
4. **Snapshot** — records every entry under `snapshot/...` as a
   `SnapshotComponent` in the engine's snapshot log.
5. **Apply** — atomically swaps `active/<filename>` for
   `snapshot/<filename>` (writes the new bytes, never deletes
   first).
6. **Reboot** — records the requested reboot and returns `Ok`.

The engine returns a typed `ApplyError` on any failure path; the
agent's retry / rollback layer already handles those.

## Files to modify

- **`system/aether-update-agent/src/lib.rs`** — add the new
  `FilesystemApplyEngine` struct, its public constructor, its
  internal helpers, and the impl of `ApplyEngine`. Add a public
  `FilesystemApplyError` enum (or fold it into `ApplyError` if it
  fits cleanly). Add unit tests.

## Reused types

- `ApplyStep` (same crate) — the step the engine is asked to run.
- `UpdatePlan` (aether-update-core) — for the target / kind /
  version metadata.
- `UpdateKind` (aether-security::signed_update) — for the
  `os-image` / `service-bundle` / `agent-model` distinction.
- `aether_security::hash::sha256` (or the in-crate `hasher`
  helper if one exists) for the verify step.
- `aether_retry_policy::BackoffStrategy` (already a dep) — no
  change.

## Engine shape

```rust
pub struct FilesystemApplyEngine {
    fs: BTreeMap<String, Vec<u8>>,           // path -> bytes
    payloads: BTreeMap<String, Vec<u8>>,     // plan_id -> payload
    expected_sha256: BTreeMap<String, [u8; 32]>, // plan_id -> hash
    snapshot: Vec<SnapshotComponent>,        // post-snapshot log
    audit: Vec<EngineAudit>,                 // engine-internal log
}
```

Helpers:

- `register_payload(plan_id, payload, expected_sha256)` — the
  caller hands the engine the bytes + expected hash before
  `run()` is called.
- `filesystem(&self) -> &BTreeMap<String, Vec<u8>>` — read-only
  view of the simulated FS for tests.
- `snapshot(&self) -> &[SnapshotComponent]` — the post-snapshot
  log.
- `audit(&self) -> &[EngineAudit]` — the engine-internal log.

`run()` dispatches on the `ApplyStep`:

- `Download` — `fs.insert("staging/<plan_id>.bin", payload)`.
- `Verify` — SHA-256 of `staging/<plan_id>.bin` must equal
  `expected_sha256`. Mismatch → `Refused { reason: "bad hash" }`.
- `Stage` — rename `staging/<plan_id>.bin` to
  `staging/<plan_id>.staged`.
- `Snapshot` — for every entry under `active/...` (the engine
  knows the plan's `target`), record a `SnapshotComponent { target,
  from_version: "0.1.0", stash_path, note: None }`.
- `Apply` — for the same entries, write the staged bytes into
  `active/...` (atomic: build new, then swap map entry).
- `Reboot` — record an audit entry; return `Ok`.

Tests (~10):

- download_writes_to_staging
- verify_rejects_hash_mismatch
- verify_accepts_matching_hash
- stage_marks_file_staged
- snapshot_records_components
- apply_swaps_active_bytes
- apply_after_download_full_pipeline (Download → Verify →
  Stage → Snapshot → Apply)
- run_returns_wrong_stage_for_step (every step in Idle stage
  fails with `WrongStage` — this is enforced by the agent, not
  the engine, so this test stays in the agent module)
- engine_records_audit
- reboot_is_a_no_op
- empty_payload_rejected_with_refused

## Verification

1. `cargo test -p aether-update-agent` — all old + new tests
   pass.
2. `cargo clippy -p aether-update-agent --all-targets` — clean.
3. `cargo test --workspace` — total count goes from 1,785 to
   ~1,795.
4. Commit on the in-progress branch.

## Out of scope (deferred to a future milestone)

- A real disk-backed `RealFsApplyEngine` (Phase 10/15).
- The `aether-update-agentd` binary that drives the agent from a
  supervisor IPC.
- Atomic cross-filesystem rename (the simulated engine uses an
  in-memory map; the real one will use `rename(2)` + `syncfs(2)`).
- Cancellation of an in-flight step.
