#!/usr/bin/env bash
set -euo pipefail
cd /workspace
test "$(uname -m)" = aarch64
test "$(git rev-parse HEAD)" = 80f5838902eb56be6dccc3ce047eb227d6c18e6c
test -z "$(git status --porcelain --untracked-files=no)"
rustc -vV
cargo --version
node --version
npm --version
ldd --version
npm --prefix web/prism ci --ignore-scripts --no-audit --no-fund
./scripts/build-release.sh
install -m 0755 target/release/gateway /candidate/gateway-aarch64-unknown-linux-gnu
./scripts/p12-release-artifact.rb build-metadata --revision "$GATEWAY_RELEASE_REVISION" --rust-toolchain 1.97.1 --target aarch64-unknown-linux-gnu --version 0.1.0 --output /candidate/gateway-build-metadata.json
./scripts/p12-release-artifact.rb identity --repository Ricardo121380/cpa-rust-gateway --workflow-path .github/workflows/release-artifact.yml --ref refs/heads/codex/prism-v4-delivery --output /candidate/signing-identity.json
/candidate/gateway-aarch64-unknown-linux-gnu --help
test -z "$(git status --porcelain --untracked-files=no)"
