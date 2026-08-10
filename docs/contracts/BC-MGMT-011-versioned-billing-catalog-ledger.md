# BC-MGMT-011 Versioned billing catalog and durable ledger

| Field | Value |
|---|---|
| Contract | `BC-MGMT-011` |
| Task | `P13-05A` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | Management operations, durable usage/cost/billing foundation |

## Entry and preconditions

The caller supplies one value-free Provider/Channel/Account/Model identity and one final Usage
summary.  A catalog version must be immutable after insertion.  The store is outside the request
hot path and may be replayed after a process restart.

## Invariants

1. Price lookup is exact on `(provider_id, channel_id, model)` within one catalog version.
2. Rates are integer micro-units per million tokens; floating point and locale-dependent decimal
   parsing are forbidden.
3. One `source_event_id` creates at most one ledger row. An identical fingerprint is a replay;
   a different fingerprint is a hard conflict.
4. Missing price is `unpriced`; missing Usage fields are `unknown` or explicit lower-bound `partial`.
   Unknown values are not treated as zero.
5. Retention purge is bounded and ordered. Restart/reopen returns the same surviving rows.
6. Persisted records contain no request body, credential ciphertext/plaintext, endpoint URL, key
   digest, Cookie, header or upstream response.
7. Catalog and ledger identity remain Provider/Channel scoped. No cross-provider fallback or
   implicit proxy behavior is introduced.

## Error semantics

Malformed identifiers, negative/overflowing integer values, catalog version mutation and conflicting
source-event replay fail closed.  A missing catalog is a valid `unpriced` observation, not a
synthetic successful price.

## Corresponding implementation and evidence

- `crates/gateway-store/migrations/0014_billing_ledger.*.sql`
- `crates/gateway-store/src/billing_ledger.rs`
- `crates/gateway-control/src/billing_service.rs`
- Focused `gateway-store` and `gateway-control` tests, plus the P13-05A report.
