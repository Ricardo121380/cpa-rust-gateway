#!/usr/bin/env bash

set -euo pipefail

mode="${1:---all}"
repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

# A field assigned from a named in-process configuration object is a reference, not a literal.
# Keep scanning literal/dynamic-looking values, but do not mistake `options.managementKey` for one.
# `LoadCredential=name:/absolute/source/path` names a systemd credential source; the source path
# is not a credential value. Keep scanning all other management-key assignments, including an
# unsafe `Environment=management-key=<literal>` fallback.
secret_regex="(?i)(authorization:\\s*bearer\\s+[A-Za-z0-9._~+/=-]{16,}|(?:api[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|(?<!LoadCredential=)management[_-]?key)\\s*[:=]\\s*['\"]?(?!(?:options|config|process\\.env|env)\\.)[A-Za-z0-9._~+/=-]{16,}|-----BEGIN[[:space:]][A-Z0-9[:space:]]*PRIVATE KEY-----|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{20,}|sk-[A-Za-z0-9_-]{16,}|ksk_[A-Za-z0-9_-]{16,})"

is_forbidden_path() {
  local path="$1"
  case "$path" in
    .env|*/.env|.env.*|*/.env.*|*.pem|*.key|*.p12|*.pfx|*.jks|*.keystore|*.oauth.json|auths/*|*/auths/*|credentials/*|*/credentials/*|secrets/*|*/secrets/*|deploy-secrets/*|*/deploy-secrets/*|*.sqlite|*.sqlite3|*.sqlite-shm|*.sqlite-wal)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

scan_index_path() {
  local path="$1"

  if is_forbidden_path "$path"; then
    printf 'secret-scan: forbidden credential path: %s\n' "$path" >&2
    return 1
  fi

  if git show ":$path" 2>/dev/null | rg --pcre2 -a "$secret_regex" >/dev/null; then
    printf 'secret-scan: possible secret in staged/tracked file: %s\n' "$path" >&2
    return 1
  fi
}

failures=0
case "$mode" in
  --staged)
    while IFS= read -r -d '' path; do
      scan_index_path "$path" || failures=$((failures + 1))
    done < <(git diff --cached --name-only --diff-filter=ACMR -z)
    ;;
  --all)
    while IFS= read -r -d '' path; do
      scan_index_path "$path" || failures=$((failures + 1))
    done < <(git ls-files -z)
    ;;
  *)
    printf 'usage: %s [--staged|--all]\n' "$0" >&2
    exit 2
    ;;
esac

if (( failures > 0 )); then
  printf 'secret-scan: blocked (%d finding(s)); values were not printed\n' "$failures" >&2
  exit 1
fi

printf 'secret-scan: ok (%s)\n' "$mode"
