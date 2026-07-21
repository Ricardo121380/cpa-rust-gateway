# P4-02 CatalogSnapshot freshness and last-success fallback

| Field | Value |
|---|---|
| Plan version | `v1.3` |
| Task | `P4-02` |
| Status | `DONE` after this docs-only closeout Gate; Code Gate passed |
| Date | `2026-07-21` |
| References | `E20`, `G28`, `L09`, `L10`, `L33`; [ADR-0024](../adr/ADR-0024-catalog-snapshot-freshness-and-last-success-fallback.md); [BC-CATALOG-002](../contracts/BC-CATALOG-002-catalog-snapshot-freshness-and-last-success-fallback.md); [ADR-0023](../adr/ADR-0023-cache-visible-delivery-and-supply-chain-split.md); [BC-DELIVERY-002](../contracts/BC-DELIVERY-002-cache-visible-delivery-and-supply-chain-split.md) |

## Scope

P4-02 adds a process-local immutable `CatalogSnapshot` store keyed strictly by Endpoint plus
Credential. It makes Fresh/Stale/Expired and the independent refresh-due deadline explicit with
deterministic Unix-millisecond inputs, preserves last success across a failed discovery, and
rejects unsafe timestamps without replacing retained state.

This Task does not implement discovery transport, SQLite persistence, HTTP/public model publication,
RouteSnapshot integration, health/quota mutation, or P4-03 diff/removal semantics. All tests are
synthetic and make no external request.

## Local verification

| Command / review | Result |
|---|---|
| `cargo test --locked -p gateway-catalog` | PASS; 11 unit tests, including explicit time boundaries, failure retention, Credential isolation, empty-success, and unsafe-time rejection. |
| `cargo clippy --locked -p gateway-catalog --all-targets --all-features -- -D warnings` | PASS after one local review correction to private policy-field names. |
| `ruby scripts/check-ci-workflow.rb`, `ruby scripts/check-plan-state.rb`, `ruby scripts/check-doc-links.rb` | PASS; pinned workflow structure, one active Task rule, and 115 Markdown links validate. |
| `CHECK_REPORT_PATH=tmp/p4-02-supply-chain-check.md ./scripts/check.sh supply-chain` | PASS; pinned versions, `cargo deny check`, and `cargo audit` completed in 37 seconds. |
| `CHECK_REPORT_PATH=tmp/p4-02-full-check.md ./scripts/check.sh full` | PASS in 43 seconds; it includes all Fast checks plus supply-chain checks, so no redundant local `fast` run was needed. |

The local full report records existing non-blocking duplicate-version notices from `cargo deny`.
`cargo audit` completed with exit status zero after scanning the lockfile; its yanked-package metadata
lookup emitted one transient timeout message. The remote Code Gate below completed its networked
supply-chain evidence without that timeout.

## Review

Review found and corrected one Clippy naming violation before the full gate. The final scope check
confirms that snapshots are exact-target keyed, successful empty lists do not imply removal, failure
does not mutate retained data, all state calculations receive explicit time, and no P4-03, network,
SQLite, RouteSnapshot, health, quota, or public API work was included.

## Accepted GitHub Code Gate and delivery-flow measurement

GitHub Actions [run 29806392391](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/29806392391)
passed for code commit `e03c192` on the cache-visible `codex/p4-01-catalog-singleflight` delivery
ref. It was the normal push Gate, not a manual warm rerun.

| Job / step | Result and duration |
|---|---|
| Classify delivery gate | PASS; selected `code`; Docs-only gate correctly skipped. |
| Fast gate | PASS; job about 177 seconds, complete `Run fast gate` about 153 seconds. |
| Full supply-chain gate | PASS; job about 42 seconds after Fast. |
| Cache | PASS; cache key hit, restored in about 6 seconds. The job summary records `Cache hit: true`. |
| Install pinned quality tools | PASS; version verification only, about 1 second; meets both the `<=10s` operational target and `<=90s` hard ceiling. |
| Supplemental supply-chain | PASS; version check, `cargo deny check`, and `cargo audit` completed in about 6 seconds without replaying Workspace Fast checks. |
| Required delivery gate | PASS; fail-closed verification of the code path's Fast + Full results. |

From first job start to Required completion the workflow took about 4 minutes 01 second; created to
completed was about 4 minutes 05 seconds. There was no observed queue interval, so the operational
`<=4min` warm workflow target missed by about one second. This is a performance observation, not a
correctness failure: cache and Full-split objectives passed, and the plan requires investigation
only after two consecutive warm misses. The next normal code Task, not a manual rerun, is the
follow-up measurement.

## Closeout boundary

This is the single docs-only closeout that records the immutable Code Gate evidence and marks P4-02
`DONE`. Its own GitHub status is external evidence and will not cause another status-only commit.
P4-03 remains `PENDING` throughout this Task.
