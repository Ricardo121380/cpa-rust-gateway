# ADR-0005: Versioned control-plane Repository and Service

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P2-05`; `J18-J20`, `L01-L05`, `L17-L35`; [BC-CONTROL-001](../contracts/BC-CONTROL-001-versioned-control-plane-repository-service.md) |

## Context

P2-01 and P2-02 created the complete structural SQLite schema, while P2-03 and P2-04 defined
opaque Secret/Client Key cryptography. No Repository or service transaction has yet connected those
pieces. P2-06 must read a complete version-scoped configuration graph to compile it, but Provider
and inference code must never receive SQLite connections, mutable control-plane entities, plaintext
Secrets, or management transactions.

## Decision

- `gateway-store` owns a `SqliteControlPlaneRepository` and a typed, version-scoped
  `ControlPlaneConfiguration` graph. It maps every P2-01/P2-02 table, keeps Credential ciphertext
  as an `EncryptedSecret`, redacts Client Key digests, and rejects structurally malformed persisted
  crypto records before returning a graph. P2-05 can create and mutate only `draft` graphs; it
  cannot create a database-only `active` Version ahead of P2-07 Snapshot publication.
- All writes use one `ControlPlaneTransaction`. A graph write inserts rows in foreign-key order;
  an error drops the transaction and rolls back every prior row. The Repository provides no
  Provider Adapter, request execution, runtime scheduler, or HTTP surface.
- `gateway-control` owns a management-only `ControlPlaneService`. It binds credential AAD to the
  tuple `(config_version_id, credential_id, upstream_id)`, seals a plaintext credential with the
  P2-03 Secret Store, issues a P2-04 Client Key, and persists both records in one transaction.
  A duplicate client-key write therefore rolls back the new encrypted credential as well.
- Provider-facing crates remain constrained to canonical request/event and later resolved runtime
  views. They do not depend on `gateway-store` or `gateway-control`, and no Repository or
  configuration-graph type is added to a Provider trait or public Provider API.
- Semantic publication checks, Alias/Route/Catalog validation, Snapshot construction, active
  version transition, rollback policy, and management HTTP/CLI are deliberately deferred to
  P2-06, P2-07, and P2-10.

## Consequences

P2-06 obtains one internally consistent persisted graph instead of performing ad-hoc per-table
queries. P2-05 does not make a graph runtime-usable: a draft can be structurally writable yet
unpublishable. Credential creation now has a stable AAD binding and cannot be moved to another
Config Version, Credential ID, or Upstream ID without authentication failure. Client Key Prefix
and digest uniqueness still come from SQLite; a future service retries an improbable generated
Prefix collision without retaining the complete Key.

## Alternatives considered

- Letting Provider implementations query SQLite was rejected because it violates the no-database
  inference-hot-path baseline and couples Provider code to mutable administrative state.
- Defining a Repository trait in `gateway-control` for `gateway-store` to implement was rejected
  because it reverses the locked dependency direction. Store-owned graph types and transaction
  APIs allow Control to depend on Store without a cycle.
- Passing plaintext credentials into a Store write API was rejected. The Service seals them before
  any Repository call and the Store only receives an opaque AEAD envelope.
- Leaving every management write as an independent autocommit was rejected because partial
  credential/key provisioning would leave a draft in an unintelligible state after a later error.

## Validation and rollback

Integration tests write and reload a full version-scoped graph, prove a Credential/Client Key
provisioning transaction rolls back its Credential if a duplicate Client Key fails, and prove the
stored Credential decrypts only with the stable service AAD. Dependency-boundary checks prove
Provider crates do not depend on Store or Control. No schema migration is added; before P2-07
publication, rollback is removal of unused Repository code. After persisted drafts exist, normal
database backups and the external Master Key/Pepper remain jointly required.
