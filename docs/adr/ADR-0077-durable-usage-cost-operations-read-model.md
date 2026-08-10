# ADR-0077 Durable usage and explicitly unpriced cost operations read model

| Field | Value |
|---|---|
| Status | Accepted for P13-04; phase gate pending |
| Date | `2026-08-11` |
| Task / Contract | `P13-04B` / [BC-MGMT-010](../contracts/BC-MGMT-010-durable-usage-cost-operations.md) |
| Scope | Read-only aggregated Usage/Cost management projection |

## Context

The gateway already emits bounded Request, Attempt and final Usage events into the append-only
event log, and P10 already exposes runtime Health/Quota availability and audit resources. The
CPAMP-like management surface needs a typed usage view without reading request bodies, replaying
Providers, or inventing a price from token counters.

## Decision

Add `GET /admin/operations/usage` as a protected, read-only projection over the durable event log.
For each Request, the highest numbered Attempt is selected and must be successful before its final
Usage event contributes to an aggregate group. Groups are keyed by Provider, Channel, Account,
public model, inbound protocol, Client Key and optional Access Group. Filters are exact identity,
model, protocol and an inclusive Attempt-end-time window.

Results use a stable keyset `(provider_id, channel_id, account_id, public_model, protocol,
client_key_id, access_group_id)`, default 50 and maximum 100. The cursor contains only those
non-secret fields and is bounded/opaque at the HTTP edge. Token counters use checked addition and
carry `exact`, `partial` or `unknown` confidence. `cost_microunits` remains null with
`cost_confidence=unpriced` until a versioned price catalog is implemented; no cost is guessed.

The production composition injects a read-only event-log facade. It opens the already-migrated
SQLite file with read-only flags and a bounded busy timeout, while the existing event writer keeps
the writable connection. Existing P10 runtime availability remains the source of live Health/
Quota/Circuit state; this endpoint does not duplicate or reinterpret it.

## Consequences

- The management backend supplies a stable usage/cost shape suitable for a future CPAMP dashboard.
- Retry attempts are not attributed to the final successful Provider group.
- Missing token fields and unavailable prices remain visible as uncertainty rather than zero.
- The read path has no Provider, credential, lease, snapshot, configuration mutation or body access.
- A future P13-05 billing ledger can add price catalogs and durable cost events without changing
  this boundary's no-fabrication rule.

## Alternatives considered

- **Read live Provider quota and billing here:** rejected; Provider-specific runtime state remains
  isolated behind P13-06/P13-12 facades and P10 already owns runtime quota visibility.
- **Treat missing token fields as zero:** rejected; it would falsify usage totals.
- **Calculate cost from a hard-coded price:** rejected; prices are mutable and provider/model
  specific, so the result would be unauditable.
- **Open the event database read-write for each GET:** rejected; management reads use the
  read-only SQLite open boundary and never run migrations or change journal mode.

## Validation and rollback

The P13-04B unit/HTTP fixtures cover lineage, retry selection, filtering, keyset ordering,
confidence labels, explicit unpriced cost, malformed query rejection, protected admission and
absence of private upstream model/body values. Rollback removes the usage facade, route, OpenAPI
operation, generated-client operation and this projection without touching the event writer or
runtime quota state.
