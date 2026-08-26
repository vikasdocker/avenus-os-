# Docker Development Environment

The Docker image provides the canonical Linux toolchain for the Phase 0.4 repository:

- Rust and Cargo
- musl target support for static initramfs binaries
- CMake and Ninja
- GCC and Clang
- Python 3
- BusyBox static binary
- GRUB ISO tooling
- QEMU
- ShellCheck

Build the image:

```bash
docker build -t aether-os-dev -f docker/Dockerfile .
```

Start a development shell:

```bash
docker run --rm -it -v "$PWD:/workspace" aether-os-dev
```

Inside the container:

```bash
bash scripts/build.sh
bash scripts/test.sh
bash scripts/iso/build-initramfs.sh
```

