#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
checker="$repo_root/scripts/check-p12-02-systemd-unit.rb"
unit="$repo_root/deploy/systemd/cpa-rust-gateway.service"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

ruby "$checker" --unit "$unit" >/dev/null

missing_credential="$work_dir/missing-credential.service"
cp "$unit" "$missing_credential"
ruby -e 'path = ARGV.fetch(0); text = File.read(path); text = text.sub(/^LoadCredential=backup-key:.*\n/, ""); File.write(path, text)' "$missing_credential"
if ruby "$checker" --unit "$missing_credential" >/dev/null 2>&1; then
  printf 'p12-02 systemd unit test: missing credential was accepted\n' >&2
  exit 1
fi

ambient_secret="$work_dir/ambient-secret.service"
cp "$unit" "$ambient_secret"
ruby -e 'path = ARGV.fetch(0); text = File.read(path).sub("[Service]\n", "[Service]\nEnvironment=management-key=forbidden\n"); File.write(path, text)' "$ambient_secret"
if ruby "$checker" --unit "$ambient_secret" >/dev/null 2>&1; then
  printf 'p12-02 systemd unit test: ambient credential fallback was accepted\n' >&2
  exit 1
fi

unsupported_condition="$work_dir/unsupported-condition.service"
cp "$unit" "$unsupported_condition"
ruby -e 'path = ARGV.fetch(0); text = File.read(path).sub("ConditionPathExists=", "ConditionPathIsExecutable="); File.write(path, text)' "$unsupported_condition"
if ruby "$checker" --unit "$unsupported_condition" >/dev/null 2>&1; then
  printf 'p12-02 systemd unit test: unsupported condition was accepted\n' >&2
  exit 1
fi

# The `systemd-analyze` arm is only reachable on a systemd host. Where it is, prove it rejects a
# directive that parses statically but is semantically invalid -- and which systemd reports while
# still exiting 0, so a naive exit-status check would pass it. Restart= is checked byte-exact by the
# static table, so mutate its value via a directive the table does not pin: SyslogLevel=.
if command -v systemd-analyze >/dev/null 2>&1; then
  ruby "$checker" --unit "$unit" >/dev/null || {
    printf 'p12-02 systemd unit test: the pristine unit was rejected on a systemd host\n' >&2
    exit 1
  }

  unparseable_directive="$work_dir/unparseable-directive.service"
  cp "$unit" "$unparseable_directive"
  ruby -e 'path = ARGV.fetch(0); text = File.read(path).sub("[Service]\n", "[Service]\nSyslogLevel=not-a-level\n"); File.write(path, text)' "$unparseable_directive"
  if ruby "$checker" --unit "$unparseable_directive" >/dev/null 2>&1; then
    printf 'p12-02 systemd unit test: an unparseable directive value was accepted\n' >&2
    exit 1
  fi
fi

printf 'p12-02 systemd unit test: ok\n'
