# syntax=docker/dockerfile:1.7
# P12 supplies the already-built, revision-bound Linux binary as the only build context input.
# No source checkout, provider configuration, credential, listener or runtime data is copied in.
FROM scratch AS release-input
COPY gateway /gateway

FROM debian:bookworm-slim@sha256:63a496b5d3b99214b39f5ed70eb71a61e590a77979c79cbee4faf991f8c0783e

ARG RELEASE_REVISION
ARG RELEASE_VERSION
ARG RELEASE_RUST_VERSION
ARG RELEASE_TARGET

LABEL org.opencontainers.image.title="cpa-rust-gateway" \
      org.opencontainers.image.revision="${RELEASE_REVISION}" \
      org.opencontainers.image.version="${RELEASE_VERSION}" \
      org.opencontainers.image.rust.toolchain="${RELEASE_RUST_VERSION}" \
      org.opencontainers.image.target="${RELEASE_TARGET}" \
      org.opencontainers.image.base.name="debian:bookworm-slim@sha256:63a496b5d3b99214b39f5ed70eb71a61e590a77979c79cbee4faf991f8c0783e"

# The release binary currently uses OpenSSL through the locked native-tls dependency. Install only
# its runtime library and CA roots, then leave no package-list cache in the image.
RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=release-input --chown=65532:65532 /gateway /usr/local/bin/gateway
USER 65532:65532
WORKDIR /nonexistent
ENTRYPOINT ["/usr/local/bin/gateway"]
