# ADR-0008: Snapshot Client Key authentication

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P2-08`; `E01/E02/E17/E20`, `H05/H06`, `J19`, `L35`; [BC-AUTH-003](../contracts/BC-AUTH-003-snapshot-client-key-authentication.md) |

## Context

P1 admits requests through an intentionally temporary in-memory full-Key map. P2-04 defined the
canonical Prefix plus HMAC digest and P2-05 persists those fields per Config Version. P2-07 makes
an immutable `RouteSnapshot` available to the request path but deliberately excluded Client Key
material until a safe live-authentication design existed.

The data path must authenticate without a `SQLite` query, preserve P2-04's canonical parsing,
HMAC, constant-time comparison, disabled/revoked/expiry semantics, and expose the Access Group
needed by later model visibility and routing without returning a complete Key or diagnostic.

## Decision

- `gateway-router::RouteSnapshot` gains a `BTreeMap<ClientKeyPrefix, SnapshotClientKeyView>`.
  A view retains a redacted/zeroizing `ClientKeyRecord`, its `AccessGroupId`, and a stable copied
  set of Routes permitted by that active Access Group. It stores no complete Client Key, Pepper,
  `SQLite` connection, Provider, or HTTP type.
- `gateway-control` extends P2-07's compiler-to-Snapshot conversion with the persisted Client Key
  records. It validates canonical Prefixes, 32-byte HMAC digests, lifecycle/expiry values, and
  active Access Group admission before publishing. A Key bound to a disabled Access Group is not
  admitted to the runtime view, so a later request fails closed.
- `gateway-router::SnapshotClientKeyAuthenticator` implements the existing
  `gateway-auth::ClientKeyAuthenticator` port. It loads the registry once, parses the presented
  Prefix to narrow the view, obtains the current clock, and delegates HMAC plus constant-time
  verification to P2-04's `ClientKeyService`.
- A successful `AuthenticatedClient` includes both its stable Client Key ID and the active Access
  Group ID when produced by the Snapshot implementation. The P1 in-memory test implementation
  retains its existing ID-only behavior so existing protocol tests remain independent of P2
  persistence.
- Unknown, malformed, disabled, revoked, expired, wrong-Pepper, and disabled-Access-Group Key
  attempts share the existing safe `ClientUnauthorized/Request` result. Clock or cryptographic
  infrastructure failure maps to a safe internal request error without exposing Key material.

## Consequences

New requests immediately observe a published Key disablement, expiry policy, Access Group update,
or newly issued Key through `ArcSwap`; neither HTTP nor authentication imports the Repository or
performs a database lookup. An in-flight request keeps the one Snapshot it loaded for its own
authentication decision, just as a stream keeps its routing Snapshot.

This deliberately supersedes P2-07's temporary “no Client Key digest” Snapshot boundary with the
smallest necessary, redacted and zeroizing HMAC view. It does not add rate limiting, quota policy,
model-list generation, Route selection, Client Key issuance UI/API, Pepper rotation, or startup
reconstruction; later P2/P3 work owns those concerns.

## Alternatives considered

- Querying `client_keys` from `SQLite` per request was rejected because it violates the locked
  hot-path database prohibition and makes configuration publication partially visible.
- Keeping the P1 full-Key map alongside a Snapshot was rejected because it creates a second source
  of truth and retains complete Keys in long-lived process memory.
- Letting `gateway-http-actix` parse HMAC records or access the Repository was rejected because
  HTTP owns only header syntax and must remain independent of persistence and Key cryptography.
- Scanning every Key record rather than Prefix-indexing was rejected because it needlessly repeats
  HMAC work and defeats the canonical Prefix design.

## Validation and rollback

Tests cover valid Snapshot authentication, hot publication from active to disabled, expiry at its
exact boundary, identical safe rejection for malformed/unknown/disabled/revoked paths, Access
Group permission projection, compiled persistence conversion, redacted Debug output, and no
Repository use in the authentication path. Rolling back P2-08 removes the Snapshot authenticator
and restores the P1 test authenticator; retained Client Key database rows still require the same
external Pepper.
