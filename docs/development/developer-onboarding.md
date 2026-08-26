# Developer Onboarding Guide

## 1. Clone and Inspect

```bash
git status
python tools/aether-doctor.py
```

The doctor verifies repository entry points and reports which local tools are available.

## 2. Install Dependencies

On Windows, use WSL2 with Ubuntu 24.04 LTS as the primary development environment.
Follow `docs/development/supported-environment.md` before building boot images.

Use Docker as a secondary reproducible path when the Linux engine is available:

```bash
bash scripts/install-deps.sh docker
docker run --rm -it -v "$PWD:/workspace" aether-os-dev
```

On an apt-based Linux host:

```bash
sudo bash scripts/install-deps.sh native
```

## 3. Build and Test

```bash
bash scripts/build.sh
bash scripts/test.sh
bash scripts/lint.sh
```

## 4. Build Boot Artifacts

Build the Phase 1.2 Buildroot image:

```bash
bash scripts/build/bootstrap.sh
bash scripts/build/build.sh
bash scripts/run/qemu-buildroot.sh
```

Run the automated boot smoke test:

```bash
bash scripts/test-boot.sh
```

The legacy development initramfs and ISO path remains available:

```bash
bash scripts/iso/build-initramfs.sh
kernel/scripts/build-linux.sh
AETHER_KERNEL_IMAGE=build/kernel/linux-6.12.103-build/arch/x86/boot/bzImage \
  bash scripts/iso/build-iso.sh
```

## 5. Boot Locally

```bash
bash scripts/run.sh qemu build/iso/aether-os-dev.iso
```

## 6. Contribution Discipline

- Keep changes scoped.
- Keep public behavior documented.
- Add or update tests with behavior changes.
- Run the validation scripts before review.
- Include requirement references in issue and pull request descriptions when behavior changes.
