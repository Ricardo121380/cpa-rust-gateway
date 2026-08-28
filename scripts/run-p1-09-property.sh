#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

seed="${P1_09_SEED:-}"
if [[ -z "$seed" ]]; then
  seed="$(od -An -N8 -tu8 /dev/urandom | tr -d '[:space:]')"
fi

printf 'P1-09 random property seed: %s\n' "$seed"
P1_09_SEED="$seed" cargo test --locked -p protocol-openai-responses \
  --test p1_09_tool_chunk_properties random_seed_tool_chunk_interleavings_are_replayable \
  -- --ignored --exact
