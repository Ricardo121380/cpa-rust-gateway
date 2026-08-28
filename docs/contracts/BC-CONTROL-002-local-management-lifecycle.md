# BC-CONTROL-002 Local management lifecycle

| Field | Value |
|---|---|
| Contract | `BC-CONTROL-002` |
| Task | `P2-10` |
| Status | DONE |
| Domain | Local draft creation, validation, publication, rollback, and audit evidence |

## Boundary

`gateway-control::ManagementService` is a management-only, transport-neutral facade around the
typed Repository and immutable Snapshot publisher. It owns its `SQLite` connection and must never
enter a Provider trait, HTTP inference handler, route selection, stream task, or request hot path.
The request path receives only the `RouteSnapshotRegistry` and continues to load a single immutable
`Arc` per request.

```text
typed draft configuration
  -> SQLite graph + config_created audit event (one transaction)
  -> validate: EgressPolicyCompiler + RouteCompiler + RouteSnapshot construction
  -> reserve Snapshot replacement
  -> SQLite: target active / prior active archived + audit event (one transaction)
  -> ArcSwap commit
  -> new requests observe the complete new Snapshot
```

The local CLI is the narrow adapter:

```text
gateway admin create   --db ... --version ... --description ...
gateway admin validate --db ... --version ...
gateway admin publish  --db ... --version ...
gateway admin rollback --db ...
gateway admin audit    --db ...
```

It is not a management HTTP API, does not listen on a port, and does not contact an upstream.

## Lifecycle invariants

- Only a complete `draft` graph can be created through the API. Graph constraints and the
  `config_created` event commit together; an insertion failure leaves neither a partial graph nor
  a creation audit record.
- Validation loads one stored Version, runs static EgressPolicy compilation, P2-06 route
  compilation, and full secret-free Snapshot construction. Validation changes no Config Version
  status, registry value, or audit row.
- Publish prepares a replacement Snapshot before it reserves the registry. Its SQLite transaction
  changes the target to `active`, archives the previous active Version, and appends exactly one
  `config_published` event. Only after that commit succeeds may `ArcSwap` expose the replacement.
  A compiler, registry, activation, or audit failure keeps the persisted active Version and the
  runtime Snapshot unchanged.
- Rollback uses exactly the retained predecessor. It atomically activates that archived Version,
  archives the former current Version, appends exactly one `config_rolled_back` event, and then
  commits the corresponding Snapshot transition. No arbitrary archived Version is selected.
- An audit event exposes only its monotonic ID, action, bounded actor, Unix-millisecond timestamp,
  target Version ID, and optional replaced Version ID. It must not contain a Credential plaintext,
  ciphertext, Master Key, Client Key, digest, URL, request content, or compiler/error detail.
  The audit table rejects ordinary `UPDATE` and `DELETE` operations, preserving append order and
  rollback-predecessor evidence.
- Process bootstrap recompiles the active Version. The most recent successful publication or
  rollback audit record reconstructs its archived predecessor into the one-step rollback slot.
  With no active Version, the registry uses an empty synthetic Snapshot that exposes no client
  traffic; it is never a valid rollback target.

## Local CLI evidence model

- CLI input uses explicit flags and rejects missing, duplicated, or unrelated options. It never
  accepts a whole YAML/JSON overwrite.
- The CLI's default Catalog and Endpoint-capability views are empty. It can safely create and
  publish empty scaffolding Versions, but it must reject a populated Route graph unless an
  embedding supplies genuine immutable compile evidence. It must not fabricate Catalog entries or
  endpoint capabilities just to make a Version publish.
- `--actor` defaults to `local-cli` and is bounded before it reaches SQLite. Audit output contains
  only the safe event metadata above.

## Error and restart semantics

- Missing Config Versions, invalid graph/compiler results, malformed persisted records, unavailable
  clocks, invalid audit metadata, and absent rollback predecessors return typed safe errors. None
  include sensitive stored contents.
- A process restart can restore a prior rollback target only when the audit record names a valid
  archived predecessor. A first publication has no durable predecessor and must reject rollback
  rather than targeting the synthetic bootstrap Snapshot.
- P2 does not construct a runtime HTTP client, resolve DNS at request time, dial an upstream,
  configure proxy/TLS/timeouts, or implement Provider retry behavior.

## Deferred behavior

P10 owns management OpenAPI, remote/local management authentication, authorization, CSRF/CORS,
full structured entity CRUD, configuration read views, UI, and audit querying UI. P4 owns catalog
discovery and persistence; P3 owns actual upstream connection and aggregation behavior. P2-10 does
not advance any P3 work.

## Corresponding tests

- Store migration and repository tests prove management audit rows are version-referential,
  append-ordered, transaction-bound, and reconstruct an archived predecessor.
- Router tests prove a bootstrap-reconstructed predecessor has the same one-step rollback behavior
  as an in-process publication.
- Management service E2E creates two draft Versions, validates/publishes them, restarts from the
  same Repository, rolls back, and verifies five ordered audit events.
- CLI parser tests reject duplicate and unrelated flags; the local CLI smoke exercise validates
  create → validate → publish → publish → fresh-process rollback → audit without network traffic.
