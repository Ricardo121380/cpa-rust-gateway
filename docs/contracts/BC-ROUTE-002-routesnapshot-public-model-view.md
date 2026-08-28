# BC-ROUTE-002 RouteSnapshot public model view and Responses force mapping

| Field | Value |
|---|---|
| Contract | `BC-ROUTE-002` |
| Task | `P3-07` |
| Status | IN_PROGRESS |
| Domain | Access Group-filtered Public Model discovery and stable public response naming |

## Entry and boundary

The P3 path authenticates a Bearer Client Key through
`gateway_router::SnapshotClientKeyAuthenticator` and receives a result that owns the exact
immutable `Arc<RouteSnapshot>` used to admit that Key. The result contains the authenticated
non-secret Client Key identity and its Access Group, and it performs all list/resolution work on
that pinned Snapshot.

`RouteSnapshot` is the only source for `GET /v1/models` and for incoming Models/Aliases used by
`POST /v1/responses`. `gateway-http-actix` serializes the public list and passes only a resolved
Public Model name to `OpenAiResponseMetadata`; protocol encoders do not see a Candidate,
Credential, Endpoint, Upstream, or upstream-model value.

The P1 generic/in-memory authentication constructor remains for pre-P3 isolated tests. It has no
Snapshot-bound model view and therefore is not a valid configuration for the P3 Models endpoint.

## Preconditions

- The Snapshot was built from one compiler-approved Config Version. A retained Candidate represents
  enabled Route/Upstream/Endpoint state, a hard-eligible Catalog admission, and at least one
  active Credential binding at compilation time.
- The authenticated Client Key is active and maps to an active Access Group in that same Snapshot.
- A Public Model has an active Route; the relevant Access Group grant references that Route.
- P3 uses the OpenAI Responses aggregation slice. It does not claim compatibility for a later
  protocol view that has not supplied a compiler-proven native/lossless predicate.

## Required behavior

| Concern | Required behavior |
|---|---|
| Snapshot pinning | Authenticate and derive the model list/resolution from the same owned Snapshot Arc. A later publication changes only new requests. |
| List source | `GET /v1/models` reads only the immutable Snapshot. It makes no SQLite/YAML/catalog/network request and does not create a parallel display list. |
| Access Group | A model appears only when the authenticated Key's Access Group permits its Route. Aliases never appear as list entries. |
| Hard eligibility | A model appears only when its granted Route retains at least one Candidate with `manual`/`fresh`/`stale` Catalog admission (or explicit `allow_unlisted_model`) and a positive active binding count. |
| Runtime separation | 429, short Cooldown, Circuit-open, concurrency saturation, and request-local retry exclusions do not remove a model from the list. Long-lived disablement/catalog expiry is reflected only by a newly compiled Snapshot. |
| Resolution | An exact Public Model name wins over an exact Alias. Both resolve to the stable Public Model name only if the Access Group can see its Route. |
| Response rewrite | Completed Responses JSON and every SSE response object use the resolved Public Model name, never the requested Alias or an upstream model. |
| Public shape | The Models list contains only deterministic gateway-owned OpenAI-compatible model fields; it exposes no Candidate, Endpoint, Upstream, Credential, Catalog, or upstream-model detail. |

## Invariants

- Model names are emitted in Snapshot `BTreeMap` order and are deterministic for one pinned
  version.
- An Alias is an input-only mapping; it cannot alter the public response model or create an extra
  visible model.
- A no-longer-visible name is rejected before executor start. The rejection does not reveal
  whether the name was unknown, forbidden, disabled, or hard-ineligible.
- The Models endpoint does not inspect mutable runtime-health state, advance a scheduler cursor,
  acquire a Credential lease, start a Provider, emit P3-08 events, or write persistence.
- Pinned result `Debug` and all public error/list/response data omit presented Key, HMAC digest,
  Candidate ID, Endpoint ID, Upstream ID, Credential ID, upstream model, URL, and Secret.

## Error semantics

| Condition | Result |
|---|---|
| Missing, malformed, duplicate, disabled, expired, or unknown Bearer Key | Existing `ClientUnauthorized/Request`, HTTP `401`, with `WWW-Authenticate: Bearer`. |
| Snapshot Models mode is not configured | `RouteNotFound/Model`, HTTP `404`; no legacy/authenticator implementation is silently used as a second model source. |
| Requested Public Model/Alias is unknown or not visible to the authenticated Access Group | `RouteNotFound/Model`, HTTP `404`, before executor start. |
| Models list serialization or metadata construction infrastructure failure | Safe `InternalError/Internal`, HTTP `500`; no partial list or response is sent. |

## Corresponding tests

- `gateway-router::route_snapshot::tests` proves ordered Access Group projection, hard-eligible
  filtering, input Alias to Public Model force mapping, and pinned Snapshot retention across
  publication.
- `gateway-http-actix::tests` proves authenticated Models listing, no executor start for listing
  or forbidden/unknown Models, and public-name rewriting in both non-streaming and SSE Responses.
- `protocol-openai-responses::tests` proves the Models list envelope serializes only public names
  and gateway-owned protocol fields.
