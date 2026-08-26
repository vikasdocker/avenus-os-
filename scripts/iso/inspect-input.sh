#!/usr/bin/env bash
# Input device availability check: QEMU virtio-input + evdev support.
K=/boot/config-6.8.0-138-generic
REL=6.8.0-138-generic
echo "== kernel config =="
grep -E '^CONFIG_VIRTIO_INPUT=|^CONFIG_VIRTIO_KEYBOARD=|^CONFIG_VIRTIO_MOUSE=|^CONFIG_TABLET_SERIAL' "$K"
grep -E '^CONFIG_INPUT_EVDEV=|^CONFIG_KEYBOARD_ATKBD=|^CONFIG_MOUSE_PS2=|^CONFIG_SERIO_I8042=' "$K"
echo "== modules present =="
find "/lib/modules/$REL/kernel/drivers/input" -name '*.ko*' 2>/dev/null | grep -iE 'virtio|evdev|atkbd|psmouse' | head -8
