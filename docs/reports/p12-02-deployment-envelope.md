# P12-02 deployment envelope acceptance

| Field | Value |
|---|---|
| Plan version | `v1.48` |
| Task | `P12-02` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Branch | `codex/p12-deployment` |
| Scope | Deployment-envelope implementation, local verification, and one temporary real-Linux syntax validation. No Unit was installed, enabled, or started; no provider, DNS, proxy, Caddy, Cloudflare, registry, or incumbent-service state changed. |

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
| Repair focused checks | PASS — the executable unit checker, its new unsupported-condition regression fixture, and the focused `gateway serve` harness all passed. |
| Real-Linux `systemd-analyze verify` | PASS — systemd `255` validated the exact repaired Unit SHA-256 `f40cf0e55116360fe8372240131f4fa69aea5e6692f0ec64a21a9d859397063d`; a temporary `/bin/true` substitute existed only to satisfy the not-yet-staged executable path and was removed after the check. |
| `CHECK_REPORT_PATH=docs/reports/evidence/p12-02-linux-unit-repair-full-gate-20260725.md ./scripts/check.sh full` | PASS — 319 seconds; all local gate steps passed. The known non-fatal duplicate dependency warnings remain limited to `getrandom`, `http`, `socket2`, and `syn`; RustSec advisories passed. |

The Full-gate receipt is retained at
[`p12-02-local-full-gate-20260725.md`](evidence/p12-02-local-full-gate-20260725.md).  `cargo deny`
still reports the previously documented non-fatal duplicate-version warnings for `getrandom`,
`http`, `socket2`, and `syn`; advisories, bans, licenses, and sources passed.
The post-repair Full and documentation receipts are retained at
[`p12-02-linux-unit-repair-full-gate-20260725.md`](evidence/p12-02-linux-unit-repair-full-gate-20260725.md)
and
[`p12-02-linux-unit-repair-docs-gate-20260725.md`](evidence/p12-02-linux-unit-repair-docs-gate-20260725.md).

## Linux repair and independent review

The first real-Linux preflight exposed an incorrect assumption in the original template:
`ConditionPathIsExecutable=` is not a valid systemd `255` directive.  The invalid candidate was
never installed.  A first empty remote temporary-file attempt was deleted and is not evidence.
The retained validation instead transferred the exact local Unit bytes, confirmed the matching
SHA-256 shown above, and ran `systemd-analyze verify` with only a disposable executable-path
substitute.  It did not create an account, credential, listener, persistent Unit, or service.

The repair changes the condition to the supported `ConditionPathExists=`, makes the checker
executable because `check.sh` invokes it directly, and adds a regression fixture that rejects the
old directive.  No signed-binary input changed relative to signed artifact revision
`111f60a416fd0a6b4a6314bac8ff32b0074cdca7`: `apps/gateway`, `Cargo.lock`, `Dockerfile`, and the
release workflow are unchanged.  The independently verified binary artifact therefore remains
eligible; P12-04 must separately record the repaired Unit SHA before installation.

The post-repair review inspected the Unit directive change, direct execution mode, negative test,
Linux validation procedure, plan states, and receipts.  It found no release-blocking issue.

## Earlier independent review

The post-gate review inspected the changed binary composition, Actix route registration, all three
P10 route composers, the credential reader, Unit/verifier/harness, crate-boundary policy, plan and
operator documentation.  It found no release-blocking issue.

In particular, the review confirmed that the three P10 route families are combined under one
protected `/admin` scope.  This avoids Actix's sibling-scope resolution ambiguity while retaining
the existing management middleware for every management path.  The expanded `gateway` dependency
allowlist is limited to this binary-only deployment root; library-layer isolation is unchanged.

## Remaining delivery boundary

P12-02 is locally accepted but not a server deployment.  The real-Linux syntax branch has passed;
P12-04 must repeat `systemd-analyze verify` against the exact repaired Unit after its verified
binary path exists and before that Unit is installed.  P12-03's timestamped backup is already
accepted.  Neither result authorizes provider traffic, a public listener, or any incumbent change.
