# BC-MGMT-018: Bounded account quota history

| Field | Value |
|---|---|
| Issue | [#74](https://github.com/Ricardo121380/cpa-rust-gateway/issues/74), OPT-07 |
| ADR | [ADR-0128](../adr/ADR-0128-account-quota-history.md) |
| API | `GET /admin/quota-history` |

## Required behavior

1. Existing ordinary/native account reads and existing Kimi background metadata persist sanitized
   samples. Cache hits are not new observations. Source, channel, account namespace, configuration
   scope, revision, window, observation time, confidence and completeness remain distinguishable.
2. Transactional fences reject late results after replacement, lifecycle/revision changes or deletion.
   Same-time equal samples merge, same-time conflicts roll back, and older samples within the same
   revision cannot replace newer evidence. Historical records survive account deletion.
3. `observed`, `exhausted`, `unknown`, `failed` statuses are separate from `partial` completeness and
   `stale` freshness. Unknown values are null, not zero. `projected` is not whole-account completeness.
4. Time is `[from_ms,to_ms)` with a maximum 31-day span. `limit` is required and 1–200;
   `after_id` defaults to zero. Optional `account_kind`, `config_version_id`, `account_id`, `provider`,
   `window` are exact filters. Native config scope is empty. A window filter includes windowless
   failures. IDs order append history; page through with unchanged filters. New appends can appear
   on later pages; this is not #72's frozen export. Invalid bounds/unknown query keys return 400.
5. The protected response is `no-store`; bad management auth remains 404 under the existing policy.
   Missing store or failed persistence/query returns 503. Native concurrent mutation returns 409;
   ordinary metadata reports its existing `configuration_changed` or `unavailable` result.
6. `gap_before` is an uncovered interval relative to the preceding same-revision/source/window
   sample and requested start. Each `valid_until_ms` marks a local freshness ceiling, capped by
   explicit reset; time after it remains unknown even when no next row exists. Empty results mean
   no recorded observation, not zero quota. `reported_reset_changed` means two different explicit
   provider reset timestamps, not a witnessed reset; Kiro positional keys do not assert this event.
7. No difference across windows becomes actual consumption. No balance combines quota, Usage and
   cost. No new polling, inference, enable/recovery or historical deletion is introduced.
8. Only allowlisted projections persist; identity and credentials never enter history. Build product
   text is hashed; reset text is parsed or dropped; unknown units remain null. History has no serving
   authority and does not populate Request, Attempt, Usage or ledger rows.

## Corresponding tests

- `quota_history_http_filters_gaps_resets_failures_and_duplicates`
- `quota_history_ordinary_revision_fence_unknown_and_reopen`
- `quota_history_kimi_partial_projection_and_source_isolation`
- `quota_history_missing_store_fails_before_native_transport`
- `native_quota_rejects_late_response_after_account_changed`
- `migrations_are_idempotent_and_create_all_known_schema_tables`

## Actual process boundary

`scripts/acceptance/cpar74-quota-history.py --binary "$CARGO_TARGET_DIR/debug/gateway" --output output/issue-74/process-run-N`
uses synthetic accounts and a controlled local TLS quota endpoint. It verifies cross-account/window
history, explicit reset changes, expiry/failure/partial state, concurrent disable/delete, actual
process exit/restart/readback and zero Usage/ledger fabrication. Its network fence and receiver
counts are part of the receipt, not a real-Provider or deployed-service qualification.
