#!/usr/bin/env bash
set -euo pipefail

# Existing isolated offline recipe, with source-bound inputs for this owned container only.
archive_sha="$1"
source_revision="$2"
[[ "$archive_sha" =~ ^[0-9a-f]{64}$ ]]
[[ "$source_revision" =~ ^[0-9a-f]{40}$ ]]
test "$(sha256sum /tmp/source.tar | cut -d ' ' -f 1)" = "$archive_sha"
test "$(rustc --version | cut -d ' ' -f 2)" = 1.97.1
test "$(uname -m)" = aarch64
mv /src/target /candidate-target
rm -rf /src
mkdir /src
tar -xf /tmp/source.tar -C /src
cd /src
git init --quiet
export GATEWAY_RELEASE_REVISION="$source_revision"
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
