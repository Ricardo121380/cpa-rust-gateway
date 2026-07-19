# ADR-0004: Client Key HMAC credential

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P2-04`; `L35`, `J19`; [BC-AUTH-002](../contracts/BC-AUTH-002-client-key-hmac-credential.md) |

## Context

P2-02 created a `client_keys` structural table with a unique searchable Prefix and an opaque
exact-32-byte `secret_digest`; it deliberately stores no complete Client Key. P1's live
authenticator remains an in-memory development implementation. The P2 security baseline requires
a key that can be displayed only at creation, a database-safe one-way digest using a separately
managed server Pepper, constant-time verification, expiration, and revocation semantics.

## Decision

- `gateway-auth` owns storage-neutral Client Key issuance and verification primitives. It does not
  add a Repository, management endpoint, database query, or live HTTP authenticator replacement;
  P2-05 and P2-08 own those integrations.
- A generated complete Key is canonical ASCII:
  `rgw_<16 lowercase-hex public-prefix>_<64 lowercase-hex random-secret>`. The Prefix persisted
  in `client_keys.prefix` is the `rgw_<public-prefix>` portion. Eight random bytes produce the
  Prefix and 32 random bytes produce the Secret; both come from the operating-system randomness
  source.
- The database digest is exactly `HMAC-SHA256(client_key_pepper, complete_key_bytes)`. The Pepper
  is an exact 32-byte raw external file, distinct from the P2-03 upstream-Secret Master Key and
  never stored in SQLite, configuration, test fixtures, logs, or error text.
- The verifier parses the canonical form, uses the Prefix to narrow a caller-supplied candidate,
  computes the HMAC over the exact complete Key, and compares the 32-byte digest with
  `subtle` constant-time equality. Active, disabled, revoked, and expiry state are applied only
  after the digest calculation; all non-success cases yield the same false result.
- An issuance result contains a persistable record plus a non-cloneable redacted complete-Key
  wrapper. P2-10 is responsible for displaying the wrapper exactly once in an API/CLI response;
  neither the record nor any `Debug`/error representation contains the complete Key.
- Pepper rotation is explicitly not silently supported in P2-04: replacing it would invalidate all
  existing digests. A dual-Pepper migration requires a later Change Request and an atomic
  reissuance/migration plan.

## Consequences

P2-02's existing `prefix`, `secret_digest`, `status`, and `expires_at_ms` columns now have a
precise cryptographic meaning without a schema migration. Prefix uniqueness remains enforced by
SQLite; a future Repository retries an astronomically unlikely generated Prefix conflict without
ever retaining the complete Key. The P1 in-memory authenticator continues to serve P1 HTTP tests
until P2-08 compiles persisted records into a Snapshot-backed implementation.

## Alternatives considered

- Plaintext or reversibly encrypted Client Keys were rejected because authentication only needs
  comparison and a database compromise must not expose usable client credentials.
- A bare SHA-256 digest was rejected because an external Pepper protects the stored digest from
  offline guessing and separates Client Key security from upstream Secret encryption.
- Password-style adaptive hashing was rejected for the first format because generated 256-bit
  Secrets have high entropy and HMAC permits bounded predictable verification in an immutable
  future Snapshot; it does not protect human-chosen passwords.
- Allowing arbitrary Client Key strings was rejected because a strict canonical shape makes Prefix
  lookup, redaction, validation, and future management behavior deterministic.

## Validation and rollback

Tests prove a generated Key verifies against its record, different/tampered Keys fail, same Prefix
with wrong HMAC material fails, disabled/revoked/expired records fail without a reason oracle,
Pepper loading rejects malformed or symbolic-link files, and debug/error output redacts complete
keys and Pepper bytes. P2-04 does not persist a record, so it has no migration rollback. Once
P2-05 persists HMAC records, rollback must retain the same external Pepper for every retained
Client Key digest.
