#!/usr/bin/env bash

set -euo pipefail

linker_arguments=()
for argument in "$@"; do
  if [[ "$argument" == "-fuse-ld=lld" ]]; then
    linker_arguments+=("-fuse-ld=bfd")
  else
    linker_arguments+=("$argument")
  fi
done

exec /usr/bin/cc "${linker_arguments[@]}"
