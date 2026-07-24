# P11-07 Upgrade and rollback execution plan

| Field | Value |
|---|---|
| Plan version | `v1.45` |
| Task | `P11-07` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` |
| Branch | `codex/p11-release-hardening` |
| Task Card | Rehearse an in-place SQLite upgrade from the immediately preceding schema, an explicit downgrade to that schema, encrypted backup restoration to a separate empty target, and the exact preservation/loss boundary of an old-version rollback. |
| References | [P11 plan](../06-development-plan.md#17-p11---发布加固), [P10-08 backup report](p10-08-encrypted-backup-restore.md), [schema ADR](../adr/ADR-0001-version-scoped-control-plane-schema.md), [backup contract](../contracts/BC-MGMT-007-encrypted-control-plane-backup.md) |

## Required acceptance

1. `gateway-store` can downgrade a supported current migration prefix to one named earlier schema
   version, transaction by transaction, and refuses unknown, future, non-prefix, or upgrade-style
   target history rather than guessing. The existing complete `rollback_all` behavior remains
   available and delegates to the same boundary.
2. A temporary file database at the immediately preceding schema is populated with safe fixture
   control-plane data, migrated in place to the current schema, and passes foreign-key and SQLite
   integrity checks without losing its legacy data.
3. A current-schema encrypted backup is made before downgrade. Rolling back to the preceding
   schema retains data represented by that schema but removes current-only audit data. Restoring
   the backup into a distinct absent target recovers both the latest schema and current-only audit
   record; no active database is overwritten.
4. The report names the downgrade as intentionally lossy for removed schema fields/tables and
   makes the encrypted backup the required recovery route. It must not claim a production
   server/service-manager rehearsal or compatibility with an unbuilt historical binary.

## Implementation and validation sequence

1. Add a typed `rollback_to_version` Store API that validates the target against the exact known
   migration prefix, applies only necessary committed down migrations, and preserves the existing
   full rollback wrapper.
2. Add one `gateway-store` integration drill using only a temporary SQLite directory and fixed
   synthetic backup key. It creates a previous-version database through the real down migration,
   upgrades it, backs it up, downgrades it, validates the expected schema boundary, and restores to
   a new empty target.
3. Run the P11-07 integration suite, `gateway-store` suite, format and focused Clippy. After
   focused checks pass, run this task's one required local Full gate.
4. Write the upgrade/rollback report, independently review target validation, transaction order,
   backup/no-clobber behavior, explicit loss semantics, and all scope exclusions. Completed: the
   local Full gate and docs-only closeout pass; P11-08 may now begin on its own execution plan.

## Explicitly out of scope

No live database, server, systemd/container process, host backup directory, production key,
credential/OAuth/API key, Provider request, public endpoint, deployment, arbitrary-path restore,
or historical binary is used. P12 owns real deployment backups, process rollback and service
manager behavior; P11-08 owns release packaging.
