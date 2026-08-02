#!/usr/bin/env bash

set -euo pipefail

mode="${1:-fast}"
if [[ "$mode" != "docs" && "$mode" != "fast" && "$mode" != "full" && "$mode" != "supply-chain" ]]; then
  printf 'usage: %s [docs|fast|full|supply-chain]\n' "$0" >&2
  exit 2
fi

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

report_path="${CHECK_REPORT_PATH:-}"
if [[ -n "$report_path" && "$report_path" != /* ]]; then
  report_path="$repo_root/$report_path"
fi

if [[ -n "$report_path" ]]; then
  mkdir -p "$(dirname "$report_path")"
  {
    printf '# Check run log\n\n'
    printf -- '- Mode: `%s`\n' "$mode"
    printf -- '- Started: `%s`\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    printf -- '- Host: `%s`\n\n' "$(uname -srm)"
    printf '| Step | Duration seconds | Result |\n'
    printf '|---|---:|---|\n'
  } > "$report_path"
fi

run_step() {
  local label="$1"
  shift
  local started finished duration status

  printf '\n==> %s\n' "$label"
  started="$(date +%s)"
  set +e
  "$@"
  status=$?
  set -e
  finished="$(date +%s)"
  duration=$((finished - started))

  if [[ -n "$report_path" ]]; then
    if (( status == 0 )); then
      printf '| %s | %d | PASS |\n' "$label" "$duration" >> "$report_path"
    else
      printf '| %s | %d | FAIL (%d) |\n' "$label" "$duration" "$status" >> "$report_path"
    fi
  fi

  if (( status != 0 )); then
    printf 'check: %s failed with status %d\n' "$label" "$status" >&2
    exit "$status"
  fi
}

check_quality_tool_versions() {
  source "$repo_root/tools/quality-tool-versions.env"
  cargo deny --version | rg -q "$CARGO_DENY_VERSION"
  cargo audit --version | rg -q "$CARGO_AUDIT_VERSION"
  cargo cyclonedx --version | rg -q "$CARGO_CYCLONEDX_VERSION"
}

check_tracked_whitespace() {
  local empty_tree
  empty_tree="$(git hash-object -t tree /dev/null)"
  git diff --check "$empty_tree" HEAD
}

install_management_spa_dependencies() {
  command -v node >/dev/null
  command -v npm >/dev/null
  (
    cd "$repo_root/web/admin-ui"
    npm ci --ignore-scripts --no-audit --no-fund
  )
}

if [[ "$mode" == "docs" ]]; then
  run_step "Document links" "$repo_root/scripts/check-doc-links.rb"
  run_step "Contract tests" "$repo_root/scripts/check-contract-tests.rb"
  run_step "Plan state" "$repo_root/scripts/check-plan-state.rb"
  run_step "Canary thresholds" "$repo_root/scripts/check-p12-08-canary-thresholds.rb"
  run_step "Caddy split" "$repo_root/scripts/check-p12-caddy-split.rb"
  run_step "Tracked secret scan" "$repo_root/scripts/secret-scan.sh" --all
  run_step "Git whitespace" check_tracked_whitespace

  if [[ -n "$report_path" ]]; then
    printf '\nCompleted: `%s`\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" >> "$report_path"
  fi

  printf '\ncheck: docs passed\n'
  exit 0
fi

if [[ "$mode" == "supply-chain" ]]; then
  run_step "Quality tool versions" check_quality_tool_versions
  run_step "Dependency policy" cargo deny check
  run_step "RustSec audit" cargo audit

  if [[ -n "$report_path" ]]; then
    printf '\nCompleted: `%s`\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" >> "$report_path"
  fi

  printf '\ncheck: %s passed\n' "$mode"
  exit 0
fi

run_step "Shell syntax" "$repo_root/scripts/check-shell-syntax.sh"
run_step "CI workflow" "$repo_root/scripts/check-ci-workflow.rb"
run_step "Release artifact workflow" "$repo_root/scripts/check-release-artifact-workflow.rb"
run_step "CI change classifier" "$repo_root/scripts/test-ci-change-classifier.sh"
run_step "Plan state" "$repo_root/scripts/check-plan-state.rb"
run_step "Plan state guard" "$repo_root/scripts/test-plan-state-check.sh"
run_step "Canary thresholds" "$repo_root/scripts/check-p12-08-canary-thresholds.rb"
run_step "Caddy split" "$repo_root/scripts/check-p12-caddy-split.rb"
run_step "Caddy split guard" "$repo_root/scripts/test-p12-caddy-split.sh"
run_step "Management SPA dependencies" install_management_spa_dependencies
run_step "Benchmark baseline comparator" "$repo_root/scripts/test-p11-03-benchmark-baseline.sh"
run_step "Soak runner argument guard" "$repo_root/scripts/test-p11-04-soak-runner.sh"
run_step "Soak receipt checker" "$repo_root/scripts/test-p11-04-soak-receipt.sh"
run_step "Quality installer cache behavior" "$repo_root/scripts/test-install-quality-tools.sh"
run_step "Release artifact verifier" "$repo_root/scripts/test-p12-release-artifact.rb"
run_step "P12 systemd unit" "$repo_root/scripts/test-p12-02-systemd-unit.sh"
run_step "P12 OpenAI differential graph" python3 "$repo_root/scripts/test-p12-06-openai-graph.py"
run_step "P12 OpenAI differential classifier" python3 "$repo_root/scripts/test-p12-06-openai-differential.py"
run_step "P12 OpenClaw migration dry-run" python3 "$repo_root/scripts/test-p12-08f3-openclaw-migration-dry-run.py"
run_step "P12 G1 production graph" python3 "$repo_root/scripts/test-p12-08g1-production-graph.py"
run_step "P12 G1 live harness" python3 "$repo_root/scripts/test-p12-08g1-live-e2e.py"
run_step "P12 G1 Chat SSE classifier" python3 "$repo_root/scripts/test-p12-08g1-chat-sse-classifier.py"
run_step "P12 G1 Responses JSON classifier" python3 "$repo_root/scripts/test-p12-08g1-responses-json-classifier.py"
run_step "Management SPA" node "$repo_root/scripts/check-management-spa.mjs"
run_step "Rust format" cargo fmt --all -- --check
run_step "Clippy" cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
run_step "Rust tests" cargo test --locked --workspace --all-features
run_step "P12 serve envelope" "$repo_root/scripts/test-p12-02-serve.sh"
run_step "Source policy" "$repo_root/scripts/check-source-policy.rb"
run_step "Secret scanner test" "$repo_root/scripts/test-secret-scan.sh"
run_step "Crate boundaries" "$repo_root/scripts/check-crate-boundaries.rb"
run_step "Document links" "$repo_root/scripts/check-doc-links.rb"
run_step "Contract tests" "$repo_root/scripts/check-contract-tests.rb"
run_step "Tracked secret scan" "$repo_root/scripts/secret-scan.sh" --all
run_step "Git whitespace" check_tracked_whitespace

if [[ "$mode" == "full" ]]; then
  run_step "Quality tool versions" check_quality_tool_versions
  run_step "Dependency policy" cargo deny check
  run_step "RustSec audit" cargo audit
fi

if [[ -n "$report_path" ]]; then
  printf '\nCompleted: `%s`\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" >> "$report_path"
fi

printf '\ncheck: %s passed\n' "$mode"
