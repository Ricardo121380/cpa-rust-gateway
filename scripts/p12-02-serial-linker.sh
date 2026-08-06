#!/usr/bin/env bash

set -euo pipefail

is_compile=0
for argument in "$@"; do
  if [[ "$argument" == "-c" ]]; then
    is_compile=1
    break
  fi
done

if (( is_compile )); then
  exec /usr/bin/cc "$@"
fi

exec /usr/bin/cc "$@" -Wl,--threads=1
