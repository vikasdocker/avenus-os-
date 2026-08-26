# Linux Kernel Integration

Aether OS uses the Linux kernel as its hardware and process substrate. The Phase 0.4
repository does not vendor kernel source. The kernel helper downloads a declared Linux
source release, applies the Aether seed configuration, and builds an x86_64 boot image.

The default version is a longterm kernel release. Override it when validating a newer
stable or longterm line:

```bash
AETHER_KERNEL_VERSION=6.12.103 kernel/scripts/build-linux.sh
```

The resulting kernel image is printed at the end of the build and can be passed to the
ISO pipeline:

```bash
AETHER_KERNEL_IMAGE=build/kernel/linux-6.12.103/arch/x86/boot/bzImage \
  bash scripts/iso/build-iso.sh
```

