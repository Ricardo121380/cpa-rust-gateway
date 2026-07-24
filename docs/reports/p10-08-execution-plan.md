# P10-08 Encrypted backup and empty-target recovery workflow plan

| Field | Value |
|---|---|
| Plan version | `v1.44` |
| Task | `P10-08` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Date | `2026-07-24` |
| Scope | Encrypted control-plane artifact, restore preflight, empty-target restore, schema compatibility, and safe operator/UI guidance. |
| Inputs | [BC-MGMT-007](../contracts/BC-MGMT-007-encrypted-control-plane-backup.md), [ADR-0074](../adr/ADR-0074-encrypted-backup-empty-target-restore.md), [BC-STORE-001](../contracts/BC-STORE-001-versioned-control-plane-schema.md), P10-01 OpenAPI, and P10-02 admission. |

## Fixed delivery boundary

P10-08 adds a `gateway-store` backup primitive over a consistent SQLite snapshot. It encrypts the
artifact with a configured 32-byte Backup Key using XChaCha20-Poly1305 and authenticates the
format/schema header. It does not export the Master Key or decrypt credential ciphertext. The
backup artifact is returned only to an explicit embedding/operator caller; the frozen management
API deliberately provides no download endpoint.

Preflight decrypts artifact material in bounded storage, runs SQLite integrity/migration checks,
and returns only `schema_version`, `quick_check_required`, and `compatible`. Restore permits only
the configured missing database target, stages/validates/migrates before the final atomic create,
and has no arbitrary destination path or online overwrite path. HTTP uses P10-02 admission and
never returns raw database, AEAD, filesystem, key, compiler, or SQLite errors.

## Explicit exclusions

- No credential Master Key export, Browser Key entry/storage, backup download response, raw
  artifact/audit display, arbitrary filesystem path, active database replacement, provider probe,
  external egress, route publication, or inference-path change.
- P10-09 exclusively owns static-resource embedding and hot-path resource/performance proof.
- P11 exclusively owns in-place migration/downgrade/rollback recovery drills.

## Implementation and verification sequence

1. Implement a typed, bounded Backup Key and authenticated artifact codec plus consistent SQLite
   snapshot/preflight/empty-target restore primitive in `gateway-store`.
2. Add a fail-closed protected P10-08 facade/HTTP routes over the frozen OpenAPI and test safe
   errors, incompatible material, binary limits, and completion state.
3. Add generated-client-only SPA controls for preflight and one-time user-selected restore
   material, with no artifact/key persistence or rendering; update the static policy check.
4. Rehearse backup from a populated source into a separate empty target, validate schema and
   configuration metadata after recovery, then review the crypto/header, migration, staging,
   redaction, target-creation, UI and P10-09/P11 boundaries.
