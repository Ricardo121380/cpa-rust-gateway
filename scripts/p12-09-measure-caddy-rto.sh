#!/usr/bin/env bash
# Measures how long a Caddy config swap takes to actually change routing, which is the RTO evidence
# CR-P12-ROLLOUT-001 requires from P12-09.
#
# `caddy reload` returning zero means Caddy accepted the config, not that the next request already
# followed the new route. So this polls a probe endpoint until the observed backend changes, and
# reports the elapsed time from the start of the reload to the first request that took the new path.
#
# Runs on the server as a user that may reload Caddy. It never prints a key, a header value or a
# response body: only a backend label, timings and a hash-free verdict.

set -Eeuo pipefail
umask 077

usage() {
    cat >&2 <<'USAGE'
usage: p12-09-measure-caddy-rto.sh --config PATH --expect-backend LABEL --probe-url URL
                                   [--caddyfile PATH] [--timeout SECONDS] [--interval SECONDS]
                                   [--header-file PATH] [--output PATH]

  --config           Caddyfile to reload into place (the rollback or the split fragment)
  --caddyfile        Live Caddyfile path Caddy runs from (default /etc/caddy/Caddyfile)
  --expect-backend   Backend label expected after the swap: 'gateway' or 'incumbent'
  --probe-url        URL to poll; must be the production hostname under test
  --timeout          Give up after this many seconds (default 120)
  --interval         Seconds between probes (default 1)
  --header-file      Optional file whose first line is a request header to send; never logged
  --output           Optional receipt path; written 0600 with no secret material
USAGE
    exit 64
}

config=''
caddyfile='/etc/caddy/Caddyfile'
expect_backend=''
probe_url=''
timeout_seconds=120
interval_seconds=1
header_file=''
output=''

while [[ $# -gt 0 ]]; do
    case "$1" in
        --config) config="${2:-}"; shift 2 ;;
        --caddyfile) caddyfile="${2:-}"; shift 2 ;;
        --expect-backend) expect_backend="${2:-}"; shift 2 ;;
        --probe-url) probe_url="${2:-}"; shift 2 ;;
        --timeout) timeout_seconds="${2:-}"; shift 2 ;;
        --interval) interval_seconds="${2:-}"; shift 2 ;;
        --header-file) header_file="${2:-}"; shift 2 ;;
        --output) output="${2:-}"; shift 2 ;;
        --help) usage ;;
        *) printf 'unknown argument: %s\n' "$1" >&2; usage ;;
    esac
done

[[ -n "$config" && -n "$expect_backend" && -n "$probe_url" ]] || usage
[[ -f "$config" ]] || { printf 'config is not a file\n' >&2; exit 64; }
[[ -f "$caddyfile" ]] || { printf 'live Caddyfile is not a file\n' >&2; exit 64; }
[[ "$expect_backend" == 'gateway' || "$expect_backend" == 'incumbent' ]] || {
    printf 'expect-backend must be gateway or incumbent\n' >&2; exit 64
}
[[ "$probe_url" == https://* || "$probe_url" == http://* ]] || {
    printf 'probe-url must be an http(s) URL\n' >&2; exit 64
}
[[ "$timeout_seconds" =~ ^[0-9]+$ ]] || { printf 'timeout must be an integer\n' >&2; exit 64; }
[[ "$interval_seconds" =~ ^[0-9]+$ ]] || { printf 'interval must be an integer\n' >&2; exit 64; }
((timeout_seconds >= 1 && timeout_seconds <= 3600)) || { printf 'timeout out of range\n' >&2; exit 64; }
((interval_seconds >= 1 && interval_seconds <= 60)) || { printf 'interval out of range\n' >&2; exit 64; }

command -v caddy >/dev/null || { printf 'caddy is not installed\n' >&2; exit 64; }
command -v curl >/dev/null || { printf 'curl is not installed\n' >&2; exit 64; }

# Validate before touching the live file: an invalid config would leave routing untouched but still
# burn the measurement window, and a failed reload is not an RTO datapoint.
caddy validate --config "$config" >/dev/null 2>&1 || {
    printf 'refusing to reload: candidate config is invalid\n' >&2
    exit 65
}

declare -a header_args=()
if [[ -n "$header_file" ]]; then
    [[ -f "$header_file" ]] || { printf 'header-file is not a file\n' >&2; exit 64; }
    IFS= read -r probe_header < "$header_file" || probe_header=''
    [[ -n "$probe_header" ]] || { printf 'header-file is empty\n' >&2; exit 64; }
    header_args=(--header "$probe_header")
fi

# The gateway and the incumbent are distinguished by a response marker that carries no secret. The
# gateway's health endpoint is an exact fixed body, so a probe that reaches the gateway is
# identifiable without inspecting inference output.
observe_backend() {
    local body
    body=$(curl --silent --show-error --max-time 5 "${header_args[@]}" "$probe_url" 2>/dev/null) || {
        printf 'unreachable\n'
        return 0
    }
    if [[ "$body" == '{"status":"ok"}' ]]; then
        printf 'gateway\n'
    elif [[ -z "$body" ]]; then
        printf 'empty\n'
    else
        printf 'incumbent\n'
    fi
}

before=$(observe_backend)

start_ns=$(date +%s%N)
if ! install -m 0644 "$config" "$caddyfile"; then
    printf 'failed to install candidate config\n' >&2
    exit 1
fi
if ! caddy reload --config "$caddyfile" --force >/dev/null 2>&1; then
    printf 'caddy reload failed; routing may be unchanged\n' >&2
    exit 1
fi
reload_returned_ns=$(date +%s%N)

observed=''
elapsed_ms=''
deadline=$((SECONDS + timeout_seconds))
while ((SECONDS < deadline)); do
    observed=$(observe_backend)
    if [[ "$observed" == "$expect_backend" ]]; then
        now_ns=$(date +%s%N)
        elapsed_ms=$(((now_ns - start_ns) / 1000000))
        break
    fi
    sleep "$interval_seconds"
done

reload_ms=$(((reload_returned_ns - start_ns) / 1000000))

if [[ -z "$elapsed_ms" ]]; then
    printf 'p12-09-rto: routing did not reach %s within %ss (last observed: %s)\n' \
        "$expect_backend" "$timeout_seconds" "$observed" >&2
    exit 1
fi

printf 'p12-09-rto: before=%s after=%s reload_returned_ms=%s effective_ms=%s\n' \
    "$before" "$expect_backend" "$reload_ms" "$elapsed_ms"

if [[ -n "$output" ]]; then
    umask 077
    cat > "$output" <<RECEIPT
{
  "schema_version": "cpa-rust-gateway-p12-09-rto-v1",
  "measured_at": "$(date -u '+%Y-%m-%dT%H:%M:%SZ')",
  "backend_before": "$before",
  "backend_after": "$expect_backend",
  "reload_returned_ms": $reload_ms,
  "effective_ms": $elapsed_ms,
  "probe_interval_seconds": $interval_seconds
}
RECEIPT
    chmod 0600 "$output"
    printf 'p12-09-rto: wrote %s\n' "$output"
fi
