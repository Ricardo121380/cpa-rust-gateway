# ADR-0070: Management HTTP admission boundary

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P10-02` |
| Matrix / Contract | `H01-H02`、`H21`、`J02`、`J08-J09`; [BC-MGMT-003](../contracts/BC-MGMT-003-management-http-admission.md) |

## Context

P10-01 froze the management HTTP names and one-way secret schemas, but intentionally supplied no
listener, authentication, network admission, browser policy or handler. Reusing a data-plane
Client Key for the administrator plane, trusting `X-Forwarded-For` without a defined proxy trust
root, or adding a permissive CORS policy before the UI exists would let a later CRUD handler widen
the attack surface by accident.

The P2 durable audit model has an actor field, but P10-02 must not create synthetic configuration
events or make the inference request path depend on SQLite merely to log an HTTP admission.

## Decision

- `gateway-http-actix::management_security` provides a separately registered `/admin/` Scope
  middleware. P10-02 registers no resource handler itself: P10-04+ must supply only their
  already-frozen OpenAPI routes through `configure_management`, so each future route is guarded
  before a handler is reachable. A missing state fails closed.
- The only accepted administrator secret is exactly one `X-Management-Key` header with a
  `mgmt_`-namespaced value. `Authorization`, `X-Api-Key`, duplicate headers and malformed values
  are not fallbacks. The configured value is `Zeroizing`, has no accessor, redacts `Debug`, and is
  compared in constant time.
- The default policy accepts only the actual loopback peer address. An explicit alternative adds
  RFC1918 IPv4 and IPv6 ULA peers. No forwarded or proxy header is examined; missing peer,
  link-local, carrier-grade NAT and public peers are rejected.
- Browser `Origin` requests are denied by default. A future embedded UI can opt into one exact
  canonical HTTP(S) origin. Its unsafe methods must also supply a separate `csrf_` token; every
  cross-origin request and `OPTIONS` preflight remains denied and no response has CORS allow
  headers. Origin-less local automation remains protected by the network and Management Key gates.
- On successful admission middleware attaches a safe fixed `ManagementActor` (`management-key`)
  as `ManagementRequestPrincipal`. Resource mutations in P10-04+ must pass that actor to the
  existing transactional durable audit path. P10-02 itself creates no audit row because it has no
  resource action.
- All rejection causes share a bounded `404`, `no-store`, non-CORS JSON envelope with no
  authentication challenge or secret-derived detail.

## Consequences

P10-03/P10-04 can add the SPA and individual management operations without inventing credential,
network or browser-admission semantics. The public `/v1/*` shell remains Client-Key authenticated
and does not receive the management state. An operator must explicitly construct and mount the
management state and Scope; no existing binary starts a management listener in P10-02.

Management Key rotation/RBAC, trusted reverse-proxy identity, public exposure through
Caddy/Cloudflare, and persistent access-attempt telemetry remain later operational work. Any such
addition must revise this boundary rather than silently adding an alternate header or origin.

## Alternatives considered

- Reuse `Authorization: Bearer` or Client-Key verification: rejected because data-plane and
  administrator scope, lifecycle and audit semantics differ.
- Trust `X-Forwarded-For`: rejected because P10 defines no trusted proxy identity or header
  stripping chain.
- Enable wildcard CORS before the SPA exists: rejected because the Management Key is a custom
  header and a permissive preflight would turn an administrative browser request into a
  cross-origin capability.
- Persist an audit event for every failed admission: rejected because it creates a write-amplified
  unauthenticated path, requires a durable storage policy not yet declared, and does not represent
  a management operation.

## Validation and rollback

`p10_02_management_security` covers missing/duplicate/wrong-header credentials, actual-peer
loopback/private/public/ULA/link-local admission, ignored forwarding headers, missing state,
same-origin read and unsafe CSRF, cross-origin rejection, CORS absence, bounded denial responses,
safe audit identity, strict configuration, and Debug redaction. It uses only Actix in-process
requests and synthetic secrets.

Rollback removes the Scope helper and its state. It does not mutate SQLite, Config Versions,
Snapshots, public inference routes, Provider traffic, server bind configuration, credentials or
Client Keys.
