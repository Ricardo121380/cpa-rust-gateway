# BC-ROUTE-009: Config-bound routing price evidence

Status: `IN_PROGRESS`

## Versioned policy contract

A routing price policy belongs to exactly one Config Version and contains exactly one immutable
billing `catalog_version_id` plus the closed comparison value `rate_dominance_v1`. No row means the
policy is disabled. A missing row is distinct from an explicit catalog entry containing six zero
rates.

The protected management resource must require Management Key admission, same-origin CSRF for
writes, `X-Config-Version`, exact `If-Match`, draft status and a durable value-free audit record.
The policy write and Config Version revision increment commit atomically. Active or archived
Versions reject writes. Setting the policy requires an existing catalog whose effective timestamp
is not after the management clock; clearing it is explicit and revisioned.

Importing or rolling forward a billing catalog does not bind it and does not alter active serving.
Publication and rollback retain the exact policy stored with their target Config Version. There is
no implicit latest-catalog lookup.

## Runtime compilation contract

At composition time, the active Config Version and its exact catalog are compiled into one
immutable, secret-free candidate-rate map. The lookup identity is:

1. Provider: the candidate's owning `upstream_id`;
2. Channel: the candidate's exact `endpoint_id`; and
3. Model: the canonical client-visible public model associated with the candidate's Route.

The compiler must not use `upstream_model`, alias text, adapter ID, endpoint label, credential,
request content or historic billing as a substitute. A missing exact tuple produces unpriced
evidence and does not disable an otherwise eligible candidate. A missing/malformed/future bound
catalog, unsupported comparison or Config/Snapshot Version mismatch rejects validation or runtime
composition without publishing a partial map.

Serving and Route Explain use the same scheduler-owned immutable map. After construction, neither
path may read SQLite, select a newer catalog, call a token-count endpoint, contact a Provider, or
estimate usage from bytes or output limits. A request already admitted to an old executor retains
the existing pinned in-flight semantics; a stale executor is rejected at the P13-07C request-start
boundary.

## Price evidence contract

`ProviderScopedPriceRates` contains six non-negative `u64` rates per one million tokens: input,
output, reasoning, cache read, cache creation and cached. It is catalog rate evidence, not the
request's `cost_microunits`.

After ordinary exact-Provider eligibility and known-quota ranking, `rate_dominance_v1` computes the
six coordinate minima across known eligible candidates:

- `dominant`: the candidate reaches every coordinate minimum and at least one known candidate has
  a different vector;
- `equal`: all known eligible vectors are identical;
- `incomparable`: coordinate minima are split and no known candidate reaches them all;
- `dominated`: another complete minimum vector is no greater in all six dimensions;
- `unpriced`: the exact catalog tuple is absent; and
- `not_evaluated`: eligibility rejected the candidate before price comparison.

Only `dominant`/`equal` form the first known price tier. Other known evidence remains ahead of
`unpriced` only as an explicit evidence-confidence policy; it is not described as cheaper unless
dominance proves that statement. Tier ties continue through least-loaded ratio, priority, weight
and stable identity. The implementation must use a bounded linear dimension pass and a total
deterministic sort key, not a pairwise partial-order comparator.

Unknown, unpriced, disabled and not-evaluated values never become zero. Only an exact catalog row
whose six rates are all zero is known zero rate evidence.

## Route Explain and security contract

The protected Route Explain projection always includes a nullable `price_policy` field: `null`
means the policy is disabled, while the object form reports the exact `rate_dominance_v1` binding
and catalog version. It also includes one closed price-evidence value per candidate. It
must use the same price map and Provider scope as serving. It does not return predicted cost,
guessed token counts, raw rate math, account-specific billing, endpoint URL, credential material,
Client Key digest, header, cookie, body or Provider response.

The public `/v1/chat/completions`, `/v1/responses` and `/v1/messages` request/response/SSE shapes do
not change. Clients cannot name a catalog, score or Provider to bypass the versioned route policy.

## Verification contract

Evidence must cover draft set/get/clear, stale revision, CSRF, active-version rejection, audit and
transaction rollback; exact catalog binding and Config rollback; future/missing catalog rejection;
canonical-public-model matching; missing tuple/unpriced and explicit zero; dominant/equal/crossed
vectors; input-order determinism; serving/Explain parity; no hot-path Store/Provider work; P13-07C
lease/retry/FSE/Provider-isolation regressions; authoritative OpenAPI to Prism synchronization; and
secret/value-free output checks. P13-07D remains local-only until a separate phase Gate decision.
