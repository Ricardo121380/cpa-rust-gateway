# Review: Autoreg Oracle Console production canary

Verdict: `PASS_WITH_BOUNDARY`

## Review checks

- Source provenance is explicit: Oracle Autoreg task → strict Console envelope → CPAR
  `grok_console` batch → batch-scoped probe → public HTTP matrix.
- The isolated import/probe/matrix was completed and rolled back before the production canary.
- The first public six-tuple observation was correctly excluded because it preceded the service
  reload required to compile the imported native pool.
- After one controlled service restart, the same-batch native probe passed and the production
  canary used the real CPAR public data plane for exactly six protocol/mode tuples. All six were
  2xx, content-type-valid and semantically complete.
- The new batch is attributed in the native probe and is retained only because the operator
  requested the channel be added to production. No implicit provider fallback was used.
- Production DB integrity, active Config Version, listener topology, old CPA, Caddy/DNS,
  CC Switch and grok2api invariants were rechecked after the canary.
- Oracle single-primary and Jakarta fencing are already in force. The scheduler remains
  intentionally manual/disabled; this is not an unattended reauth completion.
- The restart changed process state only; active Config Version, route graph, database contents
  other than the approved batch, and listener topology were rechecked unchanged.
- The lightweight GitHub workflow for commit `05da177` completed successfully; no expensive
  Delivery Gate was started.

## Remaining boundaries

The adapter's model argument is allowlisted and is proven by the same-batch native probe rather
than by an Autoreg model field. Refresh deadline metadata does not itself implement a live
Console refresh executor. Source path checks reduce, but cannot mathematically eliminate, every
same-inode/ABA race. Management display metadata and the formal reauth scheduler remain later
tasks. These boundaries do not invalidate this one-account production Console canary.
