# BC-MGMT-012 Billing materialization and protected read model

| Field | Value |
|---|---|
| Contract | `BC-MGMT-012` |
| Task | `P13-05B` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | Management operations, Usage/cost/billing reporting |

## Invariants

1. A materializer consumes only gateway-owned Request/Attempt/Usage events and never sends a
   Provider request or reads a credential Secret.
2. Each new Usage source event is resolved against its complete request lineage.  The highest
   numbered Attempt must be successful; missing/conflicting lineage fails closed.
3. A checkpoint is monotonic and advances only after the complete bounded batch is persisted.
   Ledger source-event idempotence makes crash/retry replay safe.
4. Catalog selection is Provider/Channel/public-Model scoped and uses the latest catalog effective
   no later than the Attempt end time.  No cross-Provider fallback is allowed.
5. Cost confidence is exactly `exact|partial|unknown|unpriced`; missing token values are not zero.
6. `GET /admin/operations/billing` is protected by the existing management admission boundary,
   limits rows to 100, supports bounded inclusive time and identity/status filters, and binds a
   cursor to a ledger snapshot maximum id.
7. Responses omit source event ids/fingerprints, endpoint URLs, credential material, key digests,
   request bodies, raw headers/cookies and upstream response content.

## Response shape

The page contains `snapshot_ledger_id`, rows, a summary (`records`, status counts and nullable
`known_cost_microunits`) and an opaque `next_cursor`.  A row contains only ledger id,
request/response correlation, Provider/Channel/Account/model identity, six token dimensions,
occurrence time, optional catalog version, cost and confidence.

## Error semantics

Invalid query, cursor, batch bound, malformed lineage, source failure and summary overflow return
the existing safe management error envelope.  The endpoint is `no-store`; authentication failures
remain the existing management 404/deny behavior.

## Implementation and evidence

- `crates/gateway-store/migrations/0015_billing_materializer_checkpoint.*.sql`
- `crates/gateway-store/src/event_store.rs` ordinal-bounded read
- `crates/gateway-store/src/billing_ledger.rs` checkpoint/catalog listing
- `crates/gateway-control/src/billing_materializer.rs`
- `crates/gateway-control/src/management_operations_service.rs`
- `crates/gateway-http-actix/src/management_resources.rs`
- `docs/openapi/management-v1.json` and generated management client
- [`P13-05B report`](../reports/p13-05b-billing-materialization-read-model.md)
