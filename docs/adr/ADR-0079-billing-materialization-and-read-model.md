# ADR-0079 Billing materialization and protected read model

| Field | Value |
|---|---|
| Status | Accepted for P13-05B |
| Date | `2026-08-11` |
| Task | `P13-05B` |
| Contract | [BC-MGMT-012](../contracts/BC-MGMT-012-billing-materialization-read-model.md) |

## Context

P13-05A provides an immutable price catalog and an idempotent ledger, but no durable path turns
gateway-owned final Usage observations into billing rows and no protected operator endpoint can
inspect the result.  The event log is append-only and batches can cross process restarts, so a
materializer must not assume Request and Attempt events occur in the same read batch as Usage.

## Decision

P13-05B adds a bounded Usage materializer.  It reads event ordinals after a durable checkpoint,
reloads complete request lineage for each new Usage event, selects the highest successful Attempt,
chooses the latest effective matching Provider/Channel/Model catalog, and records one ledger row.
The checkpoint advances only after every row in the finite batch has been accepted.  Re-running
after a crash is safe because `source_event_id` replay is idempotent.

The protected `GET /admin/operations/billing` endpoint exposes bounded rows, time/provider/channel/
account/model/status filters, an immutable ledger snapshot cursor, and exact/partial/unknown/
unpriced counts plus known cost.  It carries only request/response correlations, public routing
identities, token summaries and catalog/cost metadata; it never returns source fingerprints,
credentials, endpoint URLs, headers, cookies, bodies or Provider state.

Catalog insertion remains an operator mutation in the Store/Control boundary and is deliberately
not exposed as an HTTP write in this read-model slice.  A later task must add that mutation with
Management Key, CSRF, audit and revision semantics before any production price can be changed.

## Consequences

- Restart and partial-batch recovery cannot double-record a Usage event.
- A missing catalog remains `unpriced`; missing token dimensions remain `unknown` or `partial`.
- Billing pages are stable within a ledger snapshot and cannot silently include later rows.
- The endpoint is management-only and does not contact Providers or mutate active configuration.

## Validation and rollback

Store, Control and HTTP tests cover checkpoint monotonicity, catalog selection, idempotent replay,
status summary, pagination, authentication, malformed query rejection and secret/body absence.
Migration rollback to schema 14 drops only the checkpoint table; P13-05A catalog and ledger rows
remain available for an explicit operator rollback decision.
