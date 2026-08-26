# Repository Structure

```text
apps/
  aether-apps/         Rust application manifest domain contracts
assets/
  branding/            Source branding assets
brain/
  aether_brain/        Python deterministic AI brain package
core/
  aether-core/         Shared Rust domain contracts
desktop/
  aether-desktop/      Desktop session domain contracts
docker/
  Dockerfile           Canonical development toolchain
docs/
  development/         Engineering documentation
infra/
  buildroot/           Buildroot external tree, board files, overlays, packages
  ci/                  CI policy documentation
kernel/
  configs/             Linux kernel configuration seeds
  scripts/             Kernel fetch, configure, and build helpers
network/
  aether-network/      Network domain contracts
sdk/
  rust/aether-sdk/     Rust protocol and service-contract library
  python/aether_sdk/   Python developer SDK helpers
scripts/
  debug/               QEMU and system-report debug helpers
  iso/                 Initramfs and ISO assembly
  lib/                 Shared shell functions
  release/             Release packaging
  run/                 QEMU, VirtualBox, and VMware launchers
security/
  aether-security/     Security and permission domain contracts
services/
  aether-agentd/       Local agent daemon
  aether-supervisor/   Service supervisor
shell/
  README.md            Shell-domain contract notes
storage/
  aether-storage/      Storage domain contracts
system/
  aether-init/         Rust PID1
  aether-healthd/      C health daemon
  aetherctl/           C control utility
  services.d/          Boot service descriptors
tests/
  python/              Python unit tests
  integration/         Cross-repository integration tests
  repository/          Repository policy tests
  smoke/               Bootstrapping smoke tests
tools/
  aether-doctor.py     Development environment doctor
ui/
  shell/               Qt/QML shell
vision/
  aether-vision/       Vision domain contracts
voice/
  aether-voice/        Voice domain contracts
```
