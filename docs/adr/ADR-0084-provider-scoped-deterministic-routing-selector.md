# ADR-0084: Provider-scoped deterministic routing selector

Status: **Accepted — formally gated with P13-07**

## Context

The existing `RouteCredentialScheduler` correctly owns immutable route-candidate cursors,
exact Health/Quota eligibility, and Credential lease acquisition. P13-05 and P13-06 now expose
enough bounded usage, quota, cost and account-pool observations to make routing policy more
explicit, but adding cost and least-loaded comparisons directly to the request scheduler would
mix policy with cursor/lease side effects and make Provider isolation difficult to audit.

## Decision

Add a side-effect-free `ProviderScopedSelector` policy seam in `gateway-router`.

- The caller supplies only redacted Provider/channel/candidate identities and point-in-time
  priority, weight, active-lease, concurrency, cost, Health, Quota and capability observations.
- The selector accepts one exact Provider scope. A candidate owned by another Provider is always
  rejected and can never become an implicit fallback.
- Fill eligibility is evaluated first: capability mismatch, expiry, non-available Health, blocked
  or recovery-in-flight Quota, and saturated concurrency produce closed rejection reasons.
- Among eligible candidates, known quota evidence outranks unknown evidence; known cost is ordered
  ascending and unknown cost is ordered after known cost, never as zero. Least-loaded ratio is
  compared with wide integer arithmetic, followed by configured priority, weight and stable
  channel/candidate identity tie-breakers.
- The selector does not advance a cursor, acquire a lease, read SQLite, contact a Provider, refresh
  credentials or mutate runtime state. The existing `RouteCredentialScheduler` remains the only
  request-time lease owner.
- Candidate count and opaque identities are bounded; whitespace-only or duplicate candidate
  identities fail before ranking, so the selected opaque ID cannot be ambiguous or depend on
  duplicate input order.

## Consequences

The policy can be tested deterministically and explained without exposing credentials or transport
material. Integrating the policy into serving selection is a separate slice and must preserve the
existing `max_attempts`, first-semantic-event and Config Version snapshot boundaries. Unknown
price/quota observations remain visible as uncertainty rather than being silently treated as a
cheap or unlimited route.

## Verification

P13-07A covers Provider isolation, unknown cost/quota, least-loaded ratio, stable input ordering,
duplicate/identity rejection, closed rejection reasons, finite bounds and overflow-safe
arithmetic. No OpenAPI or Prism contract changed in this slice.

The complete P13-07 phase passed the local Full preflight (`43/43`) and the immutable
`phase-p13-routing-complete` Delivery Gate at commit
`0c338ee8eef76e470c55515a24728324684365c5`: [run 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495)
completed Authorize, Fast, Full supply-chain and Required successfully in `3s`, `5m57s`, `1m16s`
and `2s`. This acceptance does not claim Provider traffic, staging/production mutation or the
start of P13-08.
