# syntax=docker/dockerfile:1.4
# Rust CI image for game-mods
# Stable toolchain with system dependencies for injection toolkit crates

FROM rust:1.93-slim

# System dependencies
# - pkg-config + libclang: required for ffmpeg-next bindings (itk-video)
# - libasound2-dev: ALSA headers for cpal audio (itk-daemon)
# - libffmpeg-dev or ffmpeg: video decoding (itk-video)
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    git \
    clang \
    libclang-dev \
    libavcodec-dev \
    libavformat-dev \
    libavutil-dev \
    libswscale-dev \
    libswresample-dev \
    libavfilter-dev \
    libavdevice-dev \
    libasound2-dev \
    && rm -rf /var/lib/apt/lists/*

# Rust components
RUN rustup component add rustfmt clippy

# Install cargo-deny for license/advisory checks
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-deny --locked

# Non-root user (overridden by docker-compose USER_ID/GROUP_ID)
RUN useradd -m -u 1000 ciuser \
    && mkdir -p /tmp/cargo && chmod 1777 /tmp/cargo

WORKDIR /workspace

ENV CARGO_HOME=/tmp/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_INCREMENTAL=1 \
    CARGO_NET_RETRY=10 \
    RUST_BACKTRACE=short

CMD ["bash"]
