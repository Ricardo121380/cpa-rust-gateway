#!/usr/bin/env bash
# P12-07 pre-exposure verification: proves the new gateway's first public route exposes only what it
# should, before any production traffic is split onto it.
#
# Every check is an assertion with a fail-closed verdict, not an observation to eyeball. The script
# is read-only with respect to server state: it sends requests and inspects TLS, but installs
# nothing and reloads nothing.
#
# It never prints a key, a header value or a response body. A client key may be supplied through a
# file so it does not appear in the process list or in shell history.

set -Eeuo pipefail
umask 077

usage() {
    cat >&2 <<'USAGE'
usage: p12-07-verify-exposure.sh --domain HOST --expect-ip ADDRESS
                                 [--key-file PATH] [--output PATH]

  --domain      Test hostname to verify (for example cpar.example.com)
  --expect-ip   Address the hostname must resolve to; a mismatch means the route is not ours
  --key-file    Optional file whose first line is a full client-key header, used to prove that a
                valid key is accepted. Omit to run only the negative checks.
  --output      Optional receipt path, written 0600, containing verdicts and no secret material
USAGE
    exit 64
}

domain=''
expect_ip=''
key_file=''
output=''

while [[ $# -gt 0 ]]; do
    case "$1" in
        --domain) domain="${2:-}"; shift 2 ;;
        --expect-ip) expect_ip="${2:-}"; shift 2 ;;
        --key-file) key_file="${2:-}"; shift 2 ;;
        --output) output="${2:-}"; shift 2 ;;
        --help) usage ;;
        *) printf 'unknown argument: %s\n' "$1" >&2; usage ;;
    esac
done

[[ -n "$domain" && -n "$expect_ip" ]] || usage
[[ "$domain" =~ ^[A-Za-z0-9.-]+$ ]] || { printf 'domain must be a hostname\n' >&2; exit 64; }
[[ "$expect_ip" =~ ^[0-9.]+$ ]] || { printf 'expect-ip must be an IPv4 address\n' >&2; exit 64; }

for tool in curl dig openssl; do
    command -v "$tool" >/dev/null || { printf '%s is not installed\n' "$tool" >&2; exit 64; }
done

failures=0
declare -a verdicts=()

record() {
    local name="$1"
    local verdict="$2"
    local detail="$3"
    verdicts+=("$name=$verdict")
    if [[ "$verdict" == 'PASS' ]]; then
        printf '  PASS  %-34s %s\n' "$name" "$detail"
    else
        printf '  FAIL  %-34s %s\n' "$name" "$detail" >&2
        failures=$((failures + 1))
    fi
}

printf 'p12-07 exposure verification: %s\n' "$domain"

# 1. DNS must resolve to the intended host, from the authoritative servers rather than a possibly
#    stale recursive cache. A wrong answer here invalidates every later check.
authority="$(dig +short NS "${domain#*.}" 2>/dev/null | head -1)"
if [[ -z "$authority" ]]; then
    record 'dns_authority' 'FAIL' 'no authoritative nameserver found for the zone'
else
    resolved="$(dig +short @"$authority" "$domain" A 2>/dev/null | tr '\n' ' ' | tr -d ' ')"
    if [[ "$resolved" == "$expect_ip" ]]; then
        record 'dns_resolves_to_host' 'PASS' "authoritative answer is the expected address"
    else
        record 'dns_resolves_to_host' 'FAIL' "authoritative answer was not the expected address"
    fi
fi

# 2. The answer must be the origin address, not a proxy edge. A proxied record would insert an idle
#    cutoff we cannot observe or tune from the server, which breaks long-lived SSE.
case "$resolved" in
    104.2*|172.6[4-9].*|172.7*|188.114.*|162.15[89].*)
        record 'dns_not_proxied' 'FAIL' 'record appears to be proxied; long streams would be cut by the edge' ;;
    '') record 'dns_not_proxied' 'FAIL' 'no address to classify' ;;
    *) record 'dns_not_proxied' 'PASS' 'record points at the origin (DNS only)' ;;
esac

# 3. TLS must terminate with a certificate actually valid for this hostname.
if tls_subject="$(echo | openssl s_client -servername "$domain" -connect "$domain:443" 2>/dev/null \
    | openssl x509 -noout -checkhost "$domain" 2>/dev/null)"; then
    if [[ "$tls_subject" == *"does match"* ]]; then
        record 'tls_certificate_matches' 'PASS' 'certificate is valid for this hostname'
    else
        record 'tls_certificate_matches' 'FAIL' 'certificate does not match this hostname'
    fi
else
    record 'tls_certificate_matches' 'FAIL' 'TLS handshake or certificate parse failed'
fi

# 4. Health must be reachable, proving the route reaches the data plane at all.
health="$(curl --silent --show-error --max-time 10 "https://$domain/healthz" 2>/dev/null || true)"
if [[ "$health" == '{"status":"ok"}' ]]; then
    record 'data_plane_reachable' 'PASS' 'health endpoint returned the exact expected body'
else
    record 'data_plane_reachable' 'FAIL' 'health endpoint did not return the expected body'
fi

# 5. An unauthenticated inference request must be rejected. This is the core exposure invariant: a
#    public route that serves inference without a key would be an open relay onto real credentials.
unauth_status="$(curl --silent --output /dev/null --write-out '%{http_code}' --max-time 10 \
    --request POST --header 'content-type: application/json' \
    --data '{"model":"probe","input":"probe"}' \
    "https://$domain/v1/responses" 2>/dev/null || true)"
if [[ "$unauth_status" == '401' || "$unauth_status" == '403' ]]; then
    record 'unauthenticated_rejected' 'PASS' "status $unauth_status"
else
    record 'unauthenticated_rejected' 'FAIL' "expected 401 or 403, observed ${unauth_status:-none}"
fi

# 6. A wrong key must also be rejected, so that check 5 is not merely a missing-header path.
badkey_status="$(curl --silent --output /dev/null --write-out '%{http_code}' --max-time 10 \
    --request POST --header 'content-type: application/json' \
    --header 'authorization: Bearer rgw_not_a_real_key' \
    --data '{"model":"probe","input":"probe"}' \
    "https://$domain/v1/responses" 2>/dev/null || true)"
if [[ "$badkey_status" == '401' || "$badkey_status" == '403' ]]; then
    record 'invalid_key_rejected' 'PASS' "status $badkey_status"
else
    record 'invalid_key_rejected' 'FAIL' "expected 401 or 403, observed ${badkey_status:-none}"
fi

# 7. The management plane must be unreachable through this hostname. It authorizes by peer address,
#    so a public route would hand the management surface to the internet.
management_reachable=0
for path in /admin/config-versions /admin/observability/metrics /admin-ui/ /admin/upstreams; do
    status="$(curl --silent --output /dev/null --write-out '%{http_code}' --max-time 10 \
        "https://$domain$path" 2>/dev/null || true)"
    if [[ "$status" == '200' ]]; then
        record "management_unexposed${path//\//_}" 'FAIL' "management path answered 200"
        management_reachable=1
    fi
done
if [[ "$management_reachable" -eq 0 ]]; then
    record 'management_plane_unexposed' 'PASS' 'no management path answered 200'
fi

# 8. If a key was supplied, prove the positive path: a valid key is accepted. Without this, checks 5
#    and 6 could pass on a route that rejects everything, including legitimate traffic.
if [[ -n "$key_file" ]]; then
    [[ -f "$key_file" ]] || { printf 'key-file is not a file\n' >&2; exit 64; }
    IFS= read -r key_header < "$key_file" || key_header=''
    [[ -n "$key_header" ]] || { printf 'key-file is empty\n' >&2; exit 64; }
    models_status="$(curl --silent --output /dev/null --write-out '%{http_code}' --max-time 15 \
        --header "$key_header" "https://$domain/v1/models" 2>/dev/null || true)"
    unset key_header
    if [[ "$models_status" == '200' ]]; then
        record 'valid_key_accepted' 'PASS' 'model listing returned 200'
    else
        record 'valid_key_accepted' 'FAIL' "expected 200, observed ${models_status:-none}"
    fi
else
    printf '  SKIP  %-34s %s\n' 'valid_key_accepted' 'no --key-file supplied'
fi

printf '\n'
if [[ "$failures" -gt 0 ]]; then
    printf 'p12-07 exposure verification: %d check(s) failed\n' "$failures" >&2
    exit 1
fi
printf 'p12-07 exposure verification: all checks passed\n'

if [[ -n "$output" ]]; then
    umask 077
    {
        printf '{\n'
        printf '  "schema_version": "cpa-rust-gateway-p12-07-exposure-v1",\n'
        printf '  "verified_at": "%s",\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
        printf '  "domain": "%s",\n' "$domain"
        printf '  "verdicts": {\n'
        for index in "${!verdicts[@]}"; do
            name="${verdicts[index]%%=*}"
            verdict="${verdicts[index]#*=}"
            separator=','
            [[ "$index" -eq $((${#verdicts[@]} - 1)) ]] && separator=''
            printf '    "%s": "%s"%s\n' "$name" "$verdict" "$separator"
        done
        printf '  }\n}\n'
    } > "$output"
    chmod 0600 "$output"
    printf 'p12-07 exposure verification: wrote %s\n' "$output"
fi
