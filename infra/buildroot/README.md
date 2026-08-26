# Buildroot Foundation

Aether OS Phase 1.2 uses Buildroot as the initial image construction framework. Buildroot
is isolated under this infrastructure area and consumed through a `BR2_EXTERNAL` tree.
Aether-owned services, init scripts, overlays, kernel configuration, and board policy are
kept outside Buildroot core source.

## Pinned Versions

The pinned versions are recorded in `versions.env`.

| Component | Value |
| --- | --- |
| Buildroot | 2025.02.16 LTS |
| Linux kernel | 6.12.103 |
| Architecture | x86_64 |
| Primary target | QEMU |
| Toolchain | Buildroot internal musl toolchain with GCC 13.x |

## Build

```bash
bash scripts/build/bootstrap.sh
bash scripts/build/build.sh
```

## Output

Buildroot output is written to:

```text
artifacts/buildroot/output
```

The primary boot artifacts are:

```text
artifacts/buildroot/output/images/bzImage
artifacts/buildroot/output/images/rootfs.ext2
```
