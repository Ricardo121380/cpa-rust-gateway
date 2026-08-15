# BC-ROUTE-008: Provider-scoped serving lease revalidation

Status: `DONE_WITH_BOUNDARY`

## Contract

The request-time serving path may consume the deterministic ordering produced by
`ProviderScopedSelector`, but it must perform a fresh exact lease check through the existing
`RouteCredentialScheduler` before invoking a driver. The selector is advisory; the scheduler and
its `EndpointCredentialPools` remain the only lease owner.

The serving input is one pinned RouteSnapshot, one protocol/capability admission result and one
fresh observation time. The selector sees only bounded, secret-free Provider/channel/candidate
observations. It returns an ordered set of candidate IDs and closed rejection reasons. The serving
adapter must not treat the selector's active-lease, Health or Quota observations as a reservation.

## Provider scope and admission

- Provider identity is the owning Route Candidate `upstream_id`, converted explicitly to
  `ProviderId`. Adapter, endpoint, model and request-body values do not establish ownership.
- A route whose admitted candidates map to one Provider may enter selector-driven serving.
- A route with admitted candidates from more than one Provider and no explicit internal scope is
  rejected with a safe unavailable/provider-scope category before lease acquisition, driver startup
  or Provider I/O. The first candidate is never an implicit scope and another Provider is never a
  fallback.
- Candidate IDs supplied by the selector must belong to the same pinned route and Provider. Foreign,
  missing or duplicate identities fail closed.

## Exact lease-time revalidation

For each selector-ranked candidate, in order, the scheduler rechecks at the current observation
instant:

1. RouteSnapshot/config-version and protocol/adapter admission;
2. endpoint and binding enabled/ownership state;
3. candidate and credential expiry (`expires_at_ms <= observed_at_ms` is unavailable);
4. exact endpoint/credential/model Health and Circuit state;
5. exact endpoint/credential/model Quota, including recovery-in-flight and reset evidence; and
6. atomic credential capacity/concurrency.

Only after all checks pass may the scheduler return a lease. If capacity or another check races
after ranking, the next already-ranked eligible candidate in the same Provider may be attempted
without consuming a request attempt or widening `max_attempts`. A failed exact lease does not grant
permission to rebuild a pool, read SQLite, refresh credentials, or send a probe.

## Attempt and stream invariants

`AttemptOrchestrator` remains the owner of attempt numbering, timeout/cancellation, retry budget,
exact candidate/binding exclusions, first semantic event and final driver outcome. Before the first
semantic event, retryable driver failures may select another candidate only within the same Provider
scope. After the first semantic event, no retry or Provider switch is permitted. Cancellation,
timeout and driver failure release the lease through the existing owner. A Config Version already
stale when the executor future reaches its request-start boundary is rejected before lease/driver
work; a publication after that check follows the existing pinned in-flight Snapshot contract.

## Security and observability

The contract exposes no endpoint URL, credential plaintext/ciphertext, cookie/header/body, client-key
digest, raw Provider response, quota window or cost value outside already-approved redacted
projections. Diagnostics may report only stable candidate/provider IDs and closed reason categories.
The serving path performs no Store or network I/O before the driver is invoked, and no automatic
refresh/reauth, proxy-pool fallback or cross-Provider credential conversion is allowed.

P13-07C itself supplies no versioned price map to the serving selector, so cost remains explicitly
`Unknown` at this contract boundary and is never treated as zero. P13-07D's separate BC-ROUTE-009
contract later injects a Config-Version-compatible P13-05 price-catalog projection without adding
per-request Store I/O or changing the lease owner.

## Public boundary

P13-07C does not change public Chat, Responses or Messages request/response shapes, management
OpenAPI, Prism generated clients or frontend routes. If a later task adds an explicit public or
internal Provider/route-policy field, it must create a new Change Request, update the authoritative
OpenAPI source first, synchronize Prism, and append a `docs/cross-boundary-log.md` entry for Claude
Code.

## Formal evidence

The complete P13-07 phase passed the local Full preflight (`43/43`) and the immutable
`phase-p13-routing-complete` Delivery Gate at commit
`0c338ee8eef76e470c55515a24728324684365c5`: [run 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495)
completed Authorize, Fast, Full supply-chain and Required successfully in `3s`, `5m57s`, `1m16s`
and `2s`. This contract is closed as `DONE_WITH_BOUNDARY`; it does not claim Provider traffic,
staging/production mutation or the start of P13-08.
