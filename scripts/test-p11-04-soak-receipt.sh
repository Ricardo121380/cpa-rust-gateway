#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
checker="$repo_root/scripts/check-p11-04-soak-receipt.rb"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

valid_receipt="$work_dir/valid.log"
cat > "$valid_receipt" <<'EOF'
timestamp_unix=1 state=RUNNING elapsed_seconds=0 batches=1 streams=4 rss_bytes=100
timestamp_unix=301 state=RUNNING elapsed_seconds=300 batches=2 streams=8 rss_bytes=110
timestamp_unix=601 state=RUNNING elapsed_seconds=600 batches=3 streams=12 rss_bytes=100
timestamp_unix=901 state=RUNNING elapsed_seconds=900 batches=4 streams=16 rss_bytes=102
timestamp_unix=1201 state=RUNNING elapsed_seconds=1200 batches=5 streams=20 rss_bytes=104
timestamp_unix=1501 state=RUNNING elapsed_seconds=1500 batches=6 streams=24 rss_bytes=106
timestamp_unix=1801 state=RUNNING elapsed_seconds=1800 batches=7 streams=28 rss_bytes=108
timestamp_unix=86401 state=COMPLETED elapsed_seconds=86400 batches=8 streams=32 rss_bytes=110
runner_state=COMPLETED mode=--soak duration_seconds=86400
EOF

"$checker" "$valid_receipt" >/dev/null

expect_fail() {
  if "$checker" "$1" >/dev/null 2>&1; then
    printf 'p11-04 receipt test: expected invalid receipt to fail\n' >&2
    exit 1
  fi
}

rss_growth_receipt="$work_dir/rss-growth.log"
cp "$valid_receipt" "$rss_growth_receipt"
ruby -e 'path = ARGV.fetch(0); lines = File.readlines(path); lines[7] = lines[7].sub("rss_bytes=110", "rss_bytes=120"); File.write(path, lines.join)' "$rss_growth_receipt"
expect_fail "$rss_growth_receipt"

incomplete_receipt="$work_dir/incomplete.log"
cp "$valid_receipt" "$incomplete_receipt"
ruby -e 'path = ARGV.fetch(0); value = File.read(path).sub("runner_state=COMPLETED", "runner_state=INCOMPLETE"); File.write(path, value)' "$incomplete_receipt"
expect_fail "$incomplete_receipt"

malformed_receipt="$work_dir/malformed.log"
cp "$valid_receipt" "$malformed_receipt"
ruby -e 'path = ARGV.fetch(0); value = File.read(path).sub("streams=32", "streams=31"); File.write(path, value)' "$malformed_receipt"
expect_fail "$malformed_receipt"

printf 'p11-04 receipt test: ok\n'
