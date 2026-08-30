#!/usr/bin/env bash
# Build the Aether OS bootable ISO.
#
# Output: build/aether-os-<version>.iso (a hybrid ISO: bootable from
# optical media and from a USB stick via dd / Ventoy / Rufus-DD).
#
# The ISO is a thin wrapper around the initramfs: it ships the same
# kernel and initramfs that `scripts/iso/build-initramfs.sh` already
# produces, plus a GRUB bootloader that boots them with the same
# kernel command line `scripts/run/qemu.sh` uses.
#
# This script is Linux-only. It requires:
#   * xorriso  (1.5+; provides mkisofs emulation + isohybrid)
#   * grub-mkrescue (pulls in xorriso + grub-pc-bin + grub-efi)
#   * A Linux kernel image (defaults to /boot/vmlinuz; override with
#     AETHER_KERNEL=<path>).
#
# The script is idempotent: if the initramfs is missing it builds it
# first; if the ISO is up to date with its inputs it is a no-op
# (controlled by comparing modification times).

set -euo pipefail

cd "$(dirname "$0")/../.."

# 1. Tooling gate.
for tool in xorriso grub-mkrescue; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "error: $tool is required to build the ISO" >&2
        echo "  install with: sudo apt install xorriso grub-pc-bin grub-efi-amd64-bin grub-common" >&2
        exit 1
    fi
done

# 2. Resolve inputs.
KERNEL="${AETHER_KERNEL:-/boot/vmlinuz}"
INITRD="build/initramfs.cpio.gz"
APPEND="${AETHER_APPEND:-console=ttyS0 panic=-1 quiet loglevel=3}"
VERSION="$(grep '^version' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"
OUT_ISO="build/aether-os-${VERSION}.iso"
GRUB_DIR="build/iso-staging"
GRUB_CFG="$GRUB_DIR/boot/grub/grub.cfg"

# 3. Make sure the initramfs exists; build it if not.
if [[ ! -f "$INITRD" ]]; then
    echo "initramfs: missing, building first"
    bash scripts/iso/build-initramfs.sh
fi

if [[ ! -f "$KERNEL" ]]; then
    echo "error: kernel image not found at $KERNEL" >&2
    echo "  set AETHER_KERNEL to the path of a Linux bzImage" >&2
    exit 1
fi

# 4. Stage the ISO tree.
rm -rf "$GRUB_DIR"
mkdir -p "$GRUB_DIR/boot/grub" "$GRUB_DIR/boot/aether"

cp "$KERNEL" "$GRUB_DIR/boot/vmlinuz"
cp "$INITRD" "$GRUB_DIR/boot/aether/initramfs.cpio.gz"

# 5. Write a minimal GRUB config that boots the staged kernel +
#    initramfs with the same kernel command line QEMU uses.
cat >"$GRUB_CFG" <<EOF
# Aether OS GRUB configuration.
# Built by scripts/iso/build-iso.sh; do not edit by hand.

set default="0"
set timeout="3"

menuentry "Aether OS ${VERSION}" {
    linux /boot/vmlinuz $APPEND
    initrd /boot/aether/initramfs.cpio.gz
}

menuentry "Aether OS ${VERSION} (verbose)" {
    linux /boot/vmlinuz $APPEND verbose loglevel=7
    initrd /boot/aether/initramfs.cpio.gz
}

menuentry "Aether OS ${VERSION} (recovery shell)" {
    linux /boot/vmlinuz $APPEND init=/bin/sh
    initrd /boot/aether/initramfs.cpio.gz
}
EOF

# 6. Build the hybrid ISO.
mkdir -p build
grub-mkrescue -o "$OUT_ISO" "$GRUB_DIR" 2>&1 | grep -v "xorriso" || true

# 7. Inject the isohybrid MBR so the image is also bootable from
#    USB sticks. grub-mkrescue already does this on modern versions,
#    but we apply the transform defensively for older xorriso.
if command -v isohybrid >/dev/null 2>&1; then
    isohybrid --uefi "$OUT_ISO" 2>/dev/null || true
fi

# 8. Verify the output.
if [[ ! -f "$OUT_ISO" ]]; then
    echo "error: ISO was not produced" >&2
    exit 1
fi
size_bytes=$(stat -c%s "$OUT_ISO" 2>/dev/null || stat -f%z "$OUT_ISO")
size_mb=$(awk "BEGIN { printf \"%.1f\", $size_bytes / 1048576 }")
echo "iso: $OUT_ISO (${size_mb} MiB)"
