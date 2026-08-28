# BC-ROUTE-001 Versioned route and access schema

| Field | Value |
|---|---|
| Contract | `BC-ROUTE-001` |
| Task | `P2-02` |
| Status | DONE |
| Domain | Versioned Public Model, Route, Access Group, and Client Key persistence |

## Entry and boundary

Migration version 2 extends the P2-01 SQLite control plane. It records structural configuration
only; no request handler, Router, Provider, RouteSnapshot, Repository/service transaction, or
management API reads it in this task. All runtime selection continues to be deferred to later P2
Tasks and must not query SQLite in the inference hot path.

## Data shape

| Table | Identity and relationship | P2-02 responsibility |
|---|---|---|
| `public_models` | `(config_version_id, id)` and unique exact `model_name` | stable public name, status, display name, capabilities object |
| `model_aliases` | `(config_version_id, alias)` → Public Model | exact Alias, never Alias-to-Alias |
| `model_routes` | `(config_version_id, id)` → Public Model, one Route per Public Model | scheduling policy, attempt bound, bootstrap timeout |
| `route_candidates` | `(config_version_id, id)` → Route and Endpoint | upstream model, fixed scope, transform mode, priority, weight, capabilities override |
| `access_groups` | `(config_version_id, id)` and unique name | status and limits object |
| `access_group_routes` | `(config_version_id, access_group_id, route_id)` | enabled Access Group-to-Route permission relation |
| `client_keys` | `(config_version_id, id)` → Access Group | unique prefix and opaque 32-byte digest, status, optional expiry |

## Preconditions

- P2-01 migration version 1 is present before version 2 applies.
- Configuration IDs, labels, model names, aliases, prefixes, and policy strings are bounded,
  non-empty text. JSON configuration fields are JSON objects.
- A Client Key digest is opaque binary storage, not a presented key or a cryptographic operation.

## Invariants

- Every Public Model, Alias, Route, Candidate, Access Group, permission link, and Client Key stays
  in one Config Version. Composite foreign keys reject cross-Version rows.
- The initial set of route policies is exactly `round_robin`, `smooth_weighted_round_robin`, and
  `priority_failover`; transform modes are `passthrough`, `canonical`, and `lossless_bridge`, with
  the native-provider `canonical_bridge` extension. `canonical_bridge` preserves same-protocol
  Canonical admission and explicitly selects the reviewed cross-protocol lossless bridge matrix.
- An Access Group can grant any number of Routes, but one `(access_group, route)` relation appears
  once. A Client Key references exactly one Access Group.
- Candidate scope is exactly `endpoint_bindings` in P2-02. It does not make a Credential selection
  or alter the Endpoint-Credential Binding table.
- No complete Client Key exists in the schema. `secret_digest` must be exactly 32 bytes; P2-04
  determines how it is derived and compared.
- Alias-vs-Public-Model namespace collisions, Candidate catalog/capability validity, endpoint
  enablement, Access Group publication eligibility, expiry enforcement, and all snapshot behavior
  are deferred to P2-04 through P2-10.

## Migration and error semantics

```text
version 1 upstream/control-plane database
  -> enable foreign keys
  -> transactional migration 2
  -> record migration versions 1, 2
  -> version-scoped routing/access graph is writable

invalid duplicate or foreign reference
  -> SQLite UNIQUE / foreign-key write failure
  -> no partial row is admitted
```

`rollback_all` reverses version 2 before version 1 and preserves tables not owned by the Store.
It does not publish, validate, or activate a Config Version.

## Corresponding tests

- A migration-1 database upgrades idempotently to version 2.
- A complete valid Public Model → Alias → Route → Candidate and Access Group → Route → Client Key
  graph succeeds in one Config Version.
- Missing same-version references fail through SQLite foreign-key enforcement.
- Duplicate model names, aliases, Routes per Public Model, Candidates, access relations, Client
  Key prefixes, and Client Key digests fail through uniqueness constraints.
- Unsupported Route policies, unsupported Candidate credential scopes, and an incorrectly sized
  Client Key digest fail through CHECK constraints.
- A populated version-2 graph is removed by rollback without removing caller-owned baseline tables.
