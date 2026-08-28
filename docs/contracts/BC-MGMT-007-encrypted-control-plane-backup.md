# BC-MGMT-007 Encrypted control-plane backup and empty-target restore

| Field | Value |
|---|---|
| Contract | `BC-MGMT-007` |
| Task | `P10-08` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Domain | Encrypted SQLite control-plane artifacts, restore compatibility preflight, and empty-machine recovery. |

## Boundary

`gateway-store::backup` owns the cryptographic artifact format and SQLite snapshot/restore work.
It accepts a configured, independently supplied 32-byte Backup Key; it neither reads nor exports
the credential Master Key directory. The backup contains the existing encrypted control-plane
SQLite data, so restoring credentials also requires the original compatible Master Key directory
at subsequent gateway bootstrap. The Backup Key and Master Key are distinct secrets with distinct
purposes and must never be logged, returned by HTTP, stored in the database, committed, or put in
browser storage.

The frozen management API exposes only backup preflight and binary restore material endpoints.
Actual encrypted backup artifact creation is an operator/embedding operation, not a browser
download route. The SPA can ask for preflight and submit a user-selected artifact once; it never
renders artifact bytes, Backup Key, Master Key, plaintext credentials, SQLite rows, or filesystem
paths.

## Artifact and integrity invariants

- Each artifact has a fixed magic/version, a non-secret source schema version, a fresh 24-byte
  nonce, and XChaCha20-Poly1305 ciphertext. The fixed header is authenticated associated data.
- The Backup Key is exactly 32 raw bytes, zeroized on drop, and redacted in `Debug` output.
  Encryption uses operating-system randomness; a failed random or AEAD operation returns no
  partial artifact.
- Artifact size, header format, nonce/ciphertext minimum, schema range, and decrypted SQLite
  snapshot are bounded before use. Tampering, a wrong key, truncated material, malformed SQLite,
  a failed `quick_check`, failed foreign-key check, or an unsupported migration history exposes no
  plaintext or raw SQLite diagnostic through the management boundary.
- Restore preflight decrypts and validates material without changing the target. Its safe result
  states the source schema version, that a quick check is required, and whether the current build
  can support that migration history.
- Restore accepts no caller-selected destination. It can complete only when the configured target
  database does not yet exist. It validates and migrates a same-directory staging file first, then
  atomically creates the previously absent target. Existing target, in-place replacement, partial
  overwrite, provider configuration, endpoint request, egress, and data-plane mutation are
  rejected or out of scope.

## Operator key guidance

1. Keep the Backup Key outside the database and source checkout, with the same access controls as
   the backup artifact.
2. Keep the credential Master Key directory separately. A Backup Key decrypts the artifact; it
   does not replace the Master Key required to decrypt restored credential envelopes.
3. Test recovery only into a new, empty data directory. Do not use the restore endpoint to replace
   an active gateway database; P11 owns in-place upgrade/downgrade and rollback drills.

## Corresponding evidence

- Store tests prove encrypted artifact round-trip, wrong-key/tamper rejection, bounded malformed
  input rejection, current/schema-prefix compatibility, `quick_check` plus foreign-key validation,
  existing-target refusal, and empty-machine restore of a versioned control-plane SQLite file.
- Protected HTTP tests prove default concealment, bounded binary body handling, safe preflight
  projection, incompatible rejection, restore completion, and the fail-closed facade.
- SPA/browser evidence proves generated-client-only operations, no key/artifact persistence, and
  session clearing on reload. P10-09 remains responsible for final static-resource embedding.
