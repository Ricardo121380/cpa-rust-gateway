# G11 Release-hardening gate report

| Field | Value |
|---|---|
| Plan version | `v1.45` |
| Gate | `G11` — P11 release hardening |
| Local result | `PASS` — all required P11 task evidence, focused reviews and local verification are complete. |
| Delivery result | `PASS` — [`phase-p11-remediated-3-complete`](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/30095341123) accepted closeout target `074beb9`: Classify, Fast, Full supply-chain and Required all passed; Docs-only correctly skipped for the code scope. |
| Scope | P11-01 through P11-08 on `codex/p11-release-hardening`; loopback, offline, local artifact and source-review evidence only unless a task report expressly says otherwise. |

## Gate conditions

| G11 condition | Evidence | Local result |
|---|---|---|
| Every compatibility difference is classified | [P11-01](p11-01-differential-fixture-harness.md) supplies six source-labelled, redacted and default-deny Fixtures; intentional/compatible/regression status remains explicit. [P11-08](p11-08-release-candidate.md) carries deployment/external differences into the candidate ledger. | PASS |
| No unclassified panic, race, stream truncation or Secret leak | [P11-02](p11-02-fault-matrix.md) owns fault/truncation/slow-client/cancellation classification; [P11-05](p11-05-security-audit.md) passes SSRF, Secret, auth/access and supply-chain audit; P11-06 proves local drain/recovery behavior. Workspace lints deny unsafe code, `panic!`, `unwrap` and `expect`. | PASS |
| Benchmark regression limits hold | [P11-03](p11-03-benchmark-baseline.md) stores an approved offline baseline with fail-closed P99/RSS/throughput comparator. Its measured local HTTP P99 is 12.276 µs, below the 5 ms local warm-path limit; server 10 ms evidence remains P12 scope. | PASS — local only |
| Long-run resource/connection/SQLite behavior is bounded | [P11-04](p11-04-load-soak.md) records the accepted 10h13m synthetic loopback observation: 72,716 finite streams, no sustained RSS growth and no SQLite corruption. Its receipt truthfully remains `INCOMPLETE` after the user stop under `CR-P11-04-001`; P12 keeps the real 72h Canary obligation. | PASS — accepted local evidence |
| Recovery and rollback are rehearsed | [P11-06](p11-06-recovery-report.md) covers graceful drain, abort/replay, deterministic `SQLITE_FULL` recovery and queue degradation. [P11-07](p11-07-upgrade-rollback.md) covers in-place upgrade, lossy old-schema downgrade, encrypted empty-target recovery and no guessed target version. | PASS |
| Candidate has safe defaults and explicit handoff | [P11-08](p11-08-release-candidate.md) records source-level listener/auth/management/egress/event/backup defaults, candidate identity, deferred external authentication and P12 ownership. It creates no artifact, tag, image, listener or server change. | PASS |

## Local verification and review

| Evidence | Result |
|---|---|
| P11-05 Full local gate | PASS — workspace format/Clippy/tests, policies, documentation links, Secret scan and Rust supply-chain checks. |
| P11-06 Full local gate | PASS — 214 seconds; all workspace quality/security checks pass after recovery drills. |
| P11-07 Full local gate | PASS — 213 seconds; all workspace quality/security checks pass after upgrade/rollback rehearsal. |
| P11-08 docs-only closeout | PASS — plan state/guard, 315 Markdown files, tracked Secret scan and whitespace checks. |
| P11 Delivery remediation | PASS — `7bb3318` replaces the fixed one-second smoke wait with a bounded receipt-`RUNNING` poll, explicit early-exit/timeout diagnostics and cleanup; `68768fc` retains the final 120 Cargo-log lines, which exposed the distinct CI dependency-order defect. `c5cf847` installs SPA dependencies before every Cargo gate. Focused regression, cold-cache local smoke, ordinary Fast, and a fresh no-`node_modules` worktree Fast passed; the final remote Fast, Full supply-chain and Required checks passed on `074beb9`. |
| Focused phase review | PASS — reports distinguish local source/loopback evidence from deployment proof, retain P7/P8 external deferrals, do not reinterpret the stopped soak as complete, and do not turn candidate notes into a published release. |

## Accepted limits and P12 handoff

- P7 Kiro and P8 Official real external authentication remain deferred and are not presented as
  successful upstream-account evidence.
- This gate does not build/sign a release artifact, bind a listener, deploy a server, create
  Caddy/Cloudflare/systemd configuration, change a credential, or run a Provider probe.
- P12 must create its own immutable artifact identity, run its staging/Canary/rollback workflow,
  and retain the 72-hour real observation required by P12-10.

## Decision

G11 is accepted. The historical `phase-p11-complete` and first two remediation runs remain
failure evidence. Their pre-receipt `101` was ultimately traced to a CI ordering defect:
`gateway-http-actix`'s build script embeds the management SPA, but Fast ran the soak Cargo
invocation before `npm ci` created `web/admin-ui/node_modules`. The reviewed correction runs
dependency installation before every Cargo gate; its fresh no-`node_modules` worktree proves the
cold path locally while retaining the existing `TERM`/`130`/`INCOMPLETE` assertion. The final
remote Fast, Full supply-chain and Required checks passed, so P11-01 through P11-08 are `DONE` and
P12-01 may begin. The Full gate's fixed-tool install took 603 seconds, a cache-miss performance
fact rather than a quality exception; supply-chain policy and RustSec audit still passed.
