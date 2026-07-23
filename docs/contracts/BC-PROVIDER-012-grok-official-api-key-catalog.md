# BC-PROVIDER-012 Grok Official API-key model catalog boundary

| Field | Value |
|---|---|
| Contract | `BC-PROVIDER-012` |
| Task | `P8-01` |
| ADR | [ADR-0054](../adr/ADR-0054-grok-official-api-key-catalog-boundary.md) |
| Matrix | `C01`、`C31`、`C33`、`G24`、`G28` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` under `CR-P7-G7-001`; no Official E2E has run |
| Domain | API-key isolation, fixed model discovery, exact egress handoff, and strict catalog syntax |

## Preconditions and bounds

1. The caller supplies an already selected, header-safe API key and exact Endpoint/Credential
   IDs. This boundary does not read a database, environment, file, OAuth cache, browser session,
   proxy setting, or server account.
2. The sole P8-01 production target is `https://api.x.ai/v1/models`. Shared transport use requires
   P2 DNS-pinned egress admission for that exact URL; any other admitted URL is rejected.
3. A successful catalog body is at most 1 MiB. Parsing rejects duplicate names recursively,
   trailing bytes, non-object roots, a missing/non-array `data`, non-object entries, absent/blank/
   non-printable/overlong IDs, and duplicate IDs.
4. `CR-P7-G7-001` permits only local P8 work before G7. It does not authorize a real xAI API call,
   Phase closeout, Delivery Gate, merge, release, or a `DONE` claim.

## Required behavior

| Concern | Required behavior |
|---|---|
| Provider isolation | Provider ID is exactly `grok.official`; API-key material and catalog source are distinct from Grok Build OAuth and Grok Web state. |
| Request | Use `GET`, an empty body, `Accept: application/json`, and a request-scoped `Authorization: Bearer` value. Do not emit Build CLI headers, cookies, user-agent emulation, or hop-by-hop headers. |
| Egress | `into_transport_request` accepts only the exact admitted Official Models target. The production implementation uses the injected shared DNS-pinned pool after this check. |
| Catalog identity | The source serves exactly one `ModelCatalogTarget` whose Endpoint ID and Credential ID equal its own. A mismatch sends nothing and returns a safe request error. |
| Catalog syntax | Accept model identities only from top-level `data[*].id`; preserve source order but reject duplicate IDs and any ambiguity. P4 owns later sort/dedup singleflight publication and snapshot freshness. |
| Errors | Non-2xx and malformed/oversized successful bodies are `UpstreamProtocolError/Provider`; P8-01 does not infer 401/403/429, quota, billing, credential mutation, retry, health, or failover semantics. |
| Diagnostics | `Debug` never renders the URL, API key, Bearer header, Endpoint/Credential IDs, body bytes, or model values. |

## Explicitly deferred behavior

P8-01 does not construct Official Responses HTTP/SSE requests, decode inference events, inspect
quota/rate headers, persist billing data, declare Tool/Reasoning/Search capability, update health,
run a real probe, or alter routing/public `/v1/models`. P8-02 through P8-06 and existing P4
catalog/route boundaries own those operations.

## Corresponding tests

- `official_models_request_is_fixed_authenticated_get_and_exact_target_only`
- `catalog_source_is_exact_credential_scoped_and_parses_strict_fixture`
- `catalog_source_rejects_non_success_and_ambiguous_or_invalid_payloads`
