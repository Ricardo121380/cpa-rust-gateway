# P12-02 deployment envelope acceptance

| Field | Value |
|---|---|
| Plan version | `v1.47` |
| Task | `P12-02` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Branch | `codex/p12-deployment` |
| Scope | Local implementation and verification of the Linux systemd deployment envelope only. No server, provider, DNS, proxy, Caddy, Cloudflare, registry, or release state was changed. |

## Accepted boundary

`gateway serve` is an explicit, two-listener process composition.  Both addresses must be
different, non-zero loopback socket addresses.  The data listener exposes only `HEAD /` and
`GET /healthz`; it cannot serve `/admin` or construct an inference runtime.  The management
listener alone mounts the existing P10 API/UI behind its separate Management Key, loopback peer
policy, browser-origin policy, and CSRF protection.

The process receives exactly five named files from systemd's `%d` credential directory.  Direct
regular files are required; missing, special, symbolic-link, malformed, oversized, or wrong-size
material fails closed without a path or value in the error.  Raw secret buffers are zeroized after
conversion to the typed P10/P2 owners.  State, restore, and backup locations remain fixed below
the declared StateDirectory.

[`deploy/systemd/cpa-rust-gateway.service`](../../deploy/systemd/cpa-rust-gateway.service) runs as
the unprivileged `cpa-gateway` account and fixes `LoadCredential`, State/Runtime/Logs directories,
`UMask=0077`, restart bounds, resource limits, journald output, capability drop, filesystem,
device, kernel, namespace, realtime, process, personality, and syscall hardening.  It is a
template only: it has not been installed, enabled, or started on a host.

## Verification evidence

| Check | Result |
|---|---|
| Focused `cargo test --locked -p gateway-http-actix -p gateway` | PASS — command parsing, credential-file refusal, P10 composition, and HTTP regressions. |
| Focused Clippy with `-D warnings` | PASS. |
| `./scripts/check-p12-02-systemd-unit.rb` | PASS — all fixed directives and no `Environment=`/`EnvironmentFile=` fallback; macOS correctly reported static-only verification. |
| `./scripts/test-p12-02-serve.sh` | PASS — ephemeral loopback health, no management route on data listener, authenticated management resource/backup/UI paths. |
| `CHECK_REPORT_PATH=docs/reports/evidence/p12-02-local-full-gate-20260725.md ./scripts/check.sh full` | PASS — 114 seconds; shell/CI/plan guards, SPA, format, workspace Clippy/tests, serve envelope, source/secret/crate/doc checks, dependency policy, and RustSec audit. |

The Full-gate receipt is retained at
[`p12-02-local-full-gate-20260725.md`](evidence/p12-02-local-full-gate-20260725.md).  `cargo deny`
still reports the previously documented non-fatal duplicate-version warnings for `getrandom`,
`http`, `socket2`, and `syn`; advisories, bans, licenses, and sources passed.

## Independent review

The post-gate review inspected the changed binary composition, Actix route registration, all three
P10 route composers, the credential reader, Unit/verifier/harness, crate-boundary policy, plan and
operator documentation.  It found no release-blocking issue.

In particular, the review confirmed that the three P10 route families are combined under one
protected `/admin` scope.  This avoids Actix's sibling-scope resolution ambiguity while retaining
the existing management middleware for every management path.  The expanded `gateway` dependency
allowlist is limited to this binary-only deployment root; library-layer isolation is unchanged.

## Remaining delivery boundary

P12-02 is locally accepted but not a server deployment.  A Linux P12 Delivery Gate must run
`systemd-analyze verify` against this Unit before P12 can complete.  P12-03 is the next task and
owns the timestamped server backup and rollback inventory; its execution must precede any staging
installation or server configuration change.
