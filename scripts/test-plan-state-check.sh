#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
checker="$repo_root/scripts/check-plan-state.rb"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

expect_pass() {
  "$checker" --plan "$1" >/dev/null
}

expect_fail() {
  if "$checker" --plan "$1" >/dev/null 2>&1; then
    printf 'plan-state: expected fixture to fail: %s\n' "$1" >&2
    exit 1
  fi
}

cat > "$work_dir/one-active.md" <<'EOF'
| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P4-00 | delivery | G3 | evidence | DONE |
| P4-01 | catalog | P4-00 | evidence | IN_PROGRESS |
EOF
expect_pass "$work_dir/one-active.md"

cat > "$work_dir/two-active.md" <<'EOF'
| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P4-00 | delivery | G3 | evidence | IN_PROGRESS |
| P4-01 | catalog | P4-00 | evidence | IN_PROGRESS |
EOF
expect_fail "$work_dir/two-active.md"

cat > "$work_dir/pending-dependency.md" <<'EOF'
| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P4-00 | delivery | G3 | evidence | LOCAL_PASS_PENDING_CI |
| P4-01 | catalog | P4-00 | evidence | IN_PROGRESS |
EOF
expect_fail "$work_dir/pending-dependency.md"

cat > "$work_dir/same-phase-local-pass.md" <<'EOF'
| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P5-00 | delivery | G4 | evidence | DONE |
| P5-01 | messages | P5-00 | evidence | LOCAL_PASS_PENDING_PHASE_GATE |
| P5-02 | count tokens | P5-01 | evidence | IN_PROGRESS |
EOF
expect_pass "$work_dir/same-phase-local-pass.md"

cat > "$work_dir/cross-phase-local-pass.md" <<'EOF'
| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P5-08 | fuzz | P5-03 | evidence | LOCAL_PASS_PENDING_PHASE_GATE |
| P6-01 | grok auth | P5-08 | evidence | IN_PROGRESS |
EOF
expect_fail "$work_dir/cross-phase-local-pass.md"

cat > "$work_dir/done-before-local-dependency.md" <<'EOF'
| ID | Task | 依赖 | 完成证据 | 状态 |
|---|---|---|---|---|
| P5-01 | messages | P5-00 | evidence | LOCAL_PASS_PENDING_PHASE_GATE |
| P5-02 | count tokens | P5-01 | evidence | DONE |
EOF
expect_fail "$work_dir/done-before-local-dependency.md"

printf 'plan-state-test: ok\n'
