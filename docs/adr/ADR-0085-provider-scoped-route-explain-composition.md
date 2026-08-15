# ADR-0085: Provider-scoped Route Explain composition

Status: **Accepted — P13-07B local implementation**

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

Route Explain now reflects the same runtime observations and deterministic policy that a future
serving-path integration can reuse, while the actual request path remains on the existing lease
owner. Multi-Provider routes require an explicit operator scope, and the Prism UI must eventually
surface that choice. A later serving integration must revalidate the selected candidate at lease
time because this is an advisory snapshot.

## Verification boundary

P13-07B is local-only: router/runtime/HTTP/OpenAPI/Prism contract checks, strict Clippy, docs and
secret/diff review. It does not call a Provider, change production, or run the expensive formal
Delivery Gate.
