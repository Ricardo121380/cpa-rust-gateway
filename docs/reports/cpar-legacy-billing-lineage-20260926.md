# Legacy billing lineage rehearsal — 2026-09-26

The signed `0bc7cae` production-copy rehearsal was blocked before deployment. It recovered
82 of the 87 legacy Usage rows as `unpriced`; five remained `invalid_lineage`. Production
data and the production process were untouched. This was a useful negative gate, not a pass.

Further read-only aggregation of the original rows found:

| Historical shape | Usage rows | Distinct request IDs | Decision |
|---|---:|---:|---|
| One Usage, one successful Attempt, one Request | 76 | 76 | Safe to materialize with original source IDs |
| Two different response IDs/usages joined to one successful Attempt | 6 | 3 | Ambiguous; do not assign both to that Attempt |
| Five different response IDs/usages joined to one failed Attempt | 5 | 1 | Invalid lineage; no successful Attempt proves billing association |

The six newly identified rows were not exact duplicates: response IDs and usage values differ.
There is no trustworthy basis to choose a winner, split one Attempt, invent missing Attempts,
or rewrite historical requests. The materializer now rejects any different Usage in the same
request lineage. New append paths already reject conflicting Usage for one request; this guard
protects historical rows that predate that constraint. Original rows, source IDs, and existing
ledger rows are retained. No cleanup, direct failure-table edit, or fabricated zero-price repair
is performed. A targeted regression covers distinct response IDs even when token counts match.

The revised production-copy and post-deployment acceptance must prove 76 inserted ledger rows,
76 resolved failures, 11 remaining `invalid_lineage` rows, unchanged historical ledger rows,
and no additional insertions across fallback/candidate restart. All 76 are expected to remain
`unpriced` with null amount because no matching historical price catalog was observed.
The processing UI must truthfully remain `needs_repair`; zero pending is not a valid target for
these ambiguous records without additional original correlation evidence.

This supersedes the initial assumption that all 87 rows can be automatically restored. The
11 unresolved rows are a historical evidence gap, not an authorization to delete history or
change real consumption. Reconciliation requires an authoritative original request/attempt
mapping and remains separately documented after the safe release.
