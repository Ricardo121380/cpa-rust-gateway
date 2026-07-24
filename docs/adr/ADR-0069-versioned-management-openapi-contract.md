# ADR-0069: Versioned management OpenAPI contract

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-23` |
| Task | `P10-01` |
| Matrix / Contract | `H01-H21`、`J02`、`J08-J09`、`J11-J15`、`J18-J20`; [BC-MGMT-002](../contracts/BC-MGMT-002-versioned-management-openapi.md) |

## Context

P2-10 deliberately supplied a transport-neutral lifecycle facade, and P4 supplied secret-safe
runtime status projections. The planned P10 HTTP control plane must join those pieces without
accidentally making a public inference route into an administrator route, returning a Credential
or Client Key secret, accepting a whole configuration overwrite, or adding ad-hoc endpoints as the
SPA is built.

The P10 implementation is intentionally split: P10-01 freezes the external contract; P10-02 owns
authentication and network admission; P10-03 to P10-08 add the corresponding implementation
families. There is no safe basis to expose an HTTP listener before that separation is explicit.

## Decision

- `docs/openapi/management-v1.json` is the `OpenAPI 3.1` source contract for the management
  surface. It declares no `servers` entry and is marked `contract_only`, so it cannot be read as
  proof that the application listens on an administrator port.
- Every management operation is under `/admin/` and protected by the separate `X-Management-Key`
  scheme. This is deliberately distinct from a client inference key. P10-02 defines the concrete
  verification, only-local/private-network policy, CSRF/CORS, audit identity and Actix wiring.
- Reads and structured graph mutations select an explicit `X-Config-Version`; concurrent graph
  changes require an opaque `If-Match` revision token. Mismatch is a safe `409` with no partial
  graph update. The contract rejects a whole YAML/JSON overwrite and omits arbitrary outbound
  management-call proxy endpoints.
- Upstream, EgressPolicy, Endpoint, Credential, binding, PublicModel, Alias, Route, Candidate,
  AccessGroup, grant and Client Key operations are explicit resources. Runtime diagnostic, OAuth,
  Catalog, audit and backup/restore operations are also named now, but carry their owning future
  P10 Task as metadata rather than implying an implementation exists.
- Credential input has a `write_only` secret; Credential views have only `secret_present` and
  revision metadata. Ordinary Client Key views have only ID, group, prefix, status and expiry.
  A key may appear only in the issuance response marked `display_once`. Errors have stable code
  and bounded message only; no URL, Header, Body, Cookie, token, digest or ciphertext field exists.

## Consequences

P10-02 can implement a narrow, verifiable admin HTTP shell without first inventing resource names
or auth semantics. P10-03's generated client receives one stable operation surface. Later Tasks
must change the contract and its regression test before adding or widening an operation.

The contract does not expose every persisted field blindly. It describes the management-safe
projection and input validation boundary; the typed P2 graph remains the authority for storage and
P4 remains the authority for runtime state. It does not authorize Provider traffic, OAuth login,
backup restore, server configuration, or feature-flag changes.

## Alternatives considered

- Generate the specification from each Actix handler later: rejected because it postpones review of
  the security-critical public contract until after routes and state already exist.
- Reuse client-key authentication for administrators: rejected because operational authority and
  client inference access have different scope, secret lifecycle and network-admission rules.
- Accept full configuration documents through a generic upload endpoint: rejected because it
  bypasses structured revision control, graph validation and audit attribution.
- Offer an arbitrary management HTTP proxy for compatibility: rejected because it expands SSRF and
  secret-exfiltration risk and defeats the exact egress policy boundary.

## Validation and rollback

The P10-01 contract test parses the JSON, verifies required resource/action coverage and separate
Management Key security, requires Config Version + `If-Match` on structured writes, confirms
Secret/Client Key one-way schemas, rejects generic proxy paths, and resolves every local `$ref`.
No port, HTTP listener, database mutation, OAuth request, backup material or Provider request is
part of this Task.

Rollback removes this contract and its tests only. It does not alter P2 SQLite data, active
RouteSnapshot, public `/v1` inference APIs, Credentials, Client Keys, server state or external
traffic.
