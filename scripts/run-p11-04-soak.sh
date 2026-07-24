#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
mode=""
status_path=""
while (( $# > 0 )); do
  case "$1" in
    --smoke|--soak)
      if [[ -n "$mode" ]]; then
        printf 'p11-04 soak: choose exactly one of --smoke or --soak\n' >&2
        exit 2
      fi
      mode="$1"
      shift
      ;;
    --status)
      status_path="${2:?--status requires an absolute path}"
      shift 2
      ;;
    *)
      printf 'usage: %s (--smoke|--soak) [--status ABSOLUTE_PATH]\n' "$0" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$mode" ]]; then
  printf 'p11-04 soak: choose --smoke or --soak\n' >&2
  exit 2
fi

if [[ -z "$status_path" ]]; then
  timestamp="$(date -u '+%Y%m%dT%H%M%SZ')"
  status_path="$repo_root/docs/reports/evidence/p11-04-${mode#--}-${timestamp}.log"
fi
if [[ "$status_path" != /* || -e "$status_path" ]]; then
  printf 'p11-04 soak: status path must be a new absolute receipt path\n' >&2
  exit 2
fi
mkdir -p "$(dirname "$status_path")"

case "$mode" in
  --smoke)
    duration_seconds=10
    ;;
  --soak)
    duration_seconds=86400
    ;;
esac

interrupted=0
interruption_signal=""
record_interruption() {
  interrupted=1
  interruption_signal="$1"
}
trap 'record_interruption INT' INT
trap 'record_interruption TERM' TERM
trap 'record_interruption HUP' HUP

set +e
P11_04_SOAK_AUTH='P11-04-LOOPBACK-SOAK-v1' \
  P11_04_SOAK_SECONDS="$duration_seconds" \
  P11_04_STATUS_PATH="$status_path" \
  cargo test --locked -p gateway-http-actix --test p11_04_load_soak \
    authorized_loopback_soak_writes_a_value_free_receipt -- --ignored --exact --nocapture
status=$?
set -e

if (( status == 0 && interrupted == 0 )); then
  printf 'runner_state=COMPLETED mode=%s duration_seconds=%s\n' "$mode" "$duration_seconds" >> "$status_path"
else
  if [[ -f "$status_path" ]]; then
    printf 'runner_state=INCOMPLETE mode=%s duration_seconds=%s exit_status=%s interruption_signal=%s\n' \
      "$mode" "$duration_seconds" "$status" "${interruption_signal:-none}" >> "$status_path"
  fi
  if (( status == 0 )); then
    exit 130
  fi
  exit "$status"
fi

printf 'p11-04 soak receipt: %s\n' "$status_path"
