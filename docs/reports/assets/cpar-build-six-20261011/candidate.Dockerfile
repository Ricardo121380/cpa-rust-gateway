# BUILDER_IMAGE must be the locally verified successor dependency image digest.
ARG BUILDER_IMAGE
FROM ${BUILDER_IMAGE} AS builder
ARG CPAR_REVISION
ENV GATEWAY_RELEASE_REVISION=${CPAR_REVISION} \
    GATEWAY_RELEASE_RUST_VERSION=1.97.1 \
    GATEWAY_RELEASE_TARGET=aarch64-unknown-linux-gnu \
    CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/candidate-target
# Retain only the prior dependency/build cache. All source is the frozen archive.
RUN mv /src/target /candidate-target && rm -rf /src && mkdir /src
WORKDIR /src
COPY . .
RUN test "$CPAR_REVISION" = abd454313f6a3707d6af5a5d653b731e2e54b08b \
    && git init --quiet \
    && npm --prefix web/prism ci --offline --ignore-scripts --no-audit --no-fund \
    && bash scripts/build-release.sh \
    && strip /candidate-target/release/gateway
