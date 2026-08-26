#!/usr/bin/env bash
# Inspect the booted kernel's GPU/DRM configuration and module availability.
K=/boot/config-6.8.0-138-generic
REL=6.8.0-138-generic
echo "== built-in vs module =="
grep -E '^CONFIG_DRM=' "$K"
grep -E '^CONFIG_FB=' "$K"
grep -E 'CONFIG_DRM_VIRTIO_GPU=|CONFIG_DRM_BOCHS=|CONFIG_FB_VESA=|CONFIG_FB_EFI=|CONFIG_FB_SIMPLE=|CONFIG_SYSFB_SIMPLEFB=|CONFIG_FRAMEBUFFER_CONSOLE=' "$K" | sort
echo "== module files on host =="
find "/lib/modules/$REL/kernel/drivers/gpu/drm/virtio" -name '*.ko*' 2>/dev/null | head -3
find "/lib/modules/$REL/kernel/drivers/gpu/drm/bochs" -name '*.ko*' 2>/dev/null | head -3
