# ADR-0087: Config-bound routing price evidence

Status: **Accepted — P13-07D implementation in progress**

## Context

P13-05 stores immutable billing catalogs whose entries contain six integer rates per one million
tokens: input, output, reasoning, cache read, cache creation and cached tokens. P13-07A reserved an
optional scalar cost field in its Provider-scoped selector, but P13-07B and P13-07C intentionally
left that field unknown. At request start the gateway does not know the final output, reasoning or
cache usage. Adding the six rates, choosing one rate, estimating tokens from bytes, or using an
output limit would therefore turn an unstated workload assumption into an apparently exact request
cost.

Billing catalogs are also global immutable rows. The Config Version used when a catalog is imported
is currently only the protected mutation/revision context; it does not bind that catalog to the
versioned route graph. Selecting the newest catalog during each startup would let the same Config
Version change behavior after a later import and would prevent Config Version rollback from
restoring the previous routing policy.

## Decision

Introduce a version-scoped routing price policy and compare only truthful catalog rate evidence.

- A Config Version may bind exactly one immutable billing catalog and the closed comparison policy
  `rate_dominance_v1`. Absence of a binding means price routing is disabled; it does not mean zero
  price. The protected Route Explain response represents this state as a required nullable
  `price_policy` field (`null` for disabled) and always emits one closed `price_evidence` value per
  candidate.
- The binding is a draft-only, revisioned and audited resource. It is written through the existing
  Management Key, same-origin CSRF, `X-Config-Version` and `If-Match` boundary. Active and archived
  Versions cannot be edited in place. Clearing the binding is explicit and audited.
- A binding may reference only an existing catalog whose `effective_at_ms` is not in the future at
  the management validation boundary. Importing a new catalog never changes an active route. The
  operator must bind it to a draft, validate and publish that Config Version.
- Runtime composition loads only the catalog named by the active Config Version, once. It compiles
  an immutable, bounded candidate-rate map from exact `(Candidate upstream_id, endpoint_id,
  canonical public model)` tuples. It never uses an alias, upstream-model label, adapter, request
  body or credential as pricing identity.
- Serving and management Route Explain share the same scheduler and immutable price map. The hot
  path performs no SQLite read, catalog refresh, token count request or Provider call. A later
  catalog import cannot affect an already composed executor; existing restart/publication
  boundaries remain explicit.
- Missing entries are unpriced and remain eligible. An invalid bound catalog or mismatched Config
  Version fails validation/composition rather than publishing a partial price snapshot.
- Routing uses a six-dimensional `ProviderScopedPriceRates` value, not billing
  `cost_microunits`. It never claims to predict the bill for the current request.

## `rate_dominance_v1`

The selector applies price evidence only after ordinary Provider, capability, Health, Quota,
expiry and capacity eligibility.

For all known eligible rate vectors in the exact Provider scope, it computes the minimum value of
each of the six dimensions. A candidate is `dominant` only when its vector reaches all six minima;
such a vector is no more expensive than every other known candidate for every non-negative usage
vector. If every known vector is identical, their evidence is `equal`. If the dimension minima are
split across candidates, the known vectors are `incomparable` and price does not choose between
them. Known candidates that are strictly above a complete minimum vector are `dominated`. Missing
catalog entries are `unpriced`; candidates rejected before price evaluation are `not_evaluated`.

This is a bounded linear six-dimension pass, not a pairwise partial-order comparator. It therefore
does not create a non-transitive Rust sort order. Within the same price-evidence tier, selection
continues with least-loaded ratio, configured priority, weight and stable opaque identity. Known
evidence may rank ahead of unpriced evidence as a policy-confidence choice, but the UI and contract
must not describe that as an estimated request total.

An explicit all-zero catalog entry is known zero rate evidence. A missing entry, disabled policy,
unknown usage or absent catalog binding is never converted to zero.

## Management and frontend boundary

The public Chat, Responses and Messages protocols do not gain a Provider, catalog or price field.
The protected management API adds one versioned routing-price-policy resource and extends Route
Explain with the bound catalog/comparison plus closed candidate price-evidence categories. It does
not expose token guesses, endpoint URLs, credentials, account billing, headers, bodies or raw
Provider data.

The authoritative OpenAPI document must be synchronized into Prism through its generated contract
command. `docs/cross-boundary-log.md` records that Claude Code should display the exact binding and
evidence categories, finish the already-recorded Provider selector, and never calculate a price
score in frontend code. Formal Prism page work is outside P13-07D.

## Consequences

P13-05 prices can influence real serving order without fabricated token usage or request-time
persistence work. Config Version publication and rollback recover the exact catalog binding. Rate
vectors that cross remain deliberately unordered by price, so the gateway may fall through to
load and configured policy even when a heuristic could have guessed a cheaper provider. That is a
conservative correctness choice.

P13-07D does not start automatic catalog refresh, Provider calls, staging traffic, production
deployment, P13-08, P13-11 or P13-12. A formal phase Gate or bounded canary remains a separate
closeout decision after local implementation and independent review.
