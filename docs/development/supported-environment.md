# Supported Development Environment

## Primary Environment

Aether OS builds must run in a Linux development environment. On Windows workstations, the supported path is WSL2 with Ubuntu 24.04 LTS, using the Linux filesystem for build outputs.

| Host | Support Level | Usage |
| --- | --- | --- |
| Ubuntu 24.04 LTS on WSL2 | Primary for Windows workstations | Buildroot, Rust, C/C++, QEMU, tests, runtime validation. |
| Native Ubuntu 24.04 LTS | Primary for Linux workstations | Buildroot, Rust, C/C++, QEMU, tests, runtime validation. |
| Docker on Linux or Docker Desktop with WSL integration | Secondary reproducible shell | CI-like development when the Linux engine is available. |
| Windows PowerShell without WSL2 tools | Inspection only | Repository browsing and lightweight Python checks. Not supported for Buildroot or QEMU validation. |

Buildroot and Linux kernel builds should not be run from a Windows-mounted path such as `/mnt/c` when avoidable. Use a path under the WSL filesystem, such as `$HOME/src/aether-os`, to reduce filesystem overhead and avoid exhausting the Windows system volume.

## Required Toolchain

| Area | Required Tools | Reason |
| --- | --- | --- |
| Linux kernel and Buildroot | `make`, `gcc`, `g++`, `bc`, `bison`, `flex`, `perl`, `patch`, `rsync`, `cpio`, `unzip`, `wget`, `file`, `libelf-dev`, `libssl-dev` | Buildroot host tools, kernel configuration, kernel compilation, and root filesystem generation. |
| Rust services | `rustup`, `rustc`, `cargo`, `clippy`, `rustfmt`, `x86_64-unknown-linux-musl` target | Compile Aether System Core, services, SDK crates, tests, and lint checks. |
| C/C++ utilities | `cmake`, `ninja-build`, `clang`, `clang-format`, `build-essential`, `musl-tools` | Compile and validate native utilities and maintain consistent formatting. |
| Python tooling | `python3`, `python3-venv` | Repository tests, validation scripts, bootstrap helpers, and benchmark harnesses. |
| QEMU boot validation | `qemu-system-x86`, `qemu-utils` | Boot the generated kernel and root filesystem and run boot smoke tests. |
| ISO and boot media | `busybox-static`, `grub-pc-bin`, `mtools`, `xorriso` | Build development initramfs and ISO artifacts. |
| Shell quality | `bash`, `shellcheck` | Run repository automation and static shell checks. |
| Container fallback | Docker Desktop with WSL integration or Linux Docker Engine | Optional reproducible development shell. |

## Pinned Project Inputs

| Input | Source |
| --- | --- |
| Buildroot | `infra/buildroot/versions.env` |
| Linux kernel | `infra/buildroot/versions.env` |
| Target architecture | `infra/buildroot/versions.env` |
| Buildroot toolchain family | `infra/buildroot/versions.env` |
| Rust toolchain policy | `rust-toolchain.toml` |

The Rust toolchain currently tracks stable Rust with required `clippy`, `rustfmt`, and `x86_64-unknown-linux-musl` target support. A fixed Rust release should be selected after the first full Buildroot and QEMU validation pass confirms the compiler version.

## Windows WSL2 Setup

Run these commands from an elevated PowerShell only when WSL2 is not already installed:

```powershell
wsl --install -d Ubuntu-24.04
wsl --set-default-version 2
wsl --update
```

Confirm WSL2:

```powershell
wsl --status
wsl -l -v
```

Inside Ubuntu 24.04:

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

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup default stable
rustup component add clippy rustfmt
rustup target add x86_64-unknown-linux-musl
```

Prepare a Linux-filesystem checkout:

```bash
mkdir -p "$HOME/src"
rsync -a \
  --exclude artifacts \
  --exclude build \
  --exclude target \
  "/mnt/c/Users/Vikas Shelar/Documents/ChatGPT/os/" \
  "$HOME/src/aether-os/"
cd "$HOME/src/aether-os"
```

Run the project bootstrap after Rustup exists:

```bash
sudo bash scripts/install-deps.sh native
python3 tools/aether-doctor.py
```

## Docker Setup

Docker is optional for local development. On Windows, Docker Desktop must be running and WSL integration must be enabled for `Ubuntu-24.04`.

Validation:

```powershell
docker context ls
docker info
```

From WSL:

```bash
docker info
bash scripts/install-deps.sh docker
docker run --rm -it -v "$PWD:/workspace" aether-os-dev
```

If `docker info` fails in WSL, enable Docker Desktop WSL integration before using the Docker path.

## Required Validation Sequence

Run these commands from the Linux checkout:

```bash
python3 tools/aether-doctor.py
cargo test --workspace
bash scripts/test.sh
bash scripts/lint.sh
bash scripts/build.sh
bash scripts/build/bootstrap.sh
bash scripts/build/build.sh
bash scripts/test-boot.sh
```

Phase 1.4 cannot be closed until Buildroot build, Rust build, QEMU boot, runtime security validation, and regression validation all pass.
