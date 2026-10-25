# ADR-0128: Frozen unified Usage query and export

- Status: Accepted
- Date: 2026-10-25
- Task / Contract: [#72](https://github.com/Ricardo121380/cpa-rust-gateway/issues/72), #58 T19/T20; [BC-MGMT-020](../contracts/BC-MGMT-020-frozen-usage-query-export.md)

## Context

Existing Usage groups, request history and billing pages have different grains, filters and
independent watermarks. Ledger retention can remove rows after a watermark is issued, and
materialization can append costs after Usage exists. Watermarks alone therefore cannot freeze a
multi-request export. ADR-0077/0079 remain the contracts for their existing endpoints;
ADR-0100/0103 continue to own Usage evidence and exact Attempt attribution.

## Decision

Add a bounded management snapshot over the existing Request/Attempt/Usage log and versioned ledger.
One read-only SQLite transaction selects complete matching request lineages and associated ledger
entries. Control reuses the materializer's lineage resolver and projects only explicitly permitted
fields; it never recomputes prices. The HTTP layer retains the finished immutable result, scoped to
the authenticated management actor. Every summary, detail, bucket and CSV/JSONL page reads that
same result. New inference, materialization, catalog changes or retention cannot alter it.

Details have one row per matching terminal Attempt, plus a Request-only row when no terminal
Attempt exists. A retry is one distinct Request and multiple Attempts; account filters select the
actual matching Attempts, not all costs of any matching Request. Request outcome and driver
Attempt outcome remain separate. Usage outcome uses the exact failed/cancelled Attempt, or the
final Request outcome for the last successful driver Attempt. Unknown termination stays unknown.

UTC bounds are inclusive and apply to Attempt end time; Request-only rows use observed start time.
Historical missing times cannot be assigned to a bounded window. Buckets align to the lower bound,
contain only observed rows, and do not imply that absent buckets are zero. Requests crossing buckets
are counted distinctly in each bucket and once in the global summary.

All six source token dimensions retain null/zero and provenance. Inclusive and exclusive input
accounting are not added into one precise number: mixed/unknown accounting makes the top-level
input total null/unknown while per-accounting subtotals remain available. Ledger cost remains the
original integer microunit value and catalog version, with exact/partial/unknown/unpriced status.
Missing materialization is pending if beyond the captured worker checkpoint, otherwise unavailable;
missing Usage is unknown (or pending without a Request terminal). None of these is zero consumption.

Creation allows a 366-day window, at most 1000 buckets, 20000 source events / 16 MiB source JSON,
and an 8 MiB serialized result. The process retains at most eight results / 32 MiB for ten minutes.
Exceeded bounds fail explicitly. Pages have at most 100 rows. Expiry, cancellation, process restart
and foreign-owner access yield 410; clients explicitly create a new batch. Page delivery never
advances stored progress: an interrupted page can be fetched again with its original cursor.

## Consequences

No new schema, ledger, background export service or external queue is needed. The control layer
uses the existing workspace `serde` primitive for typed query/read-model serialization; its exact
dependency-boundary list records this direct edge. No transport or Provider edge is added. Retained snapshots
are disposable data products, not a backup. Process restart invalidates handles. Size limits are
an implementation bound, not large-scale performance qualification or a retention policy for the
source data. Existing admin permissions are global operator permissions; Client Keys cannot read
management snapshots. Exact Client Key/Access Group filters and cursor binding are backend-owned.

## Alternatives considered

- Re-query with just maximum ordinals: rejected because later deletion/materialization changes facts.
- Persist export jobs or duplicate ledger rows: unnecessary for this bounded local product.
- Keep SQLite transactions open across browser requests: rejected because client latency would pin
  WAL/read resources; a short capture transaction plus finite immutable result is easier to bound.
- Modify old endpoints' grains in place: rejected to preserve existing consumers and contracts.

## Validation and rollback

Real loopback management HTTP verifies full-filter totals against all pages and exports, including
retries, failed/cancelled Usage, missing prices, known zero, estimated evidence, scope isolation,
concurrent event append and ledger purge. Production gateway plus controlled TLS upstream validates
serving events and management export together. Tests and evidence are listed in the contract/report.
Rollback removes the new routes and projections; no persisted data or price version changes.
ADR 0128 and contract 020 are selected against common baseline 444558a8; integration with parallel
#68/#74 branches must check numbering. This decision grants no merge, deployment or Provider budget.
