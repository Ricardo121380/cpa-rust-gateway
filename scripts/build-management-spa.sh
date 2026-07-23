#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
ui_root="$repo_root/web/admin-ui"
typescript_binary="$ui_root/node_modules/.bin/tsc"

if [[ ! -x "$typescript_binary" ]]; then
  printf '%s\n' 'management-spa: dependencies are missing; run npm ci --ignore-scripts in web/admin-ui first' >&2
  exit 1
fi

node "$repo_root/scripts/generate-management-client.mjs" --check
rm -rf "$ui_root/dist"
mkdir -p "$ui_root/dist/assets"
"$typescript_binary" --project "$ui_root/tsconfig.json"
cp "$ui_root/src/index.html" "$ui_root/dist/index.html"
cp "$ui_root/src/styles.css" "$ui_root/dist/assets/styles.css"

printf 'management-spa: built %s\n' "$ui_root/dist"
