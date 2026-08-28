# ADR-0017: RouteSnapshot-derived public model view and Responses force mapping

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-20` |
| Task / Matrix / Contract references | `P3-07`; `A02`, `B26`, `L27-L31`; [BC-ROUTE-002](../contracts/BC-ROUTE-002-routesnapshot-public-model-view.md) |

## Context

P2-06/P2-07 already compile enabled Public Models, exact Aliases, Access Group Route grants, and
hard-eligible Candidates into an immutable `RouteSnapshot`. P3-03 adds the scheduler over those
same Candidates. The P1 HTTP handler still treats the inbound `model` text as the response model,
which can expose a client Alias rather than the stable Public Model. A separate display list, a
live catalog call, or a read of mutable P3-05 Runtime Health would make `/v1/models` disagree with
routing or flap on a temporary 429/Cooldown/Circuit condition.

Authentication also cannot load one Snapshot and let model lookup reload another: publication
between those reads could authenticate a Client Key against one Access Group while resolving the
request or listing models against another version.

## Decision

- `gateway-router::RouteSnapshot` owns the sole Public Model projection. For an `AccessGroupId`, it
  returns only Public Models whose Route is granted to that group and still contains at least one
  Candidate with hard-eligible Catalog admission and a positive active binding count. The
  projection is ordered by Public Model name, excludes Aliases, and never accepts Runtime Health,
  a repository, YAML, or a live upstream Catalog as input.
- The same Snapshot API resolves an exact Public Model or exact Alias only when that model is
  visible to the authenticated Access Group. An unknown, non-granted, or no-longer-hard-eligible
  name is indistinguishable at the public boundary and maps to `RouteNotFound`.
- `SnapshotClientKeyAuthenticator` gains a pinned authentication result that owns the exact
  `Arc<RouteSnapshot>` used for HMAC admission. It exposes the Access Group-filtered model
  projection and resolution methods while retaining no complete Client Key or Secret. Its existing
  generic `ClientKeyAuthenticator` implementation remains for P1 callers, but P3 HTTP uses the
  pinned form so one request cannot mix Snapshot versions.
- `gateway-http-actix` has an explicit Snapshot-authenticated state constructor. In that mode,
  `GET /v1/models` authenticates first and serializes only Public Model names in a stable
  OpenAI-compatible list envelope. `POST /v1/responses` resolves the same pinned view before it
  starts execution, then gives the resolved Public Model name to both non-streaming and SSE
  Responses encoders. Neither model list nor public response includes an Upstream, Endpoint,
  Credential, Candidate ID, upstream model, or alias text.
- P3's Models endpoint is tied to the existing OpenAI Responses aggregation slice. It does not
  invent cross-protocol compatibility rules, discover models, mutate Catalog state, emit Usage
  records, select a live Credential, or perform a real upstream request. Later protocol-specific
  views must add their own compiler-proven native/lossless compatibility predicate rather than
  weakening this Snapshot-only source of truth.

## Consequences

The authenticated client sees a deterministic, stable list and receives the exact same Public
Model name whether it requested that name directly or through an Alias. A temporary scheduler
exclusion remains a request-time availability decision and cannot remove a model from the list.
Publication can atomically replace the list and mapping for new requests, while an already
authenticated P3 request retains its earlier immutable projection.

The legacy P1 in-memory authenticator remains usable for the existing isolated Mock Responses
tests. It deliberately does not create a P3 model directory; a deployment that enables
`/v1/models` must use the Snapshot-authenticated state constructor.

## Alternatives considered

- Maintaining a second display-oriented model registry was rejected because it can drift from
  Route/Access Group publication and is not needed when the Snapshot already contains the
  compiler-approved projection.
- Filtering `/v1/models` with Runtime Health was rejected because 429/Cooldown/Circuit state is
  transient and would cause client-visible list flapping.
- Echoing the requested Alias in response metadata was rejected because aliases are ingress
  conveniences, not the stable public response identity.
- Authenticating with one Snapshot and loading a current Snapshot again for model lookup was
  rejected because concurrent publication can combine unrelated versions in one request.
- Querying SQLite, YAML, or an upstream `/models` endpoint on the HTTP path was rejected by the
  RouteSnapshot hot-path boundary and would make latency and visibility dependent on control-plane
  availability.

## Validation and rollback

Focused Router tests prove Access Group filtering, hard-eligibility filtering, exact Alias force
mapping, stable name order, and retention of a pinned pre-publication view. In-process Actix tests
prove authenticated `/v1/models`, forbidden-model rejection, and Public Model rewriting in both
completed JSON and typed SSE response events; codec tests prove the list envelope contains only
gateway-owned fields. Fast/Full gates, crate-boundary checks, document links, whitespace checks,
and Secret scans provide the remaining evidence.

Rollback removes the Snapshot projection, pinned model-authentication result, and Models handler.
It does not modify the database schema, compiler admission rules, Client Key HMAC format,
RouteSnapshot publication mechanism, runtime health registry, Provider transport, or any deployed
Endpoint.
