#!/usr/bin/env bash

# Builds the management SPA (web/prism) before its exact static bytes are
# embedded by crates/gateway-http-actix.
#
# The SPA lives in this repository since the frontend merge, so the contract it
# generates its client from is this repo's own docs/openapi/management-v1.json.
# `npm run check` fails if the generated client has drifted from it, which is
# what keeps a stale client from ever reaching the binary.

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
ui_root="$repo_root/web/prism"
vite_binary="$ui_root/node_modules/.bin/vite"

if [[ ! -x "$vite_binary" ]]; then
  printf '%s\n' 'management-spa: dependencies are missing; run npm ci --ignore-scripts in web/prism first' >&2
  exit 1
fi

# tsc --noEmit + vite build. The type check is part of the build on purpose:
# an embedded bundle that does not type-check is not a reviewed artifact.
npm --prefix "$ui_root" run build

printf 'management-spa: built %s\n' "$ui_root/dist"
