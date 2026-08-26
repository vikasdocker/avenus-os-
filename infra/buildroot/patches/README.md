# Patch Policy

Phase 1.2 does not patch Buildroot core packages or the Linux kernel.

Patches may be added only when a requirement cannot be satisfied through a defconfig,
external package, root filesystem overlay, board file, or post-build script. Every patch
must document the upstream version, reason, risk, and planned removal condition.

