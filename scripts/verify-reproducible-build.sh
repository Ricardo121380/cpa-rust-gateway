#!/usr/bin/env bash

set -euo pipefail

ref="${1:-HEAD}"
repo_root="$(git rev-parse --show-toplevel)"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/gateway-repro-build.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

sha256_file() {
  local path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
  else
    shasum -a 256 "$path" | awk '{print $1}'
  fi
}

build_once() {
  local name="$1"
  local checkout="$tmp_dir/$name/repo"
  local target="$tmp_dir/$name/target"

  git clone --quiet --no-hardlinks "$repo_root" "$checkout"
  git -C "$checkout" checkout --quiet --detach "$ref"

  (
    cd "$checkout"
    CARGO_TARGET_DIR="$target" ./scripts/build-release.sh
  )

  if [[ -n "$(git -C "$checkout" status --porcelain)" ]]; then
    printf 'repro-build: source checkout %s was modified by the build\n' "$name" >&2
    git -C "$checkout" status --short >&2
    exit 1
  fi

  "$target/release/gateway" > "$tmp_dir/$name/output.txt"
  sha256_file "$target/release/gateway" > "$tmp_dir/$name/sha256.txt"
  wc -c < "$target/release/gateway" | tr -d ' ' > "$tmp_dir/$name/size.txt"
}

build_once first
build_once second

first_hash="$(cat "$tmp_dir/first/sha256.txt")"
second_hash="$(cat "$tmp_dir/second/sha256.txt")"
first_size="$(cat "$tmp_dir/first/size.txt")"
second_size="$(cat "$tmp_dir/second/size.txt")"

if [[ "$first_hash" != "$second_hash" ]]; then
  printf 'repro-build: binary hashes differ\nfirst=%s\nsecond=%s\n' "$first_hash" "$second_hash" >&2
  exit 1
fi

if [[ "$first_size" != "$second_size" ]]; then
  printf 'repro-build: binary sizes differ\n' >&2
  exit 1
fi

if ! cmp -s "$tmp_dir/first/output.txt" "$tmp_dir/second/output.txt"; then
  printf 'repro-build: process smoke output differs\n' >&2
  exit 1
fi

report_path="${REPRO_REPORT_PATH:-}"
if [[ -n "$report_path" ]]; then
  if [[ "$report_path" != /* ]]; then
    report_path="$repo_root/$report_path"
  fi
  mkdir -p "$(dirname "$report_path")"
  {
    printf '# Reproducible build log\n\n'
    printf -- '- Ref: `%s`\n' "$ref"
    printf -- '- Timestamp: `%s`\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    printf -- '- Toolchain: `%s`\n' "$(rustc --version)"
    printf -- '- Build mode: `--release --locked --workspace`, two independent clones and target directories\n'
    printf -- '- SHA-256 first: `%s`\n' "$first_hash"
    printf -- '- SHA-256 second: `%s`\n' "$second_hash"
    printf -- '- Size first: `%s` bytes\n' "$first_size"
    printf -- '- Size second: `%s` bytes\n' "$second_size"
    printf -- '- Runtime smoke output: `%s`\n' "$(cat "$tmp_dir/first/output.txt")"
    printf -- '- Result: `PASS`\n'
  } > "$report_path"
fi

printf 'repro-build: ok (sha256=%s size=%s)\n' "$first_hash" "$first_size"
