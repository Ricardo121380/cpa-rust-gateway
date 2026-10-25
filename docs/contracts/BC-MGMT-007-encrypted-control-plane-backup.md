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

Capacity extension: [ADR-0132](../adr/ADR-0132-bounded-disk-backed-backup.md).
Disk-backed v2 accepts at most 128 MiB SQLite plaintext / 134250541 artifact bytes,
authenticates the full header, length and each ordered 64 KiB chunk, and requires EOF.
The operator file API publishes only a complete artifact. Management restore/preflight
spools to private anonymous disk and admits one operation through blocking completion;
overlapping operations return 503. Legacy v1 remains readable under its original 16 MiB
limit. Existing Vec APIs remain v1-only. Export/query budgets are unchanged.
An interrupted restore may retain private unpublished staging; the original source is
never replaced and a new gateway must explicitly adopt the verified restored target.

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

## Corresponding tests

- `encrypted_snapshot_round_trips_to_a_new_target_with_schema_and_configuration_intact`
- `wrong_key_tampering_and_existing_target_fail_without_creating_or_replacing_a_database`
- `malformed_and_oversized_material_are_rejected_before_staging_or_restore`
- `chunked_backup_authenticates_order_length_and_legacy_compatibility`
- `chunked_backup_write_failure_and_restore_interruption_preserve_source`
- `chunked_backup_keeps_embedding_read_snapshot_and_transaction`
- `chunked_backup_rejects_oversized_source_before_artifact_publication`
- `cancelled_disk_work_keeps_restore_upload_admission`

The opt-in `scripts/acceptance/cpar75-history-capacity.py` reuses #65's stock-process
and private evidence structure on the frozen schema39 dataset. Its receipt is separate
from historical #65 evidence and from real Provider/client/production qualification.
