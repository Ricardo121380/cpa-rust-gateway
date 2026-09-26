# Historical usage quarantine repair — 2026-09-26

User-approved outcome: retain and quarantine the 11 ambiguous historical Usage events; restore
all-history queries with an explicit incomplete-total warning. This repairs query availability and
retry behavior; it does not reconstruct the missing per-invocation lineage or complete the ledger.

Released signed revision `b15439636e3c3c0467d7735f480f4ea7b7cfdba7` at
2026-09-26 18:08 (Asia/Shanghai). Production all-history partial reads now return200 with699
verified usage-bearing requests and11 excluded events/four groups. Recent24h/7d/30d queries
return28/30/491 with zero exclusions. The original2,356 events and699 ledger rows are retained.

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
- Final-revision [formal gate](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36233979445):
  fast and supply-chain checks passed;1,314 Rust tests passed,0 failed,9 existing ignored.
  [Receipt](evidence/cpar-usage-quarantine-20260926/formal-gate.json),
  [counts from this run](evidence/cpar-usage-quarantine-20260926/fast-test-counts.json).
- [Signed build](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/36233981864):
  ARM64 and x86_64 manifest, SBOM and Sigstore identity independently verified.
  [ARM64](evidence/cpar-usage-quarantine-20260926/verified-artifact.json),
  [x86_64](evidence/cpar-usage-quarantine-20260926/verified-x64.json).
  Exact ARM64 artifact passed12 agent round-trip checks and the request-to-ledger chain in
  an isolated network namespace against loopback TLS mocks. Intentional malformed/cancelled
  scenarios remained failures. [Artifact gate](evidence/cpar-usage-quarantine-20260926/artifact-gate.json).
- Disconnected production copy: fallback→candidate→fallback→candidate passed, including
  all-history pagination (21 groups/11 pages at limit2), stable exclusion counts, unchanged
  permissions/admin store and all original source/ledger rows. Candidate quarantine retries
  stayed unchanged across32 seconds on both candidate starts.
  [Rehearsal](evidence/cpar-usage-quarantine-20260926/rehearsal.json).
- Production cutover took1,324ms. Running process SHA256, public four-asset hashes, CSP,
  unauthenticated API boundary, schema28,7 managed accounts, permissions and administrator
  store verified. All-history results agree with699 ledger records; the11 quarantined events
  remain unresolved, not falsely marked billed. Retry attempts remain unchanged across32 seconds.
  [Deployment receipt](evidence/cpar-usage-quarantine-20260926/production-receipt.json).
- Independent post-release API/SQLite readback at18:08:49 again verified699 accepted requests,
  11 excluded events,4 groups and stable retry attempts; Build3/Console2 inventory projection and
  source rows remain intact. [Postcheck](evidence/cpar-usage-quarantine-20260926/postcheck.json).
- Production EgoLite reload correctly locks the old session and displays the administrator
  login page. Authenticated post-release visual verification awaits the user's current password;
  the local UI evidence above is not presented as production login evidence. Space1/p1 was
  handed back to the user; no old bootstrap-password retry or administrator reset was attempted.

## Release and recovery boundaries

ARM64 binary SHA256: `eca8cce785b7621995c4d74ced11fa4991bb20d31a334707f59767376b375709`.
Rollback binary: `3a164322c5731747cceb268e26249349953281d6`; retain the latest state database.
Private pre/post-cutover SQLite backups are retained on the existing host. No rollback was needed.
Older strict callers intentionally continue to reject ambiguous all-history reads; Prism now
explicitly opts into partial results. Rolling back restores the old strict UI behavior and retry
eligibility, rather than removing the ambiguous source data.

Human entry: <https://cpar.142857142.xyz/admin-ui/#/usage>. Choose“全部历史”after login:
the page must display verified subtotals and the11-event incomplete-history notice. Prices that
were unknown remain unpriced; quarantining cannot repair absent call lineage or invent costs.

No new Provider inference, OAuth, catalog refresh, historical rewriting or deletion is part of this repair.
