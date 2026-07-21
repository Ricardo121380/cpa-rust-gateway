# P4-04 Target-local Probe, EWMA, and Circuit recovery

| Field | Value |
|---|---|
| Plan version | `v1.4` |
| Task | `P4-04` |
| Status | `DONE` after this docs-only closeout Gate; Code Gate passed |
| Scope level / execution budget | `M`; `<=25min` from Task Card to code commit |
| Task Card | `gateway-router` runtime health/probe boundary only; no real Provider request, Quota, Route Explain, SQLite, public API, P4-05+ behavior |
| References | `E08`, `E09`, `E11`, `E12`, `D20`, `D24`, `G20`, `H19`, `L30`; [ADR-0026](../adr/ADR-0026-target-local-probe-ewma-and-circuit-recovery.md); [BC-HEALTH-002](../contracts/BC-HEALTH-002-target-local-probe-ewma-and-circuit-recovery.md) |

## Scope

P4-04 adds transport-neutral, explicit-time health probe outcomes for exact Endpoint,
Endpoint/Credential, and Endpoint/Credential/model targets. It stores bounded fixed-point success
and latency EWMA, adds a model-specific runtime Circuit key to pre-lease scheduling, and turns an
expired Circuit retry instant into one controlled half-open ticket rather than ordinary traffic.

No request is sent: a later authorized executor may supply sanitized terminal results. This Task
does not classify quota, 403, or 429; persist health events; expose management HTTP; render Route
Explain; export telemetry; or retain URLs, Headers, bodies, Provider diagnostics, or Secret values.

## Local verification

| Command / review | Result |
|---|---|
| `cargo test --locked -p gateway-router` | PASS; 43 tests, including exact model-target EWMA isolation, time-regression rejection, one half-open ticket, stale-ticket rejection, success close, failure reopen, and healthy-sibling pre-lease selection. |
| `cargo clippy --locked -p gateway-router --all-targets --all-features -- -D warnings` | PASS after one direct review-correction batch. |
| `CHECK_REPORT_PATH=tmp/p4-04-full-check.md ./scripts/check.sh full` | PASS in 42 seconds (started `2026-07-21T15:55:54+08:00`, completed `2026-07-21T15:56:36+08:00`); it covers Fast, formatting, workspace Clippy/tests, links, Secret scan, whitespace, pinned tools, `cargo deny`, and RustSec audit. |

The Full report contains existing duplicate-version notices from `cargo deny`; `cargo audit` completed
successfully. No ignored real-test harness was invoked and no Provider request was sent.

## Review and execution measurement

Review confirmed that model state is exact `(EndpointId, CredentialId, upstream_model)` rather than
global/model-only; ordinary scheduling remains closed throughout half-open recovery; a stale ticket
cannot overwrite a newer Circuit; and EWMA uses explicit time plus integer arithmetic. Probe and
runtime state remain bounded and sharded. The new model predicate runs before a pool reserves a
Credential lease. No RouteSnapshot/public-model mutation, HTTP/Provider access, SQLite, quota,
management endpoint, persistent event, telemetry, body, Header, URL, or Secret enters the scope.

| Measurement | Evidence / value |
|---|---|
| Scope / budget | `M`; target `<=25min` from Task Card to code commit. |
| Task Card | `2026-07-21T15:32:40+08:00` durable plan-state anchor; the visible Task Card preceded focused code reading. |
| Local complete Gate | `2026-07-21T15:55:54+08:00` to `2026-07-21T15:56:36+08:00` (42s). |
| Repeated complete Gates | `0`; the one required complete Gate was not mechanically replayed. |
| Rework | `1` direct correction batch: focused Clippy feedback merged equivalent match arms, removed an unused test local, and simplified private helper shape without changing behavior. |
| Budget observation | The code-commit target will exceed the `M` budget by a small margin because the new model key, ticket generation, atomic completion boundary, scheduler integration, and required no-network contract were reviewed together; the next Task should reuse this health-report template rather than rediscover the boundary. |
| Code commit | `0cee513`, `2026-07-21T16:07:23+08:00`; the local complete Gate preceded the code delivery. |
| Code Gate passed | `2026-07-21T16:12:33+08:00`; [run 29813113764](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29813113764). |
| Docs closeout / docs Gate | This one docs-only closeout records immutable Code Gate evidence. Its required docs-only Gate is external evidence and will not cause a second status commit. |

## Accepted GitHub Code Gate and delivery-flow measurement

GitHub Actions [run 29813113764](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29813113764)
passed for implementation commit `0cee513` on the cache-visible
`codex/p4-01-catalog-singleflight` delivery ref. It was the normal push Gate, not a manual rerun.

| Job / step | Result and duration |
|---|---|
| Classify delivery gate | PASS; selected `code`; Docs-only Gate correctly skipped. |
| Fast gate | PASS; job about 170 seconds, complete `Run fast gate` about 143 seconds. |
| Full supply-chain gate | PASS; job about 57 seconds after Fast. |
| Cache | PASS; primary key hit; restore took about 6 seconds. |
| Install pinned quality tools | PASS; version-verified `cargo-deny` and `cargo-audit` completed in about 1 second, within the `<=10s` operational target and `<=90s` hard ceiling. |
| Supplemental supply-chain | PASS; version verification, `cargo deny check`, and RustSec audit completed in about 8 seconds without replaying Workspace Fast checks. |
| Required delivery gate | PASS; fail-closed verification of the code path's Fast + Full results. |

The workflow first started at `2026-07-21T16:08:24+08:00` and completed at
`2026-07-21T16:12:33+08:00` (about 4 minutes 9 seconds). The warm `<=4min` target missed by about
9 seconds, but the cache hit and all required Gates passed. This is a delivery-performance
observation rather than a correctness or supply-chain failure; no manual rerun is issued.

## Closeout boundary

This is the single docs-only closeout that records the immutable Code Gate evidence and marks P4-04
`DONE`. Its own GitHub status is external evidence and will not cause another status-only commit.
