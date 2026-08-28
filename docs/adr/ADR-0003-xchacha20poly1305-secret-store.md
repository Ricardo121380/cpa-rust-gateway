# ADR-0003: XChaCha20-Poly1305 Secret Store

| Field | Value |
|---|---|
| Status | Accepted |
| Date | `2026-07-19` |
| Task / Matrix / Contract references | `P2-03`; `L34`, `J19`; [BC-SEC-001](../contracts/BC-SEC-001-aead-secret-store.md) |

## Context

P2-01 stores an Upstream Credential as an opaque non-empty SQLite BLOB plus a positive
`key_version`. The control-plane schema deliberately does not define plaintext handling or key
management. The P2 security baseline requires authenticated encryption, a distinct nonce per
stored Secret, key-versioned rotation, and Master Key material outside the database.

## Decision

- `gateway-store` owns a storage-neutral Secret Store boundary. It can seal and open opaque byte
  Secrets but does not add a Repository, a management endpoint, a credential refresh flow, or a
  request-path decrypt operation.
- The AEAD algorithm is `XChaCha20-Poly1305`. Its 24-byte nonce is drawn from the operating
  system randomness source for every seal operation.
- The SQLite `ciphertext` BLOB uses an internal binary envelope:
  `format-version (1 byte) || nonce (24 bytes) || ciphertext-and-tag`. The existing SQLite
  `key_version` column remains outside that envelope. Unknown format versions and truncated
  envelopes fail closed.
- Callers provide associated data (AAD) to both seal and open. A credential service will bind
  this AAD to the stable credential identity before it persists the BLOB, so a ciphertext copied
  to a different logical record does not authenticate.
- A `MasterKeyRing` has one active Key Version for new encryption and may retain older versions
  only for decryption/rotation. Rotation opens an existing envelope with its recorded version and
  reseals it under the active version; it does not mutate SQLite itself.
- Master Keys are exact 32-byte raw files in a dedicated directory outside SQLite. Files are
  named `<positive-decimal-key-version>.key`; the loader accepts only direct regular files with
  canonical names, rejects symlinks and unexpected entries, and requires the configured active
  version to be present. A service manager (for example a systemd credentials directory) supplies
  the directory path; keys are never loaded from the database or written to repository fixtures.
- Plaintext Secret values and Master Keys have redacted `Debug` output and zero their in-memory
  buffers on drop. Errors identify only safe conditions, never key bytes or plaintext.

## Consequences

The existing P2-01 schema needs no migration: its BLOB/key-version pair now has a stable crypto
meaning. Persisting a Credential, binding row identity into AAD, transactional re-encryption,
deployment configuration, and Provider use remain the responsibility of P2-05 and later Tasks.
Backups contain only encrypted envelopes and key versions; recovery still requires the separately
managed Master Key directory.

## Alternatives considered

- `AES-256-GCM` was not selected because XChaCha20-Poly1305 provides a much larger nonce space
  for independently generated per-record nonces while retaining an audited RustCrypto AEAD API.
- A nonce column was not added because P2-01 already owns a single opaque ciphertext BLOB. A
  versioned self-describing envelope permits future format evolution without changing historical
  columns.
- Environment variables and inline configuration were not selected as Master Key sources. A
  dedicated external file/credentials directory works with service-manager credential delivery
  and keeps key material separate from configuration, database backups, and normal process logs.
- A generic `String` return value was not selected because it invites accidental plaintext
  logging; callers receive a redacted, zeroizing byte wrapper instead.

## Validation and rollback

Tests prove independent seal operations use distinct envelopes, correct AAD decrypts, wrong key
material and wrong AAD fail authentication, malformed envelopes fail closed, an external key
directory loads only valid exact-size keys, and an old envelope can be re-encrypted under a new
active version. P2-03 adds no SQLite migration, so its rollback is removal of the unused code
before a later Repository begins storing envelopes. Once persisted records exist, a later
operational rollback must retain every Key Version referenced by retained encrypted rows.
