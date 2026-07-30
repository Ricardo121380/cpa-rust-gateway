# syntax=docker/dockerfile:1.7
# P12 supplies the already-built, revision-bound Linux binary as the only build context input.
# No source checkout, provider configuration, credential, listener or runtime data is copied in.
#
# Each release target is built natively on a runner of its own architecture, so the base image is
# pinned per architecture rather than through a multi-architecture index. An ARG referenced by a
# FROM must be declared before the first FROM to be global rather than stage-scoped; the release
# workflow supplies one of the two digests below, both members of the same `debian:bookworm-slim`
# index (sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818). The amd64 member
# is a single-architecture manifest, so resolving it on arm64 would fail.
ARG RELEASE_BASE_IMAGE

FROM scratch AS release-input
COPY gateway /gateway

FROM ${RELEASE_BASE_IMAGE}

ARG RELEASE_REVISION
ARG RELEASE_VERSION
ARG RELEASE_RUST_VERSION
ARG RELEASE_TARGET
ARG RELEASE_BASE_IMAGE

LABEL org.opencontainers.image.title="cpa-rust-gateway" \
      org.opencontainers.image.revision="${RELEASE_REVISION}" \
      org.opencontainers.image.version="${RELEASE_VERSION}" \
      org.opencontainers.image.rust.toolchain="${RELEASE_RUST_VERSION}" \
      org.opencontainers.image.target="${RELEASE_TARGET}" \
      org.opencontainers.image.base.name="${RELEASE_BASE_IMAGE}"

# The release binary currently uses OpenSSL through the locked native-tls dependency. Install only
# its runtime library and CA roots, then leave no package-list cache in the image.
RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=release-input --chown=65532:65532 /gateway /usr/local/bin/gateway
USER 65532:65532
WORKDIR /nonexistent
ENTRYPOINT ["/usr/local/bin/gateway"]
