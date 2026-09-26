# CR: explicit incomplete historical usage and immutable conflict quarantine

Status: accepted, implemented and released as signed `b154396` on2026-09-26 18:08 (Asia/Shanghai).
Formal gates, signed artifact, isolated production-copy rollback and production API verification
passed. [Delivery evidence](../reports/cpar-usage-quarantine-20260926.md).

User decision (2026-09-26): retain and isolate the 11 ambiguous historical Usage events, restore
queries with an explicit incomplete-total notice. These events span four reused request IDs;
no reliable per-invocation evidence exists. Do not invent lineage, delete events or alter ledger rows.

## Contract

- `GET /admin/operations/usage?allow_partial=true`: explicit opt-in to excluding conflicting
  Usage payload groups. Omitted/false remains strict. Missing Request/Attempt, damaged payloads
  and non-conflicting failed lineage still fail closed.
- Response adds required `excluded_usage_events` and `excluded_request_groups` (nonnegative).
  Both describe the **whole filtered snapshot before the page cursor**, not page-local counts.
  The watermark describes accepted observations only. A conflicts-only result has empty items,
  a null watermark and nonzero exclusion counts; it is not zero consumption.
- `GET /admin/operations/billing-processing` adds nullable `quarantined_failures`, a subset of
  `unresolved_failures`. The state remains `needs_repair`; quarantined does not mean repaired.
- Retry eligibility derives from existing `invalid_lineage` failures joined to immutable conflicting
  Usage payloads. No schema migration, event rewrite, resolved marking, ledger backfill or deletion.
  Incomplete lineages awaiting late events continue normal bounded retry.

Prism opts into partial reads, preserves the snapshot counts once across pages, labels verified
subtotals and places the incomplete-history notice beside Token totals. Processing details distinguish
unposted, quarantined and retryable events. Existing six-family observation confidence is unchanged.

Authority: `docs/openapi/management-v1.json`; vendored contract/client generated with sync-contract.
Strict-default compatibility preserves older callers' fail-closed behavior. Same-schema rollback
retains all data, restores strict all-history failure and the former conflict retries.
