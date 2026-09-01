# syntax=docker/dockerfile:1.7

# Community source build for Linux/amd64 and Linux/arm64. BuildKit resolves every stage for the
# requested target platform, so a multi-platform build either runs on a native builder or through
# an explicitly configured emulator. The signed release workflow continues to use the repository
# root Dockerfile, which accepts a separately verified binary and has a narrower trust boundary.
FROM node:25.8.0-bookworm-slim AS builder

ARG CPAR_RUST_TOOLCHAIN=1.97.1
ARG CPAR_REVISION=source-build
ARG TARGETARCH
ARG TARGETPLATFORM

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:${PATH} \
    CARGO_INCREMENTAL=0 \
    GATEWAY_RELEASE_REVISION=${CPAR_REVISION} \
    GATEWAY_RELEASE_RUST_VERSION=${CPAR_RUST_TOOLCHAIN}

RUN apt-get update \
    && apt-get install --no-install-recommends --yes \
        build-essential \
        ca-certificates \
        clang \
        cmake \
        curl \
        git \
        libclang-dev \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/* \
    && curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
        https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain "${CPAR_RUST_TOOLCHAIN}"

WORKDIR /src
COPY . .

RUN case "${TARGETARCH}" in \
        amd64) export GATEWAY_RELEASE_TARGET=x86_64-unknown-linux-gnu ;; \
        arm64) export GATEWAY_RELEASE_TARGET=aarch64-unknown-linux-gnu ;; \
        *) echo "unsupported source-image target: ${TARGETPLATFORM}" >&2; exit 1 ;; \
    esac \
    && npm --prefix web/prism ci --ignore-scripts --no-audit --no-fund \
    && cargo build --locked --release --package gateway \
    && strip target/release/gateway

FROM debian:bookworm-slim AS runtime

ARG CPAR_REVISION=source-build
ARG CPAR_VERSION=source
ARG CPAR_RUST_TOOLCHAIN=1.97.1
ARG TARGETARCH

LABEL org.opencontainers.image.title="CPAR" \
      org.opencontainers.image.description="Provider-isolated Rust AI gateway" \
      org.opencontainers.image.source="https://github.com/Ricardo121380/cpa-rust-gateway" \
      org.opencontainers.image.revision="${CPAR_REVISION}" \
      org.opencontainers.image.version="${CPAR_VERSION}" \
      org.opencontainers.image.rust.toolchain="${CPAR_RUST_TOOLCHAIN}" \
      org.opencontainers.image.architecture="${TARGETARCH}"

RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder --chown=65532:65532 /src/target/release/gateway /usr/local/bin/gateway

USER 65532:65532
WORKDIR /nonexistent
ENTRYPOINT ["/usr/local/bin/gateway"]
