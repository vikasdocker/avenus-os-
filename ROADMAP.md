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
   - [Phase 9 — Application Platform](#phase-9--application-platform) **NOT_STARTED**
   - [Phase 10 — Real Hardware Bring-up](#phase-10--real-hardware-bring-up) **NOT_STARTED**
   - [Phase 11 — Security + Trusted AI](#phase-11--security--trusted-ai) **PARTIAL**
   - [Phase 12 — Self-Updating + System Lifecycle](#phase-12--self-updating--system-lifecycle) **NOT_STARTED**
   - [Phase 13 — Aether Autonomous OS](#phase-13--aether-autonomous-operating-system) **NOT_STARTED**
   - [Phase 14 — Multi-Device Aether](#phase-14--multi-device-aether) **NOT_STARTED**
   - [Phase 15 — Production Release](#phase-15--production-release) **NOT_STARTED**
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

**Status:** `IN_PROGRESS` (foundation only; full design system is the goal).

**Objective:** Define and implement the final Aether graphical identity. The
entire OS must share one design system.

The complete UI/UX direction is in [§12 — Aether UI / UX Design Direction](#12-aether-ui--ux-design-direction).

**Sub-milestones:**

- 6.1 Design tokens (color, type, spacing, radius, motion).
- 6.2 Component library (button, card, list, dialog, panel, nav, taskbar,
  launcher).
- 6.3 Aether Launcher (AI-first central UI).
- 6.4 AI Command Bar.
- 6.5 AI Assistant Panel + AI Agent Workspace + AI Task View.
- 6.6 AI visual states (IDLE, LISTENING, THINKING, PLANNING, WORKING,
  WAITING_FOR_PERMISSION, COMPLETED, ERROR, RECOVERING).
- 6.7 Iconography (rounded-square, soft gradients, custom Aether language).
- 6.8 Animation system (150–300 ms; smooth, premium, non-aggressive).
- 6.9 Accessibility (contrast, keyboard nav, scaling, reduced motion, focus
  rings).

**Dependencies:** Phase 1.9 (graphical OS) and Phase 3 (conversational
Aether) for AI surfaces.

**Acceptance:** every screen uses tokens from the design system; visual review
passes; accessibility checks pass.

**Known limitations:** today `aether-graphical-shell` uses a dark
monospace-painted UI; the pastel / premium identity in §12 is the target.

---

### Phase 7 — Aether Agent Deep System Control

**Status:** `NOT_STARTED`.

**Objective:** Make Aether capable of operating the entire OS.

**Sub-milestones:**

- 7.1 System diagnostics ("Why is my computer slow?" → collect, analyze,
  explain, fix).
- 7.2 Self-healing (bounded recovery: service restart, network recovery,
  application recovery, dependency recovery, resource recovery).
- 7.3 System automation (user-defined workflows, e.g. morning setup).
- 7.4 Background agent (disk space, service failure, network failure, app
  crash, battery, security events).

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

**Status:** `NOT_STARTED`.

**Sub-milestones:**

- 9.1 Aether SDK (app, UI, Agent, IPC, capabilities, manifests).
- 9.2 Application packaging (format, manifest, signing, dependencies,
  permissions, resources).
- 9.3 Application security (sandbox, filesystem scopes, network scopes,
  device scopes, process limits).
- 9.4 Aether Store (discovery, install, updates, ratings, permissions,
  publisher, signatures).

**Dependencies:** Phase 1.4, Phase 2.

**Acceptance:** third-party Aether app can be installed, sandboxed, and
updated.

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

**Status:** `PARTIAL` (capability/policy/audit present; kernel sandboxing
deferred to this phase).

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
- 11.4 **Declarative kernel-sandbox plan — PARTIAL (planning layer shipped,
  enforcement deferred to `aether-sandbox` binary)**. `core/aether-core/src/sandbox.rs`
  defines a typed `SandboxPlan` for each `SandboxProfile`:
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
  **The actual prctl(2) / unshare(2) / cgroupfs write / seccomp(2)
  invocation lives in a future `aether-sandbox` binary that runs on
  the Aether OS image; this layer is the declarative contract.**
  Evidence: `core/aether-core/src/sandbox.rs` (9 unit tests covering
  determinism, serde round-trip, distinct cgroup slices, distinct
  seccomp tags, missing-capability safety), and
  `system/aether-system-core/src/manager.rs` (5 unit tests covering
  every profile, the unknown-service `None` return, and the
  `all_sandbox_plans` iterator).
- Kernel primitives where appropriate: Linux capabilities, namespaces, cgroups,
  seccomp, MAC policy, sandboxing, signed applications, signed updates,
  credential protection, secret storage, audit retention, policy management.

**Dependencies:** Phase 1.4, Phase 2.

**Acceptance:** a documented attack surface; documented defenses; integration
tests that exercise the defenses.

---

### Phase 12 — Self-Updating + System Lifecycle

**Status:** `NOT_STARTED`.

**Sub-milestones:**

- OS update system.
- Application updates.
- Rollback.
- Recovery environment.
- Version management.
- Signed update verification.
- Atomic updates where practical.
- Safe boot / recovery.

**Dependencies:** Phase 11.

---

### Phase 13 — Aether Autonomous OS

**Status:** `NOT_STARTED`.

**Sub-milestones:**

- Aether becomes a genuinely proactive OS agent: understand, observe, plan,
  execute, verify, recover, learn from allowed feedback.
- Aether proposes action rather than silently making dangerous changes.
- Examples: "your storage is nearly full", "network connectivity is unstable",
  "an application crashed repeatedly", "your development environment is missing
  a dependency".

**Dependencies:** Phase 7, Phase 11, Phase 12.

---

### Phase 14 — Multi-Device Aether

**Status:** `NOT_STARTED`.

**Sub-milestones:**

- Same Aether identity and agent model coordinate across phone, tablet,
  laptop, desktop, IoT, home devices, external displays.
- Explicit pairing is mandatory.
- Security first.

**Dependencies:** Phase 13.

---

### Phase 15 — Production Release

**Status:** `NOT_STARTED`.

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
