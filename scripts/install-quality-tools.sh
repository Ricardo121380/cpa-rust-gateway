#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
source "$repo_root/tools/quality-tool-versions.env"

version_matches() {
  local command_name="$1"
  local expected="$2"
  local output

  if ! output="$($command_name --version 2>&1)"; then
    return 1
  fi
  [[ "$output" == *"$expected"* ]]
}

install_tool() {
  local crate="$1"
  local command_name="$2"
  local expected="$3"

  if version_matches "$command_name" "$expected"; then
    printf 'quality-tools: %s %s already installed\n' "$crate" "$expected"
    return
  fi

  printf 'quality-tools: installing %s %s\n' "$crate" "$expected"

  local -a clean_env=(
    env
    -u CC
    -u CXX
    -u CFLAGS
    -u CXXFLAGS
    -u CPPFLAGS
    -u LDFLAGS
  )

  if [[ "$(uname -s)" == "Darwin" ]]; then
    clean_env+=(CC=/usr/bin/clang CXX=/usr/bin/clang++)
  fi

  CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" "${clean_env[@]}" \
    cargo install --locked --force --version "$expected" "$crate"

  version_matches "$command_name" "$expected" || {
    printf 'quality-tools: %s version verification failed\n' "$crate" >&2
    exit 1
  }
}

install_tool cargo-deny "cargo deny" "$CARGO_DENY_VERSION"
install_tool cargo-audit "cargo audit" "$CARGO_AUDIT_VERSION"

printf 'quality-tools: ok\n'
