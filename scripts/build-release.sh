#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

target_root="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_root" != /* ]]; then
  target_root="$repo_root/$target_root"
fi
artifact="${GATEWAY_RELEASE_ARTIFACT:-$target_root/release/gateway}"

CARGO_INCREMENTAL=0 cargo build --release --locked --workspace
"$repo_root/scripts/normalize-macho-uuid.rb" "$artifact"

printf 'release-build: ready %s\n' "$artifact"
