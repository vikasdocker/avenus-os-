# Aether OS Roadmap

## Phase 0.4: First Bootable Engineering Baseline

- Establish the production repository structure.
- Build Rust PID1, Rust services, C utilities, Python brain package, Qt/QML shell assets,
  ISO pipeline, QEMU launch path, CI, Docker toolchain, and developer standards.
- Boot a custom Aether initramfs on a Linux kernel in QEMU.
- Validate deterministic service startup and first local AI control-plane response.

## Phase 0.5: Local System Control Foundation

- Add policy-governed local system commands.
- Add durable task state.
- Add system inventory, device profile, and service health surfaces.
- Add signed service manifest validation.
- Expand QEMU boot tests.

## Phase 0.6: AI Brain Integration

- Replace deterministic intent prototype with model-routed local and cloud AI interfaces.
- Add short-term memory, local semantic memory, workflow planning, tool execution, and
  permission-aware action verification.
- Add offline-first AI capability profiles.

## Phase 0.7: Shell and Compositor Prototype

- Bring up the Qt/QML shell as the first visual interface.
- Integrate voice and text command surfaces.
- Add screen context and notification primitives.

## Phase 0.8: Enterprise and Security Hardening

- Add secure update verification.
- Add enterprise policy bootstrap.
- Add audit evidence retention.
- Add plugin sandbox foundation.
- Add performance and reliability release gates.

## Phase 1.3: Aether System Core and Service Manager

- Introduce the Rust Aether System Core as the service lifecycle owner.
- Load machine-readable service manifests.
- Resolve dependencies and reject missing or circular graphs.
- Start, stop, restart, supervise, recover, and shut down services.
- Expose local development control through `aetherctl` over Aether IPC.

## Phase 1.4: System Core Hardening and Runtime Validation

- Add manifest admission checks for declared security and resource intent.
- Restrict IPC request size and protect the local control socket.
- Audit every IPC authorization decision.
- Separate read, service-control, and system-control permission decisions.
- Prepare the service model for later cgroups, seccomp, namespaces, Linux capabilities,
  and MAC policy enforcement without claiming those kernel controls are active yet.

## Phase 1.0: Developer Preview

- Ship a reproducible bootable ISO.
- Support QEMU as the reference platform.
- Publish SDK contracts and developer documentation.
- Provide a signed release pipeline and validation scorecard.
