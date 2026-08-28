#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
count=0

while IFS= read -r -d '' script; do
  bash -n "$script"
  count=$((count + 1))
done < <(find "$repo_root/scripts" -type f -name '*.sh' -print0)

if (( count == 0 )); then
  printf 'shell-syntax: no shell scripts found\n' >&2
  exit 1
fi

printf 'shell-syntax: ok (%d scripts)\n' "$count"
