#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/lib/common.sh
source "$ROOT/scripts/lib/common.sh"

KERNEL_VERSION="${AETHER_KERNEL_VERSION:-6.12.103}"
KERNEL_MAJOR="${KERNEL_VERSION%%.*}"
SOURCE_ARCHIVE="$ROOT/build/downloads/linux-$KERNEL_VERSION.tar.xz"
SOURCE_DIR="$ROOT/build/kernel/linux-$KERNEL_VERSION"
BUILD_DIR="$ROOT/build/kernel/linux-$KERNEL_VERSION-build"
CONFIG_FRAGMENT="$ROOT/kernel/configs/aether-x86_64.config"
KERNEL_URL="${AETHER_KERNEL_URL:-https://cdn.kernel.org/pub/linux/kernel/v$KERNEL_MAJOR.x/linux-$KERNEL_VERSION.tar.xz}"

need_cmd curl
need_cmd tar
need_cmd make

if command -v nproc >/dev/null 2>&1; then
  JOBS="${AETHER_BUILD_JOBS:-$(nproc)}"
else
  JOBS="${AETHER_BUILD_JOBS:-2}"
fi

make_dir "$ROOT/build/downloads"
make_dir "$ROOT/build/kernel"

if [[ ! -f "$SOURCE_ARCHIVE" ]]; then
  log "downloading Linux $KERNEL_VERSION from $KERNEL_URL"
  curl -fL "$KERNEL_URL" -o "$SOURCE_ARCHIVE"
else
  log "using cached kernel archive $SOURCE_ARCHIVE"
fi

if [[ -n "${AETHER_KERNEL_SHA256:-}" ]]; then
  need_cmd sha256sum
  printf '%s  %s\n' "$AETHER_KERNEL_SHA256" "$SOURCE_ARCHIVE" | sha256sum -c -
fi

if [[ ! -d "$SOURCE_DIR" ]]; then
  log "extracting Linux $KERNEL_VERSION"
  tar -C "$ROOT/build/kernel" -xf "$SOURCE_ARCHIVE"
fi

make_dir "$BUILD_DIR"
log "configuring Linux kernel"
make -C "$SOURCE_DIR" O="$BUILD_DIR" x86_64_defconfig
"$SOURCE_DIR/scripts/kconfig/merge_config.sh" -O "$BUILD_DIR" "$BUILD_DIR/.config" "$CONFIG_FRAGMENT"
make -C "$SOURCE_DIR" O="$BUILD_DIR" olddefconfig

log "building Linux kernel with $JOBS jobs"
make -C "$SOURCE_DIR" O="$BUILD_DIR" -j "$JOBS" bzImage

KERNEL_IMAGE="$BUILD_DIR/arch/x86/boot/bzImage"
[[ -f "$KERNEL_IMAGE" ]] || fail "kernel image was not produced at $KERNEL_IMAGE"
log "kernel image ready: $KERNEL_IMAGE"
printf '%s\n' "$KERNEL_IMAGE"

