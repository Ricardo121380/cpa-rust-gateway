# Post-release EgoLite verification and usage-scope repair

2026-09-26. User completed normal administrator login in the existing EgoLite space1/p1.
Verified production frontend asset version `3c6586561e344a4f03f59153`, deployed with signed
`b39ec32`. No login bypass, credential capture, Provider inference, OAuth start, account
mutation or configuration publication was performed.

> Release update: PB-03 shipped as signed `3a16432` at16:24. Recent production API reads now
> succeed; all-history ambiguity remains. After the user logged in again, the16:38–16:40 EgoLite
> recheck verified repaired Token rendering. See [release evidence](cpar-usage-scope-production-20260926.md).

## Post-repair logged-in recheck

- Same production space1/p1, normal user login, signed backend `3a16432`.24h/7d/30d visibly
  show28/30/491 requests with usage and no management error. These are usage-bearing requests,
  not all incoming requests. Six token categories retain their independent confidence; unobserved
  cache read/write remain unknown.
- Thirty-day provider grouping has one provider and five underlying aggregate rows. Switching to
  model grouping shows `grok-4.6`446 and `grok-4.5`45, totaling491 without changing scope.
-1440×900 and1280×720 desktop views render the table;390×844 uses stacked cards. Light/dark mobile
  cards retain all token labels and values; document/table widths remain within the viewport.
- All-history Token still shows `Management operation failed`; no complete total is fabricated.
  The separate ledger visibly retains699 records. Processing remains2356/2356 with11 unresolved
  events. The generic error text remains an open UX issue.
- Browser returned to the authenticated24h usage view at1440×900, light appearance, and was
  released with `finish({keep:["p1"]})` for user inspection. No production mutation or Provider call.

Counts and layout evidence: [recheck JSON](evidence/cpar-postrelease-browser-20260926/usage-recheck.json).
Screenshots remain in the private directory listed below; this check does not replace the earlier
eight-workspace regression or claim new real-channel authorization/inference acceptance.

## Actual browser results

- Grok Build: provider panel lists three managed native accounts with real email identities;
  endpoint shows Responses and three observed runtime accounts. No ordinary credential-binding
  controls are presented for native accounts. Runtime deep-link retains the provider filter;
  three bindings are observed, but only one is authenticated/schedulable and two are expired.
  Saved inventory count is not a claim of three usable accounts.
- Grok Console: two managed accounts and two observed runtime accounts, Responses. Both historical
  SSO identities remain unavailable. The UI does not invent emails; this is an existing limitation,
  not proof that identity acquisition now succeeds.
- Account detail opens from provider rows. Desktop panel width520px; mobile width366px with12px
  margins. Checked1440×900,1280×720,390×844 in light/dark appearance; long email wraps. Escape
  closes the panel and restores focus to its source button; Tab remains within the dialog.
- Ledger full-range summary shows699 rows, all unpriced and known cost unknown. Load-more advances
  from100 to200 displayed rows without changing the complete summary. Historical request attempt
  detail opens and closes;390px detail has no panel or document horizontal overflow.
- Processing displays needs-repair,11 unresolved events, source/checkpoint2356. This correctly
  retains ambiguity; no attempt was made to clear it by deleting or fabricating history.
- Private screenshots remain under `/private/tmp/cpar-postrelease-browser-20260926` because they
  contain real account identities. Committed evidence contains only counts, geometry and safe errors.

## New finding: PB-03

The Token usage section fails with HTTP500 `management_internal_error` / `Management operation
failed`, including24h,7d,30d. Ledger and request-summary reads remain available.
`SqliteEventStore::visit_usage_lineages` used `WHERE invalid OR (filters...)`: all four malformed
historical request lineages entered every query regardless of their known date or dimensions.

The repair applies known time/dimension filters before lineage validation. Missing metadata cannot
establish exclusion; an orphan with unknown scope still fails closed. Invalid lineages inside the
requested scope remain rejected, and validation still occurs before the cursor, preserving complete
aggregation and snapshot semantics. No storage/schema/API shape or history mutation is introduced.

Read-only query comparison against the existing frozen production backup (ordinal2356):

| Range | Old query selected / invalid | Candidate selected / invalid |
|---|---:|---:|
|24h|32 /4|28 /0|
|7d|34 /4|30 /0|
|30d|495 /4|491 /0|
|All history|703 /4|703 /4|

All-history Token aggregation remains unavailable while those four lineages are ambiguous. This
repair does not silently omit them or claim a complete total. The ledger's699 rows are independent
validated materializations; its partial historical coverage remains visible in processing status.

The focused regression covers time, all seven dimensions, exact boundary inclusion, old immutable
snapshot and unknown-scope rejection. It fails against the original SQL and passes against the fix.
Existing large-history snapshot/paging regression and strict store Clippy pass. The linked release
report records formal gates and the exact deployed revision separately from browser results.
