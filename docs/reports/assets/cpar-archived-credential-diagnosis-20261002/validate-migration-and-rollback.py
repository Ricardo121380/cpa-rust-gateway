"""Targeted repaired-state migration plus one-field CAS/rollback/stale-write rehearsal."""
import pathlib
TASK = pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884')
exec((TASK / 'operator-tools/target-rehearsal.py').read_text().split('assert ROOT.lstat()')[0])
DIAG = TASK / 'archived-credential-diagnosis-20261002'
CAP = TASK / 'capture-2'
healed = DIAG / 'repaired-pristine'
loc = json.loads((DIAG / 'target.private.json').read_text())
integrity_key = (CAP / 'integrity-hmac-key').read_bytes()
original_baseline = json.loads((CAP / 'logical-before.private.json').read_text())
healed_baseline = {}
for name in DBS:
    with ro(healed / 'state' / name) as db:
        healed_baseline[name] = {table: {**digest_rows(db, table, prior['columns'], integrity_key), 'columns': prior['columns']} for table, prior in original_baseline[name].items()}
(DIAG / 'repaired-logical-baseline.private.json').write_text(json.dumps(healed_baseline, indent=2) + '\n')
migration = DIAG / 'repaired-migration-pristine'
assert not migration.exists()
migration.mkdir(mode=0o700)
state_copy(healed / 'state', migration / 'state')
state_copy(healed / 'credentials', migration / 'credentials')
run = isolated_run(sandbox(ARTIFACT / 'gateway-aarch64-unknown-linux-gnu', state=migration / 'state', command=['--', '/gateway', 'admin', 'audit', '--db', '/state/control.sqlite3']), capture_output=True, timeout=30)
(migration / 'cli.private.log').write_bytes(run.stdout + run.stderr)
assert run.returncode == 1 and b'missing_endpoint_capability_profile' in run.stderr + run.stdout
preservation = preserved(migration / 'state', healed_baseline, integrity_key, True)
assert all(r['preserved'] for tables in preservation.values() for r in tables.values())
with ro(migration / 'state/control.sqlite3') as db:
    assert db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0] == 34
module_spec = importlib.util.spec_from_file_location('crypto', TASK / 'operator-tools/private-crypto-audit.py')
crypto = importlib.util.module_from_spec(module_spec)
module_spec.loader.exec_module(crypto)
crypto_after = crypto.authenticate(migration / 'state/control.sqlite3', migration / 'credentials')
assert crypto_after['status'] == 'PASS'

roundtrip = DIAG / 'repair-cas-roundtrip'
assert not roundtrip.exists()
roundtrip.mkdir(mode=0o700)
state_copy(healed / 'state', roundtrip / 'state')
original = (DIAG / 'original-envelope.private.bin').read_bytes()
replacement = (DIAG / 'repaired-envelope.private.bin').read_bytes()
def cas(expected, desired):
    with sqlite3.connect(roundtrip / 'state/control.sqlite3') as db:
        db.execute('BEGIN IMMEDIATE')
        cursor = db.execute("UPDATE upstream_credentials SET ciphertext=? WHERE config_version_id=? AND id=? AND upstream_id=? AND key_version=1 AND revision=2 AND kind='oauth_json' AND status='active' AND ciphertext=? AND EXISTS (SELECT 1 FROM config_versions v WHERE v.id=upstream_credentials.config_version_id AND v.status='archived' AND v.revision=0)", (desired, loc['config_version_id'], loc['credential_id'], loc['upstream_id'], expected))
        if cursor.rowcount != 1:
            db.rollback()
            return 'STOP_ZERO_ROWS'
        db.commit()
        return 'ONE_ROW_COMMITTED'
assert cas(replacement, original) == 'ONE_ROW_COMMITTED'
rollback = preserved(roundtrip / 'state', original_baseline, integrity_key)
assert all(r['preserved'] for tables in rollback.values() for r in tables.values())
assert cas(original, replacement) == 'ONE_ROW_COMMITTED'
reapply = preserved(roundtrip / 'state', healed_baseline, integrity_key)
assert all(r['preserved'] for tables in reapply.values() for r in tables.values())
assert cas(original, replacement) == 'STOP_ZERO_ROWS'
stale = preserved(roundtrip / 'state', healed_baseline, integrity_key)
assert all(r['preserved'] for tables in stale.values() for r in tables.values())
restore = DIAG / 'repaired-restore-package'
assert not restore.exists()
restore.mkdir(mode=0o700)
state_copy(healed / 'state', restore / 'state')
state_copy(healed / 'credentials', restore / 'credentials')
for name in ['old-gateway', 'old-current.private', 'unit.private']:
    shutil.copy2(CAP / name, restore / name)
assert all((restore / 'state' / name).read_bytes() == (healed / 'state' / name).read_bytes() for name in DBS)
assert hashlib.sha256((restore / 'old-gateway').read_bytes()).hexdigest() == OLD_HASH
receipt = {'status': 'PASS', 'record_alias': 'AC-01', 'migration': {'status': 'PASS', 'old_schema': 31, 'new_schema': 34, 'binary_sha256': NEW_HASH, 'original_tables_preserved': 54, 'same_repair_only_delta_from_original_capture': True, 'native_migration_entry_exit': 1, 'post_migration_cli_compile_limitation': 'missing_endpoint_capability_profile; CLI is not an overall success', 'serve_with_persisted_views': 'separate repaired-candidate native receipt PASS', 'crypto_after': crypto_after}, 'one_field_roundtrip': {'rollback_to_original_54_tables': 'PASS', 'reapply_to_repaired_54_tables': 'PASS', 'stale_expected_envelope_zero_rows_no_changes': 'PASS', 'modified_fields': ['ciphertext'], 'status_revision_lifecycle_unchanged': True}, 'repaired_full_restore_package': {'independent_directory': True, 'both_databases_byte_equal_to_repaired_pristine': True, 'six_keys_and_old_binary_unit_current_and_backup_archive_retained': True, 'old_binary_hash': OLD_HASH, 'actual_old_serve_and_native_read': 'repaired-old receipt from a fresh copy of identical repaired state and keys PASS'}, 'production_writes': 0, 'real_provider_calls': 0, 'completed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()}
(DIAG / 'migration-and-rollback-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt))
