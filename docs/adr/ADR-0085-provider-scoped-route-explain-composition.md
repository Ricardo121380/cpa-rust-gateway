# ADR-0085: Provider-scoped Route Explain composition

Status: **Accepted — formally gated with P13-07**

## Context

P13-07A froze a deterministic Provider-scoped ranking policy, but the policy must not become a
second request scheduler. The serving process already owns one immutable `RouteSnapshot`, one
`EndpointCredentialPools` assembly, and the shared Health/Quota registries. A management projection
that opens or compiles another pool could disagree with serving, leak a stale state, or acquire a
lease merely to explain a choice.

## Decision

Compose P13-07A through the existing read-only `RouteCredentialScheduler::explain` boundary.

- P13-07B receives the exact scheduler created for serving, not a reconstructed scheduler or a
  second credential compiler.
- The base Route Explain evaluates hard eligibility, protocol transform admission, expiry,
  binding Health/Quota and current pool capacity at one explicit observation time.
- Only base-eligible candidates explicitly admitted by the management protocol/capability layer
  enter the Provider-scoped selector. Provider identity is derived from the immutable Candidate
  `upstream_id`; adapter labels and endpoint IDs are not Provider identity.
- Optional versioned cost remains absent (`None`) until a catalog is explicitly injected. Quota
  state is `Available` only with retained exact evidence; absent evidence remains `Unknown`.
- A Route Explain caller may provide an exact `provider_id`. If omitted, a unique Provider may be
  inferred; a multi-Provider Route fails closed with `provider_scope_required` rather than picking
  the first candidate. A foreign explicit scope is reported as `provider_mismatch`.
- The HTTP response object is unchanged. Only an optional query parameter and closed reason codes
  are added, so existing single-Provider callers remain compatible.
- The composition never advances candidate/credential cursors, acquires a lease, contacts a
  Provider, refreshes credentials, writes SQLite, or changes serving retry/max-attempt semantics.

## Consequences

Route Explain reflects the same runtime observations and deterministic policy later reused by
P13-07C's serving-path integration, while the actual request path remains on the existing lease
owner. Multi-Provider routes require an explicit operator scope, and the Prism UI must eventually
surface that choice. P13-07C revalidates the selected candidate at lease time because this remains
an advisory snapshot.

## Verification boundary

P13-07B's slice evidence covers router/runtime/HTTP/OpenAPI/Prism contract checks, strict Clippy,
docs and secret/diff review. It does not call a Provider or change production.

The complete P13-07 phase passed the local Full preflight (`43/43`) and the immutable
`phase-p13-routing-complete` Delivery Gate at commit
`0c338ee8eef76e470c55515a24728324684365c5`: [run 31875826495](https://github.com/Ricardo121380/cpa-rust-gateway/actions/runs/31875826495)
completed Authorize, Fast, Full supply-chain and Required successfully in `3s`, `5m57s`, `1m16s`
and `2s`. This acceptance does not claim Provider traffic, staging/production mutation or the
start of P13-08.
