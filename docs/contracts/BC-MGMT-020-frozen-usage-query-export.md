# BC-MGMT-020: Unified Usage snapshots and CSV/JSONL export

- Status: Accepted; local implementation, validation recorded separately
- Task: [#72](https://github.com/Ricardo121380/cpa-rust-gateway/issues/72), #58 T19/T20
- Decision: [ADR-0128](../adr/ADR-0128-frozen-usage-query-export.md)

## Boundary and query

`POST /admin/operations/usage-snapshots` accepts `from_ms`, `to_ms`, `bucket_ms` and optional exact
`provider_id`, `channel_id`, `account_id`, `model`, `client_key_id`, `access_group_id`, `protocol`.
It requires existing management authentication and applicable Origin/CSRF checks, with no configuration
revision requirement. Public Client Keys grant no access. Malformed/unknown/duplicate parameters fail.
A valid window is inclusive UTC Unix milliseconds, at most 366 days / 1000 buckets, bucket >=1000 ms.
Time means terminal Attempt end or Request start when no terminal Attempt exists. Historical
requests with unknown start and no terminal Attempt have no provable membership in this time range.

A successful 201 includes a random `snapshot_id`, expiry, schema version `usage-snapshot-v1`, exact
filters, event/ledger watermarks, full-filter summary and row counts. Source failure is 503, capacity
is 429, selected source/result overlimit is 413. Failures never return an empty successful summary.
A 200 with no observations has requests/attempts=0 but token/cost totals=null with unknown confidence.

## Grain and evidence

- `GET /admin/operations/usage-snapshots/{id}` reads unchanged metadata/summary.
- `GET .../{id}/rows?grain=details|buckets&limit=1..100&cursor=...` returns typed JSON pages.
- Detail rows are `attempt` or `request_without_attempt`; exact request and optional Attempt IDs
  make retries explicit. All six counters, provenance/input accounting, Request/Attempt/Usage
  outcomes, existing ledger ID, original catalog version and original cost are retained.
- Requests are distinct over the full selected rows; attempts count matching terminal Attempt facts.
  Request outcome is not driver outcome; Usage on failed/cancelled attempts remains included.
- No Usage is pending/unknown, no materialized ledger is pending/unavailable, missing catalog is
  unpriced. Exact/partial/unknown priced rows remain distinct. Known cost is a nullable subtotal,
  never a claim that missing costs are zero. Estimated/unknown Usage cannot become an actual bill.
- Top-level input_tokens is not additive across mixed/unknown input accounting; it becomes null with
  unknown confidence. Use `input_tokens_by_accounting`. Other counters preserve source semantics;
  reasoning/cache are subsets, so clients must not sum token dimensions to invent a total.
- Nonempty buckets align to from_ms. Distinct request counts can overlap across buckets; use the
  full summary for distinct totals. Counts/amounts are computed before any page boundary.
- Conflicting or unverified selected lineage fails the snapshot; there is no silent partial mode.
  Raw Usage extensions, cost_in_usd_ticks, count_tokens estimates, credentials, private models,
  prompts/responses, source fingerprints and raw metadata do not enter this projection.

## Frozen export and lifecycle

`GET .../{id}/export?grain=details|buckets&format=csv|jsonl&limit=1..100&cursor=...` reads the same
rows as JSON paging. `X-Usage-Snapshot`, `X-Usage-Schema`, `X-Usage-Total-Rows` identify the result.
`X-Next-Cursor` is present only when another page exists. The cursor binds batch and grain; changing
either returns 409. Filters are fixed at creation and are not accepted on page/export requests.

CSV is UTF-8 with CRLF rows, fixed header on every page, quoted/escaped strings, empty null fields,
explicit observed 0 and named status/provenance columns. Formula-like strings (including leading
whitespace/control prefixes) gain a leading apostrophe. Aggregate metric cells contain JSON.
JSONL retains actual null, integer values and nested metric objects; one row per line. Time is UTC
Unix ms; tokens are counts; costs are the ledger's integer microunits, not a new currency conversion.
Consumers handling integers beyond JavaScript's safe range must use lossless integer parsing.

At most 20000 source events and 16 MiB source JSON are decoded. Each serialized result is <=8 MiB;
up to eight snapshots share a 32 MiB serialized-result budget per process for ten minutes.
This budget does not measure allocator overhead or process RSS. No partial result is emitted on
capture failure. Existing four-slot management read admission bounds capture jobs, including cancelled HTTP creation requests.
Expiry/cancellation/restart/foreign actor returns 410 without silently replacing a batch.
`DELETE .../{id}` releases it; already transmitted or in-flight pages cannot be recalled.
A disconnected or failed page does not advance server state: retry the same cursor, discard any
incomplete page, and append only complete pages. Resume must finish before expiry. New capture after
expiry explicitly selects a new data scope. Export does not replace encrypted backup or restore.

## Corresponding tests

- `usage_snapshot_http_freezes_totals_details_and_exports`
- `usage_snapshot_mixed_accounting_is_not_an_exact_total`
- `usage_csv_neutralizes_formulas_and_preserves_quotes`
- `usage_snapshot_expiry_and_owner_are_enforced`
- `usage_snapshot_contract_exposes_frozen_query_and_both_export_grains`

Actual gateway composition: `CPAR_USAGE_SNAPSHOT_ONLY=1 python3 scripts/acceptance/cpar-batch-b-http.py <output>`
uses independent generated credentials/database and controlled loopback TLS upstream only.

## New frontend example

```text
POST /admin/operations/usage-snapshots
{"from_ms":1792848000000,"to_ms":1792934399999,"bucket_ms":3600000,"client_key_id":"example-client"}

GET /admin/operations/usage-snapshots/{snapshot_id}
GET /admin/operations/usage-snapshots/{snapshot_id}/rows?grain=details&limit=100
GET /admin/operations/usage-snapshots/{snapshot_id}/rows?grain=buckets&limit=100
GET /admin/operations/usage-snapshots/{snapshot_id}/export?format=csv&grain=details&limit=100
GET /admin/operations/usage-snapshots/{snapshot_id}/export?format=jsonl&grain=buckets&limit=100
DELETE /admin/operations/usage-snapshots/{snapshot_id}
```

Display the full summary from metadata, not a sum of the visible page. Show partial/unknown/pending/
unavailable labels with the known subtotal. Keep the same snapshot ID throughout query/export;
show expiry and offer explicit recapture. For CSV, retain one header when combining complete pages.
[OpenAPI](../openapi/management-v1.json) owns the complete field schemas and transport parameters.
