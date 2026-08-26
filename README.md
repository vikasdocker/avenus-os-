# Aether OS

Aether OS is the Project Genesis operating-system repository. This tree is the first
bootable engineering baseline for an AI-native Linux-kernel operating system where the
AI control plane is a primary system component.

This repository is intentionally not a desktop theme, package remix, or distribution
customization. The first boot path is a custom initramfs containing Aether-owned native
system components, a Rust PID1, Rust services, C system utilities, and a minimal service
contract. The Linux kernel is supplied by the kernel build pipeline or by an explicitly
provided kernel image.

## Current Bootable Baseline

The Phase 0.4 baseline provides:

- Rust PID1 process: `system/aether-init`
- Rust agent daemon: `services/aether-agentd`
- Rust service supervisor: `services/aether-supervisor`
- C health daemon and C control utility: `system/aether-healthd`, `system/aetherctl`
- Python AI brain package for deterministic local intent handling: `brain/aether_brain`
- Qt/QML shell source assets: `ui/shell`
- Custom initramfs pipeline: `scripts/iso/build-initramfs.sh`
- GRUB ISO assembly pipeline: `scripts/iso/build-iso.sh`
- QEMU, VirtualBox, VMware, debug, release, lint, format, and test tooling
- Docker development environment with the required compiler and ISO toolchain

The Phase 1.2 baseline adds:

- Pinned Buildroot `2025.02.16` LTS integration through `BR2_EXTERNAL`
- Buildroot-managed Linux `6.12.103` kernel build for x86_64 QEMU
- Buildroot-generated minimal root filesystem with BusyBox
- Aether-owned `/sbin/aether-init`
- Non-AI `/usr/sbin/aether-core` service with structured logs, health state, local IPC readiness, and graceful shutdown
- Direct QEMU boot of `bzImage` plus `rootfs.ext2` over a serial console
- Boot smoke test that verifies kernel, init, service, shell, network, and shutdown stages

## Repository Map

```text
.
|-- .github/                 CI, issue templates, pull request template
|-- apps/                    Application manifest domain contracts
|-- assets/                  Source assets for docs, packaging, and shell surfaces
|-- brain/                   Python AI brain package
|-- core/                    Shared Rust domain contracts
|-- desktop/                 Desktop session domain contracts
|-- docker/                  Reproducible Linux development environment
|-- docs/                    Requirements and development documentation
|-- infra/                   Non-image operational and CI policy docs
|-- kernel/                  Linux kernel configuration and build helpers
|-- network/                 Network domain contracts
|-- sdk/                     Rust and Python SDK packages
|-- scripts/                 Build, test, ISO, run, debug, and release tooling
|-- security/                Security and permission domain contracts
|-- services/                Long-running Aether OS services
|-- shell/                   Shell-domain contracts
|-- storage/                 Storage domain contracts
|-- system/                  PID1, C system utilities, service descriptors
|-- tests/                   Repository and Python test suites
|-- tools/                   Developer tools outside the target OS image
|-- ui/                      Qt/QML shell
|-- vision/                  Vision domain contracts
`-- voice/                   Voice domain contracts
```

## Build

Bootstrap a native apt-based Linux machine:

```bash
sudo bash scripts/bootstrap-dev.sh
```

Build the Docker development image:

```bash
bash scripts/bootstrap-docker.sh
```

The host build path is intentionally simple:

```bash
bash scripts/build.sh
```

Build the Phase 1.2 Buildroot image:

```bash
bash scripts/build/bootstrap.sh
bash scripts/build/build.sh
```

For a reproducible Linux build environment:

```bash
docker build -t aether-os-dev -f docker/Dockerfile .
docker run --rm -it -v "$PWD:/workspace" aether-os-dev
bash scripts/build.sh
```

## Test

```bash
bash scripts/test.sh
```

The test script runs Rust tests when Cargo is available, CMake and C tests when CMake is
available, and Python unit tests with the system Python.

## Create the Development Initramfs

```bash
bash scripts/iso/build-initramfs.sh
```

The generated image is written to:

```text
build/iso/aether-initramfs.cpio.gz
```

## Create the Development ISO

Build a Linux kernel image:

```bash
kernel/scripts/build-linux.sh
```

Or provide an existing Linux kernel image:

```bash
AETHER_KERNEL_IMAGE=/path/to/bzImage bash scripts/iso/build-iso.sh
```

The generated ISO is written to:

```text
build/iso/aether-os-dev.iso
```

## First Boot in QEMU

For the Phase 1.2 Buildroot image:

```bash
bash scripts/run/qemu-buildroot.sh
```

For the development ISO path:

```bash
bash scripts/run/qemu.sh build/iso/aether-os-dev.iso
```

The default QEMU profile enables serial output, local reboot behavior, 2 GiB RAM, and a
modern x86_64 machine profile suitable for the initial boot milestone.

## Boot Smoke Test

After building the Buildroot image:

```bash
bash scripts/test-boot.sh
```

The boot test fails if QEMU does not reach the kernel, Aether init, Aether Core, shell,
network, and clean shutdown milestones.

## VirtualBox and VMware

```bash
bash scripts/run/virtualbox.sh build/iso/aether-os-dev.iso
bash scripts/run/vmware.sh build/iso/aether-os-dev.iso
```

On Windows hosts with the tools installed on `PATH`:

```powershell
.\scripts\run\qemu.ps1 -IsoPath build\iso\aether-os-dev.iso
.\scripts\run\virtualbox.ps1 -IsoPath build\iso\aether-os-dev.iso
.\scripts\run\vmware.ps1 -IsoPath build\iso\aether-os-dev.iso
```

## Development Standards

Read these documents before changing the tree:

- `docs/development/getting-started.md`
- `docs/development/supported-environment.md`
- `docs/development/build-system.md`
- `docs/development/bootable-iso.md`
- `docs/development/dependency-installation.md`
- `docs/development/phase-1-4-runtime-validation.md`
- `docs/development/developer-onboarding.md`
- `docs/development/iso-bootstrap-guide.md`
- `docs/development/coding-standards.md`
- `docs/development/testing.md`
- `docs/development/repository-structure.md`
- `docs/development/repository-standards.md`
- `docs/build/buildroot-foundation.md`
- `docs/build/artifacts.md`
- `docs/architecture/phase-1-2-boot-flow.md`
- `docs/architecture/root-filesystem.md`
- `docs/testing/boot-smoke-tests.md`
