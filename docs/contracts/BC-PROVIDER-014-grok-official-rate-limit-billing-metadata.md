# BC-PROVIDER-014 Grok Official rate-limit and billing metadata boundary

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-014` |
| Task | `P8-03` |
| ADR | [ADR-0056](../adr/ADR-0056-grok-official-rate-limit-billing-metadata.md) |
| Matrix | `C01`、`C31`、`C33`、`E11`、`G16`、`G26` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-DEFER-002`; no Official E2E has run |
| Domain | Strict fixed-header rate-limit/reset evidence and non-financial usage metadata |

## Preconditions and bounds

1. Only the Official production transport reads raw response headers, and it asks the shared
   response boundary for the seven fixed Header names. No database, environment, file, OAuth cache,
   browser session, proxy configuration, server account, or live probe is read.
2. Accepted request/token evidence is a complete exact-once three-header tuple: `limit`,
   `remaining`, and `reset`. Value strings are at most 32 decimal bytes. Reset accepts only
   positive `ms`, `s`, `m`, or `h` durations no longer than 24 hours. `Retry-After` accepts only a
   bounded non-negative delta-seconds value, not an HTTP date.
3. The parser retains only typed counters/durations and header-presence structure. It retains no
   raw Header text, model, endpoint, credential, account, pricing, or body value.
4. This local task does not authorize a real xAI request. `P8-07` / `BC-E2E-004` own the separately
   authorized Official E2E; `CR-P7-DEFER-002` makes that P8 prerequisite independent of P7/G7.

## Required behavior

| Concern | Required behavior |
|---|---|
| Duplicate visibility | Shared upstream response access exposes every value for a requested Header name. The Official parser rejects a duplicate recognized header; it never chooses first/last. |
| Header allow-list | Inspect only `x-ratelimit-limit/remaining/reset-requests`, `x-ratelimit-limit/remaining/reset-tokens`, and `retry-after`, case-insensitively. Unknown headers have no semantic effect and are not retained. |
| Window validity | A resource is absent or an exact complete triplet. Limit is nonzero, remaining cannot exceed limit, and reset must be positive/bounded. Partial, duplicate, malformed, non-ASCII, overflowed, or impossible input is `UpstreamProtocolError/Provider`. |
| Reset/retry | `Retry-After` delta-seconds is separate from resource reset. A date form or over-limit duration fails closed; no local clock or fallback estimate is injected here. |
| Billing metadata | Project provider-reported Canonical input/output/reasoning/cached token counts only. Do not turn usage/rate headers into a billing plan, account balance, price, currency, charge, or scheduling state. |
| State isolation | P8-03 does not mutate quota/account/health/retry/persistence state. P8-05 owns source-labelled exact Official state application and cross-source isolation proof. |
| Diagnostics | `Debug` exposes only window count and field-presence flags, never Header values, counters, delays, token counts, URL, API key, credential, model, or account data. |

## Corresponding tests

- `fixed_complete_rate_limit_windows_are_case_insensitive_and_redacted`
- `ambiguous_partial_unsafe_or_impossible_headers_fail_closed`
- `token_usage_is_billing_metadata_without_inventing_a_plan_or_price`
