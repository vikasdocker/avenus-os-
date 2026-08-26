# Bootable ISO Pipeline

The Phase 0.4 ISO boots a Linux kernel with an Aether-owned initramfs.

## Boot Components

| Component | Source | Runtime Path |
| --- | --- | --- |
| Linux kernel image | `AETHER_KERNEL_IMAGE` or `kernel/scripts/build-linux.sh` | `/boot/vmlinuz` |
| Rust PID1 | `system/aether-init` | `/init` |
| BusyBox static binary | Host or Docker image | `/bin/busybox` |
| Agent daemon | `services/aether-agentd` | `/opt/aether/bin/aether-agentd` |
| Health daemon | `system/aether-healthd` | `/opt/aether/bin/aether-healthd` |
| Service descriptors | `system/services.d` | `/etc/aether/services.d` |
| GRUB menu | Generated during ISO assembly | `/boot/grub/grub.cfg` |

## Build Initramfs

```bash
bash scripts/iso/build-initramfs.sh
```

## Build Kernel

```bash
kernel/scripts/build-linux.sh
```

The script writes the kernel image under `build/kernel/` and prints the image path.

## Build ISO

```bash
AETHER_KERNEL_IMAGE=build/kernel/linux/arch/x86/boot/bzImage bash scripts/iso/build-iso.sh
```

## Boot

```bash
bash scripts/run/qemu.sh build/iso/aether-os-dev.iso
```

The first boot target is successful when the serial console shows the Aether OS boot
banner, service launch results, and a responsive BusyBox shell.

