# Phase 1.4 Runtime Validation Report

## Status

`PHASE 1.4 BLOCKED`

Phase 1.4 source-level validation passes, but runtime closure is blocked because the Linux development toolchain required for Rust, Buildroot, and QEMU validation is not installed in the active WSL2 environment.

## Environment Audit

| Area | Result |
| --- | --- |
| Windows OS | Microsoft Windows 11 Pro 10.0.26200 build 26200 |
| Windows architecture | AMD64 |
| CPU | 12th Gen Intel Core i7-12700H |
| CPU topology | 14 cores, 20 logical processors |
| RAM | 39.63 GiB |
| Windows C: disk | 31.93 GiB free of 475.38 GiB, 94 percent used |
| WSL distro | Ubuntu 24.04.2 LTS |
| WSL version | WSL 2.4.12.0, default distro `Ubuntu-24.04`, default version 2 |
| WSL kernel | 5.15.167.4-microsoft-standard-WSL2 |
| WSL root disk | 954 GiB available on `/` |
| Workspace mount | `/mnt/c`, 32 GiB available |
| Virtualization | Hypervisor detected; virtualization-based security running; WSL CPU virtualization flags present |
| Docker | Docker CLI 29.6.1 installed on Windows; Docker Desktop Linux engine unavailable; WSL integration unavailable |

## Tool Audit

| Tool | Windows Host | WSL Ubuntu 24.04 |
| --- | --- | --- |
| `git` | Present, 2.53.0.windows.1 | Present, 2.43.0 |
| `bash` | Present through WSL, 5.2.21 | Present, 5.2.21 |
| `make` | Missing | Missing |
| `gcc` | Missing | Missing |
| `clang` | Missing | Missing |
| `cmake` | Missing | Missing |
| `rustc` | Missing | Missing |
| `cargo` | Missing | Missing |
| `python3` | Present through Windows launcher, 3.14.5 | Present, 3.12.3 |
| `qemu-system-x86_64` | Missing | Missing |
| `docker` | CLI present, daemon unavailable | Docker Desktop integration unavailable |

## Source-Level Validation Completed

| Validation | Command | Result |
| --- | --- | --- |
| Python tests | `bash scripts/test.sh` | Passed |
| Integration tests | `bash scripts/test.sh` | Passed |
| Smoke tests | `bash scripts/test.sh` | Passed |
| Repository policy | `bash scripts/test.sh` | Passed |
| Lint wrapper | `bash scripts/lint.sh` | Passed with Rust, C, and ShellCheck portions skipped due missing tools |
| Build wrapper | `bash scripts/build.sh` | Passed with Rust and C portions skipped due missing tools |
| Python compilation | `python -m compileall -q brain sdk/python tests tools` | Passed |
| Shell syntax | `bash -n` over scripts and Buildroot overlay shell files | Passed |
| Forbidden marker scan | Repository policy scan excluding generated trees | Passed |

## Runtime Validation Not Yet Completed

| Validation | Blocker |
| --- | --- |
| Rust workspace build and tests | `cargo` and `rustc` are missing in WSL |
| CMake build and tests | `cmake`, GCC/Clang toolchain are missing in WSL |
| Buildroot build | `make` and build dependencies are missing in WSL |
| Linux kernel compilation | Buildroot toolchain path is blocked by missing build dependencies |
| Root filesystem generation | Buildroot build cannot start without `make` |
| Aether System Core target compilation | Buildroot Rust package cannot compile without Rust host tooling |
| QEMU boot | `qemu-system-x86_64` is missing |
| IPC runtime validation | Requires a running Aether System Core in the generated image |
| Audit runtime validation | Requires a booted image with `/var/log/aether/aether-audit.log` |
| Security policy runtime validation | Requires a running daemon and local IPC socket |

## Required Installation Commands

Run inside Ubuntu 24.04 WSL:

```bash
sudo apt update
sudo apt install -y \
  bc \
  bison \
  build-essential \
  busybox-static \
  ca-certificates \
  clang \
  clang-format \
  cmake \
  cpio \
  curl \
  file \
  flex \
  git \
  grub-pc-bin \
  g++ \
  gcc \
  libelf-dev \
  libssl-dev \
  make \
  mtools \
  musl-tools \
  ninja-build \
  patch \
  perl \
  python3 \
  python3-venv \
  qemu-system-x86 \
  qemu-utils \
  rsync \
  shellcheck \
  unzip \
  wget \
  xorriso
```

Install Rust inside WSL:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup default stable
rustup component add clippy rustfmt
rustup target add x86_64-unknown-linux-musl
```

## Validation Commands After Installation

```bash
cd "$HOME/src/aether-os"
python3 tools/aether-doctor.py
cargo test --workspace
bash scripts/test.sh
bash scripts/lint.sh
bash scripts/build.sh
bash scripts/build/bootstrap.sh
bash scripts/build/build.sh
bash scripts/test-boot.sh
```

Expected result:

| Command | Expected Result |
| --- | --- |
| `python3 tools/aether-doctor.py` | Required tools present; optional runtime tools available for Phase 1.4 validation. |
| `cargo test --workspace` | Rust crates compile and tests pass. |
| `bash scripts/test.sh` | Rust, CMake, Python, integration, smoke, and policy tests pass. |
| `bash scripts/lint.sh` | Rustfmt, Clippy, clang-format, ShellCheck, and policy checks pass. |
| `bash scripts/build.sh` | Rust, C/C++, and Python validation complete without skipped build stages. |
| `bash scripts/build/build.sh` | Buildroot configures, compiles Linux, builds rootfs, compiles Aether packages, and writes image artifacts. |
| `bash scripts/test-boot.sh` | QEMU reaches Linux, Aether init, Aether System Core, Aether Core, shell, network, and shutdown milestones. |

## Runtime Security Validation Checklist

Run after QEMU boots and `AETHER_SYSTEM_CORE_READY` appears:

```sh
aetherctl services
aetherctl system status
aetherctl system metrics
aetherctl system audit
stat -c '%a %n' /run/aether/ipc/aether-system-core.sock
```

Expected result:

| Check | Expected Result |
| --- | --- |
| IPC socket mode | `600 /run/aether/ipc/aether-system-core.sock` |
| Audit command | Returns retained authorization decisions. |
| Service-control command | Authorized through private local IPC and audited. |
| System-control command | Authorized through private local IPC and audited. |
| Invalid command | Returns an `ERR` response and does not crash the daemon. |
| Oversized request | Returns an error for exceeding the 8192-byte request limit. |
| Sensitive logging | Secret-like values are redacted from structured logs. |
| Resource declarations | Manifests show bounded CPU, IO, process, restart, and shutdown values. |
| Restart/failure behavior | Restart limit is enforced and repeated failures enter `FAILED`. |

## Known Limitations

The Phase 1.4 implementation validates service security and resource declarations but does not yet enforce service isolation with cgroups, namespaces, seccomp, Linux capabilities, or MAC policy. IPC peer credential fields exist in the model, but kernel-backed peer credential verification is not active. The current access boundary is a local Unix-domain control socket with mode `0600`.
