# ADR-0086: Provider-scoped serving lease revalidation

Status: **Accepted — formally gated with P13-07**

## Context

P13-07A introduced a deterministic Provider-scoped selector and P13-07B exposed the same policy
through a read-only Route Explain projection. Both slices deliberately stopped before request-time
lease acquisition. A selector result is an observation, not a reservation: another request can
consume capacity, a credential can expire, or Health/Quota can change between the projection and
the serving path. Reusing an advisory lease or adding a second scheduler would make those races
observable as stale or cross-Provider routing.

The existing `AttemptOrchestrator` already owns request-scoped attempts, retry/max-attempts,
first-semantic-event handling, cancellation and final driver invocation. The existing
`RouteCredentialScheduler` owns the compiled RouteSnapshot, credential pools, exact lease checks,
Health/Quota reads and binding admission. P13-07C must connect the reviewed ranking policy to that
serving path without weakening either ownership boundary.

## Decision

Integrate Provider-scoped ranking as an advisory candidate order followed by exact lease-time
revalidation in the existing scheduler.

- A request first admits its protocol/capability candidates from the pinned RouteSnapshot. If all
  admitted candidates belong to one Provider, the serving path may use the P13-07A selector order.
  Candidate/provider identity is derived from the immutable route candidate's owning `upstream_id`;
  adapter, endpoint, model and request-body values are not Provider identity.
- The selector contributes only an ordered list of opaque candidate IDs. It does not acquire a
  lease, advance the legacy cursor, read Store, refresh credentials, contact a Provider or mutate
  Health/Quota.
- Immediately before each lease, the same `RouteCredentialScheduler`, endpoint credential pool and
  shared Health/Quota registries revalidate the selected candidate at a fresh observation time.
  Expiry, endpoint/candidate/credential/model Health and Quota, binding admission and capacity are
  checked again. A lease race can move to the next already-ranked eligible candidate in the same
  Provider without consuming an attempt or increasing `max_attempts`.
- The lease path remains the sole owner of admission and lease state. An advisory Route Explain
  result or its active-lease count is never reused as a serving reservation.
- A Route with admitted candidates from multiple Providers has no implicit scope in this slice and
  fails closed before lease acquisition or Provider execution. Explicit multi-Provider routing
  policy is deferred to a later task; there is no cross-Provider fallback.
- The existing AttemptOrchestrator continues to decide retry eligibility, first semantic event,
  cancellation, timeout and final error classification. Pre-semantic retry may consider another
  candidate only within the same Provider scope; post-semantic failures remain non-retryable.
- The request continues to use one pinned Config Version. A Version already stale at the executor
  request-start boundary is rejected before selector/lease work; publication after that boundary
  follows the existing pinned in-flight Snapshot contract. A driver/Provider failure drops the
  lease through the existing owner.
- Public Chat/Responses/Messages request shapes, management OpenAPI, Prism contracts and frontend
  routes are unchanged. No Provider request, production deployment, automatic reauth or proxy-pool
  behavior is introduced by this slice.

## Consequences

Serving selection now uses the deterministic quota/load/priority ordering and preserves the
race-safe exact lease semantics already exercised by the scheduler. The optional versioned cost is
still `Unknown` inside this slice because P13-07C does not inject the P13-05 catalog; P13-07D later
adds the exact Config-bound price projection without changing this lease contract, and missing
evidence is never guessed as zero.
A candidate can be rejected after the advisory ranking, which is intentional: live state wins over
an older explanation. A multi-Provider route must wait for an explicit policy rather than silently
changing ownership. The same Provider boundary is therefore visible in management explanations and
in real request admission, without introducing a second source of truth.

## Verification boundary

P13-07C evidence covers selector-driven ordering,
lease races, expiry/Health/Quota/capacity rechecks, candidate/binding exclusions, max-attempts and
first-semantic-event behavior, Config Version staleness, cancellation/driver lease release, and
multi-Provider fail-closed behavior. Tests use fixtures or loopback drivers only. They do not send
a Provider request, change production/server state or refresh credentials.

The complete P13-07 phase passed the local Full preflight (`43/43`) and the immutable
`phase-p13-routing-complete` Delivery Gate at commit
`0c338ee8eef76e470c55515a24728324684365c5`: [run 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495)
completed Authorize, Fast, Full supply-chain and Required successfully in `3s`, `5m57s`, `1m16s`
and `2s`. This acceptance does not claim Provider traffic, staging/production mutation or the
start of P13-08.
