# Dependency Installation Guide

## Supported Windows Path

Windows workstations must use WSL2 with Ubuntu 24.04 LTS for Buildroot, Linux kernel,
Rust, C/C++, QEMU, and runtime security validation. Build from the WSL Linux filesystem,
not directly from `/mnt/c`, when possible.

From elevated PowerShell, only if WSL2 is not already installed:

```powershell
wsl --install -d Ubuntu-24.04
wsl --set-default-version 2
wsl --update
```

Inside Ubuntu 24.04:

```bash
sudo apt update
sudo apt install -y \
  bc bison build-essential busybox-static ca-certificates clang clang-format \
  cmake cpio curl file flex git grub-pc-bin g++ gcc libelf-dev libssl-dev \
  make mtools musl-tools ninja-build patch perl python3 python3-venv \
  qemu-system-x86 qemu-utils rsync shellcheck unzip wget xorriso
```

Install Rust before running the native bootstrap:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup default stable
rustup component add clippy rustfmt
rustup target add x86_64-unknown-linux-musl
```

## Docker Path

```bash
bash scripts/install-deps.sh docker
docker run --rm -it -v "$PWD:/workspace" aether-os-dev
```

This path provides Rust, CMake, C compiler tooling, Python, BusyBox, GRUB ISO tools, and
QEMU in a consistent Linux environment.

## Native apt-Based Linux Path

```bash
sudo bash scripts/install-deps.sh native
```

The native path installs:

- Linux build essentials
- Kernel build dependencies
- Buildroot host dependencies
- Rust musl target support when `rustup` is already installed
- CMake and Ninja
- Python 3
- BusyBox static binary
- GRUB ISO tools
- QEMU
- ShellCheck
- Clang Format

The Buildroot path also requires standard source-build tools including `make`, `patch`,
`perl`, `rsync`, `unzip`, `wget`, `bc`, `bison`, `flex`, `libelf-dev`, and `libssl-dev`.

## Windows Host Path

Use WSL2 Ubuntu 24.04 as the primary path. Docker Desktop is supported only when the
Linux engine is running and WSL integration is enabled for the Ubuntu distro. Build
artifacts that require Rust, CMake, GRUB, BusyBox, Buildroot, and QEMU must be produced
inside Linux.
