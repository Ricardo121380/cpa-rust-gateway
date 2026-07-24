#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
checker="$repo_root/scripts/check-p11-03-benchmark-baseline.rb"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

baseline="$work_dir/baseline.json"
candidate="$work_dir/candidate.json"

cat > "$baseline" <<'JSON'
{
  "schema_version": 1,
  "recorded_at": "2026-07-24T00:00:00Z",
  "git_revision": "0123456789abcdef0123456789abcdef01234567",
  "environment": {"os": "Darwin", "arch": "arm64", "rustc": "rustc test"},
  "method": {"tool": "criterion", "sample_size": 30, "warmup_seconds": 1, "measurement_seconds": 3, "latency_source": "Criterion raw per-operation samples", "rss_source": "per-command operating-system peak RSS"},
  "thresholds": {"max_p99_growth_ratio": 1.15, "max_rss_growth_ratio": 1.15, "min_throughput_ratio": 0.9, "local_http_p99_ns": 5000000},
  "benchmarks": [
    {"id": "mock_provider_canonical_drain", "p50_ns": 100, "p99_ns": 100, "throughput_ops_per_sec": 10000000, "max_rss_bytes": 1000},
    {"id": "http_responses_warm_path", "p50_ns": 1000, "p99_ns": 1000, "throughput_ops_per_sec": 1000000, "max_rss_bytes": 2000}
  ]
}
JSON

cp "$baseline" "$candidate"
"$checker" --baseline "$baseline" --candidate "$candidate" >/dev/null

expect_fail() {
  if "$checker" --baseline "$baseline" --candidate "$candidate" >/dev/null 2>&1; then
    printf 'p11-03 comparator test: expected candidate to fail\n' >&2
    exit 1
  fi
}

ruby -rjson -e 'path = ARGV.fetch(0); value = JSON.parse(File.read(path)); value.fetch("benchmarks").last["p99_ns"] = 1151; File.write(path, JSON.generate(value))' "$candidate"
expect_fail

cp "$baseline" "$candidate"
ruby -rjson -e 'path = ARGV.fetch(0); value = JSON.parse(File.read(path)); value.fetch("benchmarks").first["throughput_ops_per_sec"] = 8_999_999; File.write(path, JSON.generate(value))' "$candidate"
expect_fail

cp "$baseline" "$candidate"
ruby -rjson -e 'path = ARGV.fetch(0); value = JSON.parse(File.read(path)); value.fetch("benchmarks").first["max_rss_bytes"] = 1151; File.write(path, JSON.generate(value))' "$candidate"
expect_fail

cp "$baseline" "$candidate"
ruby -rjson -e 'path = ARGV.fetch(0); value = JSON.parse(File.read(path)); value.fetch("benchmarks").first["id"] = "unexpected"; File.write(path, JSON.generate(value))' "$candidate"
expect_fail

printf 'p11-03 comparator test: ok\n'
