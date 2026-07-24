#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
work_dir="$(mktemp -d)"
gateway_pid=""
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"

cleanup() {
  if [[ -n "$gateway_pid" ]] && kill -0 "$gateway_pid" 2>/dev/null; then
    kill -TERM "$gateway_pid" 2>/dev/null || true
    set +e
    wait "$gateway_pid"
    set -e
  fi
  rm -rf "$work_dir"
}
trap cleanup EXIT

next_port() {
  ruby -rsocket -e 'socket = TCPServer.new("127.0.0.1", 0); puts socket.addr[1]; socket.close'
}

data_port="$(next_port)"
management_port="$(next_port)"
while [[ "$management_port" == "$data_port" ]]; do
  management_port="$(next_port)"
done

state_dir="$work_dir/state"
credentials_dir="$work_dir/credentials"
fixture_mgmt='mgmt_abcdefghijklmnopqrstuvwxyz0123456789'
mkdir "$state_dir" "$credentials_dir"
printf '%s' "$fixture_mgmt" > "$credentials_dir/management-key"
printf '%s' 'csrf_abcdefghijklmnopqrstuvwxyz0123456789' > "$credentials_dir/management-csrf"
ruby -e 'File.binwrite(ARGV.fetch(0), "\xA1" * 32); File.binwrite(ARGV.fetch(1), "\xB2" * 32); File.binwrite(ARGV.fetch(2), "\xC3" * 32)' \
  "$credentials_dir/master-key" "$credentials_dir/backup-key" "$credentials_dir/client-key-pepper"

cargo build --locked --package gateway >/dev/null
"$target_dir/debug/gateway" serve \
  --data-listen "127.0.0.1:$data_port" \
  --management-listen "127.0.0.1:$management_port" \
  --state-dir "$state_dir" \
  --credential-dir "$credentials_dir" \
  >"$work_dir/gateway.log" 2>&1 &
gateway_pid=$!

deadline=$((SECONDS + 15))
while ! curl --noproxy '*' --silent --show-error --fail --max-time 1 "http://127.0.0.1:$data_port/healthz" >/dev/null 2>&1; do
  if ! kill -0 "$gateway_pid" 2>/dev/null; then
    wait "$gateway_pid" || true
    printf 'p12-02 serve test: process exited before readiness\n' >&2
    tail -n 60 "$work_dir/gateway.log" >&2
    exit 1
  fi
  if (( SECONDS >= deadline )); then
    printf 'p12-02 serve test: readiness deadline exceeded\n' >&2
    tail -n 60 "$work_dir/gateway.log" >&2
    exit 1
  fi
  sleep 0.1
done

curl --noproxy '*' --silent --show-error --fail --max-time 3 "http://127.0.0.1:$data_port/healthz" | rg -Fx '{"status":"ok"}'
if curl --noproxy '*' --silent --show-error --fail --max-time 3 "http://127.0.0.1:$data_port/admin/config-versions" >/dev/null 2>&1; then
  printf 'p12-02 serve test: data listener exposed a management route\n' >&2
  exit 1
fi
curl --noproxy '*' --silent --show-error --fail --max-time 3 \
  -H "X-Management-Key: $fixture_mgmt" \
  "http://127.0.0.1:$management_port/admin/config-versions" | rg -Fx '[]'
curl --noproxy '*' --silent --show-error --fail --max-time 3 \
  -X POST \
  -H "X-Management-Key: $fixture_mgmt" \
  "http://127.0.0.1:$management_port/admin/backups/preflight" | rg -Fx '{"schema_version":9,"secret_key_required":true}'
curl --noproxy '*' --silent --show-error --fail --max-time 3 \
  "http://127.0.0.1:$management_port/admin-ui/" | rg -q '<title>CPA Rust Gateway'

printf 'p12-02 serve test: loopback readiness, listener isolation and protected management passed\n'
