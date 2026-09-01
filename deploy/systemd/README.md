# P12-02 systemd deployment envelope

This directory is a reviewed **template**, not an instruction that has already changed a server.
P12-03 must first record a timestamped backup/rollback inventory, and P12-04 alone may install a
staging instance after that evidence is accepted.

## Fixed service boundary

[`cpa-rust-gateway.service`](cpa-rust-gateway.service) runs the accepted P12-01 Linux binary as
the dedicated `cpa-gateway` user. It owns only systemd-created paths:

- state: `/var/lib/cpa-rust-gateway` (`StateDirectory`), including `control.sqlite3` and the
  fixed `backups/` staging directory;
- runtime: `/run/cpa-rust-gateway` (`RuntimeDirectory`);
- log directory: `/var/log/cpa-rust-gateway` (`LogsDirectory`), while process output itself goes
  to journald.

It starts two distinct loopback listeners: data readiness at `127.0.0.1:18180` and protected
management at `127.0.0.1:18181`. The P12-02 data listener has only `HEAD /` and `GET /healthz`.
It is not a Provider or inference deployment. The management listener carries the existing P10
API/UI and remains loopback-only at both systemd and P10 admission layers.

## Credential hand-off contract

Before a future staging installation, an operator supplies six root-owned direct regular files
outside this repository under `/etc/cpa-rust-gateway/credentials/`. The unit copies them through
`LoadCredential=` and passes only the systemd-provided read-only credential directory (`%d`) to the
process. The process rejects missing, symbolic-link, special, empty, oversized, malformed, or
wrong-size material. It does not read Secret environment variables or a home directory.

| Credential name | Required form | Owner |
|---|---|---|
| `management-key` | one `mgmt_`-namespaced ASCII value, 32–512 bytes | P10 admission |
| `management-csrf` | one independent `csrf_`-namespaced ASCII value, 32–512 bytes | P10 browser boundary |
| `master-key` | exactly 32 raw bytes, active version `1` | encrypted control-plane Secrets |
| `backup-key` | exactly 32 raw bytes | encrypted backup/restore material |
| `client-key-pepper` | exactly 32 raw bytes | Client-Key issuance and verification |
| `grok-build-cache-key` | exactly 32 raw bytes | tenant-isolated Grok Build prompt-cache identities |

Credential values, endpoints, provider keys, database snapshots, restore artifacts, and server
paths must never be committed. The unit also sets `UMask=0077`, uses an unprivileged service
account, leaves the credential directory read-only, and has no `Environment=` or `EnvironmentFile=`
credential fallback.

## Resource and hardening posture

The template declares `Restart=on-failure`, 30/45-second start/stop bounds, `LimitNOFILE=65536`,
`MemoryMax=768M`, `CPUQuota=200%`, and `TasksMax=512`. It drops capabilities and applies strict
filesystem, device, kernel, namespace, realtime, personality, and syscall restrictions. These are
intentional operating limits to observe during P12 Canary, not a claim that a server has already
been configured.

`scripts/check-p12-02-systemd-unit.rb` validates every locked directive. On Linux it additionally
runs `systemd-analyze verify`; on macOS it retains the same static invariant check and reports that
the Linux-only syntax verifier is unavailable.
