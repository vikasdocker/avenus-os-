FROM rust:1-bookworm

ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /workspace

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

RUN rustup target add x86_64-unknown-linux-musl

CMD ["/bin/bash"]
