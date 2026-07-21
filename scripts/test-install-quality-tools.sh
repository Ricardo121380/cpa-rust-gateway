#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
installer="$repo_root/scripts/install-quality-tools.sh"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT
mkdir -p "$work_dir/bin" "$work_dir/state"

cat > "$work_dir/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

state_dir="${FAKE_CARGO_STATE:?}"
case "${1:-}:${2:-}" in
  deny:--version)
    test -f "$state_dir/deny" || exit 1
    printf 'cargo-deny 0.20.2\n'
    ;;
  audit:--version)
    test -f "$state_dir/audit" || exit 1
    printf 'cargo-audit 0.22.2\n'
    ;;
  install:*)
    crate="${!#}"
    case "$crate" in
      cargo-deny) touch "$state_dir/deny" ;;
      cargo-audit) touch "$state_dir/audit" ;;
      *) exit 2 ;;
    esac
    printf '%s\n' "$crate" >> "$state_dir/installs"
    ;;
  *)
    exit 2
    ;;
esac
EOF
chmod +x "$work_dir/bin/cargo"

PATH="$work_dir/bin:$PATH" FAKE_CARGO_STATE="$work_dir/state" "$installer" >/dev/null
test -f "$work_dir/state/deny"
test -f "$work_dir/state/audit"
test "$(wc -l < "$work_dir/state/installs")" -eq 2

rm -f "$work_dir/state/installs"
PATH="$work_dir/bin:$PATH" FAKE_CARGO_STATE="$work_dir/state" "$installer" >/dev/null
test ! -e "$work_dir/state/installs"

printf 'quality-tools-installer: ok (miss then version-verified hit)\n'
