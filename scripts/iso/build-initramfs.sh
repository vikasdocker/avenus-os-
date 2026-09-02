#!/usr/bin/env bash
# Assemble the Aether OS initramfs: Aether userspace binaries + BusyBox.
# Output: build/initramfs.cpio.gz
#
# Staging happens under ${TMPDIR:-/tmp} so that repositories checked out on
# network-synced filesystems (OneDrive/Dropbox via WSL) don't suffer file
# churn while cpio packs thousands of small files.
set -euo pipefail

cd "$(dirname "$0")/../.."

# Dedicated Linux target directory keeps ELF artifacts separate from any
# Windows-host builds sharing the checkout. Prefer static musl binaries:
# the initramfs ships no libc, so PID1 must not need a dynamic interpreter.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target-linux}"

TARGET_ARGS=""
if rustup target list --installed 2>/dev/null | grep -q x86_64-unknown-linux-musl; then
    TARGET_ARGS="--target x86_64-unknown-linux-musl"
    BIN_DIR="$CARGO_TARGET_DIR/x86_64-unknown-linux-musl/release"
else
    echo "warning: musl target missing; building dynamic binaries (install with: rustup target add x86_64-unknown-linux-musl)" >&2
    BIN_DIR="$CARGO_TARGET_DIR/release"
fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/aether-initramfs.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
STAGE="$WORK/root"
mkdir -p "$STAGE"/{bin,sbin,usr/bin,proc,sys,dev,run,tmp,root,etc/aether/services.d}
chmod 1777 "$STAGE/tmp"

# 1. Aether binaries.
cargo build --release $TARGET_ARGS -p aether-init -p aether-system-core \
    -p aetherctl -p aether-shell -p aether-graphical-shell -p aether-agentd \
    -p aether-calculator -p aether-notes -p aether-sandbox

copy_binary() {
    local src="$1" dst="$2" tries=0
    until cp "$src" "$dst" 2>/dev/null; do
        tries=$((tries + 1))
        if [[ $tries -ge 10 ]]; then
            echo "failed to copy $src after $tries attempts" >&2
            exit 1
        fi
        sleep 1
    done
}

copy_binary "$BIN_DIR/aether-init"        "$STAGE/init"
copy_binary "$BIN_DIR/aether-system-core" "$STAGE/sbin/"
copy_binary "$BIN_DIR/aethersh"           "$STAGE/bin/"
copy_binary "$BIN_DIR/aetherctl"          "$STAGE/usr/bin/"
copy_binary "$BIN_DIR/aether-graphical-shell" "$STAGE/bin/"
copy_binary "$BIN_DIR/aether-calculator"     "$STAGE/bin/"
copy_binary "$BIN_DIR/aether-notes"          "$STAGE/bin/"
copy_binary "$BIN_DIR/aether-agentd"         "$STAGE/sbin/"
copy_binary "$BIN_DIR/aether-sandbox"         "$STAGE/bin/"

# Fail loudly if a dynamic binary slipped through: the initramfs has no loader.
if file "$STAGE/init" | grep -q "dynamically linked"; then
    echo "error: /init is dynamically linked; initramfs requires static musl builds" >&2
    exit 1
fi

# 2. Service manifests.
cp system/services.d/*.json "$STAGE/etc/aether/services.d/"

# 3. BusyBox with the applets PID1 and users expect.
BUSYBOX="$(command -v busybox)"
cp "$BUSYBOX" "$STAGE/bin/busybox"
for applet in sh mount umount sleep cat ls ps echo hostname poweroff reboot \
              mdev mkdir ln rm cp date grep head tail clear uname ip ifconfig \
              modprobe stty udhcpc route; do
    ln -sf busybox "$STAGE/bin/$applet"
done
ln -sf ../bin/stty "$STAGE/sbin/stty" 2>/dev/null || true

# 3b. udhcpc hook: applies the DHCP lease to eth0.
DHCP_SCRIPT="$STAGE/usr/share/udhcpc/default.script"
mkdir -p "$(dirname "$DHCP_SCRIPT")"
cat >"$DHCP_SCRIPT" <<'EOF'
#!/bin/sh
case "$1" in
    bound | renew)
        ifconfig "$interface" "$ip" netmask "${subnet:-255.255.255.0}"
        [ -n "$router" ] && route add default gw "$router" "$interface" 2>/dev/null
        ;;
esac
exit 0
EOF
chmod +x "$DHCP_SCRIPT"

# 4. GPU/DRM + NIC modules: stage virtio_gpu and virtio_net dependency chains.
REL="${AETHER_KERNEL_RELEASE:-6.8.0-138-generic}"
MODDIR="/lib/modules/$REL"
if [[ -d "$MODDIR" ]]; then
    DEPS_LIST="$WORK/module-deps.txt"
    {
        modprobe --set-version="$REL" --show-depends virtio_gpu 2>/dev/null || true
        modprobe --set-version="$REL" --show-depends bochs 2>/dev/null || true
        modprobe --set-version="$REL" --show-depends virtio_net 2>/dev/null || true
        modprobe --set-version="$REL" --show-depends psmouse 2>/dev/null || true
    } >"$DEPS_LIST"
    staged=0
    while read -r line; do
        case "$line" in
            insmod\ *)
                src="${line#insmod }"
                src="${src%% *}"
                if [[ -f "$src" ]]; then
                    rel_path="lib/modules/$REL/${src#"$MODDIR"/}"
                    mkdir -p "$STAGE/$(dirname "$rel_path")"
                    case "$src" in
                        # BusyBox insmod cannot read zstd-compressed modules;
                        # stage plain .ko so the guest loader accepts them.
                        *.zst)
                            unzstd -q -f -o "$STAGE/${rel_path%.zst}" "$src"
                            ;;
                        *)
                            cp "$src" "$STAGE/$rel_path"
                            ;;
                    esac
                    staged=$((staged + 1))
                fi
                ;;
        esac
    done <"$DEPS_LIST"
    if [[ $staged -gt 0 ]]; then
        depmod -b "$STAGE" "$REL"
        echo "modules: staged $staged for $REL (virtio_gpu + deps)"
    else
        echo "warning: no virtio_gpu module dependencies found; guest will lack /dev/dri" >&2
    fi
else
    echo "warning: $MODDIR missing; skipping module staging" >&2
fi

# 5. Pack directly to a single output file.
mkdir -p build
( cd "$STAGE" && find . -print0 | cpio --null -o --format=newc ) | gzip -9 > build/initramfs.cpio.gz

echo "initramfs: build/initramfs.cpio.gz ($(du -h build/initramfs.cpio.gz | cut -f1))"
