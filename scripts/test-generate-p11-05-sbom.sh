#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
output_path="$repo_root/docs/reports/evidence/p11-05-rust-sbom.cdx.json"
temporary_copy="$(mktemp "${TMPDIR:-/tmp}/p11-05-sbom.XXXXXX")"
trap 'rm -f "$temporary_copy"' EXIT

"$repo_root/scripts/generate-p11-05-sbom.rb"
cp "$output_path" "$temporary_copy"
"$repo_root/scripts/generate-p11-05-sbom.rb"
cmp --silent "$temporary_copy" "$output_path"

ruby -rjson -e '
  bom = JSON.parse(File.read(ARGV.fetch(0)))
  abort "empty SBOM" if bom.fetch("components").empty?
  abort "wrong format" unless bom.fetch("bomFormat") == "CycloneDX"
  abort "wrong spec" unless bom.fetch("specVersion") == "1.5"
' "$output_path"

if rg -n -i 'path\+file:|file://|/Users/|/home/' "$output_path"; then
  printf 'p11-05-sbom: local reference remains\n' >&2
  exit 1
fi

printf 'p11-05-sbom: deterministic and publishable\n'
