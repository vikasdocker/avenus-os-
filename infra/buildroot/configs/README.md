# Buildroot Configurations

The active Phase 1.2 Buildroot defconfig is maintained in the external tree:

```text
infra/buildroot/external/configs/aether_x86_64_qemu_defconfig
```

This directory records configuration policy for future targets. New Buildroot defconfigs
must remain target-specific, reproducible, and free of host-local paths. The first
supported target is `aether_x86_64_qemu`.

