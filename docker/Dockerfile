FROM rust:1-bookworm AS builder

ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /workspace

# Cache system packages layer
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    busybox-static \
    bc \
    bison \
    ca-certificates \
    clang \
    clang-format \
    cmake \
    cpio \
    curl \
    flex \
    file \
    git \
    grub-pc-bin \
    libelf-dev \
    libssl-dev \
    mtools \
    musl-tools \
    ninja-build \
    patch \
    perl \
    python3 \
    python3-venv \
    qemu-system-x86 \
    rsync \
    shellcheck \
    unzip \
    wget \
    xorriso \
    && rm -rf /var/lib/apt/lists/*

# Cache Rust target separately
RUN rustup target add x86_64-unknown-linux-musl

# Development image inherits from builder
FROM builder AS final

ENV DEBIAN_FRONTEND=noninteractive \
    RUST_BACKTRACE=1 \
    CARGO_TERM_COLOR=always \
    CARGO_BUILD_JOBS=4 \
    RUSTFLAGS="-C opt-level=2 -C codegen-units=256"

WORKDIR /workspace

# Pre-create cargo cache directory
RUN mkdir -p /workspace/.cargo && chmod 777 /workspace/.cargo

CMD ["/bin/bash"]
