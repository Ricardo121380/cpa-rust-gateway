# BC-CONTROL-001 Versioned control-plane Repository and Service

| Field | Value |
|---|---|
| Contract | `BC-CONTROL-001` |
| Task | `P2-05` |
| Status | DONE |
| Domain | Version-scoped configuration Repository, management transactions, and Provider isolation |

## Entry and boundary

`gateway-store` maps one persisted Config Version to a typed `ControlPlaneConfiguration` and
writes changes through `ControlPlaneTransaction`. `gateway-control` invokes those transaction APIs
for management-only services. Neither receives an HTTP request, enters the inference execution
path, or is exposed through a Provider trait. Provider-facing APIs continue to use canonical and
later immutable runtime values, never SQLite connections, Config Version rows, Route candidates,
Client Key digests, or encrypted Credential records.

## Configuration graph and transaction sequence

```text
typed ControlPlaneConfiguration
  -> transaction inserts ConfigVersion
  -> Upstream -> Endpoint/Credential -> Binding
  -> PublicModel -> Alias/Route -> Candidate
  -> AccessGroup -> AccessGroupRoute -> ClientKey
  -> commit only when every write succeeds

credential + client-key provisioning service
  -> construct stable credential AAD(config-version, credential, upstream)
  -> P2-03 AEAD seal plaintext credential
  -> P2-04 issue Client Key and HMAC digest
  -> one ControlPlaneTransaction writes opaque Credential + Client Key row
  -> commit and return one presented Client Key
  |-> any write failure drops transaction; no partial Credential or Client Key row
```

## Invariants

- Repository reads and writes always remain inside one Config Version. Foreign keys and unique
  constraints remain the SQLite admission authority; Repository errors do not reveal ciphertext,
  digest, complete Client Key, or plaintext credential.
- P2-05 admits only `draft` graph creation and draft Credential/Client Key mutations. `active`
  and `archived` transitions remain P2-07 publication/rollback work, so this Repository cannot
  create a database-only "active" version without a matching immutable Snapshot.
- A loaded Credential contains only a validated opaque `EncryptedSecret`; a loaded Client Key
  contains only an exact 32-byte opaque digest. Malformed stored crypto data fails closed before it
  becomes a configuration graph.
- Credential AAD deterministically binds Config Version ID, Credential ID, and Upstream ID using a
  length-delimited internal encoding. The same service must supply exactly that AAD to decrypt;
  copying a ciphertext between these logical records fails authentication.
- The service seals plaintext before Repository entry. The Store transaction never sees plaintext
  credential bytes or a complete Client Key, only opaque AEAD/HMAC artifacts.
- A failed later write rolls back earlier writes from the same transaction. A duplicate Client Key
  must not leave its newly created Credential row behind.
- Provider, Provider-private, protocol, Router, and HTTP crates have no direct dependency on
  `gateway-store` or `gateway-control`; no Provider trait accepts a control-plane graph or
  Repository handle. This preserves the P2 no-SQLite hot-path boundary.
- P2-05 does not validate Alias/Route semantics, publish a Version, construct a Snapshot, run a
  route, expose management HTTP/CLI, or retry Client Key Prefix conflicts.

## Error semantics

```text
invalid graph reference/uniqueness/schema constraint
  -> safe Store error; transaction rolls back

malformed persisted envelope/key version/digest
  -> safe Repository load error; no partial graph

Secret seal, Client Key issuance, cross-Version provisioning request, or Store write failure
  -> safe Service error; no complete Key or plaintext credential returned
```

## Corresponding tests

- A complete Config Version graph writes and reloads through the Repository with all rows scoped to
  its Version and opaque crypto material redacted.
- The Service provisions a Credential and Client Key atomically, reloads the opaque records, and
  decrypts the Credential only using the stable AAD binding.
- A later duplicate Client Key failure rolls back a preceding new Credential in the same service
  transaction; neither partial row becomes visible.
- Mechanical crate-boundary and source tests prove no Provider crate imports Store/Control and no
  request-path component receives a Repository handle.
