# BC-ROUTE-007: Provider-scoped Route Explain composition

Status: `DONE_WITH_BOUNDARY`

## Contract

`RouteCredentialScheduler::explain_provider_scoped` composes one immutable Route Explain snapshot
with `ProviderScopedSelector`. It consumes only secret-free pool diagnostics and exact runtime
Health/Quota state. The returned selection is advisory diagnostic data, never a lease or a serving
decision.

The composition input contains:

- one fixed `RouteExplainInput` and one exact `ProviderId`;
- a bounded set of capability/protocol-admitted Candidate IDs;
- an optional bounded map of versioned candidate costs (empty means unknown cost).

Provider identity is the Candidate's owning `upstream_id`, converted explicitly to `ProviderId`.
The adapter ID, endpoint ID, model name and request body are not used as a Provider identity.

## Eligibility and isolation

The base explain must find at least one exact eligible Credential for a candidate. Expiry at or
before the observation instant, Health/Quota blocks, missing pools, saturation and protocol
transform rejection keep a candidate outside the selector. A foreign Provider is rejected by the
selector and never becomes fallback material. A Route with several Providers requires an explicit
`provider_id`; omission yields the value-free `provider_scope_required` reason.

Known quota evidence outranks unknown evidence; unknown quota and cost are not coerced to zero or
unlimited. Active leases and maximum concurrency are summed only across currently eligible
Credentials with checked arithmetic. A real request must perform its normal lease-time checks
again after this advisory projection.

## HTTP boundary

`GET /admin/routes/{route_id}/explain` retains the existing `{route_id,candidates[]}` response.
It adds optional query parameter `provider_id` and permits the closed reason values
`provider_scope_required` and `provider_mismatch` in addition to the existing reason set. The
authoritative OpenAPI document is the only contract source; Prism's vendored contract/client is
updated only by the sync command.

## Security and ownership

No URL, credential plaintext/ciphertext, cookie, header, request/response body, raw quota window or
client-key digest crosses the boundary. No Provider I/O, SQLite mutation, refresh/reauth, proxy
selection or cross-Provider fallback is allowed.

## Formal evidence

The complete P13-07 phase passed the local Full preflight (`43/43`) and the immutable
`phase-p13-routing-complete` Delivery Gate at commit
`0c338ee8eef76e470c55515a24728324684365c5`: [run 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495)
completed Authorize, Fast, Full supply-chain and Required successfully in `3s`, `5m57s`, `1m16s`
and `2s`. This contract is closed as `DONE_WITH_BOUNDARY`; it does not claim Provider traffic,
staging/production mutation or the start of P13-08.
