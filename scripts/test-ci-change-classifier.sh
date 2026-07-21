#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
classifier="$repo_root/scripts/classify-ci-change.sh"

assert_scope() {
  local expected="$1"
  shift
  local observed
  # GitHub always supplies GITHUB_OUTPUT to a job step. The classifier correctly writes to that
  # file in production, but this black-box test needs its CLI result on stdout.
  observed="$(env -u GITHUB_OUTPUT "$classifier" "$@")"
  if [[ "$observed" != "$expected" ]]; then
    printf 'ci-change-classifier: expected %s, got %s\n' "$expected" "$observed" >&2
    exit 1
  fi
}

assert_scope docs \
  --changed-file docs/06-development-plan.md \
  --changed-file docs/reports/p4-00-execution-acceleration.md
assert_scope docs --changed-file README.md
assert_scope code --changed-file crates/gateway-core/src/lib.rs
assert_scope code --changed-file Cargo.lock
assert_scope code --changed-file .github/workflows/ci.yml
assert_scope code --changed-file crates/gateway-core/src/deleted.rs
assert_scope code \
  --changed-file docs/06-development-plan.md \
  --changed-file scripts/check.sh
assert_scope code --event workflow_dispatch --changed-file docs/06-development-plan.md
assert_scope code --ref-type tag --changed-file docs/06-development-plan.md

printf 'ci-change-classifier: ok\n'
