# BC-AUTH-003 Snapshot Client Key authentication

| Field | Value |
|---|---|
| Contract | `BC-AUTH-003` |
| Task | `P2-08` |
| Status | IN_PROGRESS |
| Domain | Version-pinned Client Key HMAC admission from RouteSnapshot |

## Boundary

`gateway-router::RouteSnapshot` contains a Prefix-indexed immutable
`SnapshotClientKeyView` for every Key whose referenced Access Group is active in that Version. A
view contains its non-secret ID, Access Group ID, canonical Prefix, redacted 32-byte HMAC record,
lifecycle/expiry state, and copied allowed Route IDs. It contains no complete Key, Pepper,
`SQLite` connection, `ControlPlaneConfiguration`, Provider, or HTTP type.

`SnapshotClientKeyAuthenticator` implements the existing transport-neutral
`gateway_auth::ClientKeyAuthenticator` port. It performs exactly one `RouteSnapshotRegistry::load`
per call and retains the returned `Arc` while it parses and validates the presented Key. It never
calls `SqliteControlPlaneRepository` or a control-plane service.

```text
Authorization header parser (HTTP)
  -> complete presented Key
  -> SnapshotClientKeyAuthenticator
  -> load one RouteSnapshot
  -> canonical Prefix -> ClientKeyView
  -> ClientKeyService HMAC + constant-time compare + lifecycle
  -> AuthenticatedClient { ClientKeyId, AccessGroupId }
  |-> GatewayError(ClientUnauthorized, Request)
```

## Admission semantics

- A persisted Prefix must be canonical `rgw_<16 lowercase-hex>` and its digest must be exactly 32
  bytes before it can enter a published Snapshot.
- Prefix lookup selects at most one Client Key view. The P2-04 verifier still parses the full Key,
  calculates HMAC over its exact bytes, compares the digest in constant time, and applies active,
  disabled, revoked, and `now < expires_at_ms` lifecycle admission.
- An active Key bound to a disabled Access Group is absent from the runtime map and fails closed.
  An active group with no granted Routes can authenticate, but its resulting view permits no Route.
- Invalid Prefix shape, unknown Prefix, wrong secret, wrong Pepper, disabled/revoked Key, expiry,
  and disabled Access Group all return the same `ClientUnauthorized/Request` result. They never
  include a full Key, Prefix, digest, Key status, Access Group, or verification reason in the error
  envelope.
- The Snapshot authentication path maps clock/HMAC infrastructure failures to a safe request error
  and does not retry against another Snapshot or query persistence.

## Publication and hot updates

- Publication compiles the complete Client Key view before the matching database activation and
  `ArcSwap` commit. A malformed Prefix/digest or a broken active Access Group reference rejects
  publication without changing the current Snapshot.
- A newly started authentication observes the latest published Snapshot. A call that already
  loaded Version A remains internally consistent on Version A even if Version B publishes while
  it is verifying.
- Disabling/revoking a Key, changing its expiry, or changing an Access Group is represented by a
  new Config Version and takes effect through ordinary P2-07 publication/rollback.

## Deferred behavior

P2-08 does not issue or display a Key, rotate Pepper, apply rate limits/quotas, generate
`/v1/models`, choose a Route or Credential, execute a Provider, create a management HTTP/CLI API,
or bootstrap from an existing database. P2-10, P3, and later phases own those capabilities.

## Corresponding tests

- A valid persisted Client Key compiles into a redacted Snapshot view and authenticates to its
  stable Client Key/Access Group identity without a Repository call.
- Publishing a newer Version that disables the Key rejects later authentication while an Arc held
  from the older Version remains unchanged.
- Disabled, revoked, unknown, malformed, wrong-secret, and exact-expiry-boundary attempts share
  the same safe rejection shape.
- Active and disabled Access Group behavior, duplicate Snapshot Prefix/ID rejection, and copied
  Route permission views are covered by deterministic tests.
- Actix `ResponsesHttpState` accepts the real Snapshot authenticator through the unchanged auth
  port and admits a valid `Bearer` request without importing persistence into HTTP.
