#!/usr/bin/env bash
# Run only in the task-owned native ARM64 builder with --network=none.
set -euo pipefail
test "$(uname -m)" = aarch64
test "$(rustc --version | cut -d ' ' -f 2)" = 1.97.1
test "$(node --version)" = v24.19.0
test ! -e /work/source-candidate
test ! -e /work/target-candidate
python3 - <<'PY'
import hashlib,json,pathlib
root=pathlib.Path('/input');binding=json.loads((root/'source-binding.json').read_text())
for name,key in [('source.tar','source_archive_sha256'),('input-manifest.json','input_manifest_sha256'),('overlay.json','overlay_sha256')]:
    assert hashlib.sha256((root/name).read_bytes()).hexdigest()==binding[key], name
print(json.dumps(binding,sort_keys=True))
PY
mkdir /work/source-candidate
tar -xf /input/source.tar -C /work/source-candidate
cd /work/source-candidate
python3 scripts/acceptance/cpar66-source.py verify --source . --manifest /input/input-manifest.json
git init --quiet
export GATEWAY_RELEASE_REVISION
GATEWAY_RELEASE_REVISION="$(python3 -c 'import json;print(json.load(open("/input/source-binding.json"))["base_commit"])')"
export GATEWAY_RELEASE_RUST_VERSION=1.97.1
export GATEWAY_RELEASE_TARGET=aarch64-unknown-linux-gnu
export SOURCE_DATE_EPOCH
SOURCE_DATE_EPOCH="$(python3 -c 'import json;print(json.load(open("/input/source-binding.json"))["source_date_epoch"])')"
export CARGO_TARGET_DIR=/work/target-candidate CARGO_HOME=/work/cargo CARGO_NET_OFFLINE=true
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 npm_config_cache=/work/npm-cache
export LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC
npm --prefix web/prism ci --offline --ignore-scripts --no-audit --no-fund
bash scripts/build-release.sh
mkdir /work/build-output
install -m 0755 /work/target-candidate/release/gateway /work/build-output/gateway-aarch64-unknown-linux-gnu
readelf -h /work/build-output/gateway-aarch64-unknown-linux-gnu > /work/build-output/elf-header.txt
readelf -d /work/build-output/gateway-aarch64-unknown-linux-gnu > /work/build-output/elf-dynamic.txt
ldd /work/build-output/gateway-aarch64-unknown-linux-gnu > /work/build-output/ldd.txt
/work/build-output/gateway-aarch64-unknown-linux-gnu --help > /work/build-output/help.txt
scripts/generate-p12-01-sbom.rb --target aarch64-unknown-linux-gnu --output /work/build-output/gateway-sbom.cdx.json
python3 scripts/acceptance/cpar66-source.py verify --source . --manifest /input/input-manifest.json
sha256sum /work/build-output/gateway-aarch64-unknown-linux-gnu /work/build-output/gateway-sbom.cdx.json
