#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
scanner="$repo_root/scripts/secret-scan.sh"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/gateway-secret-scan.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

git -C "$tmp_dir" init -q
git -C "$tmp_dir" config user.name "Secret Scan Test"
git -C "$tmp_dir" config user.email "secret-scan@example.invalid"
mkdir -p "$tmp_dir/scripts"
cp "$scanner" "$tmp_dir/scripts/secret-scan.sh"
chmod +x "$tmp_dir/scripts/secret-scan.sh"

printf 'synthetic fixture without credentials\n' > "$tmp_dir/safe.txt"
git -C "$tmp_dir" add safe.txt
(
  cd "$tmp_dir"
  scripts/secret-scan.sh --staged >/dev/null
)

# A systemd LoadCredential source is a file path, not an embedded credential. Its precise syntax
# must remain accepted without weakening detection of management-key values in other assignments.
printf '%s\n' 'LoadCredential=management-key:/etc/gateway/credentials/management-key' > "$tmp_dir/unit.service"
git -C "$tmp_dir" add unit.service
(
  cd "$tmp_dir"
  scripts/secret-scan.sh --staged >/dev/null
)

canary='CANARY_SECRET_1234567890'
printf 'api_key = "%s"\n' "$canary" > "$tmp_dir/canary.txt"
git -C "$tmp_dir" add canary.txt

set +e
output="$(cd "$tmp_dir" && scripts/secret-scan.sh --staged 2>&1)"
status=$?
set -e

if (( status == 0 )); then
  printf 'secret-scan-test: scanner accepted a synthetic secret\n' >&2
  exit 1
fi

if [[ "$output" == *"$canary"* ]]; then
  printf 'secret-scan-test: scanner leaked the matched value\n' >&2
  exit 1
fi

if [[ "$output" != *"canary.txt"* ]]; then
  printf 'secret-scan-test: scanner did not identify the rejected file\n' >&2
  exit 1
fi

printf 'Environment=management-key=%s\n' "$canary" > "$tmp_dir/unsafe-unit.service"
git -C "$tmp_dir" add unsafe-unit.service

set +e
output="$(cd "$tmp_dir" && scripts/secret-scan.sh --staged 2>&1)"
status=$?
set -e

if (( status == 0 )) || [[ "$output" != *"unsafe-unit.service"* ]]; then
  printf 'secret-scan-test: scanner accepted an ambient management key\n' >&2
  exit 1
fi

printf 'secret-scan-test: ok (safe file accepted, canary rejected without value leak)\n'
