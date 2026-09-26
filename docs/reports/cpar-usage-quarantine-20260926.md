# Historical usage quarantine repair — 2026-09-26

User-approved outcome: retain and quarantine the 11 ambiguous historical Usage events; restore
all-history queries with an explicit incomplete-total warning. This repairs query availability and
retry behavior; it does not reconstruct the missing per-invocation lineage or complete the ledger.

## Diagnosis and implementation

The frozen production copy has four reused legacy request IDs: one group contains five distinct
Usage responses, three contain two each. Every group has only one Request and one Attempt;
Usage timestamps and RequestFinished events are absent. No unique call-to-usage assignment can
be recovered. [Earlier investigation](cpar-legacy-billing-lineage-20260926.md).

[Contract decision](../change-requests/CR-20260926-usage-quarantine.md): explicit `allow_partial=true`
returns verifiable groups with snapshot-wide excluded-event/group counts. Strict callers still
fail closed. Prism shows exclusions and verified subtotals, including a conflicts-only empty state.
The materializer keeps failure records but excludes immutable conflicting groups from retries;
late incomplete events remain retryable. Schema28 and original ledger/event rows are unchanged.

## Verification progress

- Local: scoped store regression, durable materializer replay/late-event/quarantine regressions,
  HTTP opt-in validation, frontend type checking, 407 frontend unit tests, authoritative contract
  synchronization and deterministic four-asset embedded SPA gate.
- Local real `serve` plus loopback mock: strict query500; partial query200 with one verified
  request,11 excluded events/four groups; conflicts-only query returns an explicit nonzero exclusion
  count with no invented zero usage. Original source is retained and automatic retry count stays zero.
  [API evidence](evidence/cpar-usage-quarantine-20260926/local-api.json).
- EgoLite with the real local gateway:1440×900,1280×720,390×844; partial totals,
  conflicts-only empty state and light/dark rendering checked. Mobile notice wrapping corrected
  after screenshot inspection. [Browser evidence](evidence/cpar-usage-quarantine-20260926/browser-local.json),
  [final mobile screenshot](evidence/cpar-usage-quarantine-20260926/usage-390-dark.png).
- Formal final-revision gate, signed binary, disconnected production-copy rollback rehearsal,
  production API and EgoLite acceptance: pending. No new production completion claim yet.

No new Provider inference, OAuth, catalog refresh, historical rewriting or deletion is part of this repair.
