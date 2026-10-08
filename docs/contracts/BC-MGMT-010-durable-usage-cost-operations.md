# BC-MGMT-010 Durable usage and explicitly unpriced cost operations

| Field | Value |
|---|---|
| Contract | `BC-MGMT-010` |
| Task | `P13-04B` |
| Status | Accepted; local implementation and review passed; phase gate pending |
| Domain | Secret-free management Usage/Cost read model |

## Boundary

`GET /admin/operations/usage` is a protected management read. It uses the existing management
key, peer/origin and safe error admission. It reads only the gateway-owned append-only Request,
Attempt and final Usage event records. It never reads request/response bodies, headers, URLs,
credentials, Provider state or control-plane secrets.

## Lineage and aggregation

1. A Request event supplies the public model, inbound protocol, Client Key and optional Access
   Group.
2. An explicit Usage AttemptId selects that exact Attempt, including a failed bootstrap, for
   Provider, Channel, Account and non-negative end timestamp. Legacy Usage without AttemptId
   retains the highest-numbered successful-Attempt requirement.
3. Exactly one final known Usage snapshot per actual Attempt contributes to its group. Missing
   lineage, conflicting event identities or counter overflow fails closed. Unknown legacy and
   conflicting explicit ownership cannot be reassigned; existing partial reads report exclusions.
4. Groups are sorted by `(provider_id, channel_id, account_id, public_model, protocol,
   client_key_id, access_group_id)` and use default-50/max-100 keyset pagination.

## Query

Supported filters are `from_ms`, `to_ms`, `provider_id`, `channel_id`, `account_id`, `model`,
`client_key_id`, `access_group_id`, `protocol`, `limit` and opaque `cursor`. Time bounds are
inclusive and apply to the selected Attempt end time. Unknown/repeated/malformed
parameters, reversed/negative windows, zero/oversized limits and malformed cursors return a safe
`400` response.

## Usage and cost semantics

Every token field returns `{total, confidence}`. `confidence` is `exact` when all observations in
the group supplied the field, `partial` when some did, and `unknown` when none did. Totals use
checked addition. Cost is always `{cost_microunits: null, cost_confidence: "unpriced"}` until a
versioned price catalog and billing ledger are added; token counts are never converted to money
by inference.

`request_count` counts distinct Requests in each group; `usage_observations` counts separate
Attempt snapshots. Accumulating different Attempts does not add successive cumulative snapshots
of one Attempt. The six counters retain the existing provenance/confidence rules.
This B-04 amendment follows [ADR-0100](../adr/ADR-0100-exact-protocol-semantics-and-usage-evidence.md)
and [ADR-0103](../adr/ADR-0103-failed-bootstrap-usage-attribution.md).

## Invariants and verification

- The response is a closed JSON object with at most 100 items and no private upstream model,
  request body, URL, Secret, token, Cookie or digest value.
- Retried failed Attempts are not attributed to the final successful group.
- The cursor is stable for the same event-log snapshot and query ordering.
- Production reads use a read-only SQLite connection and a bounded event scan; they do not run
  migrations or alter the journal mode.
- Unit and protected HTTP tests cover grouping, confidence, filters, pagination, malformed input,
  cost non-fabrication, secret safety and Management Key concealment.

Live Health/Quota/Circuit remains the existing P10 runtime availability contract. Provider-owned
native account pools, refresh/reauth and price/billing ledgers remain P13-05/P13-06/P13-12 work.

## Corresponding tests

- `failed_attempt_usage_stays_separate_from_a_usable_fallback`
- `explicit_attempt_usage_counts_one_request_and_two_observations`
