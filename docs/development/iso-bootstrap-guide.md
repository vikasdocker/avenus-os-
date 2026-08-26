# ISO and Buildroot Bootstrap Guide

## Buildroot Image Path

Phase 1.2 uses Buildroot as the primary bootable image pipeline.

```bash
bash scripts/build/bootstrap.sh
bash scripts/build/build.sh
bash scripts/run/qemu-buildroot.sh
bash scripts/test-boot.sh
```

Primary generated artifacts:

```text
artifacts/buildroot/output/images/bzImage
artifacts/buildroot/output/images/rootfs.ext2
artifacts/buildroot/output/images/SHA256SUMS
```

## Legacy Development ISO Path

## Stage 1: Validate Source

```bash
bash scripts/build.sh
bash scripts/test.sh
bash scripts/lint.sh
```

## Stage 2: Build Initramfs

```bash
bash scripts/iso/build-initramfs.sh
```

This produces:

```text
build/iso/aether-initramfs.cpio.gz
```

## Stage 3: Build Kernel

```bash
kernel/scripts/build-linux.sh
```

The default kernel version is controlled by `AETHER_KERNEL_VERSION`. The initial default
is `6.12.103`.

## Stage 4: Build ISO

```bash
AETHER_KERNEL_IMAGE=build/kernel/linux-6.12.103-build/arch/x86/boot/bzImage \
  bash scripts/iso/build-iso.sh
```

This produces:

```text
build/iso/aether-os-dev.iso
```

## Stage 5: First Boot

```bash
bash scripts/run.sh qemu build/iso/aether-os-dev.iso
```

Successful first boot shows the Aether OS banner, starts local services, and provides a
BusyBox shell through the boot console.
