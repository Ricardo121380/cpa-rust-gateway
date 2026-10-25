# Issue 72: unified Usage query and snapshot export

Date: 2026-10-25 (Asia/Shanghai). Branch: `codex/72-usage-query-export`.
Common baseline: `444558a8de930f5f1febd5a40ec5bdbbc30a15fb`.

Implementation: [contract](../contracts/BC-MGMT-020-frozen-usage-query-export.md),
[decision](../adr/ADR-0128-frozen-usage-query-export.md).

## Local implementation

The new protected management snapshot API freezes existing Request/Attempt/Usage facts and their
original versioned billing ledger entries in a single SQLite read transaction. It provides a
full-filter summary, bounded detail/bucket pages, and CSV/JSONL pages from that same immutable result.
There is no schema migration, alternate ledger, re-pricing, price refresh or quota-history change.
Old Usage/billing/history endpoints retain their contracts.

## Validation record

- Targeted real-loopback HTTP test: `usage_snapshot_http_freezes_totals_details_and_exports` passed.
- Production gateway with controlled loopback TLS upstream: 22 checks passed, including retries,
  failed/cancelled usage, missing prices, concurrent writes, cross-Key scope and restart invalidation.
- Mixed input-accounting and overflow, expiry/owner scope, CSV safety and OpenAPI tests are included
  in the workspace gate. Full-gate and two-axis review results will be recorded after completion.
- Original failed iterations are retained under `output/issue-72/logs`: enum typo, lint findings,
  and a synthetic fixture that incorrectly combined inclusive/exclusive cache fields. The fixture
  was corrected; the existing billing admission rules and expected monetary assertions were kept.

All databases, generated local test credentials and logs belong to this worktree's ignored
`output/issue-72`. Compilation uses its own `target`; loopback listeners use dynamic ports.
Only sanitized receipts are intended for reporting. No real Provider, production database,
actual external client, deployment or remote CI qualification is claimed.

## Shared integration surface

- Additive `/admin/operations/usage-snapshots` and ID-scoped metadata/rows/export/delete operations.
- `ManagementUsageFacade::create_usage_snapshot` has a default unavailable implementation; the
  gateway composition supplies the real read-only implementation.
- New store capture and control read-model modules; the existing billing lineage resolver is reused.
- OpenAPI schemas/paths, management route registration and HTTP state, `apps/gateway/src/deployment.rs`,
  ADR index, traceability, roadmap, and the existing synthetic acceptance harness are touched.
- ADR-0128 and BC-MGMT-020 are chosen against the common baseline. Parallel #68/#74 integration must
  check numbering and shared files; it has not been performed here. Migration/schema version unchanged.
- `gateway-control` directly declares the already-workspace-owned `serde`; no dependency version changes.

## Operational limits

Capture is limited to 20000 source events /16 MiB source JSON and an 8 MiB serialized result.
Each process retains at most eight snapshots /32 MiB for ten minutes. Pages have 1–100 rows;
limits return explicit errors. Snapshot expiry, cancellation or process restart returns 410.
These are implementation bounds, not measured large-scale capacity. Exports are readable products,
not encrypted backups or a complete control-plane recovery format.
