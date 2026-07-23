# ADR-0054: Grok Official API-key catalog boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P8-01` |
| Matrix / Contract | `C01`、`C31`、`C33`、`G24`、`G28`; [BC-PROVIDER-012](../contracts/BC-PROVIDER-012-grok-official-api-key-catalog.md) |

## Context

The xAI Official API is an API-key Provider, while Grok Build uses OAuth and Grok Web will use
browser SSO. They may have overlapping public model names but must never share credentials,
catalog results, quota, health, or failure meaning. P4 already supplies an exact
Endpoint/Credential catalog-source interface and singleflight scheduler. P8-01 needs a native,
fixed Official `GET /v1/models` boundary that can feed that interface without opening a parallel
catalog path or embedding an HTTP client in application code.

## Decision

1. Define the stable `grok.official` Provider ID, a zeroizing API-key type, and an immutable
   `https://api.x.ai/v1/models` endpoint in `provider-grok`. API-key validation checks only that
   it is non-empty and HTTP-header-safe; it does not encode a mutable vendor prefix.
2. Construct only an authenticated body-free `GET` request with `Accept: application/json` and
   request-scoped Bearer authorization. An `AdmittedEgressTarget` must equal the exact Models URL
   before the shared DNS-pinned transport receives the key.
3. Bind discovery to one injected Endpoint ID plus Credential ID. A mismatching
   `ModelCatalogTarget` fails before transport dispatch, so an Official account result cannot
   become a Provider-wide or sibling-credential catalog claim.
4. Decode only bounded, strict JSON with a top-level `data` array and unique printable model IDs.
   Duplicate object fields, duplicate IDs, trailing bytes, malformed entries, oversized payloads,
   and non-2xx responses fail as safe provider-protocol errors. The common duplicate-key parser
   is a neutral crate-private utility, not an Official dependency on the Build request module.
5. Keep quota headers, status ownership, Responses HTTP/SSE, Tool/Reasoning/Search semantics,
   persistence, routing, retries, and real E2E outside P8-01. They remain P8-02 through P8-06.

## Consequences

- Official credentials cannot be substituted for Build OAuth or eventual Web SSO types.
- P4's scheduler can singleflight only the exact Official Endpoint/Credential lookup, then apply
  its existing last-success, freshness, and route-publication rules.
- The production catalog transport is injectable and performs no ambient credential, file, proxy,
  or server configuration discovery. Tests use a scripted transport and send no network traffic.
- A future Official response implementation can share only the neutral strict JSON parser and the
  shared upstream transport; it must define its own request/response semantics.

## Alternatives considered

- Reuse Grok Build OAuth/catalog state: rejected because it violates `C31` source isolation.
- Reuse a generic OpenAI-compatible API-key Provider: rejected because Official Provider identity,
  fixed endpoint, state partition, and future quota/error behavior must remain explicit.
- Accept ordinary `serde_json::Value`: rejected because duplicate fields would overwrite earlier
  values and turn an ambiguous catalog into an entitlement claim.
- Introduce a real account probe now: rejected by `CR-P7-G7-001`; P8 may only collect local
  evidence until G7 and P7's Delivery Gate are complete.

## Validation and rollback

The synthetic test corpus proves exact GET method/headers/target admission, zero-body handoff,
diagnostic redaction, exact Endpoint/Credential scope, strict successful fixture decoding, and
safe rejection of non-success, duplicate, malformed, and oversized payloads. Full `provider-grok`
tests, Clippy, formatting, crate-boundary, documentation-link, and Secret checks must pass before
this Task has local evidence.

Rollback removes the Official catalog module, its narrow `gateway-catalog` dependency, synthetic
tests, and this documentation. It changes no schema, persisted state, account, proxy/TUN setting,
server configuration, network traffic, or production route.
