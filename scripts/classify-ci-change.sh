#!/usr/bin/env bash

set -euo pipefail

event_name="${CI_EVENT_NAME:-push}"
ref_type="${CI_REF_TYPE:-branch}"
before_sha="${CI_BEFORE_SHA:-}"
base_sha="${CI_BASE_SHA:-}"
head_sha="${CI_HEAD_SHA:-HEAD}"
declare -a changed_files=()

usage() {
  printf 'usage: %s [--event NAME] [--ref-type TYPE] [--before SHA] [--base SHA] [--head SHA] [--changed-file PATH]...\n' "$0" >&2
}

while (( $# > 0 )); do
  case "$1" in
    --event)
      event_name="${2:?missing event name}"
      shift 2
      ;;
    --ref-type)
      ref_type="${2:?missing ref type}"
      shift 2
      ;;
    --before)
      before_sha="${2:?missing before SHA}"
      shift 2
      ;;
    --base)
      base_sha="${2:?missing base SHA}"
      shift 2
      ;;
    --head)
      head_sha="${2:?missing head SHA}"
      shift 2
      ;;
    --changed-file)
      changed_files+=("${2:?missing changed file path}")
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

emit_scope() {
  local scope="$1"
  if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    printf 'scope=%s\n' "$scope" >> "$GITHUB_OUTPUT"
  else
    printf '%s\n' "$scope"
  fi
}

is_docs_only_path() {
  local path="$1"
  case "$path" in
    README.md|docs/*.md)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

if [[ "$event_name" == "workflow_dispatch" || "$ref_type" == "tag" ]]; then
  emit_scope "code"
  exit 0
fi

if (( ${#changed_files[@]} == 0 )); then
  case "$event_name" in
    push)
      if [[ -z "$before_sha" || "$before_sha" =~ ^0+$ ]]; then
        emit_scope "code"
        exit 0
      fi
      # Deletions are delivery-relevant too: deleting a Rust file, workflow, or lockfile must
      # never be hidden behind a docs-only classification.
      mapfile -t changed_files < <(git diff --name-only --diff-filter=ACMRD "$before_sha" "$head_sha")
      ;;
    pull_request)
      if [[ -z "$base_sha" ]]; then
        emit_scope "code"
        exit 0
      fi
      mapfile -t changed_files < <(git diff --name-only --diff-filter=ACMRD "$base_sha" "$head_sha")
      ;;
    *)
      emit_scope "code"
      exit 0
      ;;
  esac
fi

if (( ${#changed_files[@]} == 0 )); then
  emit_scope "code"
  exit 0
fi

for path in "${changed_files[@]}"; do
  if ! is_docs_only_path "$path"; then
    emit_scope "code"
    exit 0
  fi
done

emit_scope "docs"
