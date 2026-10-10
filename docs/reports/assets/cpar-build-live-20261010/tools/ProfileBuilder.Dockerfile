FROM cpar-build-candidate:f346a03f AS builder
ARG CPAR_REVISION
ENV GATEWAY_RELEASE_REVISION=${CPAR_REVISION} CARGO_BUILD_JOBS=2
WORKDIR /src
COPY crates/provider-grok/src/build_responses.rs /src/crates/provider-grok/src/build_responses.rs
COPY crates/provider-grok/tests/p6_03_build_responses.rs /src/crates/provider-grok/tests/p6_03_build_responses.rs
COPY docs/contracts/BC-PROVIDER-003-grok-build-responses-boundary.md /src/docs/contracts/BC-PROVIDER-003-grok-build-responses-boundary.md
COPY docs/reports/assets/cpar-build-live-20261010/public-profile-source.json /src/docs/reports/assets/cpar-build-live-20261010/public-profile-source.json
RUN cargo build --locked --release --package gateway && strip target/release/gateway
