#!/usr/bin/env bash
# Collects the P12-06 Kiro evidence: functional verification plus a paired latency comparison
# against kiro-rs, the reference implementation cpa-rust's Kiro channel was modelled on.
#
# Runs on the server. Both gateways are measured with the SAME harness, prompt, model and
# credential, so the difference reported is the difference between the two implementations rather
# than between two workloads.
#
# Per CR-P12-06-003 this is explicitly NOT the incumbent differential: the incumbent CPA has no
# Kiro channel at all. kiro-rs is a *reference* for the same upstream, which makes the comparison
# meaningful in a way a one-sided baseline would not be.
#
# No credential, prompt or response text is written to the output. Reads keys from files only.

set -Eeuo pipefail
umask 077

cpa_url="http://127.0.0.1:18180/v1/messages"
kiro_rs_url="http://127.0.0.1:8990/v1/messages"
samples=10
warmup=1
max_tokens=512
model="claude-sonnet-5"
out_dir=""
cpa_key_file=""
kiro_rs_key_file=""
prompt_file=""

usage() {
    cat >&2 <<'USAGE'
usage: p12-06-run-kiro-comparison.sh --cpa-key-file PATH --kiro-rs-key-file PATH
                                    --prompt-file PATH --out-dir PATH
                                    [--samples N] [--warmup N] [--model NAME]
                                    [--max-tokens N] [--cpa-url URL] [--kiro-rs-url URL]
USAGE
    exit 2
}

while (( $# > 0 )); do
    case "$1" in
        --cpa-key-file) cpa_key_file="${2:?}"; shift 2;;
        --kiro-rs-key-file) kiro_rs_key_file="${2:?}"; shift 2;;
        --prompt-file) prompt_file="${2:?}"; shift 2;;
        --out-dir) out_dir="${2:?}"; shift 2;;
        --samples) samples="${2:?}"; shift 2;;
        --warmup) warmup="${2:?}"; shift 2;;
        --model) model="${2:?}"; shift 2;;
        --max-tokens) max_tokens="${2:?}"; shift 2;;
        --cpa-url) cpa_url="${2:?}"; shift 2;;
        --kiro-rs-url) kiro_rs_url="${2:?}"; shift 2;;
        *) usage;;
    esac
done

[[ -n "$cpa_key_file" && -n "$kiro_rs_key_file" && -n "$prompt_file" && -n "$out_dir" ]] || usage
for path in "$cpa_key_file" "$kiro_rs_key_file" "$prompt_file"; do
    [[ -r "$path" ]] || { printf 'comparison: %s is not readable\n' "$path" >&2; exit 2; }
done
mkdir -p "$out_dir"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
measure="$script_dir/p12-06-measure-stream.py"
[[ -x "$measure" || -r "$measure" ]] || { printf 'comparison: measurement harness missing\n' >&2; exit 2; }

# Interleave the two arms rather than running all of one then all of the other. Upstream latency
# drifts with time of day and with Kiro's own load; a block design would attribute that drift to
# whichever implementation ran during the slower window. Alternating keeps both arms exposed to the
# same conditions.
run_arm() {
    local label=$1 url=$2 key_file=$3 sample_count=$4 out=$5
    python3 "$measure" \
        --url "$url" \
        --key-file "$key_file" \
        --prompt-file "$prompt_file" \
        --model "$model" \
        --max-tokens "$max_tokens" \
        --samples "$sample_count" \
        --warmup 0 \
        --sleep-between 1 \
        --label "$label" \
        --out "$out"
}

printf 'comparison: warming both arms (%s discarded sample(s) each)\n' "$warmup"
if (( warmup > 0 )); then
    run_arm cpa-rust-warmup "$cpa_url" "$cpa_key_file" "$warmup" "$out_dir/.warmup-cpa.json" >/dev/null 2>&1 || true
    run_arm kiro-rs-warmup "$kiro_rs_url" "$kiro_rs_key_file" "$warmup" "$out_dir/.warmup-kiro-rs.json" >/dev/null 2>&1 || true
    rm -f "$out_dir/.warmup-cpa.json" "$out_dir/.warmup-kiro-rs.json"
fi

for (( index = 1; index <= samples; index++ )); do
    printf 'comparison: round %s/%s\n' "$index" "$samples"
    run_arm "cpa-rust-kiro" "$cpa_url" "$cpa_key_file" 1 "$out_dir/round-$index-cpa-rust.json" \
        || printf 'comparison: cpa-rust round %s failed\n' "$index" >&2
    sleep 2
    run_arm "kiro-rs" "$kiro_rs_url" "$kiro_rs_key_file" 1 "$out_dir/round-$index-kiro-rs.json" \
        || printf 'comparison: kiro-rs round %s failed\n' "$index" >&2
    sleep 2
done

python3 "$script_dir/p12-06-summarise-comparison.py" --in-dir "$out_dir" --out "$out_dir/summary.json"
printf 'comparison: wrote %s\n' "$out_dir/summary.json"
