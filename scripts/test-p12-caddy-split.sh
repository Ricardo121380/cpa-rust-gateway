#!/usr/bin/env bash
# Verifies that the direct-cutover checker actually rejects the regressions it claims to guard. Each
# case is a real edit to a copy of the shipped fragment, not a synthetic fixture.

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
checker="$repo_root/scripts/check-p12-caddy-split.rb"
split="$repo_root/deploy/caddy/canary.Caddyfile"
rollback="$repo_root/deploy/caddy/rollback.Caddyfile"
staging="$repo_root/deploy/caddy/staging-domain.Caddyfile"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

ruby "$checker" --split "$split" --rollback "$rollback" --staging "$staging" >/dev/null

# Each case: label, then a Ruby expression mutating the split copy.
reject_split() {
    local label="$1"
    local mutation="$2"
    local candidate="$work_dir/split.Caddyfile"
    cp "$split" "$candidate"
    ruby -e "path = ARGV.fetch(0); text = File.read(path); $mutation; File.write(path, text)" "$candidate"
    if ruby "$checker" --split "$candidate" --rollback "$rollback" >/dev/null 2>&1; then
        printf 'p12-caddy-split test: %s was accepted\n' "$label" >&2
        exit 1
    fi
}

reject_rollback() {
    local label="$1"
    local mutation="$2"
    local candidate="$work_dir/rollback.Caddyfile"
    cp "$rollback" "$candidate"
    ruby -e "path = ARGV.fetch(0); text = File.read(path); $mutation; File.write(path, text)" "$candidate"
    if ruby "$checker" --split "$split" --rollback "$candidate" >/dev/null 2>&1; then
        printf 'p12-caddy-split test: %s was accepted\n' "$label" >&2
        exit 1
    fi
}

inject_split() {
    local label="$1"
    local mutation="$2"
    local candidate="$work_dir/split.Caddyfile"
    cp "$split" "$candidate"
    ruby -e "path = ARGV.fetch(0); text = File.read(path); $mutation; File.write(path, text)" "$candidate"
    if ruby "$checker" --split "$candidate" --rollback "$rollback" >/dev/null 2>&1; then
        printf 'p12-caddy-split test: %s was accepted\n' "$label" >&2
        exit 1
    fi
}

# A global servers block would retime every site sharing the :443 listener, not just this one.
inject_split 'a global servers block' \
    'text = text.sub("cpa.example.invalid {", "{\n\tservers {\n\t\ttimeouts {\n\t\t\tidle 1h\n\t\t}\n\t}\n}\n\ncpa.example.invalid {")'
# If a timeout is ever introduced anyway, it must still clear the gateway's ceilings.
inject_split 'an idle timeout below the keepalive interval' \
    'text = text.sub("cpa.example.invalid {", "{\n\tservers {\n\t\ttimeouts {\n\t\t\tidle 10s\n\t\t}\n\t}\n}\n\ncpa.example.invalid {")'
inject_split 'a non-zero write deadline' \
    'text = text.sub("cpa.example.invalid {", "{\n\tservers {\n\t\ttimeouts {\n\t\t\twrite 60s\n\t\t}\n\t}\n}\n\ncpa.example.invalid {")'
# Exposure invariants.
reject_split 'a route to the management listener' \
    'text = text.sub("reverse_proxy 127.0.0.1:18180", "reverse_proxy 127.0.0.1:18181")'
reject_split 'compression in front of the event stream' \
    'text = text.sub("\treverse_proxy 127.0.0.1:18180", "\tencode gzip\n\treverse_proxy 127.0.0.1:18180")'
# Direct replacement must never regress to key/header routing or an incumbent fallback.
reject_split 'a bearer key matcher' \
    'text = text.sub("cpa.example.invalid {", "cpa.example.invalid {\n\t@legacy {\n\t\theader Authorization \"Bearer rgw_*\"\n\t}")'
reject_split 'an incumbent fallback' \
    'text = text.sub("reverse_proxy 127.0.0.1:18180", "handle { reverse_proxy 127.0.0.1:18180 }\n\thandle { reverse_proxy 127.0.0.1:8317 }")'
# Rollback must remove the gateway from the path, on the same hostname.
reject_rollback 'a rollback that still routes to the gateway' \
    'text = text.sub("reverse_proxy 127.0.0.1:8317", "reverse_proxy 127.0.0.1:18180")'
reject_rollback 'a rollback for a different hostname' \
    'text = text.sub("cpa.example.invalid", "other.example.invalid")'

reject_staging() {
    local label="$1"
    local mutation="$2"
    local candidate="$work_dir/staging.Caddyfile"
    cp "$staging" "$candidate"
    ruby -e "path = ARGV.fetch(0); text = File.read(path); $mutation; File.write(path, text)" "$candidate"
    if ruby "$checker" --split "$split" --rollback "$rollback" --staging "$candidate" >/dev/null 2>&1; then
        printf 'p12-caddy-split test: %s was accepted\n' "$label" >&2
        exit 1
    fi
}

# The staging domain is the gateway's first public route, so its exposure invariants matter most.
reject_staging 'a staging route to the management listener' \
    'text = text.sub("reverse_proxy 127.0.0.1:18180", "reverse_proxy 127.0.0.1:18181")'
reject_staging 'a staging global servers block' \
    'text = text.sub("cpar.example.invalid {", "{\n\tservers {\n\t\ttimeouts {\n\t\t\tidle 1h\n\t\t}\n\t}\n}\n\ncpar.example.invalid {")'
reject_staging 'compression on the staging domain' \
    'text = text.sub("\treverse_proxy 127.0.0.1:18180", "\tencode gzip\n\treverse_proxy 127.0.0.1:18180")'
reject_staging 'a staging domain that also fronts the incumbent' \
    'text = text.sub("reverse_proxy 127.0.0.1:18180", "reverse_proxy 127.0.0.1:8317")'

printf 'p12-caddy-split test: ok (12 rejection paths)\n'
