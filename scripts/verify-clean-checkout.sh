#!/usr/bin/env bash

set -euo pipefail

ref="${1:-HEAD}"
mode="${2:-full}"
report_path="${3:-}"
repo_root="$(git rev-parse --show-toplevel)"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/gateway-clean-checkout.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

git clone --quiet --no-hardlinks "$repo_root" "$tmp_dir/repo"
git -C "$tmp_dir/repo" checkout --quiet --detach "$ref"

export CARGO_TARGET_DIR="$tmp_dir/target"
if [[ -n "$report_path" ]]; then
  if [[ "$report_path" != /* ]]; then
    report_path="$repo_root/$report_path"
  fi
  export CHECK_REPORT_PATH="$report_path"
fi

(
  cd "$tmp_dir/repo"
  ./scripts/check.sh "$mode"
)

if [[ -n "$(git -C "$tmp_dir/repo" status --porcelain)" ]]; then
  printf 'clean-checkout: verification modified tracked or untracked source files\n' >&2
  git -C "$tmp_dir/repo" status --short >&2
  exit 1
fi

printf 'clean-checkout: ok (%s at %s)\n' "$mode" "$ref"
