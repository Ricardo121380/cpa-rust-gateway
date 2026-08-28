#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

candidate_path="$repo_root/target/p11-03-benchmark-candidate.json"
compare=0
while (( $# > 0 )); do
  case "$1" in
    --candidate)
      candidate_path="${2:?--candidate requires a path}"
      shift 2
      ;;
    --compare)
      compare=1
      shift
      ;;
    *)
      printf 'usage: %s [--candidate PATH] [--compare]\n' "$0" >&2
      exit 2
      ;;
  esac
done

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_dir" != /* ]]; then
  target_dir="$repo_root/$target_dir"
fi
criterion_root="$target_dir/criterion"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

build_benchmark() {
  local package="$1"
  local bench="$2"
  local cargo_output
  local executable

  cargo_output="$(cargo bench --locked -p "$package" --bench "$bench" --no-run --message-format=json)"
  executable="$(printf '%s\n' "$cargo_output" | ruby -rjson -e '
    wanted = ARGV.fetch(0)
    STDIN.each_line do |line|
      message = JSON.parse(line)
      next unless message["reason"] == "compiler-artifact"
      next unless message.dig("target", "name") == wanted
      next unless message["executable"].is_a?(String)

      puts message["executable"]
      exit 0
    end
    exit 1
  ' "$bench")"
  if [[ ! -x "$executable" ]]; then
    printf 'p11-03 benchmarks: cargo did not produce %s\n' "$bench" >&2
    exit 1
  fi
  printf '%s\n' "$executable"
}

measure_rss() {
  local label="$1"
  local package="$2"
  local bench="$3"
  local log_path="$work_dir/${label}.time.log"
  local benchmark_path="$work_dir/${label}.benchmark.log"
  local executable
  local rss_bytes

  executable="$(build_benchmark "$package" "$bench")"

  case "$(uname -s)" in
    Darwin)
      if ! /usr/bin/time -l "$executable" --bench --noplot >"$benchmark_path" 2>"$log_path"; then
        cat "$benchmark_path" >&2
        cat "$log_path" >&2
        exit 1
      fi
      rss_bytes="$(awk '$2 == "maximum" && $3 == "resident" && $4 == "set" && $5 == "size" { print $1; exit }' "$log_path")"
      ;;
    Linux)
      if ! /usr/bin/time -v "$executable" --bench --noplot >"$benchmark_path" 2>"$log_path"; then
        cat "$benchmark_path" >&2
        cat "$log_path" >&2
        exit 1
      fi
      rss_bytes="$(awk '/Maximum resident set size/ { print $NF * 1024; exit }' "$log_path")"
      ;;
    *)
      printf 'p11-03 benchmarks: unsupported operating system for peak RSS capture\n' >&2
      exit 1
      ;;
  esac

  cat "$benchmark_path" >&2
  cat "$log_path" >&2
  if [[ ! "$rss_bytes" =~ ^[1-9][0-9]*$ ]]; then
    printf 'p11-03 benchmarks: could not parse peak RSS for %s\n' "$label" >&2
    exit 1
  fi
  printf '%s\n' "$rss_bytes"
}

mock_rss_bytes="$(measure_rss mock_provider gateway-provider p11_03_mock_provider)"
http_rss_bytes="$(measure_rss http_warm_path gateway-http-actix p11_03_http_warm_path)"

mkdir -p "$(dirname "$candidate_path")"
ruby "$repo_root/scripts/record-p11-03-benchmark-baseline.rb" \
  --criterion-root "$criterion_root" \
  --mock-rss-bytes "$mock_rss_bytes" \
  --http-rss-bytes "$http_rss_bytes" \
  --output "$candidate_path"

if (( compare == 1 )); then
  ruby "$repo_root/scripts/check-p11-03-benchmark-baseline.rb" --candidate "$candidate_path"
fi

printf 'p11-03 benchmarks: candidate ready at %s\n' "$candidate_path"
