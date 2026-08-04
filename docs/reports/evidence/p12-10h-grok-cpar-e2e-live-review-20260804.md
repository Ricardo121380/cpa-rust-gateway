# P12-10H native Grok CPAR live E2E review

Status: `REVIEWED_BLOCKED_EXTERNAL_RATE_LIMIT`

## Scope reviewed

- Exact source diff: native Grok bridge override admission and the staging helper's revision
  refresh before publish.
- Exact artifact: signed ARM build for revision `a8455ca77dcbdfa95b1c392b42a117ccb02ba7de`.
- Evidence: [live E2E receipt](p12-10h-grok-cpar-e2e-live-20260804.md), staging database checks,
  and the protected rollback result.

## Findings

1. The runtime admission repair is narrowly scoped to `grok.build.responses` and the exact
   `reasoning=false` narrowing; arbitrary capability claims remain rejected.
2. The direct harness authenticated the CPAR route and completed 26 calls across Responses, Chat,
   and Messages before the first provider-level rate-limit classification. The protected Attempt
   record and the value-free receipt agree on the stop boundary.
3. The single eligible Build account remained active without a persisted account cooldown or quota
   mutation. The run therefore does not indicate a CPAR account-state bug.
4. A requested five-account follow-up was not forced: the source exporter reported insufficient
   eligible active Build accounts. No expired or disabled credentials were admitted to inflate the
   sample.
5. Rollback restored the staging predecessor and removed the temporary account. Production CPAR
   remained active and its listeners were unchanged; no production or source-pool mutation was
   observed.

## Verdict

`PASS` for implementation, signed-artifact provenance, protected staging lifecycle, protocol
coverage before the external stop, and rollback. `BLOCKED` for the requested 100-call live
acceptance because the external provider rate-limited the only eligible account. Do not mark
P12-10H or final P12-10 retirement complete until a separately authorized run has a sufficient
eligible account pool or a documented provider-side recovery window.
