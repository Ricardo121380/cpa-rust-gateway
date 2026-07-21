# P4-02 CatalogSnapshot freshness and last-success fallback

| Field | Value |
|---|---|
| Plan version | `v1.3` |
| Task | `P4-02` |
| Status | `LOCAL_PASS_PENDING_CI`; local review passed, GitHub Code Gate pending |
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
lookup emitted one transient timeout message, so the remote Code Gate remains the authoritative
networked supply-chain evidence.

## Review

Review found and corrected one Clippy naming violation before the full gate. The final scope check
confirms that snapshots are exact-target keyed, successful empty lists do not imply removal, failure
does not mutate retained data, all state calculations receive explicit time, and no P4-03, network,
SQLite, RouteSnapshot, health, quota, or public API work was included.

## Remote code Gate and delivery-flow measurement

P4-02 remains on the cache-visible `codex/p4-01-catalog-singleflight` delivery ref. Its normal
GitHub code Gate, not a manual rerun, is the measurement for cache hit/miss summary, warm tool
installation, Fast plus supplemental supply-chain Full, and end-to-end workflow duration.

## Closeout boundary

After the code Gate passes, exactly one docs-only closeout will record its immutable evidence and
mark P4-02 `DONE`. P4-03 remains `PENDING` throughout this Task.
