#!/usr/bin/env bash
# This replaces macOS Seatbelt with a task-owned network namespace, kernel firewall,
# read-only root/mounts and an unprivileged, capability-free process tree.
set -euo pipefail
test "$(uname -m)" = aarch64
test "$(id -u)" = 0
test ! -e /work/run
iptables -A OUTPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
for port in 19180 19181 19443; do
  iptables -A OUTPUT -o lo -d 127.0.0.1 -p tcp --dport "$port" -j ACCEPT
done
iptables -A OUTPUT -j REJECT
ip6tables -A OUTPUT -j REJECT
mkdir /work/run
chmod 0700 /work/run
iptables-save > /work/run/ipv4.rules
ip6tables-save > /work/run/ipv6.rules
cp -R /fixtures /work/run/fixtures
chown -R 65532:65532 /work/run
exec setpriv --reuid=65532 --regid=65532 --clear-groups --bounding-set=-all \
  --inh-caps=-all --ambient-caps=-all python3 /source/scripts/acceptance/cpar66-reader-serve.py \
  --artifact /artifact --fixtures /work/run/fixtures --output /work/run
