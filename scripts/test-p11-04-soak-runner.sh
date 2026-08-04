#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
runner="$repo_root/scripts/run-p11-04-soak.sh"
work_dir="$(mktemp -d)"
runner_pid=""

cleanup() {
  if [[ -n "$runner_pid" ]] && kill -0 "$runner_pid" 2>/dev/null; then
    kill -TERM "$runner_pid" 2>/dev/null || true
    set +e
    wait "$runner_pid"
    set -e
  fi
  rm -rf "$work_dir"
}
trap cleanup EXIT

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
runner_output="$work_dir/interrupted-runner.log"
"$runner" --smoke --status "$interrupted_receipt" >"$runner_output" 2>&1 &
runner_pid=$!
# The first invocation in a clean CI workspace may compile the ignored test
# binary before it can create the receipt. Keep this guard bounded, but do not
# confuse a cold build with a runner protocol failure.
readiness_timeout_seconds=420
readiness_deadline=$((SECONDS + readiness_timeout_seconds))
while true; do
  if [[ -f "$interrupted_receipt" ]] && rg -q '(^|[[:space:]])state=RUNNING([[:space:]]|$)' "$interrupted_receipt"; then
    break
  fi
  if ! kill -0 "$runner_pid" 2>/dev/null; then
    set +e
    wait "$runner_pid"
    status=$?
    set -e
    runner_pid=""
    printf 'p11-04 soak runner test: smoke exited with %d before receipt readiness\n' "$status" >&2
    tail -n 120 "$runner_output" >&2
    exit 1
  fi
  if (( SECONDS >= readiness_deadline )); then
    printf 'p11-04 soak runner test: receipt did not reach RUNNING within %d seconds\n' \
      "$readiness_timeout_seconds" >&2
    tail -n 120 "$runner_output" >&2
    exit 1
  fi
  sleep 0.2
done
kill -TERM "$runner_pid"
set +e
wait "$runner_pid"
status=$?
set -e
runner_pid=""
if (( status != 130 )); then
  printf 'p11-04 soak runner test: interrupted smoke exited with %d, expected 130\n' "$status" >&2
  exit 1
fi
rg -qx 'runner_state=INCOMPLETE mode=--smoke duration_seconds=10 exit_status=0 interruption_signal=TERM' \
  "$interrupted_receipt"

printf 'p11-04 soak runner test: ok\n'
