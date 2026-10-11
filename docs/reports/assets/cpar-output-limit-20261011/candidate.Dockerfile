# Existing local ARM64 builder image; never pull or mutate the historical image.
FROM cpar-build-candidate:1b6023f5 AS builder
ARG CPAR_REVISION
ENV GATEWAY_RELEASE_REVISION=${CPAR_REVISION} \
    GATEWAY_RELEASE_RUST_VERSION=1.97.1 \
    GATEWAY_RELEASE_TARGET=aarch64-unknown-linux-gnu \
    CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/candidate-target
# Keep only dependency/build cache; every source file comes from the clean archive.
RUN mv /src/target /candidate-target && rm -rf /src && mkdir /src
WORKDIR /src
COPY . .
# build-release.sh locates the root with git; the revision is supplied above from
# the archived commit, never inferred from this empty structural repository.
RUN git init --quiet \
    && npm --prefix web/prism ci --offline --ignore-scripts --no-audit --no-fund \
    && bash scripts/build-release.sh \
    && strip /candidate-target/release/gateway
