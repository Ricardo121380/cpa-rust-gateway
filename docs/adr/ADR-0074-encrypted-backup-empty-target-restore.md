# ADR-0074 Encrypted backup with empty-target restore

## Status

Accepted and locally verified for P10-08; awaiting the single P10 Phase Delivery Gate.

## Context

The P10 management contract requires encrypted backup, binary restore preflight/operation, schema
visibility, and an empty-machine recovery rehearsal. Its frozen HTTP surface has no backup-download
endpoint and the existing control-plane database may contain encrypted credential envelopes.

## Decision

Use a separate 32-byte Backup Key and XChaCha20-Poly1305 to encrypt a consistent SQLite backup
snapshot. Authenticate a fixed artifact header (magic, format version, source schema version) as
AEAD associated data. Keep Backup Key configuration outside HTTP and browser state; do not reuse
or export the credential Master Key. The artifact preserves credential ciphertext unchanged, so
the original Master Key directory remains an independent post-restore bootstrap requirement.

Restore first decrypts and validates a staging database with `quick_check`, foreign-key integrity,
and supported-migration checks. It is allowed only when the configured destination database is
absent; after staging validation/migration it creates the target atomically. The API returns only
safe schema/compatibility/state projections. Actual artifact creation remains an embedding/operator
operation because the frozen API intentionally has no browser download route.

## Consequences

- Backup material and keys never transit JSON, audit output, browser storage, or a repository
  commit.
- Existing gateway data cannot be overwritten through the restore API; P11 owns in-place rollback
  and migration recovery work.
- An encrypted artifact alone is insufficient for restored credential use: operators retain both
  Backup Key and the credential Master Key directory.
- P10-09 may later embed static UI assets but cannot expand this backup/restore authority.
