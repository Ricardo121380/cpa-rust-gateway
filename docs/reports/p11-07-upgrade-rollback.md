# P11-07 Upgrade and rollback report

| Field | Value |
|---|---|
| Plan version | `v1.45` |
| Task | `P11-07` |
| Status | `LOCAL_PASS_PENDING_PHASE_GATE` — implementation, focused verification, Full local gate, and independent review are complete; G11 remains pending P11-08. |
| Branch | `codex/p11-release-hardening` |
| Test boundary | One temporary SQLite directory, fixed synthetic 32-byte backup key, and no network, listener, account, host data path, or historical binary. |

## Rehearsal result

| Stage | Operation | Observed result | Result |
|---|---|---|---|
| Previous-schema baseline | Create a new file database, apply all migrations, then use `rollback_to_version` to reach the immediately preceding schema. | Schema version is `8`; the current-only `management_resource_audit_events` table is absent. | PASS |
| In-place upgrade | Insert a safe legacy `config_versions` record at schema `8`, then run `migrate` in place. | Schema reaches `9`; the legacy record remains readable; `quick_check` and foreign-key check pass. | PASS |
| Current backup | Insert one schema-`9` management audit record and create an encrypted backup before any downgrade. | The backup is an in-memory artifact; no current database is replaced. | PASS |
| Old-version rollback | Downgrade the same source database back to schema `8`. | The legacy configuration remains; migration `0009` removes the audit table and therefore its current-only audit row. Integrity checks pass. | PASS — intentionally lossy boundary recorded |
| Re-upgrade source | Run `migrate` on the downgraded source again. | Schema returns to `9`, but audit count is zero: an up migration creates structure and never fabricates the lost row. | PASS |
| Backup recovery | Restore the pre-downgrade encrypted artifact to a distinct, previously absent target. | The target reaches schema `9`, preserves the legacy configuration and restores the one audit record. Integrity checks pass. | PASS |
| Target admission | Ask a schema-`8` database to rollback to `9`, and request target `-1`. | Both calls return `UnsupportedRollbackTarget`; no upgrade, unknown target, or guessed migration history is accepted. | PASS |

## Implementation boundary

`gateway_store::rollback_to_version` accepts only zero (the unmigrated base) or an exact schema
version known to the current build, and only when that version is no newer than the database's
currently applied supported prefix. It applies each required down migration in its own transaction
and deletes its matching migration record in that transaction. `rollback_all` delegates to the
same function with target zero, preserving the existing complete-development-rollback behavior.

The immediately preceding schema is used intentionally: it exercises the real `0009` down
migration and makes the loss boundary concrete. A direct downgrade is not a backup mechanism:
the removed audit table and its rows are deliberately unavailable to an old schema. The required
operator sequence is therefore **backup while current → downgrade only if necessary → restore the
backup into a new empty target to recover current-only data**.

## Verification record

| Command | Result |
|---|---|
| `cargo test --locked -p gateway-store --test p11_07_upgrade_rollback -- --nocapture` | PASS — complete temporary-file upgrade, downgrade and separate-target backup recovery drill. |
| `cargo test --locked -p gateway-store` | PASS — 31 unit tests plus 5 integration tests, including existing full down-migration, encrypted backup, P11-06 recovery and P11-07 rehearsal coverage. |
| `cargo clippy --locked -p gateway-store --all-targets --all-features -- -D warnings` | PASS. |
| `cargo fmt --all -- --check` and `git diff --check` | PASS. |
| `CHECK_REPORT_PATH=/tmp/p11-07-full-check.md ./scripts/check.sh full` | PASS — 213 seconds total; all 21 workspace packages passed format, Clippy and tests, followed by source/crate policy, document links, tracked Secret scan, pinned quality tools, Cargo policy, and RustSec audit. |

## Independent review

- PASS — `rollback_to_version` validates supported history before considering a target, accepts
  only zero or an exact version from the current migration list, and rejects a target newer than
  the active prefix. It applies every down script and `schema_migrations` deletion in one
  transaction; `rollback_all` delegates with target zero.
- PASS — the drill writes legacy data only after reaching schema `8`, proves in-place migration to
  schema `9`, and executes both `quick_check` and foreign-key validation after every state change.
- PASS — the report accurately labels the migration-`0009` downgrade as lossy. Re-upgrade creates
  only the table, while the pre-downgrade encrypted artifact restores the missing audit event into
  a separate absent target. Existing P10-08 tests retain the no-clobber rejection coverage.
- PASS — no historical binary, deployed service, production database/key, Provider/account,
  network or caller-selected restore path enters this task. P12 retains real process/deployment
  rollback proof.
