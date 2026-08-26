#!/usr/bin/env bash
set -euo pipefail

TARGET_DIR="${1:?target directory is required}"
SOURCE_ROOT="${AETHER_SOURCE_ROOT:-unknown}"

mkdir -p \
  "$TARGET_DIR/bin" \
  "$TARGET_DIR/sbin" \
  "$TARGET_DIR/etc/aether" \
  "$TARGET_DIR/dev" \
  "$TARGET_DIR/proc" \
  "$TARGET_DIR/sys" \
  "$TARGET_DIR/run/aether/health" \
  "$TARGET_DIR/run/aether/ipc" \
  "$TARGET_DIR/tmp" \
  "$TARGET_DIR/var/log/aether" \
  "$TARGET_DIR/home" \
  "$TARGET_DIR/usr/bin" \
  "$TARGET_DIR/usr/sbin"

chmod 1777 "$TARGET_DIR/tmp"
chmod 0755 "$TARGET_DIR/sbin/aether-init"
chmod 0755 "$TARGET_DIR/sbin/aether-shutdown"
chmod 0755 "$TARGET_DIR/usr/sbin/aether-core"
chmod 0755 "$TARGET_DIR/usr/sbin/aether-filesystemd"
chmod 0755 "$TARGET_DIR/usr/sbin/aether-system-core"
chmod 0755 "$TARGET_DIR/usr/bin/aetherctl"
chmod 0755 "$TARGET_DIR/usr/share/udhcpc/default.script"
ln -sf /sbin/aether-init "$TARGET_DIR/init"

cat > "$TARGET_DIR/etc/aether/build-manifest" <<MANIFEST
project=Aether OS
phase=1.5
buildroot_version=${AETHER_BUILDROOT_VERSION:-2025.02.16}
linux_version=${AETHER_LINUX_VERSION:-6.12.103}
target_arch=x86_64
target_board=aether_x86_64_qemu
toolchain=buildroot-internal-musl
source_tree=$SOURCE_ROOT
br2_external=AETHER
MANIFEST

printf 'Aether OS Phase 1.5\n' > "$TARGET_DIR/etc/hostname"
