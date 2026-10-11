#!/usr/bin/env bash
set -euo pipefail

# Runs only in the owned offline container; copied inputs are hash-checked before use.
test "$(sha256sum /tmp/source.tar | cut -d ' ' -f 1)" = f21bec53d792eda65452537f5b190ec4bd037b0f6fe0dca61d42bead856a748d
test "$(rustc --version | cut -d ' ' -f 2)" = 1.97.1
test "$(uname -m)" = aarch64
mv /src/target /candidate-target
rm -rf /src
mkdir /src
tar -xf /tmp/source.tar -C /src
cd /src
git init --quiet
export GATEWAY_RELEASE_REVISION=abd454313f6a3707d6af5a5d653b731e2e54b08b
export GATEWAY_RELEASE_RUST_VERSION=1.97.1
export GATEWAY_RELEASE_TARGET=aarch64-unknown-linux-gnu
export CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/candidate-target
npm --prefix web/prism ci --offline --ignore-scripts --no-audit --no-fund
bash scripts/build-release.sh
strip /candidate-target/release/gateway
readelf -h /candidate-target/release/gateway
readelf -d /candidate-target/release/gateway
ldd /candidate-target/release/gateway
/candidate-target/release/gateway --help
