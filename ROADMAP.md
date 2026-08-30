# Aether OS — Master Roadmap

> **Authority:** This document is the PRIMARY DEVELOPMENT AUTHORITY for the Aether
> OS project. Every phase, milestone, and acceptance criterion below was reconstructed
> from the actual repository state on 2026-08-29 — not from prior roadmap text, prior
> conversation history, or aspirational design notes.
>
> **Rule:** Future agents and contributors **MUST** read this file first, identify the
> current phase and current milestone, and then work only on that milestone. Phases
> must be executed in numeric order. A phase is only `COMPLETE` when its acceptance
> criteria are met in the repository and validated by build / tests / runtime evidence.

---

## Table of Contents

1. [Project Vision](#1-project-vision)
2. [Long-Term Architecture](#2-long-term-architecture)
3. [Critical Agent Principle](#3-critical-agent-principle)
4. [AI-First Operating System](#4-ai-first-operating-system)
5. [Core Development Principles](#5-core-development-principles)
6. [Phase Status Legend](#6-phase-status-legend)
7. [Current State Snapshot](#7-current-state-snapshot)
8. [Phases 0 → 15](#8-phases)
   - [Phase 0 — Project Foundation](#phase-0--project-foundation) **COMPLETE**
   - [Phase 1 — Core Operating System](#phase-1--core-operating-system) **IN_PROGRESS (parts done, parts partial)**
   - [Phase 2 — Aether Agent Core](#phase-2--aether-agent-core) **IN_PROGRESS**
   - [Phase 3 — Conversational Aether](#phase-3--conversational-aether) **IN_PROGRESS (3.1–3.4 shipped)**
   - [Phase 4 — Voice + Audio](#phase-4--voice--audio) **NOT_STARTED**
   - [Phase 5 — Vision + Computer Understanding](#phase-5--vision--computer-understanding) **NOT_STARTED**
   - [Phase 6 — Aether UI / UX](#phase-6--aether-ui--ux) **IN_PROGRESS (foundation only)**
   - [Phase 7 — Aether Agent Deep System Control](#phase-7--aether-agent-deep-system-control) **NOT_STARTED**
   - [Phase 8 — Device + Hardware Ecosystem](#phase-8--device--hardware-ecosystem) **NOT_STARTED**
   - [Phase 9 — Application Platform](#phase-9--application-platform) **PARTIAL**
   - [Phase 10 — Real Hardware Bring-up](#phase-10--real-hardware-bring-up) **NOT_STARTED**
   - [Phase 11 — Security + Trusted AI](#phase-11--security--trusted-ai) **PARTIAL**
   - [Phase 12 — Self-Updating + System Lifecycle](#phase-12--self-updating--system-lifecycle) **NOT_STARTED**
   - [Phase 13 — Aether Autonomous OS](#phase-13--aether-autonomous-operating-system) **NOT_STARTED**
   - [Phase 14 — Multi-Device Aether](#phase-14--multi-device-aether) **PARTIAL**
   - [Phase 15 — Production Release](#phase-15--production-release) **PARTIAL**
9. [Global Agent Development Rules](#9-global-agent-development-rules)
10. [Phase Execution Protocol](#10-phase-execution-protocol)
11. [Roadmap Governance](#11-roadmap-governance)
12. [Aether UI / UX Design Direction](#12-aether-ui--ux-design-direction)
13. [Release Quality Gates](#13-release-quality-gates)

---

## 1. Project Vision

Aether OS is a Linux-based operating system whose primary interface and intelligence
are provided by an integrated Aether Agent.

Aether is **not**:

- "Linux + chatbot"
- "an AI application running on Linux"

Aether is an **AI-native operating system**: the AI control plane is a first-class
system component, not an installed application. The agent is the primary means by
which the user — and the system itself — observes, plans, and acts.

The long-term architecture is:

```text
HUMAN
  ↓
AETHER OS
  ↓
AETHER AGENT
  ↓
REASONING / PLANNING / MEMORY
  ↓
AETHER SYSTEM CONTROL PLANE (capability + policy + audit)
  ↓
AETHER SERVICES (filesystem, storage, process, application, network, window, graphics, audio, device)
  ↓
LINUX KERNEL
  ↓
HARDWARE
```

The operating system itself should behave as an agent. The agent must be capable of
understanding the system from:

- boot, kernel, services, processes, applications
- filesystem, storage, network, graphics, audio, devices
- hardware, power, security, logs, system state
- user context

---

## 2. Long-Term Architecture

Aether is structured as a stack of trusted layers. The agent never reaches the
kernel directly; it always goes through a structured system control plane.

```text
Aether Agent (text / voice / UI / tap-to-talk)
    ↓
Intent ─→ Plan ─→ Structured Action
    ↓
Validator ─→ Capability ─→ Policy ─→ Approval
    ↓
Aether IPC (typed, schema-validated, audit-logged)
    ↓
Privileged Aether Services
    ↓
Linux Kernel / Hardware
```

**Design invariants (enforced across every phase):**

1. **No arbitrary shell execution.** Privileged operations always go through typed
   Aether actions resolved by the system control plane.
2. **No silent privilege.** Every privileged action declares its required capabilities
   and risk level at definition time.
3. **Every action is auditable.** Authorization decisions and outcomes are written
   to the audit log.
4. **Every action is cancellable and time-bounded.** The agent never holds
   unbounded autonomy.
5. **The agent is privileged but structured.** It is a trusted OS component — not
   an untrusted external chatbot.
6. **One design system, one identity.** Every visible surface belongs to Aether.

---

## 3. Critical Agent Principle

The Aether Agent is the **primary intelligence and control layer** of the OS. It will
have system-wide authority sufficient to operate the entire OS.

**However:** "Full OS access" does **NOT** mean bypassing architecture. Authority
must be implemented through the Aether trusted system-control plane.

```text
Aether Agent
    ↓
Intent
    ↓
Planning
    ↓
Structured Actions
    ↓
Capability / Policy
    ↓
Aether IPC
    ↓
Privileged Aether Services
    ↓
Linux Kernel / Hardware
```

The agent must:

- be capable of performing system-wide operations when authorized by Aether system
  policy;
- **not** depend on arbitrary shell execution as its primary control mechanism;
- **not** be treated as an untrusted external chatbot.

The agent is a privileged OS component, but its actions must be **structured,
auditable, cancellable, and policy-aware** at all times.

---

## 4. AI-First Operating System

The final OS supports natural interaction through:

- text
- voice
- tap-to-talk
- desktop UI
- application UI
- future multimodal interaction

Users will eventually be able to say:

- "Open my project."
- "Check why my system is slow."
- "Connect my headphones."
- "Open the browser and go to my project."
- "Find the file I edited yesterday."
- "Install this application."
- "Close the application using too much memory."
- "Set up my development environment."
- "Restart the network."
- "Prepare the system for development."

The agent must understand intent, plan actions, execute them through Aether
services, observe results, and recover from failures.

---

## 5. Core Development Principles

1. Build a real operating system.
2. Prefer real implementations over demos.
3. Prefer native Linux integration over fake abstractions.
4. Keep the system modular.
5. Use strong security boundaries.
6. Use structured APIs.
7. **Avoid arbitrary shell execution.**
8. Validate every phase.
9. Never silently skip failures.
10. Keep QEMU as the reference development platform.
11. Progress toward real hardware.
12. Keep the AI agent deeply integrated with the OS.
13. Keep the graphical experience completely Aether-owned.
14. Preserve backwards compatibility where practical.
15. Never mark unfinished functionality as complete.

---

## 6. Phase Status Legend

Each phase and milestone uses exactly one of:

| Status         | Meaning                                                                              |
| -------------- | ------------------------------------------------------------------------------------ |
| `NOT_STARTED`  | No implementation or design evidence exists yet.                                    |
| `IN_PROGRESS`  | Implementation exists but acceptance criteria are not fully met.                     |
| `PARTIAL`      | Some sub-milestones complete; others `NOT_STARTED` or `IN_PROGRESS`.                 |
| `BLOCKED`      | Cannot progress without external dependency, missing information, or failed build.   |
| `COMPLETE`     | Acceptance criteria verified by tests, build, and (where applicable) runtime / QEMU. |

A phase is `COMPLETE` only when every sub-milestone's acceptance criteria are
verified by concrete evidence in the repository.

---

## 7. Current State Snapshot

| Layer                            | Repository Evidence (2026-08-29)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Build                            | `cargo check --workspace` PASS; 22 Rust crates in `Cargo.toml`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Tests                            | `cargo test --workspace` — **256 passed, 0 failed, 1 ignored** (all Rust crates; 7 dispatch-policy tests in 11.3, 9 sandbox-plan tests + 5 manager-level tests in 11.4, 179 tests in `aether-agentd` including the runtime e2e test).                                                                                                                                                                                                                                                                                                                                                                                |
| Lints                            | Workspace `clippy::all = deny`, `unwrap_used = deny`, `expect_used = deny`, `unsafe_code = forbid`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Boot                             | Buildroot 2025.02 + Linux 6.12 QEMU image builds; initramfs/ISO pipeline present. Smoke test in `tests/boot/test_qemu_boot.py` gated by `AETHER_BOOT_TEST=1`.                                                                                                                                                                                                                                                                                                                                                                                                            |
| Init                             | `system/aether-init` — boot stages, kernel-param parser, shutdown plan. Unit-tested.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| System Core                      | `system/aether-system-core` — manifest loader, dependency graph (cycle + missing detection), `ServiceManager` (start/stop/restart/supervise), TCP loopback control plane (port 4747). Audits every capability request.                                                                                                                                                                                                                                                                                                                                                  |
| Storage                          | `storage/aether-storage` — sandboxed `FileManager` (workspace-rooted, traversal-safe, symlink-safe, extension allowlist, size cap), `system_info` (mounts/disk/CPU/mem/net).                                                                                                                                                                                                                                                                                                                                                                                                |
| Process                          | `system/aether-process-manager` — discovery, lifecycle, inspection, security.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Application                      | `system/aether-application-manager` — registry, single-instance launch, lifecycle. `apps/calculator` and `apps/notes` exist as real graphical Aether apps.                                                                                                                                                                                                                                                                                                                                                                                                              |
| Network                          | TCP control plane exposes `network.status`, `network.interfaces`, `network.connectivity`, `network.config`, `network.events`. No dedicated long-running `aether-network` service crate yet (`src/` empty).                                                                                                                                                                                                                                                                                                                                                              |
| Shell                            | `shell/aether-shell` (`aethersh`) — 35 commands across filesystem/process/application/network/system; session, history, JSON output. Wired through Aether IPC.                                                                                                                                                                                                                                                                                                                                                                                                            |
| Graphics                         | `graphics/libaether-graphics` — display, renderer, input, window, cursor, output, compositor, session, workspace, desktop-shell primitives, security, IPC. `graphics/aether-wm` — window state machine. `graphics/aether-graphical-shell` — framebuffer desktop shell: header, taskbar, multi-window chrome, application launcher, system panel (network/storage), agent chat strip.                                                                                                                       |
| Agent runtime (library)          | `agent/aether-agent-runtime` — Session, Request, Intent, Action, Tool, Validator, Executor (IPC-only), Observation, Planner, Approval, Cancellation, Memory, LLM (Mock + Echo providers), Audit, Events, Errors. **Library is embedded in `aether-agentd` via `runtime_host::RuntimeBridge`**: `agent.session.*`, `agent.intent`, `agent.approval.*`, `agent.audit.*`, `agent.action.cancel`, `agent.stop` all delegate to the runtime host. End-to-end test `e2e_open_test_application_through_runtime` exercises the full intent → plan → action → observation → audit pipeline against a mock control plane.                                                                                                                                                                                                                                                                            |
| Agent daemon                     | `services/aether-agentd` — bounded event ring, task state, conversation context, intent classifier, planner, confirmation, ndjson TCP (`4748`) and stdio. EchoProvider default; provider is replaceable.                                                                                                                                                                                                                                                                                                                                                                |
| LLM provider                     | `LlmProvider` trait with `MockLlmProvider` and `EchoLlmProvider`. **Ollama backend lives in the repo** (`aether_agent_runtime::llm_provider::OllamaLlmProvider` and the daemon's `OllamaProvider`); selected via `AETHER_AI_PROVIDER=ollama` (or `runtime-ollama` for the runtime-backed path). No cloud provider (OpenAI/etc.) yet.                                                                                                                                                                                                                                                                                                          |
| Voice                            | `voice/aether-voice` — empty stub (`lib.rs` only, ~57 lines, no STT/TTS/wake-word).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Vision                           | `vision/aether-vision` — `src/` directory empty. No implementation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| SDK                              | `sdk/rust/aether-sdk` — TCP control-plane client. `sdk/python/aether_sdk` — wire-protocol helpers (`AETHER/1`).                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Tooling                          | `tools/aetherctl` — CLI control client. `tools/aether-process-manager` (Cargo listed) and related tooling.                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Security                         | `core/aether-core` Capability + RiskLevel types. `security/aether-security` `DefaultPermissionPolicy` (allow / require-consent / deny). Manifests declare `sandbox_profile`, `permission_profile`, `ipc_access`, `capabilities`, `resource_*`. System core audits every capability request. **Phase 11.3 (system-core policy gate) is live**: every IPC request is evaluated against the policy + `ActorTrust` before any capability runs; untrusted actors are denied outright, high-risk capabilities return `REQUIRES_CONFIRMATION`. **Phase 11.4 (declarative sandbox plan) is live**: `core/aether-core/src/sandbox.rs` emits a typed `SandboxPlan` per profile; `sandbox.plan` IPC returns it. **No actual kernel sandboxing (cgroups/seccomp/namespaces) is enforced yet — that is honest text in the existing code, not a lie; the enforcement binary `aether-sandbox` is the next concrete deliverable.**                                                                                                                                                                                                                   |
| Documentation                    | `docs/development/*` (16 files), `docs/architecture/*` (10), `docs/security/*` (2), `docs/testing/*` (1), `docs/build/*` (2), `docs/phase-1-8/*` (10).                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Tests                            | `tests/boot/*`, `tests/integration/*`, `tests/python/*`, `tests/repository/*`, `tests/smoke/*` — Python harness wired via `scripts/test.sh`. Rust integration tests live inside each crate's `tests/` directory (most are currently empty).                                                                                                                                                                                                                                                                                                                              |

**Current phase:** **Phase 1** (Core Operating System) — Parts A, B, C, F, H are
substantially complete. Part D (security hardening) is partial. Part E (filesystem/
storage) is complete. Part G (network) is partial (control-plane commands only, no
dedicated service). Part I (graphical OS) is in progress — software-framebuffer
multi-window desktop is working in QEMU; native DRM/KMS backend not yet implemented.

**Next milestone:** **Phase 1.4 / 1.9 closure** — close the remaining Phase 1
sub-milestones. Phase 1.7 `aether-network` is complete and Phase 2.1
(agent-runtime embedded in `aether-agentd`) is complete and exercised
end-to-end by `e2e_open_test_application_through_runtime`. The open
work is the security/capability hardening under Phase 1.4 and the
graphical OS native backend under Phase 1.9 / Phase 6.

---

## 8. Phases

---

### Phase 0 — Project Foundation

**Status:** `COMPLETE` (2026-08-10 / commit `4cb638a` and earlier; reaffirmed by
present `cargo test` run).

**Objective:** Establish the engineering foundation.

**Major components delivered:**

- Repository structure (Rust workspace, Python packages, CMake, shell, graphics, services, system, infra).
- Rust workspace (`Cargo.toml`) with shared strict lints.
- CMake native build for C system utilities.
- Python AI brain package (`brain/aether_brain`) and Python SDK (`sdk/python/aether_sdk`).
- Qt6/QML shell source assets (`ui/shell`).
- Linux kernel config seed and `kernel/scripts/build-linux.sh`.
- Initramfs + ISO creation pipeline (`scripts/iso/build-initramfs.sh`, `scripts/iso/build-iso.sh`).
- QEMU, VirtualBox, VMware launchers.
- Docker dev environment + devcontainer config.
- CI workflows, lint, build, release automation, issue + PR templates.
- Developer documentation, security policy, repository standards.

**Dependencies:** none.

**Milestones:**

| ID    | Milestone               | Status     | Evidence                                                  |
| ----- | ----------------------- | ---------- | --------------------------------------------------------- |
| 0.1   | Repository scaffolding  | COMPLETE   | Full directory tree present.                              |
| 0.2   | Rust + CMake + Python   | COMPLETE   | Workspace builds, all crates compile.                     |
| 0.3   | QEMU development path   | COMPLETE   | `scripts/run/qemu*.sh`, `tests/boot/test_qemu_boot.py`.   |
| 0.4   | First bootable baseline | COMPLETE   | Buildroot + initramfs + kernel boot in QEMU.              |
| 0.5   | Local system control    | COMPLETE   | `aetherctl`, control-plane commands.                      |
| 0.6   | AI brain integration    | COMPLETE   | `brain/aether_brain`, Python tests passing.               |
| 0.7   | Shell + compositor prep | COMPLETE   | `aethersh` 35 commands, graphics primitives.              |
| 0.8   | Enterprise hardening    | PARTIAL    | Manifest validation, IPC limits, audit, policy.           |

**Acceptance criteria:**

- [x] `cargo check --workspace` exits 0.
- [x] `cargo test --workspace` shows 0 failed (current run: 242/242).
- [x] `scripts/build.sh`, `scripts/test.sh`, `scripts/lint.sh`, `scripts/format.sh`
  all exist and are executable.
- [x] `scripts/iso/build-initramfs.sh` produces an initramfs image in `build/`.
- [x] `scripts/run/qemu.sh` boots the Buildroot image in QEMU.

**Validation requirements:** build, cargo test, QEMU smoke test (gated).

**Security requirements:** workspace `unsafe_code = forbid`, `unwrap_used = deny`,
`expect_used = deny`, `clippy::all = deny` (in place; must remain).

**Performance requirements:** none beyond "compiles + tests in reasonable time."

**Known limitations:** CI uses host Cargo; Buildroot image only on Linux/WSL
binfmt-enabled hosts.

**Next phase unlock:** Phase 1.

---

### Phase 1 — Core Operating System

**Status:** `IN_PROGRESS` (parts complete, parts partial, one blocked by host).

**Objective:** Build the actual Aether OS foundation: kernel, init, system core,
security model, filesystem, storage, process/application management, network,
shell, and graphical subsystem.

**Architecture overview:**

```text
Aether Init (PID 1) ── boots → System Core ── orchestrates → Services / Apps
                                              │                 │
                                              │ manifests       │ IPC (port 4747)
                                              │ graphs          │
                                              │ audit           ▼
                                              └─────────── Aether services
                                                           (storage, process,
                                                            application, …)

Aether Shell (aethersh) ── uses ──▶ IPC ──▶ System Core
Aetherctl ────────────── uses ──▶ IPC ──▶ System Core
Aether Agentd ────────── uses ──▶ IPC ──▶ System Core + Surface Server
Aether Graphical Shell ─ uses ──▶ IPC ──▶ System Core + Surface Server
```

#### 1.1 Linux + Buildroot

**Status:** `COMPLETE`.

- Linux kernel 6.12.103 for x86_64 QEMU (`infra/buildroot/external/` + kernel config).
- Buildroot 2025.02.16 LTS integrated via `BR2_EXTERNAL`.
- Initramfs + rootfs + ISO pipelines.
- QEMU reference platform boots to console session.
- Boot smoke test (`tests/boot/test_qemu_boot.py`) verifies kernel, init, service,
  shell, network, shutdown.

**Acceptance:** QEMU boots the Buildroot image and reaches an Aether console;
`scripts/test-boot.sh` passes when `AETHER_BOOT_TEST=1`.

#### 1.2 Aether Init

**Status:** `COMPLETE`.

- `system/aether-init` — boot stage state machine (`EarlyMounts → KernelParams → Services → Ready → Shutdown`).
- `aether=` kernel-parameter parser (`quiet`, `single`, `port`, `manifests`).
- Ordered shutdown plan.
- Console session on `/dev/console`.
- Unit-tested (`system/aether-init/src/lib.rs`).

**Acceptance:** unit tests pass; QEMU smoke confirms init stages log on serial.

#### 1.3 Aether System Core

**Status:** `COMPLETE`.

- `system/aether-system-core`:
  - Manifest loader (`loader.rs`)
  - Dependency graph with cycle + missing detection (`graph.rs`)
  - `ServiceManager` with start/stop/restart/supervise (`manager.rs`)
  - TCP JSON control plane on `127.0.0.1:4747` (`main.rs`)
- `system/services.d/*.json` — manifests for `aether-system-core`, `aether-agentd`, `aether-application-manager`.
- `aetherctl` over IPC supports `status`, `start`, `stop`, `restart`, `shutdown`.

**Acceptance:** all status/restart/shutdown commands succeed in QEMU;
manifest validation rejects cycles and missing deps (tested in
`tests/integration/test_system_core_contract.py`).

#### 1.4 Security Hardening

**Status:** `PARTIAL` (capability / policy / audit present; no kernel sandboxing).

**Completed:**

- Manifest security fields: `security_identity`, `capabilities`, `sandbox_profile`,
  `permission_profile`, `ipc_access`, `resource_cpu_weight`, `resource_memory_max_kib`,
  `resource_io_weight`.
- `core/aether-core` `Capability` + `RiskLevel` types.
- `security/aether-security` `DefaultPermissionPolicy` with `Allow / RequireConsent / Deny`.
- System core audits every capability request (file content redacted).
- IPC request size limits (declarative in manifest + enforced in `core/aether-core`).
- Local-loopback socket only; `ipc_access` declares `LocalPrivate` / `LocalPublic` / `Remote`.

**Not yet implemented (must remain unclaimed):**

- Actual cgroups, namespaces, seccomp, Linux capabilities, MAC policy.
- Per-app sandboxing.
- Verified end-to-end isolation under hostile input.

**Acceptance:** capability declarations + audit + policy + manifest admission
checks pass integration tests; **do not** claim kernel-level sandboxing until
enforced in code.

#### 1.5 Filesystem + Storage

**Status:** `COMPLETE`.

- `storage/aether-storage`:
  - Sandboxed `FileManager` — workspace-rooted, traversal/symlink-safe, extension
    allowlist, 512 KiB read cap, structured errors (`AetherError`).
  - `system_info` — mount, disk, CPU, memory, network probes.
- `system/aether-system-core` exposes `file.list`, `file.read`, `file.create`,
  `file.write`, `file.search`, `file.rename`, `file.move`, `file.delete`,
  `storage.status`, etc. through IPC.

**Acceptance:** unit tests in `storage/aether-storage/src/lib.rs`; integration
with system core; `tests/integration/test_system_core_hardening_contract.py`
and `tests/boot/test_filesystem_runtime_security.py` exercise rejection of
out-of-workspace paths.

#### 1.6 Process + Application Management

**Status:** `COMPLETE` (process manager + application manager + first-class apps).

- `system/aether-process-manager` — discovery, lifecycle, inspection, security.
- `system/aether-application-manager` — registry, single-instance launch, app state
  queries, lifecycle.
- `apps/calculator`, `apps/notes` — real Aether graphical apps (framebuffer-painted).
- `apps/aether-surface` — surface registration helpers.
- `apps/aether-apps` — manifest contracts.
- IPC commands: `app.list`, `app.launch`, `app.close`, `app.status`,
  `process.list`, `process.inspect`.

**Acceptance:** launch + close + state round-trip; graphical apps paint in window
content area; `aetherctl` and shell `app.*` commands work.

**No raw shell execution API** is exposed; the application manager spawns argv
vectors directly.

#### 1.7 Network + Connectivity

**Status:** `COMPLETE` (dedicated `aether-network` service crate shipped; shell
`network` subcommands now backed by it).

**Completed (this milestone):**

- New crate **`aether-network`** at `network/aether-network/`
  (`Cargo.toml`, `src/lib.rs`, `src/manager.rs`, `src/proc.rs`, `src/main.rs`).
- Typed domain models: `Interface`, `InterfaceKind`, `InterfaceState`,
  `Address` (+ `AddressFamily`), `Route`, `DnsConfig`, `ConnectivityStatus`,
  `InterfaceStats`, `Event` (all serde-derivable).
- `NetworkManager` with a single cached snapshot served to all read queries
  (`status`, `interfaces`, `inspect`, `addresses`, `routes`, `dns`,
  `connectivity`, `stats`, `events`).
- `NetworkBackend` trait + `StubBackend` (deterministic seed for QEMU and
  tests) + `ProcBackend` (real `/proc/net/dev`, `/proc/net/route`,
  `/proc/net/if_inet6`, `/etc/resolv.conf`).
- Backend selector: env `AETHER_NET_BACKEND = stub|proc|auto`
  (default `auto` = proc on Linux when `/proc/net/dev` is readable,
  else stub).
- REPL daemon (`aether-network` binary) — newline-delimited JSON,
  one command per line, matching the `aether-application-manager` pattern.
- `aether-shell`'s `network` subcommands (status, interfaces, inspect,
  addresses, routes, dns, connectivity, stats) now call into
  `aether-network::NetworkManager` and return real, structured data
  instead of empty stubs. The `network.events` and `network.inspect`
  paths are also wired.

**Security properties preserved:**

- The crate is read-only: no `network.apply`, no DNS/DHCP/interface
  mutation, no `sh -c`/popen shortcuts. All queries go through a
  snapshot in the manager.
- Backend selection and `auto` fallback are explicit — no ambient
  filesystem probing; the procfs reader is the only thing that opens
  files, and it surfaces `NetworkError::Io(_)` on missing files
  rather than panicking.
- The shell still gates every `network` subcommand behind
  `Capability::Network.read` (via `required_capability`).

**Test coverage:** 55 unit tests in the new crate
(`aether-network` lib: 41, `aether-network` binary: 14) plus 14
shell-side wiring tests. All `cargo test --workspace` and
`cargo clippy --workspace --all-targets` pass.

**Acceptance:** the dedicated `aether-network` service crate exists,
powers the shell, and the Aether OS does not depend on any stub for
its network surface. Real DNS / DHCP / Wi-Fi / Bluetooth / VPN
control and per-application network scopes remain explicitly deferred
to the future.

#### 1.8 Aether Shell

**Status:** `COMPLETE` (per `PHASE_1_8_COMPLETION_REPORT.md`; 35 commands; 10/10 unit tests).

- `shell/aether-shell` (`aethersh`):
  - Command registry trait architecture.
  - 6 command modules: `system`, `filesystem`, `process`, `application`,
    `network`, `agent` (chat-through-agent).
  - Session, history (with secret filtering), JSON output.
  - Wired through Aether IPC, not raw syscalls.
- Documentation: `docs/phase-1-8/*` (10 files), `docs/architecture/aether-shell.md`,
  `docs/security/shell-security.md`, `docs/development/shell.md`.

**Acceptance:** `aethersh` runs as REPL, dispatches all 35 commands, secrets are
filtered, output is JSON-capable; unit tests pass.

#### 1.9 Graphical Operating System

**Status:** `IN_PROGRESS` (software framebuffer + WM + multi-window + agent chat
strip + application launcher; native DRM/KMS backend not yet implemented).

**Part A — DRM/KMS / GPU detection:** `BLOCKED / NOT_STARTED` (no native backend).
Software framebuffer works; virtio-gpu exposed in QEMU (commit `5ab0743`).

**Part B — Wayland runtime:** `PARTIAL` (surface server on TCP `:4750` with
framebuffer surface protocol; not Wayland-spec).

**Part C — Window Manager:** `COMPLETE` (`graphics/aether-wm` — `WindowManager`
state machine, workspaces, focus, move, resize, minimize, maximize, restore, close,
events).

**Part D — Aether Desktop Shell:** `IN_PROGRESS`:
- `graphics/aether-graphical-shell`:
  - Framebuffer (`fb.rs`): RGB fill, rect, text, primitives.
  - Input (`input.rs`): evdev keyboard + mouse decode.
  - Surface server (`surface_server.rs`): window list/register/close.
  - `main.rs`: header bar (clock, workspace indicators, status pills, NET/STOR),
    taskbar, workspace quick-switch, window taskbar buttons, active-window
    indicator, AI conversation strip, application launcher, system panel,
    cursor sprite.
- `aetherctl`-style commands available from `aethersh` (`window.*` group).

**Acceptance:** A real graphical Aether desktop boots under QEMU
(`scripts/run/qemu-window.sh`, `scripts/run/qemu-visual-check.sh`).
A window-manager–driven multi-window desktop is rendered from the framebuffer,
apps can be launched from the launcher, and `aetherctl window.*` works.

**Next milestone (Phase 1 close-out):**

- Real DRM/KMS backend (Part A).
- Wayland protocol implementation (Part B).
- Wire `aether-agent-runtime` into `aether-agentd` so agent intent → plan → action
  → observation runs end-to-end.

**Dependencies:** Phase 0. Phase 2 unblocks as soon as Agent Runtime is wired into
`aether-agentd`.

**Security requirements:** graphical session runs as a non-root service user
(declarative in manifest); framebuffer access gated by capability
(`graphics.framebuffer`); window events are typed and validated.

**Performance requirements:** the software framebuffer currently paints at
~30 FPS in QEMU at 1024×768 with a small window count; the DRM/KMS backend is
the path to sustained 60 FPS.

**Known limitations:**

- Software framebuffer (no GPU acceleration yet).
- No native Wayland protocol.
- Surface server uses a private TCP framing; not the Wayland wire protocol.
- DRM/KMS detection is for the kernel/driver layer, not the user-mode compositor.

---

### Phase 2 — Aether Agent Core

**Status:** `IN_PROGRESS` (all sub-phases 2.1–2.9 shipped; end-to-end
wiring inside `aether-agentd` is the open milestone).

**Objective:** Establish the actual OS agent.

#### 2.1 Agent Runtime

**Status:** `COMPLETE` (embedded in `aether-agentd` via `runtime_host::RuntimeBridge`).

- `agent/aether-agent-runtime` library:
  - `Session` + `SessionState` (Created/Ready/Thinking/Planning/WaitingApproval/
    Executing/Observing/Completed/Failed/Cancelled).
  - `Request` + `RequestActor`.
  - `Intent` + `IntentType` + `Confidence`.
  - `Plan` + `PlanStep` + `PlanId`.
  - `Action` + `ActionVariant` + `ActionId` + `ActionRisk`.
  - `ToolDefinition` + `ToolId` + `ToolRegistry` + `ToolRisk`.
  - `Validator` + `ValidationResult`.
  - `ActionExecutor` (TCP IPC, never raw commands).
  - `Observation` + `ObservationId` + `ObservationType`.
  - `ApprovalRequest` + `ApprovalDecision` + `ApprovalStatus`.
  - `CancellationToken`.
  - `Memory` (`ConversationMemory` + `SessionMemory`).
  - `LlmProvider` trait + `MockLlmProvider` + `EchoLlmProvider` + `OllamaLlmProvider`.
  - `Audit` + `Events` + `Errors`.

The runtime is embedded inside `aether-agentd` through
`services/aether-agentd/src/runtime_host.rs` (the `RuntimeBridge`). The
daemon's own `intent/context/planner/conversation/confirmation` modules
remain as the legacy/CLI-shaped surface for ndjson TCP port 4748; every
`agent.*` IPC command now delegates to the runtime host. See 2.4 for
the end-to-end test that proves intent → plan → action →
observation → audit round-trips through the embedded runtime.

#### 2.2 System Action Framework

**Status:** `IN_PROGRESS`.

`ActionVariant` currently covers Application, Window, Filesystem, Process,
Network, System, Storage, Context. The full long-term coverage is
`filesystem, storage, process, application, network, window, display, system,
device, power, security`. Device / Display / Power / Security actions are
**planned** but not yet defined.

Every action today: typed parameters, required capabilities, risk level,
timeout, reason. The framework supports the universal action model.

#### 2.3 Tool System

**Status:** `IN_PROGRESS`.

`ToolDefinition` + `ToolRegistry` exist in the runtime. Real tool bindings to
Aether services are partial; the executor currently inlines IPC calls. The
goal of Phase 2.3 is to factor IPC calls into registered tools with schemas,
risk classification, and timeouts.

#### 2.4 Agent ↔ OS Integration

**Status:** `COMPLETE`.

`aether-agentd` is now wired to the `aether-agent-runtime` library as the
single executor for AI-initiated actions. The full pipeline runs as:

```
User / Shell
  -> agentd TCP (port 4748)
  -> AgentRuntimeHost (lifecycle, identity, audit, event bus)
  -> ActionExecutor (intent -> plan -> action)
  -> Capability + Policy gate
  -> Aether IPC (typed JSON-RPC over loopback)
  -> aether-system-core / aether-application-manager / aether-network
  -> Observation -> Audit -> Event publication
```

**What shipped:**

- `aether-agent-runtime` exposes `AgentRuntimeHost`, the lifecycle state
  machine (Starting -> Ready -> Running -> Stopping -> Stopped), the
  `ActionExecutor`, the planner, the audit ring, the event bus, the
  session registry, the LLM boundary, the structured-intent parser, and
  the recovery policy.
- `aether-agentd` integrates the host via `services/aether-agentd/src/runtime_host.rs`
  and routes every `agent.*` IPC command through it. New commands:
  `agent.status`, `agent.session.create`, `agent.session.list`,
  `agent.session.status`, `agent.session.cancel`, `agent.intent`,
  `agent.audit.session`, `agent.audit.recent`, `agent.action.cancel`,
  `agent.stop`.
- `intent_to_action.rs` maps LLM-proposed capabilities to typed
  `ActionVariant`s. Shell capabilities are rejected at the type
  boundary — no `agent.execute_shell` ever reaches the executor.
- `shell/aether-shell` is a thin TCP client to agentd via the new
  `agentd_client` module; `agent status`, `agent sessions`,
  `agent inspect`, `agent intent`, `agent cancel`, `agent audit` all
  reach the daemon.
- Identity (Agent ID, Session ID, Request ID, Action ID) is produced
  by the host and recorded in every audit entry and observation.
- The event bus (`InMemoryEventBus`) publishes typed `AetherAgentEvent`s
  to subscribers, with a bounded ring and a snapshot accessor.
- 12 security tests cover: prompt injection, provider hostility,
  malformed payloads, invalid capability, cross-session lookup,
  replay (cancel-twice), invalid service, privilege escalation,
  malicious tool output, unauthorized actions, policy denial, empty
  chat. Every attack class is rejected cleanly.
- 6 failure-recovery tests cover: service unavailable, IPC failure
  mid-stream, app launch failure, control-plane timeout, session
  cancellation, agentd restart.
- The LLM boundary has a pure-selection helper (`provider_from_selection`)
  plus a real Ollama adapter path; selection is env-gated and
  tested without env mutation (which the workspace `unsafe_code` lint
  forbids).
- End-to-end test `e2e_open_test_application_through_runtime` drives
  the full pipeline against a loopback mock of `aether-system-core`
  and asserts the audit log + observations.
- QEMU validation script (`scripts/run/qemu-agent-validate.sh`) plus
  a phase-2.4 step-17 documentation file.

**Test counts:**
- `aether-agentd`: 123 unit tests (was 79 before phase 2.4; +44 added).
- `aether-agent-runtime`: 60 unit tests.
- `aether-shell`: 28 unit tests (was 18; +10 added for agentd proxy).
- Workspace total: 447 tests passing, clippy clean, fmt clean.

#### 2.5 LLM Provider Layer

**Status:** `COMPLETE`.

The runtime library now ships two real LLM backends behind the
`LlmProvider` trait, both using std-only HTTP/1.1 (no reqwest, no
hyper):

- `OllamaLlmProvider` — talks to a local Ollama daemon on
  `http://127.0.0.1:11434/api/chat`. Supports the `format` field for
  structured output.
- `OpenAILlmProvider` — talks to any server that speaks
  `/v1/chat/completions` (LM Studio, llama.cpp, vLLM, OpenAI). Bearer
  auth, `response_format`, `max_tokens`, `temperature`.

Selection is a pure function (`aether_agent_runtime::llm_provider::select`).
The daemon drives it with `AETHER_LLM_PROVIDER`, `AETHER_LLM_URL`,
`AETHER_LLM_MODEL`, `AETHER_LLM_API_KEY`. Unknown kinds fall back to
the echo provider.

A new `RuntimeBackedProvider` adapter in the agentd routes the
existing `AiProvider` interface to the runtime's `LlmProvider` so
the daemon and the runtime share one HTTP path. The new
`runtime-ollama` selection kind activates this.

**Where it lives:**
- `agent/aether-agent-runtime/src/llm_provider.rs`:
  `OllamaLlmProvider`, `OpenAILlmProvider`, `select`,
  `select_from_env` (11 unit tests).
- `services/aether-agentd/src/lib.rs`: `RuntimeBackedProvider`
  adapter + extended `provider_from_selection` (1 unit test).

**Test counts:**
- aether-agent-runtime: 71 unit tests (was 60; +11 added).
- aether-agentd: 124 unit tests (was 123; +1 added for the bridge).
- Workspace total: 459 tests passing, clippy clean, fmt clean.

#### 2.6 Structured AI Output

**Status:** `COMPLETE`.

The LLM no longer produces free-form text. It must respond with a JSON
envelope (`{ capability, confidence, entities, reason }`) that is validated
against a JSON schema. The LLM can propose an `IntentType` / `CapabilityId`
and a confidence score; it CANNOT assign risk or authority — that boundary
remains in trusted planner + capability-policy code.

**Where it lives:**
- `agent/aether-agent-runtime/src/structured_intent.rs`: canonical
  `INTENT_SCHEMA`, `IntentEnvelope`, `parse_envelope`, `parse_intent`,
  `build_intent_prompt` (11 unit tests).
- `agent/aether-agent-runtime/src/intent.rs`: `IntentType::from_str` and
  `IntentType::all_slugs` so any consumer can validate LLM-produced slugs.
- `services/aether-agentd/src/structured_llm.rs`: local mirror of the schema
  + `try_structured` bridge that wraps the existing `AiProvider`, validates
  the envelope, maps the proposed capability to the daemon's
  `CapabilityId` (including `application.*` → `app.*` runtime-slug aliases),
  and returns `Intent` on success, `Chat` on empty capability, or
  `Fallback(reason)` otherwise. 17 unit tests.
- `services/aether-agentd/src/lib.rs`: chat handler now has three paths:
  (1) deterministic parser, (2) structured LLM, (3) plain chat. The LLM is
  only consulted when the deterministic parser finds no intent. The risk
  and confirmation policy is unchanged. 3 wiring tests.

**Security properties preserved:**
- LLM cannot grant additional authority — only propose a capability,
  entities, and a reason. The planner still validates against policy,
  pre-check, and confirmation before any action.
- Unknown capability strings are rejected and degrade to plain chat.
- Invalid JSON, non-object entities, out-of-range confidence, and empty
  reason are all rejected and degrade to plain chat.
- The schema (`INTENT_SCHEMA`) is embedded into the prompt so any
  provider — local or cloud, with or without native JSON-schema support —
  can satisfy it.

#### 2.7 Planning

**Status:** `COMPLETE`.

`aether-agentd::planner` and `aether-agent-runtime::planner` both exist.
Multi-step plans, dependencies, expected outcomes, recovery policies,
bounded execution, cancellation: the types and the semantics are now
fully specified.

**Where it lives:**
- `agent/aether-agent-runtime/src/recovery.rs` (NEW, 26 unit tests):
  - `RecoveryAction` (Retry / Abort / Skip)
  - `FailureKind` (Transient / Permanent / Unknown), with
    `FailureKind::from_error(&AgentError)` mapping
  - `RecoveryPolicy { max_retries, backoff_base_ms, backoff_max_ms,
    timeout_ms }`, with `transient_default()` and `no_retry()` presets
  - `backoff_delay(&policy, attempt) -> Duration` (capped exponential)
  - `decide_recovery(&policy, attempt, kind, optional) -> RecoveryAction`
    as the single source of truth for retry/skip/abort decisions
  - A reference `PlanRunner` with tests for every policy branch.
- `agent/aether-agent-runtime/src/planner.rs`: `PlanStep` now carries a
  `RecoveryPolicy` (with `#[serde(default)]` so older serialized plans
  keep working). `Plan` carries `max_plan_retries`. `Planner` exposes
  `plan_single_with_recovery`. 4 new tests, plus a
  `legacy_plan_step_without_recovery_field_deserializes` regression
  test.
- `agent/aether-agent-runtime/src/lib.rs`: re-exports the recovery
  surface.
- `services/aether-agentd/Cargo.toml`: takes `aether-agent-runtime` as
  a path dependency.
- `services/aether-agentd/src/planner.rs`: the runtime is now the
  source of truth — the daemon imports `decide_recovery`,
  `backoff_delay`, `FailureKind`, `RecoveryAction`, `RecoveryPolicy`
  and uses them to bound its own `execute` loop.
  - `recovery_policy_for(cap)` returns the per-capability policy
    (read-only caps get 3 retries with 5 s cap; mutating caps get 1
    retry with 1 s cap).
  - `classify_failure(&str)` maps raw IPC errors to
    `FailureKind` (`connect`/`timeout`/`unavailable` → Transient,
    `not_found`/`denied`/`rejected`/`approval` → Permanent,
    everything else → Unknown).
  - The execution loop now retries on `Retry`, aborts on `Abort`
    and reports the final outcome. The previous "first failure
    breaks the plan" behaviour is preserved when recovery decides
    to abort.
  - 7 new wiring tests pin down the per-capability policy, the
    classifier, and the runtime `decide_recovery` integration.

**Security properties preserved:**
- The runtime's `decide_recovery` is the only place that decides
  retry/abort/skip. The daemon, the planner, and the executor all
  share it.
- The LLM cannot influence the recovery policy; the policy is
  attached at plan-construction time by trusted code.
- A `AETHER_FAST_RETRY=1` env var is recognised by the daemon to
  skip backoff sleeps during tests. It does not change the
  decision; only the wait.
- `FailureKind::Permanent` is never retried, even when
  `max_retries > 0`. `CapabilityDenied`, `PolicyDenied`,
  `ApprovalRequired`, `Validation`, and `NotFound` are all
  permanent.
- A failure classified as `Unknown` is retried at most once, then
  aborted. The runtime never silently spins on a misclassified
  error.

**Test coverage:** 26 new unit tests in `recovery.rs` (16 core +
8 runner), 7 new wiring tests in the daemon's planner. The whole
runtime + daemon still passes `cargo test --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings`.

#### 2.8 Observation + Recovery

**Status:** `IN_PROGRESS`.

`Observation` types mirror every `ActionVariant`. The agent can evaluate the
result of an action. **Automated recovery / retry policy** is not yet
defined; current behavior is fail-and-report.

#### 2.9 Agent Memory Foundation

**Status:** `COMPLETE`.

`ConversationMemory` + `SessionMemory` exist as runtime types. The daemon
also has `ConversationContext` for pronoun resolution. Three persistence
surfaces survive `aether-agentd` restarts:

- `conversation` — last-mentioned app/window/file and the bounded turn
  ring, so pronoun resolution works across sessions.
- `working` — the daemon's free-form working memory, exposed to the
  user via `agent.memory.set / get / show / delete` and persisted on
  explicit `agent.memory.flush` and at shutdown.
- `audit_recent` — the most recent 256 `AuditEntry` records, restored
  on startup so the audit view picks up where the previous daemon left
  off.

Persistence goes through a `MemoryStore` trait with two implementations:

- `FileMemoryStore` (default) — writes to
  `<AETHER_WORKSPACE>/aether-agent/<name>` with atomic `.tmp` → rename,
  256 KiB per-file cap, path-traversal-safe name validation.
- `InMemoryStore` (fallback) — used when `AETHER_MEMORY_BACKEND=in-memory`
  is set, or when no `AETHER_WORKSPACE` is available. The daemon
  never panics on a missing store; it stays alive and reports
  `PersistenceOutcome::Missing` for that surface.

Each blob is wrapped in a `Persisted<T>` envelope (version, saved-at
timestamp, FNV-1a content checksum). Version drift is rejected on
read; a mismatched hash is surfaced as `MemoryStoreError::Corrupt` and
the daemon keeps its in-memory defaults so a corrupt file never
crashes the agent.

**Dependencies:** Phase 1.3 (system core), Phase 1.5 (storage), Phase 1.9
(graphical surface).

**Security requirements:** every agent action is capability-checked, audited,
and (for Medium+ risk) requires consent. The agent must never be the source of
truth for the OS; services are. The new `agent.memory.write` capability is
in the default capability set so the working-memory IPC commands are
auditable like every other agent capability.

**Performance requirements:** agent round-trip (intent → plan → action →
observation) must remain interactive (< 1 s for simple actions in QEMU).

**Known limitations:** LLM provider is mock/echo; structured output is not
schema-enforced; runtime is not embedded inside `aether-agentd`.

**Acceptance criteria (Phase 2 closure):**

- [x] `aether-agentd` uses `aether-agent-runtime` for session / intent / plan /
      executor / observation. ← **DONE in 2.4**
- [x] At least one real LLM provider (Ollama or OpenAI-compatible) is wired. ←
      **DONE in 2.5**
- [x] Structured-output schema validation is enforced. ← **DONE in 2.6**
- [x] Bounded recovery semantics are enforced (Phase 2.7). The runtime
      exposes `RecoveryPolicy`, `FailureKind`, `decide_recovery`,
      and the daemon's planner uses the same source of truth.
- [x] Memory persists across session restart. ← **DONE in 2.9** (file
      store under `AETHER_WORKSPACE`, atomic writes, version + checksum
      envelope, 256 KiB per-file cap, `in-memory` fallback for CI).
- [x] End-to-end demo: user text → agentd → intent → plan → action via IPC →
      service → observation → agentd → response → UI. ←
      **DONE in 2.4** (`e2e_open_test_application_through_runtime`)
- [x] All agent tests pass (`cargo test -p aether-agent-runtime`,
      `cargo test -p aether-agentd`). ← 200 + 160 = 360 tests passing.

---

### Phase 3 — Conversational Aether

**Status:** `IN_PROGRESS` (3.1, 3.2, 3.3, 3.4 shipped).

**Objective:** Make Aether naturally conversational.

**Sub-milestones:**

- 3.1 Natural-language command interface (typed, context-aware). — DONE
- 3.2 AI-driven system control (apps, files, network, system, windows, settings,
  devices). — DONE
- 3.3 Permission interaction (explain action, ask for approval, support cancel).
  — DONE (approval-gated `submit_action`, `agent.approval.list/grant/deny` IPC,
  shell `agent approve`/`agent deny`).
- 3.4 Task progress visibility (thinking, planning, working, waiting, completed,
  failed, recovering). — DONE (discrete `ProgressState` ring in
  `aether-agentd::progress`, `agent.progress.current/history` IPC, shell
  `agent progress`).

**Acceptance:** user can say "open my project" and Aether opens it; user can
deny a permission; the UI shows progress.

**Dependencies:** Phase 2 complete.

---

### Phase 4 — Voice + Audio

**Status:** `NOT_STARTED`.

**Sub-milestones:**

- 4.1 Speech-to-text (local-first).
- 4.2 Text-to-speech (Aether's own voice).
- 4.3 Wake word (optional, configurable).
- 4.4 Tap-to-talk (hardware button / desktop button / keyboard shortcut).
- 4.5 Audio service (mic, speakers, headphones, device switching, volume, mute).

**Flow:** Tap/Hold → Listen → STT → Agent → Action → Response (TTS).

**Dependencies:** Phase 3.

**Acceptance:** user can press a button, speak, and have Aether act.

---

### Phase 5 — Vision + Computer Understanding

**Status:** `NOT_STARTED`.

**Sub-milestones:**

- 5.1 Screen understanding (screenshot, window awareness, UI element detection).
- 5.2 Visual agent (buttons, menus, dialogs, app states).
- 5.3 Controlled computer interaction (mouse, keyboard, window actions) **only
  through explicit capabilities**.
- 5.4 Multimodal agent (text + voice + screen + system state).

**Dependencies:** Phase 4.

**Acceptance:** Aether can describe what is on screen and perform a UI action
through a typed capability (not screen-scraped shell calls).

---

### Phase 6 — Aether UI / UX

**Status:** `IN_PROGRESS` (6.1 design tokens, 6.2 component
library, 6.3 Aether Launcher, 6.4 AI Command Bar,
6.5 AI Assistant surfaces, 6.6 AI state consumers,
6.7 iconography, 6.8 animation runtime, 6.9
accessibility shipped). The crate set is complete;
the renderer / shell that paints these surfaces is
the next stage of work and the entry stays
`IN_PROGRESS` until the first paint is in place.

**Objective:** Define and implement the final Aether graphical identity. The
entire OS must share one design system.

The complete UI/UX direction is in [§12 — Aether UI / UX Design Direction](#12-aether-ui--ux-design-direction).

**Sub-milestones:**

- 6.1 **Design tokens (color, type, spacing, radius, motion) — COMPLETE**.
  New crate `ui/aether-design-tokens` is the typed, code-readable
  version of §12. Six modules: `color` (the warm-white +
  cream surfaces, the 6 pastels in default + deep forms,
  the INK 900/700/400 text ramp, soft shadow, hairline; the
  `Role` enum and `Color::role(Role::...)` semantic accessor
  that every surface must use), `spacing` (4 px base, Xs..Xxxl),
  `radius` (Sm..Xl + Pill, with Lg=18 the §12 default for
  panels and AI cards), `type_scale` (Display..Micro with
  size, weight, line-height — the family is platform-
  specific), `motion` (DurationMs + Easing + the standard
  constants TAP=150, HOVER=180, NAV=240, WINDOW_STATE=400,
  AI_CROSSFADE=600), `ai_state` (the 9 §12 AI states with
  their kebab-case wire form and pastel colors:
  Idle=lavender, Listening=pink, Thinking=blue, Planning=
  lavender, Working=mint, WaitingForPermission=yellow,
  Completed=mint-deep, Error=peach-deep, Recovering=peach).
  58 unit tests. The role indirection means a re-skin is a
  one-file change. Files:
  `ui/aether-design-tokens/src/{lib, color, spacing,
  radius, type_scale, motion, ai_state}.rs`.
- 6.2 **Component library (button, card, list, dialog, panel, nav, taskbar,
  launcher) — COMPLETE**. New crate `ui/aether-ui-components`
  defines the typed, non-painting component primitives
  that every Aether surface composes. The architecture is
  "components are descriptions, not framebuffer calls":
  each `Component` trait impl exposes `layout() ->
  LayoutBox`, `style() -> ComponentStyle`, `padding() ->
  Insets`, and a derived `content_rect()`. The renderer /
  layout pass consumes these and applies its own paint
  logic, so the headless test renderer, the Wayland
  compositor, the graphical shell, and the accessibility
  auditor all share the same source of truth. Every
  component resolves its colors through
  `aether_design_tokens::Color::role(Role::...)` — a
  re-skin is one file. The component set today: `Button`
  (3 variants × 2 sizes, focused/pressed/disabled
  modifiers), `Card` (3 elevations, `with_padding`
  builder), `List` (single / multi / no selection, row
  height = body line + 2 × Lg padding), `Dialog` (title +
  body + right-aligned action row, `SCRIM` const for the
  scrim), `Panel` (Left/Right/Top/Bottom anchored, with
  the §12 default 240 px / 48 px), `Nav` (horizontal /
  vertical rail, item length = body line + 2 × Lg), the
  AI-anchored `Taskbar` (running-window chips, network /
  volume / battery / AI tray colors, clock), and the
  AI-first `Launcher` (3-column grid of `LauncherTile`s
  with 2-pastel gradient stops, search query, selected
  index). 98 unit tests, 0 warnings, 0 clippy lints.
  Files: `ui/aether-ui-components/src/{lib, button, card,
  list, dialog, panel, nav, taskbar, launcher}.rs`.
- 6.3 **Aether Launcher (AI-first central UI) — COMPLETE**.
  New crate `ui/aether-launcher` composes the central
  surface from the component library primitives. The
  surface has three regions: a 64-px-wide vertical mode
  rail (Apps / Files / AI) on the left, a 40-px-tall
  search box on the top-right (with a mode-aware
  placeholder: "Search apps" / "Search files" / "Ask
  Aether"), and the tile grid below it. The launcher
  carries three layers of state: `LauncherMode` (the
  three modes plus their label, placeholder, and accent
  color), `LauncherContent` (the pure resolver that
  joins installed + catalog tiles by mode + query —
  prefix matches outrank substring matches, and a no-
  match query falls back to the store catalog), and
  `LauncherView` (the resolved state for one frame,
  with an `apply(ViewAction)` state machine for
  type / backspace / clear / move-up/down/left/right /
  switch-mode / submit / close). The launcher is
  *non-painting*: `LauncherView` is what the renderer
  consumes, and the same value drives the headless test
  renderer, the accessibility auditor, and the snapshot
  tests. 59 unit tests, 0 warnings, 0 clippy lints.
  Files: `ui/aether-launcher/src/{lib, mode, content,
  view}.rs`.
- 6.4 **AI Command Bar — COMPLETE**. New crate
  `ui/aether-command-bar` ships the prompt
  surface: a `Panel::Top` thin horizontal strip
  carrying the mode tabs (`CommandTabs` —
  horizontal `Nav` of Apps / Files / AI), the
  prompt field (`PromptField` — focused = lavender
  border, unfocused = hairline, 40 px tall, mode-
  aware placeholder), and the send button
  (`SendButton` — `Button::Primary::Large`, disabled
  when prompt is empty). The state machine
  (`CommandView` / `CommandAction`) is the same
  shape as the launcher: `TypeChar` / `Backspace` /
  `DeletePrevWord` / `ClearPrompt` /
  `SwitchMode` / `FocusNext` / `FocusPrev` / `Submit` /
  `ShowHelp` / `Close`. The submit resolves to a
  `SubmitIntent` enum (`LaunchApp` / `OpenFile` /
  `AskAgent` / `Noop`) that the renderer / router
  dispatches on. The AI mode is the default active
  tab because the command bar is the type-to-AI
  surface. 51 unit tests, 0 warnings, 0 clippy
  lints. Files: `ui/aether-command-bar/src/{lib,
  state, action, view}.rs`.
- 6.5 **AI Assistant Panel + AI Agent Workspace + AI Task
  View — COMPLETE**. New crate `ui/aether-assistant`
  ships three Aether AI surfaces, all consuming
  `AiVisualState` for their accent color: the
  `AssistantPanel` (a `Panel::Right` sidebar at the
  §12 default 360 px showing the agent's current
  state + recent history + a quick-prompt input),
  the `Agent Workspace` (`WorkspaceView` —
  `Panel::Left` at 480 px with a goal header, a
  progress bar, a vertical timeline of `PlanStep`s,
  and a state footer; each step carries a
  `PlanStepKind` glyph, a title, and a
  `PlanStepState` chip), and the `TaskView` (a
  floating card 560 × 480 showing the focused
  step's inputs, outputs, and a permission
  prompt with `TaskDecision` accept / reject
  controls). The `PlanStepState` is its own
  per-step state machine (Pending / Running
  (AiVisualState) / Done / Failed / Skipped) that
  collapses to the 9-state `AiVisualState`
  vocabulary for styling. 67 unit tests, 0
  warnings, 0 clippy lints. Files:
  `ui/aether-assistant/src/{lib, plan, panel,
  task, workspace}.rs`.
- 6.6 AI visual states (IDLE, LISTENING, THINKING, PLANNING, WORKING,
  WAITING_FOR_PERMISSION, COMPLETED, ERROR, RECOVERING) — the
  state colors are now in `aether-design-tokens` (6.1); this
  sub-milestone covers the surfaces that consume them.
- 6.5 AI Assistant Panel + AI Agent Workspace + AI Task View.
- 6.6 AI visual states (IDLE, LISTENING, THINKING, PLANNING, WORKING,
  WAITING_FOR_PERMISSION, COMPLETED, ERROR, RECOVERING) — the
  state colors are now in `aether-design-tokens` (6.1); this
  sub-milestone covers the surfaces that consume them.
- 6.6 AI visual states (IDLE, LISTENING, THINKING, PLANNING, WORKING,
  WAITING_FOR_PERMISSION, COMPLETED, ERROR, RECOVERING) — the
  state colors are now in `aether-design-tokens` (6.1) and the
  consumer surfaces that read them shipped in 6.5 (the
  Assistant Panel, the Agent Workspace, and the Task View all
  drive their accents from `AiVisualState::color()`). 6.6
  closes with the tray indicator on the taskbar (6.2's
  `Taskbar` already has the `ai_tray_color(state: AiVisualState)
  -> Color` helper — that's the consumer surface).
- 6.7 **Iconography — COMPLETE**. New crate
  `ui/aether-icons` defines the typed, non-painting
  icon system. Every Aether surface that needs an
  icon constructs an `Icon { kind, size, tint,
  background, focused }` value and hands it to the
  renderer; the renderer resolves the kind to a
  glyph. The `IconKind` enum ships 38 kinds across
  four families: app categories (Calculator /
  Document / Notes / Folder / File / Image /
  Music / Video / Settings / Terminal), AI glyphs
  (Aether / Spark / Globe / Key / Gear), system /
  tray (Network / Volume / Battery / Microphone /
  Camera / Lock / Shield / Search), action /
  control (Plus / Minus / Close / Back / Forward /
  Check / Menu / Send / Refresh / Trash), and
  status (Info / Warning / Error / Done / Pending).
  Each kind has a §12 default tint (the AI glyphs
  get the AI palette — Aether/Spark=lavender,
  Key=yellow, Globe=blue, Gear=mint) and a default
  background (tiles get a pastel; tray icons get
  no background). `IconSize` covers the 4-px grid
  (Xs=16, Sm=20, Md=24, Lg=32, Xl=40), with `Md`
  as the §12 default. `IconBackground` is `None` /
  `RoundedSquare` / `Circle`. 22 unit tests, 0
  warnings, 0 clippy lints. Files:
  `ui/aether-icons/src/lib.rs`.
- 6.8 **Animation system — COMPLETE**. New crate
  `ui/aether-animation` ships the runtime engine
  for the §12 motion vocabulary. The runtime is
  intentionally minimal and deterministic — there
  is no timer thread, no `Instant::now()`, no
  platform-specific code. The shell drives every
  animation by calling `AnimationQueue::advance
  (delta_ms)` once per frame and reading
  `progress(name)`. The `Animation` value carries
  a `DurationMs` + `Easing` + `from` / `to`; the
  `AnimationQueue` is a small fixed-capacity list
  of named animations. The crate ships 5
  pre-baked constructors that map to the §12
  vocabulary — `Animation::tap()` (150 ms /
  Standard), `Animation::hover()` (180 ms /
  Standard), `Animation::nav()` (240 ms /
  Standard), `Animation::window_state()` (400 ms
  / Standard), and `Animation::ai_crossfade()`
  (600 ms / Emphasized, the curve the AI surfaces
  use to "settle" into a new state). The
  `apply_easing` function implements Newton-method
  inverse-axis sampling of the cubic-bezier
  control points, so the curve value the caller
  reads is the Y value at the supplied X. 28
  unit tests, 0 warnings, 0 clippy lints. Files:
  `ui/aether-animation/src/lib.rs`.
- 6.9 **Accessibility — COMPLETE**. New crate
  `ui/aether-a11y` ships the accessibility model
  every Aether surface composes. The crate
  defines: 12 `Role`s (Button / TextInput / List /
  ListItem / Nav / Tab / Dialog / Status /
  ProgressBar / Tile / Heading / Region) that the
  AT-SPI / screen-reader bridge uses to announce
  surfaces; a `Description` value (label + detail
  + state + shortcut) that the screen reader
  reads; a `Focusable` value (id + role +
  description + disabled) and a `KeyboardNav`
  chain (`push`, `focus_next`, `focus_prev`,
  `focused_id`, `enabled_count`) that
  implements tab navigation with disabled-skip
  and wrap; `MotionPreference` (`Standard` /
  `Reduced`) with an `apply_motion_preference
  (d, pref)` helper that zeros out any duration
  > 100 ms when the user has the OS-level
  "Reduce motion" toggle on; `ContrastPreference`
  (`Standard` / `High`) for the OS-level
  "Increase contrast" toggle; and a `Scale`
  value (1..=4) that the renderer multiplies
  into every `Spacing` and `TypeScale` value.
  26 unit tests, 0 warnings, 0 clippy lints.
  Files: `ui/aether-a11y/src/lib.rs`.

**Dependencies:** Phase 1.9 (graphical OS) and Phase 3 (conversational
Aether) for AI surfaces.

**Acceptance:** every screen uses tokens from the design system; visual review
passes; accessibility checks pass.

**Known limitations:** today `aether-graphical-shell` uses a dark
monospace-painted UI; the pastel / premium identity in §12 is the target.
6.1 ships the tokens; the next sub-milestones (6.2 components onward)
will paint the surfaces with them.

---

### Phase 7 — Aether Agent Deep System Control

**Status:** `IN_PROGRESS` (7.1 system diagnostics shipped;
7.2 self-healing shipped;
7.3 system automation shipped;
7.4 background agent is the remaining work).

**Objective:** Make Aether capable of operating the entire OS.

**Sub-milestones:**

- 7.1 **System diagnostics — COMPLETE**. New crate
  `system/aether-diagnostics` ships the typed
  "why is my computer slow?" model. The pipeline
  has four steps: **collect** (`Signal` values
  from each subsystem — CPU / Memory / Disk /
  Network / Service / App / Security / Power /
  FileSystem), **symptom** (correlated
  `Symptom` values that represent a specific
  problem — "high CPU *and* OOM-kill *and* app
  crash = system_unstable"), **explain**
  (human-readable `Explanation` with a cause
  and a proposed fix, tagged with whether the
  fix requires user consent), and **score**
  (`DiagnosticReport::score()` returns 0..=100;
  every `Critical` symptom drops the score by
  30, every `Warning` by 5). The rules table is
  data: the crate ships a `default_rules()` that
  handles the common cases (cpu_overload,
  memory_pressure, disk_full, service_down,
  app_crash_loop, system_unstable) and callers
  can extend it at runtime. The report's
  `to_observations()` method bridges the
  diagnostics vocabulary to the agent's existing
  `Observation` type so the proposal pipeline
  can consume symptoms without caring that they
  came from diagnostics. 24 unit tests, 0
  warnings, 0 clippy lints. Files:
  `system/aether-diagnostics/src/lib.rs`.
- 7.2 **Self-healing — COMPLETE**. New crate
  `system/aether-recovery` ships the bounded
  recovery-action model. The contract is *typed
  review*: every recovery step is a
  `RecoveryAction` variant the agent (and the
  user) can read before execution. Eight
  variants cover the five recovery families per
  the ROADMAP — service (`RestartService`),
  network (`ReconnectNetwork`), application
  (`RestartApp`), dependency
  (`ResolveDependency`), resource
  (`FreeDiskCache` / `DropPageCache` /
  `KillProcess`) — plus `InformUser` for the
  "no auto-recovery available" case. Each
  action exposes `subsystem()` (the
  `Subsystem` it targets), `summary()` (a
  single-sentence human description), and
  `requires_consent()` — only `KillProcess` is
  consent-gated; the rest run automatically
  once the agent approves them. `RecoveryPlan`
  is the ordered list of actions the agent
  executes for a single symptom;
  `RecoveryPolicy` is the symptom-id → recipe
  table, with a `default_policy()` that
  handles the six default diagnostics symptoms
  (`cpu_overload`, `memory_pressure`,
  `disk_full`, `service_down`,
  `app_crash_loop`, `system_unstable`).
  `plan_recovery(symptoms, policy)` is the
  one-shot: give it the symptoms from a
  `DiagnosticReport`, get back the plans
  ready to execute. 19 unit tests, 0
  warnings, 0 clippy lints. Files:
  `system/aether-recovery/src/lib.rs`.
- 7.3 **System automation — COMPLETE**. New
  crate `system/aether-automation` ships the
  user-defined workflow model. A *workflow*
  is a named, ordered list of `WorkflowStep`s
  with a `Trigger` (Manual / TimeOfDay /
  OnEvent). Each step is a typed `StepAction`
  (LaunchApp / OpenFile / AgentTask /
  RecoveryAction / Notify / Wait) with a
  per-step `FailurePolicy` (Abort / Skip /
  Continue / RetryThenAbort / RetryThenSkip).
  The `WorkflowRegistry` is the named
  collection; the runtime boots with
  `default_registry()` (morning_setup at
  09:00, end_of_day at 18:00, before_meeting
  on demand) and the user can register more.
  `compile_to_tasks(workflow, prefix, ts)` is
  the one-shot that turns a workflow into the
  ordered `AgentTask` list the runtime
  executes; the failure policy and the
  structured step payload ride along as JSON
  arguments so the runtime can read them
  back at execution time. 19 unit tests, 0
  warnings, 0 clippy lints. Files:
  `system/aether-automation/src/lib.rs`.

**Dependencies:** Phase 2 + Phase 3 + Phase 8 (devices).

**Acceptance:** Aether can detect a failing service, propose and (after
consent) perform a restart, and report success.

---

### Phase 8 — Device + Hardware Ecosystem

**Status:** `NOT_STARTED`.

**Sub-milestones:**

- Aether Hardware Service exposing CPU, GPU, display, keyboard, touchpad,
  mouse, audio, mic, camera, Wi-Fi, Bluetooth, Ethernet, USB, storage, battery,
  thermal, external displays, printers, future sensors.

**Dependencies:** Phase 1.7 (network), Phase 4 (audio).

**Acceptance:** a user can say "connect my headphones" and Aether switches
the audio route via a typed capability.

---

### Phase 9 — Application Platform

**Status:** `COMPLETE` (9.1 / 9.2 / 9.3 / 9.4 all shipped;
9.4 update / rating flows remain out of scope for the
typed contract).

**Sub-milestones:**

- 9.1 **Aether SDK (app, UI, Agent, IPC, capabilities, manifests) — COMPLETE**.
  `sdk/rust/aether-sdk` is the developer-facing surface for
  third-party apps. The crate now ships the packaging half
  (the runtime `AetherClient` was already there):
  * `manifest_builder` — `AppManifestBuilder::new(app_id, name,
    version, publisher, payload)` with `.permission(...)`,
    `.resources(...)`, `.depends_on(...)`, `.min_os_version(...)`,
    `.description(...)`, `.timestamp_ms(...)`. Defaults:
    `schema_version = APP_PACKAGE_SCHEMA_VERSION`,
    `sandbox_profile = RestrictedService`,
    `min_os_version = SDK.Major.Minor.0`, `binary_sha256` and
    `payload_len` filled from the payload on `.build()`. Strict
    validation up front: bad app id, empty name/version/
    publisher, empty payload all surface as typed
    `ManifestBuildError` variants.
  * `package_builder` — `AppPackageBuilder::build_signed(manifest,
    payload, signer)` is the one-shot. Internally it runs a
    pre-sign `validate_manifest_pre_sign` (strict minus the
    `publisher_key_id` check, which the signer fills in),
    then the strict `AppManifest::validate` after the
    fingerprint is set. The signature is computed by
    `aether_security::app_signing::AppPackageSigner::sign_package`
    over the manifest's canonical bytes; the resulting
    `AppPackage` verifies against the signer's public key
    via `AppPackageVerifier::verify_with_key`.
  * `permissions` — re-exports `app_permission_capability`
    as a `(permission, capability, risk_level)` tuple list
    for SDK callers that want a pre-install consent preview
    UI.
  * `install` — typed IPC command builders:
    `install_request(package_path, actor_trust)`,
    `launch_request(app_id, instance_label, actor_trust)`,
    `uninstall_request(app_id, actor_trust)`. All three fix
    `service_id = "aether-store"` and the verb name; the
    caller controls `actor_trust` so tests can assert that
    the dispatcher denies store commands from `Untrusted`
    actors.
  35 new unit tests + 2 doc-tests, all green.
  Files: `sdk/rust/aether-sdk/src/{manifest_builder,
  package_builder, permissions, install}.rs`,
  `sdk/rust/aether-sdk/src/lib.rs`,
  `sdk/rust/aether-sdk/Cargo.toml`.
- 9.2 **Application packaging (format, manifest, signing, dependencies,
  permissions, resources) — COMPLETE**. `aether_core::app` defines the
  typed `AppManifest`, `AppPackage`, and the 11-variant
  `AppPermission` enum (ReadUserFiles, WriteUserFiles,
  NetworkEgress, NetworkListen, ReadPersonalData, Notify,
  CaptureScreen, Camera, Microphone, Location, PairDevices).
  `is_valid_app_id` validates reverse-DNS identifiers,
  `app_cgroup_slice` derives the per-app cgroup slice name,
  `AppResourceLimits` carries cgroup v2-shaped budgets.
  `aether_security::app_signing` is the Ed25519 layer
  (`AppPackageSigner` signs the canonical manifest bytes;
  `AppPackageVerifier` recomputes the payload SHA-256, checks
  the publisher fingerprint, and verifies the signature). 26
  new unit tests. Files: `core/aether-core/src/app.rs`,
  `security/aether-security/src/app_signing.rs`.
- 9.3 **Application security (sandbox, filesystem scopes, network scopes,
  device scopes, process limits) — COMPLETE**.
  `aether_security::app_security` is the bridge: it maps every
  `AppPermission` to a typed `Capability` with a chosen risk
  level (CaptureScreen at Critical so the DefaultPermissionPolicy
  requires consent even after install-time approval), builds
  the persistent `AppConsentRecord` (publisher fingerprint
  bound to the manifest, monotonic version, sorted grants),
  drives the install-time flow through `AppInstaller` (validates
  the manifest, rejects unrequested grants, derives the
  refused set, writes the `app.install` audit log line), and
  serves the runtime `AppPermissionGate` (pure function: app_id
  + capability → allow/deny verdict against the consent
  record). `sandbox_plan_for_app` derives a
  `RestrictedService` plan for an `AppManifest` with the
  cgroup slice renamed to the app's own slice and the
  manifest's memory_max_bytes copied into the plan.
  `verify_consent_for_package` protects against payload-swap
  attacks by requiring both `app_id` and `publisher_key_id`
  to match between the consent record and the on-disk
  package. 22 new unit tests. File:
  `security/aether-security/src/app_security.rs`.
- 9.4 **Aether Store (discovery, install, updates, ratings,
  permissions, publisher, signatures) — COMPLETE**. New crate
  `system/aether-store` ships the user-facing install /
  launch / uninstall state machine. Three sub-modules:
  * `fs` — `StoreFs` trait with a `MemoryFs` (tests) and
    `LocalFs` (production) implementation.
  * `registry` — `TrustedPublisherRegistry`: sorted,
    deduplicated list of trusted fingerprints persisted
    to `trust.json` via `StoreFs`. A missing file is the
    safe default (every install rejected); a malformed
    file is treated as evidence of tampering.
  * `store` — the state machine. `install_signed` runs the
    full chain (manifest validate → trust check → grant
    subset check → no-overwrite check → consent + plan +
    install receipt + audit log). `launch` verifies the
    consent record still matches the on-disk manifest
    before handing the payload to the injected `Launcher`
    (production = `aether-sandbox`). `uninstall` clears
    the in-memory record and the payload cache. The
    `StoreError` enum is a 10-variant typed result with
    unique Display per variant. 36 new unit tests.

**Dependencies:** Phase 1.4, Phase 2.

**Acceptance:** a third-party Aether app can be authored
(via the SDK), signed, installed, sandboxed, launched, and
uninstalled. Update and rating flows are out of scope for
the typed contract; the foundation for them (publisher
trust, install receipt with manifest_digest, refused set,
plan_digest) is in place.

---

### Phase 10 — Real Hardware Bring-up

**Status:** `NOT_STARTED`.

**Objective:** Move from QEMU to real x86_64 hardware.

**Sub-milestones:**

- 10.1 Hardware compatibility profiles (not one hard-coded laptop).
- 10.2 Driver integration (UEFI, kernel, GPU, display, kbd, touchpad, mouse,
  Wi-Fi, BT, Ethernet, audio, mic, camera, USB, battery, thermal,
  suspend/resume, shutdown, reboot).
- 10.3 Recovery mode (when graphics/network/audio fails, Aether remains
  diagnosable).

**Dependencies:** Phase 1, Phase 8.

**Acceptance:** Aether boots, runs, and survives reboot on a documented
hardware profile; recovery path is exercisable.

---

### Phase 11 — Security + Trusted AI

**Status:** `PARTIAL` (capability/policy/audit/hash-chain/sealed
credentials/signed manifests/signed updates/kernel sandboxing
and the tamper-evident boot-measurement chain are all
present; Phase 11.1 prompt-injection defences are still
in-progress; remaining defence-in-depth items live in
future phases).

**Sub-milestones:**

- 11.1 AI security: treat model output as untrusted input. Defend against
  prompt injection, malicious files, malicious web content, malicious app
  output, tool poisoning, command injection, privilege escalation, action
  replay.
- 11.2 High-risk action gating: delete, install, system modification, network
  modification, credentials, external communication, security changes,
  firmware changes — all require explicit user approval.
- 11.3 **System-core policy gate (defence-in-depth) — COMPLETE**. Every IPC
  request entering `aether-system-core` is evaluated by
  `aether_system_core::policy::evaluate` against the cross-domain
  `DefaultPermissionPolicy`, combined with the request's `ActorTrust`
  (Trusted/Untrusted, carried on `IpcRequest`). Untrusted actors are
  denied outright before the policy is even consulted (the system-core
  dispatcher must not execute capabilities for an unauthenticated peer).
  Higher-risk capabilities (`file.delete`, `system.shutdown`, …) return
  `REQUIRES_CONFIRMATION` so the agentd's approval-gated flow can collect
  explicit user consent before re-issuing. Distinct error codes
  (`POLICY_DENIED`, `POLICY_DENIED_UNTRUSTED`, `REQUIRES_CONFIRMATION`)
  let the audit log and red-team suite distinguish the failure modes.
  Evidence: `system/aether-system-core/src/policy.rs`,
  `system/aether-system-core/src/main.rs` (`dispatch_inner` calls the
  gate; `gate_response` converts the verdict to an `IpcResponse`),
  `core/aether-core/src/ipc.rs` (`ActorTrust`),
  `system/aether-system-core/Cargo.toml` (depends on
  `aether-security`). 7 dispatch-policy unit tests cover trusted /
  untrusted / low-risk / high-risk / critical combinations.
- 11.4 **Declarative kernel-sandbox plan + enforcement — COMPLETE**.
  `core/aether-core/src/sandbox.rs` defines a typed `SandboxPlan`
  for each `SandboxProfile`:
    * `Internal` — no kernel primitives (in-process).
    * `SystemService` — user+mount+uts namespaces, no_new_privs, the
      minimum Linux capabilities the service actually needs (no
      `sys_admin` / `sys_module` / `sys_rawio`), a `system-service-v1`
      seccomp filter tag, and a `aether.slice/system.service.slice`
      cgroup slice.
    * `RestrictedService` — full user+pid+network+ipc+uts namespace
      isolation, every ambient capability dropped, no_new_privs,
      a `restricted-app-v1` seccomp filter tag, and a
      `aether.slice/restricted.app.slice` cgroup slice with a
      bounded memory cap.
  `aether_system_core::manager::ServiceManager::sandbox_plan` /
  `all_sandbox_plans` expose the plan for one (or every) service;
  a new `sandbox.plan` IPC command (gated by the Phase 11.3 policy
  as a low-risk System capability) returns the plan as JSON.
  **The Linux enforcement binary `aether-sandbox` is now live**:
  `system/aether-sandbox/src/main.rs` parses the plan, validates
  it (forbidden capabilities, cgroup slice policy, weight bounds),
  and `src/linux.rs` applies the primitives in deterministic order
  — `prctl(PR_SET_NO_NEW_PRIVS)`, `unshare(CLONE_NEW*)`, cgroup
  v2 slice write, `capset()` to keep only the whitelist — then
  `execvp()`s the child. The non-Linux build is a clear-error
  stub; the contract is never silently weakened on a non-Linux
  target. The seccomp filter is *tagged* by this binary and
  *installed* by the supervisor before user code runs.
  Evidence: `core/aether-core/src/sandbox.rs` (9 unit tests
  covering determinism, serde round-trip, distinct cgroup
  slices, distinct seccomp tags, missing-capability safety),
  `system/aether-system-core/src/manager.rs` (5 unit tests),
  `system/aether-sandbox/src/main.rs` (10 unit tests covering
  plan validation, plan resolution, CLI surface, and profile
  handling), `system/aether-sandbox/src/linux.rs` (3 unit tests
  covering `CLONE_NEW*` flag mapping, `CAP_*` numbering, and
  capability-coverage exhaustiveness). 13 new Rust tests in
  total; 855 Rust tests passing project-wide.
- 11.5 **Audit log with hash-chain integrity — COMPLETE**. Every IPC
  dispatch through the system-core daemon writes a tamper-evident
  `AuditEntry` to a SHA-256 hash-chained `AuditChain`. Each entry's
  `content_hash` covers `(prev_hash || timestamp || event ||
  component || detail)`; `verify_chain()` detects content mutation,
  broken links, and index gaps. Retention is bounded by both
  `max_entries` and `max_age_ms`. The chain is exposed via three
  IPC commands — `audit.recent` (last N entries), `audit.verify`
  (whole-chain integrity check), and `audit.prune` (apply
  retention). Evidence: `security/aether-security/src/audit.rs`
  (18 unit tests) and `system/aether-system-core/src/main.rs`
  (`record_audit`, the audit IPC commands, the `AuditChain` /
  `RetentionPolicy` / `ChainStatus` JSON conversions).
- 11.6 **Sealed credential storage — COMPLETE**. Secrets are sealed
  with AES-256-GCM under a process-lifetime key, with a
  `RandomKeyProvider` and a `StaticKeyProvider` behind a common
  `KeyProvider` trait. The `Secret<T>` wrapper zeroes its inner
  value on `Drop`. The `SealedStore` supports `seal` / `unseal` /
  `remove` / `clear` / `get` / `contains` / `names` / `len` /
  `is_empty`, with a per-credential `force` flag for explicit
  overwrite. Wire format is `nonce(12) || ciphertext_with_tag`
  (the AES-GCM tag is appended to the ciphertext by `aes-gcm`).
  IPC: `credentials.seal`, `credentials.unseal`,
  `credentials.list`, `credentials.remove`, `credentials.metadata`.
  Evidence: `security/aether-security/src/credentials.rs` (16 unit
  tests including round-trip, tamper-detection, duplicate, and
  wrong-key rejection) and the `credentials.*` test module in
  `system/aether-system-core/src/main.rs`.
- 11.7 **Signed service manifests (Ed25519) — COMPLETE**. Every
  service manifest can be shipped alongside a `.json.sig` envelope
  containing the manifest bytes, the signer's public key, and an
  Ed25519 signature. The system-core daemon's
  `load_manifests_with_trust(dir, &trust)` rejects: missing
  signature files, signatures from untrusted signers, and
  tampered manifests (either the bytes or the signature have
  been mutated). Backward compatibility is preserved —
  `load_manifests_from_dir` continues to load unsigned manifests
  for dev / test. A new `manifest.trust_store` IPC command
  reports the currently-loaded trust list to the operator.
  Evidence: `security/aether-security/src/manifest_signing.rs`
  (12 unit tests), `system/aether-system-core/src/loader.rs`
  (5 trust-aware loader tests), and the `trust_store_ipc_tests`
  module in the system-core main.
- 11.8 **Signed Aether updates (out-of-scope shell) — COMPLETE**.
  `security/aether-security/src/signed_update.rs` defines a
  `SignedUpdate` envelope (header + payload + 64-byte Ed25519
  signature) and a verifier that pins to a single trusted
  public key plus a fingerprint trust list. Update kinds:
  `os-image`, `service-bundle`, `agent-model`. The
  `UpdateSigner` produces envelopes; `UpdateVerifier` (via
  `verify_signed_update_trusted`) rejects: bad magic, empty
  target, payload-length mismatch, bad signature length,
  unknown signer, fingerprint not in trust list, and bad
  signature. The IPC layer exposes `update.verify` (the
  caller hands a JSON header + base64 payload + base64
  signature + hex public key; the daemon returns
  `ok: true|false`) and `update.fingerprint` (compute the
  manifest-signing fingerprint for a hex public key).
  Delivery, journaling, and atomic apply are out of scope
  for this phase and live in the future `aether-update-agent`
  daemon. Evidence: `security/aether-security/src/signed_update.rs`
  (14 unit tests) and the `update_ipc_tests` module in the
  system-core main.
- 11.9 **Tamper-evident boot-measurement chain — COMPLETE**.
  `security/aether-security/src/boot_measure.rs` is the
  root-of-trust companion to `AuditChain`. The
  `BootMeasurementChain` records the boot-time artifacts
  in the order they are encountered, so a verifier
  reading the chain from the start can answer "did the
  kernel cmdline, the loaded initramfs, the active kernel
  modules, and the registered service manifests match
  the last-known-good state?". Stages are typed:
  `KernelCommandLine`, `InitramfsComponent`,
  `KernelModule`, `ServiceManifest`, `BootComplete`.
  The `BootComplete` marker carries the audit-chain
  genesis hash so the boot chain binds to the runtime
  chain. `verify_chain` is strict (requires the
  `BootComplete` marker); `verify_chain_lenient` accepts
  in-progress chains. The chain is content-addressed:
  every entry's `content_hash` is SHA-256 over a
  canonical byte buffer; the verifier recomputes and
  rejects on mismatch, broken link, or index gap.
  `kernel_cmdline_digest` is a canonical SHA-256 over
  the kernel command line, sorted by argument so two
  cmdlines that differ only in argument order hash to
  the same digest. 18 new unit tests. File:
  `security/aether-security/src/boot_measure.rs`.
- Kernel primitives where appropriate: Linux capabilities, namespaces, cgroups,
  seccomp, MAC policy, sandboxing, signed applications, signed updates,
  credential protection, secret storage, audit retention, policy management.

**Dependencies:** Phase 1.4, Phase 2.

**Acceptance:** a documented attack surface; documented defenses; integration
tests that exercise the defenses.

---

### Phase 12 — Self-Updating + System Lifecycle

**Status:** `PARTIAL` (planning layer + state machine + IPC shipped;
delivery, atomic-apply, and rollback execution deferred to
`aether-update-agent`).

**Sub-milestones:**

- 12.1 **Update planning layer — COMPLETE**. New
  `system/aether-update-core` crate defines the
  declarative contract a future update-agent daemon
  will operate on:
    * `UpdatePlan` — target, kind, action (upgrade vs
      reinstall), version, timestamp, signer
      fingerprint, payload length, and the
      `VersionPolicyDecision` that allowed it.
    * `UpdateAction` — the five canonical actions:
      `upgrade-os-image`, `reinstall-os-image`,
      `upgrade-service-bundle`,
      `reinstall-service-bundle`,
      `upgrade-agent-model`.
    * `plan_from_signed_update` — the bridge from a
      verified `aether_security::signed_update::SignedUpdate`
      to an `UpdatePlan`. Rejects empty target,
      empty version, empty payload, and any update
      denied by the version policy. Tests cover
      every rejection path and the upgrade /
      reinstall / downgrade branches.
    * `VersionPolicy` — pure-logic policy with two
      flags: `allow_downgrade` and
      `allow_prerelease`. Recognises four
      requirement categories (`Upgrade`,
      `Downgrade`, `Same`, `Prerelease`). Allows
      reinstalls of `os-image` and `service-bundle`
      at the same version; rejects reinstalls of
      `agent-model` (a model downgrade must use
      `allow_downgrade`).
  Evidence: `system/aether-update-core/src/plan.rs`
  and `system/aether-update-core/src/version.rs`
  (32 unit tests across both modules).
- 12.2 **Update state machine — COMPLETE**. `UpdateStatus`
  is the in-memory record of "where is the current
  update, if any, and what happened the last time we
  tried one?" Eight stages: `Idle | Downloading |
  Verifying | Staging | Applying | Done | Failed |
  RolledBack`. The status carries a bounded
  `HistoryEntry` log (max 64 entries, oldest dropped
  on overflow) of every transition. The state
  machine is driven via `UpdateStatus::transition` —
  the future update-agent daemon is the only thing
  that calls it; the IPC layer's `update.simulate`
  command is a thin wrapper for tests and operator
  smoke-tests. Tests cover every stage, attempt
  increment / reset, last-error clearing, history
  bounding, and plan attach / clear. Evidence:
  `system/aether-update-core/src/state.rs` (10 unit
  tests).
- 12.3 **Recovery snapshot — COMPLETE**. The
  `RecoverySnapshot` type describes "the pre-update
  state" that rollback restores from. A snapshot
  carries an id, a wall-clock timestamp, and a list
  of `SnapshotComponent` records (target, from
  version, stash path, optional note). The shell
  includes `is_complete`, `version_of`, `component_count`,
  and `targets` helpers. The future daemon writes
  the actual data; this type only describes *what*
  was snapshotted, not *where the bytes live*.
  Evidence: `system/aether-update-core/src/recovery.rs`
  (8 unit tests).
- 12.4 **IPC surface for the planning layer — COMPLETE**.
  The system-core daemon now exposes four new
  commands (gated by the Phase 11.3 policy as
  low-risk `System` capabilities):
    * `update.plan` — accepts a JSON header + base64
      payload + base64 signature + hex public key +
      optional `installed_version`. Re-verifies the
      signature against the supplied public key,
      then runs `plan_from_signed_update`. Returns
      the resulting `UpdatePlan` on success, or
      `VERIFICATION_FAILED` / `POLICY_DENIED` /
      `INVALID_INPUT` on the various failure paths.
    * `update.status` — returns the live
      `UpdateStatus` (stage, attempt counter, last
      error, current plan).
    * `update.history` — returns the bounded
      transition log.
    * `update.simulate` — operator / test helper
      that drives the state machine through a
      comma-separated stage sequence.
  Evidence: `system/aether-system-core/src/main.rs`
  (the four new commands) and the
  `update_plan_ipc_tests` module (9 integration
  tests including plan-on-upgrade, downgrade
  rejection, bad-signature rejection, status,
  history, simulate, and unknown-stage rejection).
- **Out of scope (lives in the future `aether-update-agent`
  daemon):** actual download, stage, atomic-apply,
  rollback execution, and reboot coordination. The
  planning layer is the contract; the I/O code is a
  separate daemon that drives the state machine
  against this contract.

**Dependencies:** Phase 11 (signed updates, sealed
credentials, audit chain).

---

### Phase 13 — Aether Autonomous OS

**Status:** `PARTIAL` (13.1 landed; the runtime daemon is still out of scope).

**Sub-milestones:**

- Aether becomes a genuinely proactive OS agent: understand, observe, plan,
  execute, verify, recover, learn from allowed feedback.
- Aether proposes action rather than silently making dangerous changes.
- Examples: "your storage is nearly full", "network connectivity is unstable",
  "an application crashed repeatedly", "your development environment is missing
  a dependency".

**Dependencies:** Phase 7, Phase 11, Phase 12.

**13.1 — Agent planning surface (landed):**

- New crate `agent/aether-agent-core` carries the out-of-scope shell: typed
  `AgentTask`, `TaskGraph` (DAG with cycle detection + ready queue), `Proposal`
  (action + risk + evidence), `Observation` (component + severity), `TaskStage`
  (Proposed → Approved → Executing → Done / Failed / Cancelled), `AgentStatus`
  (live state machine: tasks, proposals, history, observations).
- A local `TaskRisk` enum mirrors `aether_core::RiskLevel` with serde derives
  so the agent crate can be serialised independently; `From` conversions live
  at the IPC boundary.
- A `propose_from_observations` validator partitions drafts into accepted /
  rejected and enforces the per-kind risk floor (ProposeUpdate /
  ProposeInstall / Custom require `High`).
- `validate_proposal` checks that every `evidence` id is present in the live
  observation log, that the description / reasoning are non-empty, and that
  the risk is at least the kind's default.
- `proposal_to_task` is the bridge that turns an approved `Proposal` into a
  live `AgentTask` (with risk, target, and arguments carried through).
- `aether-system-core` now exposes the `agent.*` IPC family:
  `agent.observe`, `agent.propose`, `agent.proposals`, `agent.tasks`,
  `agent.history`, `agent.observations`, `agent.cancel`, `agent.approve`.
- `agent.approve` is the only path that turns a proposal into a live task; the
  conversion uses `proposal_to_task` so the shape is identical to what the
  future runtime will produce.
- 39 unit tests in `aether-agent-core`; 12 integration tests in
  `aether-system-core::agent_ipc_tests`. Full workspace: 798 tests pass,
  zero clippy warnings.

**Out of scope (deferred):**

- The future `aether-agent-runtime` daemon (the only thing allowed to call
  `add_observation` and `add_proposal` from outside the IPC layer in
  production).
- The model / LLM layer that turns observation batches into proposal drafts.
- The actual executor (today tasks sit in the graph; nothing runs them).
- Cross-device coordination (Phase 14).

---

### Phase 14 — Multi-Device Aether

**Status:** `PARTIAL`.

**Sub-milestones:**

- Same Aether identity and agent model coordinate across phone, tablet,
  laptop, desktop, IoT, home devices, external displays.
- Explicit pairing is mandatory.
- Security first.

**Dependencies:** Phase 13.

**14.1 — Cross-device typed contract (this commit)**

- New crate `agent/aether-device-core`:
  - `identity` — `DeviceId`, `DeviceClass` (Phone / Tablet / Laptop /
    Desktop / Iot / Server / External / Other).
  - `fingerprint` — `DeviceFingerprint` (32-byte SHA-256 of public key).
  - `pairing` — `PairingState` (Available / Pairing / Paired /
    Cancelled / Revoked / Expired), `PairingCode` (6 decimal digits),
    `PairingGrant` (receive_observations / receive_proposals /
    execute_remote_tasks), `PairingRequest` / `PairingAcceptance` /
    `PairingError` / `validate_acceptance`.
  - `registry` — bounded `DeviceRegistry` (256 entry limit) with
    `register` / `get` / `devices` / `paired` / `transition` /
    `unregister`.
  - `remote` — `RemoteSource` (device id, fingerprint, monotonic
    `seq`, timestamp) and `accept_remote_delivery` (validates
    state, grant, fingerprint, seq ordering, skew window).
- Out-of-scope shell (`aether-system-core`):
  - Six new IPC commands: `device.list`, `device.register`,
    `device.pair.begin`, `device.pair.complete`, `device.revoke`,
    `device.unregister`. Pair code display, BLE / QR / NFC transport,
    and remote observation / proposal *delivery* are deferred to
    the future `aether-device-runtime`; the shell only stores
    the typed state machine.
- 8 new integration tests in `device_ipc_tests`.
- 36 unit tests in the `aether-device-core` crate.
- Total tests passing: 842.

---

### Phase 15 — Production Release

**Status:** `PARTIAL`.

**Objective:** Production-ready Aether OS.

**Sub-milestones:**

- Stable installer.
- Bootable ISO.
- Hardware images.
- Secure update mechanism.
- Recovery environment.
- Application platform.
- SDK.
- Developer tools.
- Documentation.
- Privacy-safe telemetry (where explicitly designed).
- Release validation.
- Performance benchmarks.
- Security audit.
- Compatibility matrix.

**Dependencies:** every prior phase.

**Acceptance:** release quality gates in [§13](#13-release-quality-gates) all
pass.

**15.1 — Release validation, performance benchmarks, security audit (this commit)**

- **Release validation script** — `scripts/release-validate.sh`
  runs 9 CI-friendly gates on every change:
  1. `cargo build --workspace` (debug).
  2. `cargo build --workspace --release`.
  3. `cargo test --workspace`.
  4. `cargo clippy --workspace --all-targets`.
  5. `cargo fmt --all -- --check`.
  6. `scripts/release.sh` stage (7 binaries).
  7. Python SDK / brain test suite.
  8. Workspace `Cargo.toml` membership check (every crate must
     be a workspace member or it silently fails to build).
  9. Phase 15 documentation existence
     (`docs/RELEASE-NOTES.md`,
      `docs/phase-15/compatibility-matrix.md`,
      `docs/phase-15/security-audit.md`).

  Optional flags: `--skip-release-build`, `--skip-python`. The
  script is portable (POSIX `bash`) and CI-friendly. The
  cross-platform workspace-membership check normalises
  Windows backslashes to forward slashes so it works on both
  `ubuntu-latest` and `windows-latest` runners.

- **Micro-benchmark harness** — `tools/aether-bench` (`cargo run
  --release --bin aether-bench`) measures the operations the
  system-core dispatch loop hits on every IPC request:
  audit chain record+verify, sealed-store seal/unseal,
  pairing acceptance, device registry, and IPC
  encode/decode. Numbers (Windows, release profile, 5000
  iterations):

  | benchmark                   | ns/op     | op/s       |
  | --------------------------- | --------: | ---------: |
  | audit chain record+verify   |    ~460   |  ~2.2M     |
  | sealed store seal+unseal    |  ~1050    |  ~950K     |
  | SHA-256 (32 B)              |    ~37    |  ~26.7M    |
  | fingerprint from_public_key |    ~41    |  ~24.3M    |
  | pairing validate_acceptance |    <1     |   >5G      |
  | device registry register+get|   ~500    |  ~2.0M     |
  | IPC encode+decode (JSON)    |  ~1180    |  ~850K     |

- **Security audit** — `docs/phase-15/security-audit.md` reviews
  the security posture of every shipping primitive: the
  cryptographic primitives (SHA-256 fingerprinting, AES-256-GCM
  sealed store, Ed25519 manifest signing), the key-handling
  story (process-lifetime keys, sealed-store wrapping, no
  persistent plaintext secrets), the capability / permission
  policy, the audit chain integrity guarantee, IPC transport
  security (loopback-only, no auth assumed at the socket
  layer), cross-device security (paired-peer capability
  gating), the supply chain (deterministic Cargo.lock, locked
  version policy), the update mechanism (signed
  `SignedUpdate`, version policy, rollback plan), known
  limitations, and audit sign-off.

- **Compatibility matrix** — `docs/phase-15/compatibility-matrix.md`
  documents Tier 1 (reference: QEMU virtio_gpu, virtio_net,
  text console, Linux host), Tier 2 (best-effort on common
  x86_64 desktop hardware), and Tier 3 (deferred: native
  graphics via DRM/KMS, mobile SoCs, ARM server boards).

- **CI** — `.github/workflows/ci.yml` runs the build, tests,
  clippy, rustfmt, Python tests, repository contract tests,
  ShellCheck, and markdownlint on every push and pull
  request, on both `ubuntu-latest` and `windows-latest`.

- **Release notes** — `docs/RELEASE-NOTES.md` ships the 0.2.0
  highlights, test results, compatibility summary, known
  limitations, and upgrade path from 0.1.0.

- **Test results at 0.2.0** — 842 Rust tests passing across 27
  crates, 0 clippy errors, release build clean, 10 release
  binaries, `release-validate.sh` reports 9/9.

**15.2 — Bootable ISO pipeline (this commit)**

- **ISO assembly** — `scripts/iso/build-iso.sh` produces
  a hybrid ISO from the existing initramfs and the host
  kernel. The script is Linux-only: it requires
  `xorriso` and `grub-mkrescue` (from
  `grub-pc-bin` / `grub-efi-amd64-bin` /
  `grub-common`). Output:
  `build/aether-os-<version>.iso`, bootable from
  optical media or from USB via `dd` / Ventoy /
  Rufus-DD.
- **GRUB config** — Default boot, verbose boot, and
  recovery shell (`init=/bin/sh`) entries; the kernel
  command line matches the one the QEMU-from-initramfs
  runner uses so the two boot paths behave identically.
- **QEMU-from-ISO runner** — `scripts/run/qemu-iso.sh`
  picks the freshest `build/aether-os-*.iso` (or
  honours `AETHER_ISO`) and supports the same
  `--smoke` headless gate as
  `scripts/run/qemu.sh`.
- **Release validation** — A tenth step in
  `release-validate.sh` runs the ISO build on Linux
  runners and skips it on Windows runners (and with
  `--skip-iso`).
- **Contract tests** —
  `tests/python/test_release_scripts.py` (19 tests)
  asserts that every new script exists, is executable
  on POSIX, carries a bash shebang, and exposes the
  documented CLI flags.
- **Out of scope for this commit** — Hardware image
  templates for real laptop / desktop / IoT boards
  (covered by Phase 10 Real Hardware Bring-up) and a
  full installer (deferred to 15.4).

**15.3 — Privacy-safe telemetry (future, by design)**

Telemetry is off by default. When designed, it MUST be
opt-in, with a discoverable UI, a clean uninstall path, and
a documented data-set; the audit chain MUST record every
consent change.

**15.4 — Installer (future)**

Stable installer with disk partitioning (GPT + ESP),
Aether partition, recovery partition, and rollback
images. Out-of-scope for this commit.

---

## 9. Global Agent Development Rules

Every future phase MUST follow:

1. **Inspect before modifying.** Read the existing code in the area you are
   about to change.
2. **Reuse existing architecture.** Extend services; do not fork them.
3. **Never duplicate existing services.** If a service exists, use it.
4. **Never bypass Aether IPC.** All privileged actions go through the typed
   control plane.
5. **Never create arbitrary shell execution as a shortcut.** Aether is
   structured, auditable, cancellable.
6. **Every privileged action must be capability / policy controlled.**
7. **Every important action must be auditable.**
8. **Every phase must include tests.** Unit + integration + (where
   applicable) QEMU.
9. **QEMU is the reference validation platform** until real hardware is
   validated.
10. **Do not mark incomplete functionality as complete.**
11. **Do not silently skip failures.** Report them.
12. **Keep changes modular.**
13. **Keep dependencies minimal.**
14. **Prefer real functionality over placeholders.**
15. **Do not automatically start the next phase.** Finish the current
    milestone, validate, report, then move on.

---

## 10. Phase Execution Protocol

For every future phase:

| Step | Action                                                            |
| ---- | ----------------------------------------------------------------- |
| 1    | Read this `ROADMAP.md`.                                           |
| 2    | Identify the **current phase** and **current milestone**.         |
| 3    | Inspect existing implementation.                                  |
| 4    | Identify what is already complete.                                |
| 5    | Implement only the **current** milestone.                         |
| 6    | Run unit tests.                                                   |
| 7    | Run integration tests.                                            |
| 8    | Run lint / format checks.                                         |
| 9    | Run build validation.                                             |
| 10   | Run QEMU validation when applicable.                              |
| 11   | Run security validation (capability / audit / manifest admission).|
| 12   | Update documentation.                                             |
| 13   | Report exact result.                                              |

Never skip a step. If a step is skipped, report why and what was done
instead.

---

## 11. Roadmap Governance

This `ROADMAP.md` is the **authoritative project execution document**. Future
agents MUST:

1. **Read this file first.**
2. **Determine:** current phase, current milestone, dependencies, acceptance
   criteria.
3. **Work only on that milestone.**
4. **Never** skip phases.
5. **Never** invent new phases outside this list.
6. **Never** jump ahead.
7. **Never** rewrite architecture unnecessarily.
8. **Never** start future phases automatically.
9. **Never** mark incomplete work complete.

If the repository changes (new service, new phase, deprecation), update this
file as part of that change. ROADMAP updates follow the same phase-execution
protocol (inspect → update → run tests → run build → commit).

**Commit convention for roadmap updates:**

```text
docs: rebuild authoritative Aether OS master roadmap
```

---

## 12. Aether UI / UX Design Direction

### Visual language: **Premium Pastel — "Apple × Windows"**

The interface must feel:

- Premium
- Clean
- Colorful
- Friendly
- Modern
- Elegant
- Soft
- AI-native
- Professional
- High-end

It must NOT look like:

- a Linux desktop theme
- a web dashboard
- a childish toy
- a cyberpunk interface

### Visual inspiration

- **Windows 11:** desktop usability, familiar structure, centered taskbar,
  rounded windows, desktop interactions.
- **macOS:** visual polish, icon quality, spacing, typography, animation,
  premium details.
- **Pastel design systems:** soft colors, subtle gradients, friendly identity.

Do **not** copy Windows or macOS. Aether has its own identity.

### Color system

Light-first.

Primary:

- Warm white
- Soft cream

Supporting pastels:

- Pastel pink
- Soft blue
- Mint green
- Lavender
- Peach
- Soft yellow

Use colors intentionally. Do not create a rainbow interface. Calm and clean.

### Windows

- Large rounded corners
- Soft shadows
- Clean borders
- Comfortable spacing
- Subtle depth
- Smooth transitions
- Consistent controls

Windows should feel lightweight and premium.

### Taskbar

Centered, inspired by Windows 11 but redesigned for Aether.

Includes:

- Aether AI launcher
- Pinned applications
- Running applications
- System tray
- Network
- Volume
- Battery
- Clock

Visual: rounded containers, subtle transparency where appropriate, consistent
spacing.

### Application icons

Custom Aether icon language:

- Rounded-square foundation
- Soft gradients
- Subtle depth
- Consistent lighting
- Consistent proportions
- Distinctive colors
- Modern minimal shapes

Do **not** copy Apple icons.

### AI-first experience

Aether AI is the central interaction layer:

- Aether Launcher
- AI Command Bar
- AI Assistant Panel
- AI Agent Workspace
- AI Task View
- Context-aware suggestions
- Voice interaction
- AI visual feedback

The AI must feel like part of the operating system — not a separate chatbot.

### AI visual states

- `IDLE`
- `LISTENING`
- `THINKING`
- `PLANNING`
- `WORKING`
- `WAITING_FOR_PERMISSION`
- `COMPLETED`
- `ERROR`
- `RECOVERING`

Use pastel colors, subtle glow, smooth animation. Avoid aggressive effects.

### Typography

Modern, readable sans-serif. Clear hierarchy, excellent readability,
comfortable spacing, consistent weights, large headings, compact readable
system information.

### Animation

- Smooth, fast, natural, premium.
- Roughly **150–300 ms**.
- Used to communicate: window state, AI state, selection, loading,
  navigation, completion.
- Do not over-animate.

### Accessibility

- Readability
- Keyboard navigation
- Contrast
- Scaling
- Reduced motion
- Clear focus states
- Accessible controls

### Design consistency

Every screen must look like part of the same OS. Shared: spacing, color,
typography, icons, corners, shadows, animations, components.

---

## 13. Release Quality Gates

Before production:

- [ ] Boot reliability
- [ ] Graphics reliability
- [ ] Network reliability
- [ ] Storage reliability
- [ ] Application reliability
- [ ] Agent reliability
- [ ] Voice reliability
- [ ] Security
- [ ] Recovery
- [ ] Performance
- [ ] Upgrade / rollback
- [ ] Hardware compatibility

must all pass defined acceptance tests.

---

## Appendix A — Crate Inventory (verified 2026-08-29)

| Crate                                          | Path                                                | State                            |
| ---------------------------------------------- | --------------------------------------------------- | -------------------------------- |
| `aether-apps`                                  | `apps/aether-apps`                                  | manifest contracts               |
| `aether-surface`                               | `apps/aether-surface`                               | surface registration helpers     |
| `aether-calculator`                            | `apps/calculator`                                   | real graphical app               |
| `aether-notes`                                 | `apps/notes`                                        | real graphical app               |
| `aether-core`                                  | `core/aether-core`                                  | shared types (capability, IPC)   |
| `aether-security`                              | `security/aether-security`                          | permission policy                |
| `aether-storage`                               | `storage/aether-storage`                            | sandboxed FS + system info       |
| `aether-process-manager`                       | `system/aether-process-manager`                     | process lifecycle                |
| `aether-init`                                  | `system/aether-init`                                | PID1                             |
| `aether-system-core`                           | `system/aether-system-core`                         | service manager + control plane  |
| `aether-application-manager`                   | `system/aether-application-manager`                 | app registry + lifecycle         |
| `aether-agentd`                                | `services/aether-agentd`                            | agent daemon (ndjson)            |
| `aether-supervisor`                            | `services/aether-supervisor`                        | restart policies                 |
| `aether-voice`                                 | `voice/aether-voice`                                | stub                             |
| `aether-shell`                                 | `shell/aether-shell`                                | 35-command REPL                  |
| `libaether-graphics`                           | `graphics/libaether-graphics`                       | graphics primitives              |
| `aether-graphical-shell`                       | `graphics/aether-graphical-shell`                   | desktop shell (framebuffer)      |
| `aether-wm`                                    | `graphics/aether-wm`                                | window manager                   |
| `aether-sdk`                                   | `sdk/rust/aether-sdk`                               | Rust control-plane client        |
| `aetherctl`                                    | `tools/aetherctl`                                   | CLI control client               |
| `aether-agent-runtime`                         | `agent/aether-agent-runtime`                        | agent runtime library            |
| `aether-network`                               | `network/aether-network`                            | network service (typed surface)   |
| `aether-vision` (empty)                        | `vision/aether-vision`                              | placeholder                      |

---

## Appendix B — Service Manifests

| Service ID                       | Type     | Sandbox Profile   | Permission Profile | IPC Access   |
| -------------------------------- | -------- | ----------------- | ------------------ | ------------ |
| `aether-system-core`             | Internal | Internal          | SystemInternal     | LocalPublic  |
| `aether-agentd`                  | Internal | SystemService     | ServiceRuntime     | LocalPrivate |
| `aether-application-manager`     | Internal | SystemService     | ServiceRuntime     | LocalPublic  |

(See `system/services.d/*.json`.)

---

## Appendix C — Aether IPC Surface (subset)

| Command               | Service                  | Description                                            |
| --------------------- | ------------------------ | ------------------------------------------------------ |
| `status`              | `aether-system-core`     | Service + app health summary.                          |
| `start` / `stop` / `restart` | `aether-system-core` | Per-service lifecycle.                                 |
| `app.list`            | `aether-system-core`     | List discovered apps.                                  |
| `app.launch` / `app.close` / `app.status` | `aether-system-core` | App lifecycle.                                |
| `process.list` / `process.inspect` | `aether-system-core` | Process queries.                                |
| `file.list` / `file.read` / `file.create` / `file.write` / `file.search` / `file.rename` / `file.move` / `file.delete` | `aether-system-core` | Sandboxed FS.    |
| `storage.status`      | `aether-system-core`     | Storage + system info.                                 |
| `network.status` / `network.interfaces` | `aether-system-core` | Network queries.                            |
| `window.list` / `window.focus` / `window.minimize` / `window.maximize` / `window.close` | `aether-system-core` (proxied to surface server) | Window control. |
| `context.get`         | `aether-system-core`     | System context snapshot (apps, windows, services).     |
| agentd requests      | `aether-agentd` (`:4748`) | text/chat round-trips through the agent.             |

---

*End of Aether OS Master Roadmap — 2026-08-29.*
