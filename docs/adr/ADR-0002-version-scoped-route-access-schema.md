# ADR-0002: Version-scoped route and access schema

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P2-02`; `D01/D04/D10/D11/D20/D21/D25/D31`, `H05/H06`, `J18-J20`, `L17-L36`; [BC-ROUTE-001](../contracts/BC-ROUTE-001-versioned-route-access-schema.md) |

## Context

P2-01 established a Config Version-scoped upstream, endpoint, credential, and binding graph.
The next schema layer must describe the client-visible model namespace, routes and Candidates, and
the Access Group/Client Key relations that will later compile into a `RouteSnapshot`. It must keep
all those relationships inside the same Config Version and must not introduce P2-03/P2-04 secret
handling or P2-06 routing behavior.

## Decision

- Add migration version 2 with `public_models`, `model_aliases`, `model_routes`,
  `route_candidates`, `access_groups`, `access_group_routes`, and `client_keys`. The
  `access_group_routes` join table is part of the already-frozen core data model; it expresses the
  many-to-many access policy and is not an additional product feature.
- All rows use Config Version-scoped composite keys. Route, Candidate, Access Group relation, and
  Client Key foreign keys therefore cannot mix entities from different versions.
- A Public Model has one exact model name per Config Version; a Route has one Public Model; an
  Alias has one exact name per Config Version; a Candidate is unique per
  `(route, endpoint, upstream_model, credential_scope)`; and Client Key prefixes and digests are
  separately unique per Config Version.
- P2-02 models the initial Candidate credential scope as the sole literal
  `endpoint_bindings`, meaning the candidate's second scheduling stage considers that Endpoint's
  bindings. This preserves the planned two-stage selection without creating a premature
  Credential Scope service. A later migration is required before adding another scope kind.
- `capabilities_json`, `capability_override_json`, and `limits_json` are JSON objects stored as
  structural configuration only. Their semantic validation and compilation belong to P2-06.
- `client_keys.secret_digest` is a fixed 32-byte opaque digest and the schema has no plaintext-key
  column. P2-04 owns key generation, the HMAC calculation, constant-time verification, redaction,
  expiry enforcement, and revocation behavior.
- Cross-table namespace conflicts (Alias versus Public Model), disabled/unpublishable references,
  catalog/capability validity, and RouteSnapshot publishing remain compiler and management
  validations in P2-06 through P2-10; they are not hidden as P2-02 triggers.

## Consequences

The database can reject orphaned or duplicate structural configuration immediately, while the
future compiler remains the single authority for semantic publishability. The migration runner is
extended to preserve migration 1 as version `1` and add version `2`; changing the public
`CURRENT_SCHEMA_VERSION` must never rewrite the historical migration's version number.

## Alternatives considered

- Storing allowed Route IDs inside `access_groups.limits_json` was rejected because it loses
  foreign-key enforcement and makes policy changes opaque to later compilers.
- A candidate directly selecting one Credential was rejected because it violates the designed
  Candidate-then-Credential two-stage scheduler.
- Storing a complete Client Key for deferred hashing was rejected because it violates the locked
  one-way digest and no-plaintext persistence boundary.
- Implementing cross-table Alias/Public Model collision triggers now was rejected because the
  plan assigns complete publish validation to P2-06; an application trigger would duplicate and
  prematurely freeze compiler semantics.

## Validation and rollback

P2-02 tests will prove migration 1 upgrades to version 2, valid version-scoped routing/access
rows insert successfully, missing references fail as foreign keys, and table-specific uniqueness
constraints reject duplicates. `rollback_all` will remove both migrations in reverse order while
preserving caller-owned baseline tables.
