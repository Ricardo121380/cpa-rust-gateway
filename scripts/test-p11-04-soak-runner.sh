#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
runner="$repo_root/scripts/run-p11-04-soak.sh"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

expect_fail() {
  if "$runner" "$@" >/dev/null 2>&1; then
    printf 'p11-04 soak runner test: expected rejection\n' >&2
    exit 1
  fi
}

expect_fail
expect_fail --smoke --soak
expect_fail --smoke --status relative-receipt.log

existing_receipt="$work_dir/existing-receipt.log"
touch "$existing_receipt"
expect_fail --smoke --status "$existing_receipt"

interrupted_receipt="$work_dir/interrupted-receipt.log"
"$runner" --smoke --status "$interrupted_receipt" >/dev/null 2>&1 &
runner_pid=$!
sleep 1
kill -TERM "$runner_pid"
set +e
wait "$runner_pid"
status=$?
set -e
if (( status != 130 )); then
  printf 'p11-04 soak runner test: interrupted smoke exited with %d, expected 130\n' "$status" >&2
  exit 1
fi
rg -qx 'runner_state=INCOMPLETE mode=--smoke duration_seconds=10 exit_status=0 interruption_signal=TERM' \
  "$interrupted_receipt"

printf 'p11-04 soak runner test: ok\n'
