# Root Filesystem Overlay Policy

The active root filesystem overlay is:

```text
infra/buildroot/external/overlays/rootfs/
```

Overlay content must be Aether-owned runtime configuration, init scripts, service
configuration, or target filesystem policy. Generated files, build outputs, secrets,
credentials, and host-local data must not be placed in overlays.

