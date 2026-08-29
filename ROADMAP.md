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
   - [Phase 3 — Conversational Aether](#phase-3--conversational-aether) **NOT_STARTED**
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
| Tests                            | `cargo test --workspace` — **242 passed, 0 failed, 1 ignored** (all Rust crates).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
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
| Agent runtime (library)          | `agent/aether-agent-runtime` — Session, Request, Intent, Action, Tool, Validator, Executor (IPC-only), Observation, Planner, Approval, Cancellation, Memory, LLM (Mock + Echo providers), Audit, Events, Errors. **Library is wired to Aether IPC; not yet embedded inside `aether-agentd`.**                                                                                                                                                                                                                                                                            |
| Agent daemon                     | `services/aether-agentd` — bounded event ring, task state, conversation context, intent classifier, planner, confirmation, ndjson TCP (`4748`) and stdio. EchoProvider default; provider is replaceable.                                                                                                                                                                                                                                                                                                                                                                |
| LLM provider                     | `LlmProvider` trait with `MockLlmProvider` and `EchoLlmProvider`; no real Ollama/OpenAI backend in repo yet.                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Voice                            | `voice/aether-voice` — empty stub (`lib.rs` only, ~57 lines, no STT/TTS/wake-word).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Vision                           | `vision/aether-vision` — `src/` directory empty. No implementation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| SDK                              | `sdk/rust/aether-sdk` — TCP control-plane client. `sdk/python/aether_sdk` — wire-protocol helpers (`AETHER/1`).                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Tooling                          | `tools/aetherctl` — CLI control client. `tools/aether-process-manager` (Cargo listed) and related tooling.                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Security                         | `core/aether-core` Capability + RiskLevel types. `security/aether-security` `DefaultPermissionPolicy` (allow / require-consent / deny). Manifests declare `sandbox_profile`, `permission_profile`, `ipc_access`, `capabilities`, `resource_*`. System core audits every capability request. **No actual kernel sandboxing (cgroups/seccomp/namespaces) is enforced yet — that is honest text in the existing code, not a lie.**                                                                                                                                                                                                                   |
| Documentation                    | `docs/development/*` (16 files), `docs/architecture/*` (10), `docs/security/*` (2), `docs/testing/*` (1), `docs/build/*` (2), `docs/phase-1-8/*` (10).                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Tests                            | `tests/boot/*`, `tests/integration/*`, `tests/python/*`, `tests/repository/*`, `tests/smoke/*` — Python harness wired via `scripts/test.sh`. Rust integration tests live inside each crate's `tests/` directory (most are currently empty).                                                                                                                                                                                                                                                                                                                              |

**Current phase:** **Phase 1** (Core Operating System) — Parts A, B, C, F, H are
substantially complete. Part D (security hardening) is partial. Part E (filesystem/
storage) is complete. Part G (network) is partial (control-plane commands only, no
dedicated service). Part I (graphical OS) is in progress — software-framebuffer
multi-window desktop is working in QEMU; native DRM/KMS backend not yet implemented.

**Next milestone:** **Phase 1.4 / 1.7 / 1.9 closure** — complete remaining Phase 1
sub-milestones, then transition to Phase 2 (Aether Agent Core) end-to-end wiring
(embed the agent-runtime inside `aether-agentd` and exercise intent → plan → action
→ observation against the live system).

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

**Status:** `PARTIAL` (control-plane commands only; no dedicated service crate).

**Completed:**

- IPC commands for status, interfaces, connectivity, configuration, events,
  statistics (`network.status`, `network.interfaces`, …) declared in
  `system/aether-system-core/src/main.rs` and exposed to shell / agentd / runtime.

**Not yet implemented:**

- Dedicated `aether-network` service crate (`network/aether-network/src/` is empty).
- Real DNS / DHCP / Wi-Fi / Ethernet / Bluetooth / VPN control.
- Per-application network scopes.
- Network event subscription.

**Acceptance (current):** status + interface queries return values; **the dedicated
service is a Phase 1.7 follow-up milestone**, not a blocker for the rest of Phase 1.

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
- Close the dedicated `aether-network` service crate.
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

**Status:** `IN_PROGRESS` (most building blocks present; end-to-end wiring
inside `aether-agentd` is the open milestone).

**Objective:** Establish the actual OS agent.

#### 2.1 Agent Runtime

**Status:** `IN_PROGRESS`.

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
  - `LlmProvider` trait + `MockLlmProvider` + `EchoLlmProvider`.
  - `Audit` + `Events` + `Errors`.

**Open:** `aether-agent-runtime` is **not yet embedded** inside `aether-agentd`;
today `aether-agentd` re-implements the daemon side with its own
`intent/context/planner/conversation/confirmation` modules. The two stacks must
converge on the runtime as the single source of truth.

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

**Status:** `IN_PROGRESS`.

`aether-agentd` already talks to:
- `aether-system-core` (port 4747) for system/app/process/network/filesystem
  queries.
- `aether-graphical-shell` (port 4750, surface server) for window operations.

**Open:** the `aether-agent-runtime` library is not the executor; the daemon
has its own implementation. Phase 2.4 closes this gap.

#### 2.5 LLM Provider Layer

**Status:** `IN_PROGRESS` (trait + mocks only).

`LlmProvider` trait exists. Implementations: `MockLlmProvider`, `EchoLlmProvider`.
**No real Ollama / OpenAI-compatible / local-inference backend** is in the
repository. Phase 2.5 ships the first non-mock provider.

#### 2.6 Structured AI Output

**Status:** `IN_PROGRESS` (intent + plan + action are typed; no schema
enforcement for LLM output yet).

The intent and plan types are typed Rust structs with `serde` derive. The
`LlmProvider::structured_output` method exists but only attempts naive JSON
parse. The next step is JSON-schema-driven parsing + invalid-output rejection.

#### 2.7 Planning

**Status:** `IN_PROGRESS`.

`aether-agentd::planner` and `aether-agent-runtime::planner` both exist. Multi-step
plans, dependencies, expected outcomes, failure recovery, bounded execution,
cancellation: the types are present. **Bounded execution semantics and
recovery policies are partially specified; full semantics land in Phase 2.7.**

#### 2.8 Observation + Recovery

**Status:** `IN_PROGRESS`.

`Observation` types mirror every `ActionVariant`. The agent can evaluate the
result of an action. **Automated recovery / retry policy** is not yet
defined; current behavior is fail-and-report.

#### 2.9 Agent Memory Foundation

**Status:** `IN_PROGRESS`.

`ConversationMemory` + `SessionMemory` exist as runtime types. The daemon
also has `ConversationContext` for pronoun resolution. **No persistent
long-term memory yet.**

**Dependencies:** Phase 1.3 (system core), Phase 1.5 (storage), Phase 1.9
(graphical surface).

**Security requirements:** every agent action is capability-checked, audited,
and (for Medium+ risk) requires consent. The agent must never be the source of
truth for the OS; services are.

**Performance requirements:** agent round-trip (intent → plan → action →
observation) must remain interactive (< 1 s for simple actions in QEMU).

**Known limitations:** LLM provider is mock/echo; structured output is not
schema-enforced; runtime is not embedded inside `aether-agentd`.

**Acceptance criteria (Phase 2 closure):**

- [ ] `aether-agentd` uses `aether-agent-runtime` for session / intent / plan /
      executor / observation.
- [ ] At least one real LLM provider (Ollama or OpenAI-compatible) is wired.
- [ ] Structured-output schema validation is enforced.
- [ ] Memory persists across session restart.
- [ ] End-to-end demo: user text → agentd → intent → plan → action via IPC →
      service → observation → agentd → response → UI.
- [ ] All agent tests pass (`cargo test -p aether-agent-runtime`,
      `cargo test -p aether-agentd`).

---

### Phase 3 — Conversational Aether

**Status:** `NOT_STARTED`.

**Objective:** Make Aether naturally conversational.

**Sub-milestones:**

- 3.1 Natural-language command interface (typed, context-aware).
- 3.2 AI-driven system control (apps, files, network, system, windows, settings,
  devices).
- 3.3 Permission interaction (explain action, ask for approval, support cancel).
- 3.4 Task progress visibility (thinking, planning, working, waiting, completed,
  failed, recovering).

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
| `aether-network` (empty)                       | `network/aether-network`                            | placeholder                      |
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
