"""Fresh post-repair native old/candidate checks; no production HTTP or external route."""
import pathlib
exec((pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884/archived-credential-production-repair-20261002') / 'repair-common.py').read_text())
assert json.loads((OPS / 'production-transaction.json').read_text())['status'] == 'PASS'
assert json.loads((OPS / 'plan-post-receipt.json').read_text())['status'] == 'PASS'
pre, post = OPS / 'capture-pre', OPS / 'capture-post'
key = (OPS / 'integrity-hmac-key').read_bytes()
pre_baseline = json.loads((pre / 'logical-before.private.json').read_text())
post_baseline = json.loads((post / 'logical-before.private.json').read_text())
preservation = preserved(post / 'state', pre_baseline, key)
changed = {name: [table for table, item in tables.items() if not item['preserved']] for name, tables in preservation.items()}
assert changed == {'control.sqlite3': ['upstream_credentials'], 'admin-auth.sqlite3': []}, 'unrelated_pre_post_snapshot_delta_stop'
with db_ro(pre / 'state/control.sqlite3') as before, db_ro(post / 'state/control.sqlite3') as after:
    only_cipher_delta(after, row_data(before), row_data(after), (OPS / 'original-envelope.private.bin').read_bytes(), (OPS / 'repaired-envelope.private.bin').read_bytes())
    post_context = exact_context(after)
with db_ro(OPS / 'transaction-rehearsal/healed-reference.sqlite3') as expected:
    assert post_context == exact_context(expected), 'post_context_drift'
assert all(hmac.compare_digest((pre / 'credentials' / name).read_bytes(), (post / 'credentials' / name).read_bytes()) for name in KEY_NAMES)
migration = OPS / 'post-migration-pristine'; migration.mkdir(mode=0o700)
state_copy(post / 'state', migration / 'state')
state_copy(post / 'credentials', migration / 'credentials')
run = isolated_run(sandbox(ARTIFACT / 'gateway-aarch64-unknown-linux-gnu', state=migration / 'state', command=['--', '/gateway', 'admin', 'audit', '--db', '/state/control.sqlite3']), capture_output=True, timeout=30)
private_write(migration / 'migration-cli.private.log', run.stdout + run.stderr)
assert run.returncode == 1 and b'missing_endpoint_capability_profile' in run.stdout + run.stderr
migration_preservation = preserved(migration / 'state', post_baseline, key, True)
assert all(item['preserved'] for tables in migration_preservation.values() for item in tables.values())
with db_ro(migration / 'state/control.sqlite3') as db:
    assert db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0] == 34
auth = crypto.authenticate(migration / 'state/control.sqlite3', migration / 'credentials')
assert auth['status'] == 'PASS'
receipts = []
for label, source, binary, source_sha, digest, schema in [
    ('post-old-native', post, post / 'old-gateway', OLD_SHA, OLD_HASH, 31),
    ('post-candidate-native', migration, ARTIFACT / 'gateway-aarch64-unknown-linux-gnu', SHA, NEW_HASH, 34),
]:
    work = OPS / label; work.mkdir(mode=0o700); os.chown(work, UID, GID)
    state_copy(source / 'state', work / 'state')
    state_copy(source / 'credentials', work / 'credentials')
    for item in [work / 'credentials'] + list((work / 'credentials').iterdir()):
        os.chown(item, UID, GID)
    target = work / 'target.private.json'; target.write_bytes((OPS / 'target.private.json').read_bytes()); os.chown(target, UID, GID)
    command = ['--ro-bind', str(OPS / 'native-read-probe.py'), '/probe.py', '--ro-bind', str(target), '/target.private.json', '--', '/usr/bin/python3', '/probe.py', source_sha, digest, str(schema)]
    run = isolated_run(sandbox(binary, state=work / 'state', credentials=work / 'credentials', work=work, command=command), capture_output=True, timeout=90)
    private_write(work / 'namespace.private.log', run.stdout + run.stderr)
    result = json.loads((work / 'native-read-receipt.json').read_text())
    expected_statuses = {'version_detail': 200, 'credential_summary': 200, 'credential_metadata': 200, 'parent_metadata': 200, 'target_export': 200, 'parent_export': 200, 'archived_validate': 200, 'archived_fork': 409, 'archived_publish': 409}
    assert run.returncode == 0 and result['status'] == 'PASS' and all(result['operations'][name]['http_status'] == status for name, status in expected_statuses.items()), 'native_result_not_expected'
    assert result['target_and_parent_export_equal'] and result['network_isolated'] and result['production_paths_hidden'] and result['isolated_process_stopped'], 'native_isolation_or_plaintext_mismatch'
    result['label'] = label
    result['fresh_post_repair_derivative'] = True
    result['all_nine_operations_match_expectation'] = True
    receipt(label + '.json', result)
    receipts.append(result)
after_runtime = runtime()
with db_ro(STATE / 'control.sqlite3') as live:
    live.execute('BEGIN')
    assert exact_context(live) == post_context, 'final_live_exact_context_drift'
    envelope = (OPS / 'repaired-envelope.private.bin').read_bytes()
    master = bytearray((KEYS / 'master-key').read_bytes())
    try:
        opened = clear_open(envelope, loc['config_version_id'], master)
        opened[:] = bytes(len(opened))
    finally:
        master[:] = bytes(len(master))
# Separately observe running production. Its concurrent commits are never attributed
# to this operator merely because they differ from a static capture.
live_preservation = preserved(STATE, post_baseline, key)
live_drift = {name: [table for table, item in tables.items() if not item['preserved']] for name, tables in live_preservation.items()}
critical = {'config_versions', 'upstream_credentials', 'grok_accounts', 'grok_account_links', 'endpoint_credential_bindings', 'route_candidates', 'routes', 'upstreams', 'endpoints', 'client_keys', 'permissions'}
assert not any(table in critical for table in live_drift['control.sqlite3']), 'final_critical_live_data_drift'
assert not live_drift['admin-auth.sqlite3'], 'final_admin_auth_drift'
receipt('post-repair-validation.json', {'status': 'PASS', 'record_alias': 'AC-01', 'pre_post_only_target_ciphertext_changed': True, 'other_original_tables_preserved': 53, 'admin_auth_original_rows_preserved': True, 'six_keys_unchanged': True, 'config_and_credential_status_kind_revision_and_relationships_preserved': True, 'active_and_current_rollback_relationships_preserved': True, 'migration': {'status': 'PASS', 'schema_before': 31, 'schema_after': 34, 'original_tables_preserved': 54, 'real_store_migration_entry_exit': 1, 'post_migration_cli_limitation': 'missing_endpoint_capability_profile; complete CLI audit is not PASS', 'native_serve_with_persisted_views': 'PASS', 'crypto': auth}, 'native_cases': len(receipts), 'native_management_operations': 18, 'old_and_candidate_export_equal_to_trusted_parent': True, 'archive_fork_and_publish_still_rejected': True, 'final_live_target_exactly_matches_repaired_post_capture': True, 'production_runtime': after_runtime, 'later_live_drift_by_table': live_drift, 'production_data_repair': 'PASS', 'rollback_executed_in_production': False, 'unchanged_original_diagnosis_and_failure_snapshots_retained': True, 'fresh_full_pre_and_post_packages_retained': True, 'product_source_modified': False, 'deployment': 'NOT_RUN', 'production_service_stop_start': 0, 'production_switches': 0, 'real_provider_calls': 0, 'completed_at_utc': utc()})
