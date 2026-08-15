# BC-ROUTE-006: Provider-scoped deterministic selector

Status: `DONE_WITH_BOUNDARY`

## Contract

`ProviderScopedSelector` evaluates a bounded list of secret-free candidate observations for one
exact `ProviderId` and returns a deterministic selection plus closed rejection reasons.

Each candidate carries:

- Provider, channel and route-candidate opaque identities;
- non-negative priority, positive weight, active leases and maximum concurrency;
- optional versioned `cost_microunits`;
- closed Health and Quota observations;
- capability-match and expiry predicates.

The candidate list is bounded at 4096 entries and each non-whitespace identity at 128 Unicode
scalar values. Duplicate candidate identities (including repeated IDs across Provider/channel
observations) and invalid bounds fail closed before ranking, so the selected opaque ID cannot be
ambiguous or reintroduce input-order dependence.

## Selection semantics

1. Reject a foreign Provider, capability mismatch, expired candidate, blocked Health, blocked or
   recovery-in-flight Quota, or saturated concurrency.
2. Prefer known quota evidence over unknown quota evidence; unknown remains eligible but is not
   interpreted as unlimited.
3. Prefer known lower cost; unknown cost is after known cost and is never represented as zero.
4. Prefer the lower active-leases/max-concurrency ratio, using `u128` cross multiplication.
5. Apply lower configured priority, higher weight, then channel and candidate identity as stable
   tie-breakers.

The result is policy data only. It is not a lease, a Provider request, a retry decision, or a
promise that a later concurrent request will obtain the same candidate.

## Security and ownership

No credential plaintext/ciphertext, endpoint URL, header, cookie, request body, client-key digest,
raw quota window or Provider response is accepted or returned. `RouteCredentialScheduler` remains
the owner of cursor advancement, exact runtime reads and Credential leases. Cross-Provider
fallback and implicit credential conversion are forbidden.

## Frontend boundary

P13-07A changes no management OpenAPI shape and no Prism generated client. A future Route Explain
shape change must update the authoritative OpenAPI source, sync Prism, and append a
`docs/cross-boundary-log.md` entry before frontend work.

## Formal evidence

The complete P13-07 phase passed the local Full preflight (`43/43`) and the immutable
`phase-p13-routing-complete` Delivery Gate at commit
`0c338ee8eef76e470c55515a24728324684365c5`: [run 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495)
completed Authorize, Fast, Full supply-chain and Required successfully in `3s`, `5m57s`, `1m16s`
and `2s`. This contract is closed as `DONE_WITH_BOUNDARY`; it does not claim Provider traffic,
staging/production mutation or the start of P13-08.
