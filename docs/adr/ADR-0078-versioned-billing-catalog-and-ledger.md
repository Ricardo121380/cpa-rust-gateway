# ADR-0078 Versioned billing catalog and idempotent ledger

| Field | Value |
|---|---|
| Status | Accepted for P13-05A |
| Date | `2026-08-11` |
| Task | `P13-05A` |
| Contract | [BC-MGMT-011](../contracts/BC-MGMT-011-versioned-billing-catalog-ledger.md) |

## Context

P13-04 exposes durable Usage facts while deliberately returning `cost=null` and
`cost_confidence=unpriced` when no authoritative price is available.  The management backend now
needs a durable billing foundation that can be replayed after restart, cannot double-charge a
replayed Usage event, and does not require the request path to write SQLite synchronously.

## Decision

CPAR stores immutable price catalog versions keyed by `(Provider, Channel, Model)`.  Rates use
integer micro-units per million tokens; all multiplication, addition and conversion are checked
integer operations.  A ledger row references the source Usage event, request/response identity,
public routing identity, token observations, catalog version, cost and an explicit confidence.
The source event id is unique and its SHA-256 fingerprint makes an identical replay a no-op while
conflicting replay fails closed.

Missing catalog entries remain `unpriced`.  Missing token dimensions remain `unknown` or produce a
lower-bound `partial` result only when at least one reported dimension is priceable; missing data
is never converted to zero.  Ledger retention is explicit and deletion is bounded by operator
batch size.  No request body, credential, endpoint URL, key digest or upstream response is stored.

## Consequences

- Billing can be reconstructed deterministically from the immutable catalog and ledger rows.
- Catalog corrections require a new version rather than mutating historical prices.
- The initial slice is transport-neutral; management HTTP and time-series aggregation are follow-up
  P13-05 tasks.
- Price catalogs are provider/channel scoped, so this foundation cannot introduce cross-provider
  credential, proxy or fallback behavior.

## Validation and rollback

Focused Store and Control tests cover catalog immutability, idempotent/conflicting replays, fixed
integer pricing, unknown/unpriced confidence, retention purge and file reopen.  Rollback is a
schema downgrade to version 13 after deleting only the P13-05 billing tables; existing P13-04
event and usage facts are not rewritten.
