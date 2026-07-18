# BC-AUTH-001 Client Key authentication port

| Field | Value |
|---|---|
| Contract | `BC-AUTH-001` |
| Task | `P1-08` |
| Status | `IN_REVIEW` |
| Domain | Client Key admission authentication |

## Entry and boundary

P1-08 defines one transport-neutral Client Key authentication port in `gateway-auth`:

```text
complete presented Client Key
  -> ClientKeyAuthenticator::authenticate
  -> AuthenticatedClient { ClientKeyId }
  |-> GatewayError(ClientUnauthorized, Request)
```

`AuthenticatedClient` exposes only a stable `ClientKeyId`, never the presented secret. The port
does not decide Access Group policy, model visibility, route selection, quota, rate limits,
credentials, or Provider execution. Those are P2-and-later concerns.

P1's implementation is `InMemoryClientKeyAuthenticator`: an immutable, private `BTreeMap` built
once from enabled/disabled records. A record and authenticator `Debug` output redact complete
keys; configuration rejects empty and duplicate complete keys rather than silently replacing one.

## HTTP admission

`POST /v1/responses` accepts exactly one ASCII `Authorization: Bearer <key>` header. Missing,
multiple, non-ASCII, wrong-scheme, empty, or whitespace-containing Bearer credentials are rejected
before the already-admitted raw request body is interpreted or decoded, before a request context
is allocated, and before any router or Provider method is invoked. `GET /healthz` remains public.

The HTTP parser owns header syntax. `gateway-auth` owns semantic key lookup and has no Actix or
HTTP type dependency. The authenticated P1 identity is retained only as an admission result; P1
does not yet use it for access-policy enforcement.

## Error semantics

- Missing, malformed, unknown, and disabled keys all return the same safe
  `ClientUnauthorized/Request` error envelope with HTTP `401` and `WWW-Authenticate: Bearer`.
  They do not disclose whether a key exists or is disabled.
- The response contains only P1-05's safe error type/code/message/`param: null`; it contains no
  complete key, prefix, record state, or authentication diagnostic.
- Authentication rejection is terminal for the HTTP request. The decoder, metadata factory,
  bounded transport, event source, and Provider are not called.

## P2 replacement seam

P1 intentionally stores generated development/test keys directly in memory and compares them as
ordinary strings. It does **not** implement key issuance, persistence, prefix lookup, HMAC/pepper,
constant-time digest comparison, expiry, revocation store, Access Groups, quotas, or management
API operations.

P2 can replace the in-memory implementation with a persisted, compiled snapshot implementation of
the same `ClientKeyAuthenticator` interface: prefix narrows a candidate, a server-pepper HMAC is
compared in constant time, and the resulting `ClientKeyId` resolves to P2's `ClientKeyView` and
Access Group. HTTP need not import persistence or cryptographic implementation types.

## Corresponding tests

- Unit tests cover valid identity return, identical unknown/disabled safe rejection, invalid
  in-memory configuration, and redaction in `Debug`/configuration diagnostics.
- HTTP E2E covers a valid Bearer request plus missing, malformed, duplicate, unknown, and disabled
  credentials. Rejecting requests use an invalid body to prove authentication occurs before decode,
  and a counting executor proves no Provider execution occurs.
- Existing health E2E proves `/healthz` remains available without credentials.
