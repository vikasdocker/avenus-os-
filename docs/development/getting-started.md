# Getting Started

This guide brings a new engineer from a clean checkout to a validated local build.

## Prerequisites

The recommended path is Docker:

```bash
docker build -t aether-os-dev -f docker/Dockerfile .
docker run --rm -it -v "$PWD:/workspace" aether-os-dev
```

Native Linux development requires:

- Rust stable toolchain
- `x86_64-unknown-linux-musl` Rust target for static initramfs builds
- CMake 3.22 or newer
- Ninja or Make
- C compiler
- Python 3.11 or newer
- BusyBox static binary
- `cpio`
- `xorriso`
- GRUB rescue tooling
- QEMU for first boot

## First Validation

```bash
bash scripts/build.sh
bash scripts/test.sh
```

## First Initramfs

```bash
bash scripts/iso/build-initramfs.sh
```

## First ISO

Use a kernel image built by `kernel/scripts/build-linux.sh` or provide an existing Linux
kernel image:

```bash
AETHER_KERNEL_IMAGE=/path/to/bzImage bash scripts/iso/build-iso.sh
```

## First Boot

```bash
bash scripts/run/qemu.sh build/iso/aether-os-dev.iso
```

