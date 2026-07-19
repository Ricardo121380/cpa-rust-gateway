# P3-06 Attempt Orchestrator report

| Field | Value |
|---|---|
| Plan | `v1.1` |
| Task | `P3-06` |
| Matrix / behavior | `A22`, `E11`, `E12`, `E15`, `E16`, `G21`, `K03-K06`, `L20-L26`, `L30`; Behavior 1/6/17/20; BL-05 |
| Date | `2026-07-20` |
| Branch | `codex/p3-06-attempt-orchestrator` |
| Rust | `1.97.1` |
| Result | PASS locally and for the implementation commit in GitHub Fast/Full; verification-record acceptance pending |

## Delivered scope

- Added `gateway-router::AttemptOrchestrator`, a request-scoped asynchronous loop over the
  immutable Route's `max_attempts` and cumulative `bootstrap_timeout_ms`. It makes no SQLite
  query, Snapshot publication, unbounded queue, wait-for-cooldown operation, HTTP client, Provider
  decoder, or event write.
- Added a non-secret `AttemptDriver` port and `StartedAttempt<T>` wrapper. A successful wrapper
  retains the selected `CredentialLease` through the caller's output lifetime; a failed, timed-out,
  or cancelled driver future releases its lease before any later selection.
- Added `AttemptExclusionSet` plus Candidate-and-Credential selection predicates. A retry excludes
  exactly the failed binding before pool CAS and therefore cannot re-acquire it, while a healthy
  sibling Credential remains eligible.
- Classified retryable failures without raw upstream diagnostics: connection, 429, 5xx, and
  pre-semantic truncation. A 429 cools only the Endpoint/Credential pair; the other retryable
  failures cool the Endpoint. Circuit recovery, account/quota/403 classification, and probes remain
  outside this task.
- Added the transport-neutral `gateway-core::TransparentRetryGate`, implemented by
  `gateway-stream::StreamControl`. It lets the orchestrator cancel and drop an in-flight driver
  future immediately, and preserves the transparent-retry boundary at actual downstream
  first-semantic-event delivery rather than decoding or queueing time.
- Added a stream-side first-semantic-event-or-cancellation wait primitive. A future bridge that has
  handed off its first canonical event can wait before reading later upstream output, preventing an
  unwithdrawable queued start from being followed by a transparent duplicate retry.
- Added [ADR-0016](../adr/ADR-0016-request-scoped-attempt-orchestration.md) and
  [BC-ROUTER-003](../contracts/BC-ROUTER-003-request-scoped-attempt-orchestration.md), plus a
  documented Tokio timeout allowance in the crate-boundary policy.

## Local verification evidence

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-core -p gateway-router -p gateway-stream` | PASS; 38 Core, 34 Router, and 14 Stream tests, including connection/429/5xx/truncation fallback, binding exclusion, budget exhaustion, FSE closure, cancellation, and lease release |
| `cargo clippy --locked -p gateway-core -p gateway-router -p gateway-stream --all-targets --all-features -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `ruby scripts/check-crate-boundaries.rb`, `ruby scripts/check-source-policy.rb`, and `ruby scripts/check-doc-links.rb` | PASS; 21 package boundaries, 55 Rust files, and 90 Markdown documents |
| `scripts/secret-scan.sh --staged` and `git diff --cached --check` | PASS; all implementation files were staged and scanned before commit `b4c797f` |
| `./scripts/check.sh fast` | PASS; complete workspace Fast gate |
| `./scripts/check.sh full` | PASS; complete workspace Full gate, including dependency policy and RustSec audit; existing duplicate-version notices are policy-allowed warnings |

## Review

Review passed. The hot path remains bounded and local: route selection retains its existing atomic
cursor behavior, pool eligibility executes before CAS, transient availability reads one health
shard, the exclusion set is request-local, and the retry budget bounds each start. The change adds
no Repository, `SecretStore`, raw body, URL, Authorization value, or global scheduler lock to the
request path. Public errors and `Debug` surfaces retain only stable safe categories or counts.

The review confirmed the intended failure scopes: 429 does not block the Endpoint or its healthy
sibling Credential; connection/5xx/truncation create an Endpoint Cooldown; Circuit state is neither
opened nor automatically recovered; cancellation itself does not mutate health. It also confirmed
that a success owns its live lease, while a failed/cancelled Attempt drops it before fallback.

Review found one cancellation-liveness gap in the first implementation draft: checking cancellation
only between attempts could leave a driver future running. The final implementation moves the gate
to `gateway-core`, makes `StreamControl` implement it, and uses a biased cancellation select to
drop the in-flight future. A regression test proves that this produces `Cancelled/Request` without
starting a retry. The review additionally added the downstream delivery wait to prevent a queued
first event from racing with a later retry decision.

## Scope and deferred work

P3-06 does not decode OpenAI Responses bodies or SSE, construct a real `UpstreamClientPool`
request, emit P3-08 Attempt/Usage events, write SQLite, publish `/v1/models`, re-write model names,
perform actual mock HTTP E2E, contact a deployed Endpoint, add 401/403/Quota policy, wait for
Cooldown expiry, persist health, open/recover Circuits, run probes, or start P3-07. All tests use
synthetic IDs, Credentials, and outputs only.

## GitHub CI

GitHub Actions run [29706009714](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29706009714)
passed for implementation commit `b4c797f`.

| Job | Result |
|---|---|
| Fast gate | PASS; completed `2026-07-19T22:24:34Z` |
| Full supply-chain gate | PASS; completed `2026-07-19T22:35:17Z` |

This completes P3-06 implementation acceptance. The separate verification-record commit must pass
the same two jobs before the final status record can be created; that final record must also pass
before P3-07 can begin.
