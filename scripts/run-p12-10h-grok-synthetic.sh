#!/usr/bin/env bash
set -euo pipefail

# P12-10H synthetic Grok smoke harness.
#
# This intentionally does not read a credential, call grok2api, or claim an upstream
# Grok success. It exercises CPAR's native Grok code with fixed local fixtures and,
# when requested, checks only the loopback service/authentication boundary.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
loopback_endpoint="${CPAR_GROK_LOOPBACK_ENDPOINT:-}"
iterations="${CPAR_GROK_SYNTHETIC_ITERATIONS:-1}"

if [[ ! "$iterations" =~ ^[1-9][0-9]*$ ]] || (( iterations > 1000 )); then
  printf 'grok_synthetic=FAIL category=invalid_iterations\n' >&2
  exit 1
fi

run_target() {
  local target="$1"
  if ! cargo test --locked -p provider-grok --test "$target" --quiet >/dev/null; then
    printf 'grok_synthetic=FAIL category=rust_target target=%s\n' "$target" >&2
    exit 1
  fi
  printf 'grok_synthetic=PASS target=%s\n' "$target"
}

cd "$repo_root"

# One complete Console fixture execution is the repeatable synthetic call. It uses
# the reviewed mock transport in the test and never opens a network socket.
for ((iteration = 1; iteration <= iterations; iteration++)); do
  if ! cargo test --locked -p provider-grok --test p12_10e_console_web_runtime \
      console_inference_adapter_executes_json_sse_and_safe_http_failures --quiet -- --exact >/dev/null; then
    printf 'grok_synthetic=FAIL category=console_fixture iteration=%s\n' "$iteration" >&2
    exit 1
  fi
done
printf 'grok_synthetic=PASS target=console_fixture iterations=%s\n' "$iterations"

run_target p12_10b_native_account_pool
run_target p12_10c_native_account_scheduling
run_target p12_10d_native_account_workers
run_target p12_10f_grok2api_memory_migration

if [[ -n "$loopback_endpoint" ]]; then
  case "$loopback_endpoint" in
    http://127.0.0.1:*|http://localhost:*) ;;
    *)
      printf 'grok_synthetic=FAIL category=non_loopback_endpoint\n' >&2
      exit 1
      ;;
  esac
  health_status="$(curl -sS -o /dev/null -w '%{http_code}' --max-time 5 "${loopback_endpoint%/}/healthz" || true)"
  if [[ "$health_status" != "200" ]]; then
    printf 'grok_synthetic=FAIL category=loopback_health\n' >&2
    exit 1
  fi
  printf 'grok_synthetic=PASS target=loopback_health status_class=2xx\n'

  auth_status="$(curl -sS -o /dev/null -w '%{http_code}' --max-time 5 "${loopback_endpoint%/}/v1/models" || true)"
  if [[ "$auth_status" != "401" ]]; then
    printf 'grok_synthetic=FAIL category=data_plane_auth_boundary\n' >&2
    exit 1
  fi
  printf 'grok_synthetic=PASS target=data_plane_auth_boundary status_class=4xx\n'
fi

printf 'grok_synthetic=COMPLETE synthetic_calls=%s upstream_request=not_sent\n' "$iterations"
