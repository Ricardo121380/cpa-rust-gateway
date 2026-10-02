"""Finish with a bounded, read-only reconciliation of the already identified live rotation."""
import pathlib
exec((pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884/archived-credential-production-repair-20261002') / 'repair-common.py').read_text())
pre, post = OPS / 'capture-pre', OPS / 'capture-post'
key = (OPS / 'integrity-hmac-key').read_bytes()
baseline = json.loads((post / 'logical-before.private.json').read_text())
native = [json.loads((OPS / name).read_text()) for name in ['post-old-native-v2.json', 'post-candidate-native.json']]
assert all(value['status'] == 'PASS' and value['all_nine_operations_match_expectation'] and value['target_and_parent_export_equal'] for value in native)
migration = OPS / 'post-migration-pristine'
kept = preserved(migration / 'state', baseline, key, True)
assert sum(len(tables) for tables in kept.values()) == 54 and all(value['preserved'] for tables in kept.values() for value in tables.values())
assert crypto.authenticate(migration / 'state/control.sqlite3', migration / 'credentials')['status'] == 'PASS'
initial = json.loads((OPS / 'pre-post-runtime-drift.private.json').read_text())
initial_event = dict(zip(initial['columns']['management_resource_audit_events'], initial['added']['management_resource_audit_events'][0]))
runtime_before = runtime()
with db_ro(post / 'state/control.sqlite3') as original, db_ro(STATE / 'control.sqlite3') as live:
    original.execute('BEGIN'); live.execute('BEGIN')
    old, new = row_data(original), row_data(live)
    assert exact_context(live) == exact_context(original), 'final_target_parent_config_active_or_transition_drift'
    changed = [table for table in old if old[table] != new[table]]
    assert changed == ['credential_refresh_state', 'management_resource_audit_events', 'upstream_credentials'], 'unexplained_live_table_delta'
    columns = [row[1] for row in live.execute('PRAGMA table_info(upstream_credentials)')]
    ident = lambda row: tuple(row[columns.index(c)] for c in ('config_version_id', 'id', 'upstream_id'))
    old_credentials = {ident(row): row for row in old['upstream_credentials']}
    new_credentials = {ident(row): row for row in new['upstream_credentials']}
    assert old_credentials.keys() == new_credentials.keys()
    rotations = [identity for identity in old_credentials if old_credentials[identity] != new_credentials[identity]]
    assert len(rotations) == 1, 'unexplained_live_credential_delta'
    rotation = rotations[0]
    assert rotation[0] == initial_event['config_version_id'] and rotation[1] == initial_event['resource_id'], 'unrelated_live_credential_delta'
    previous, current = old_credentials[rotation], new_credentials[rotation]
    assert [c for c in columns if previous[columns.index(c)] != current[columns.index(c)]] == ['ciphertext', 'revision']
    assert current[columns.index('kind')] == 'oauth_json' and current[columns.index('status')] == 'active'
    assert rotation[0] == live.execute("SELECT id FROM config_versions WHERE status='active'").fetchone()[0]
    audit_table = 'management_resource_audit_events'
    assert all(row in new[audit_table] for row in old[audit_table]), 'live_audit_history_rewritten'
    added = [row for row in new[audit_table] if row not in old[audit_table]]
    assert 1 <= len(added) <= 3, 'live_reconciliation_bound_exceeded'
    audit_columns = [row[1] for row in live.execute('PRAGMA table_info(' + audit_table + ')')]
    events = [dict(zip(audit_columns, row)) for row in added]
    earliest = datetime.datetime.fromisoformat(json.loads((OPS / 'capture-post-receipt.json').read_text())['completed_at_utc']).timestamp()
    observed = datetime.datetime.now(datetime.timezone.utc).timestamp()
    assert all(event['action'] == 'credential_oauth_rotated' and event['resource_kind'] == 'credential' and event['actor'] == initial_event['actor'] and event['config_version_id'] == rotation[0] and event['resource_id'] == rotation[1] and earliest < event['occurred_at_ms'] / 1000 <= observed for event in events), 'unmatched_live_rotation_audit'
    assert current[columns.index('revision')] == previous[columns.index('revision')] + len(events), 'unmatched_live_rotation_revision'
    refresh_table = 'credential_refresh_state'
    removed = [row for row in old[refresh_table] if row not in new[refresh_table]]
    added_refresh = [row for row in new[refresh_table] if row not in old[refresh_table]]
    assert len(removed) == len(added_refresh) == 1
    refresh_columns = [row[1] for row in live.execute('PRAGMA table_info(' + refresh_table + ')')]
    previous_refresh, current_refresh = dict(zip(refresh_columns, removed[0])), dict(zip(refresh_columns, added_refresh[0]))
    assert [c for c in refresh_columns if previous_refresh[c] != current_refresh[c]] == ['credential_revision', 'observed_at_ms']
    assert current_refresh['config_version_id'] == rotation[0] and current_refresh['credential_id'] == rotation[1]
    assert previous_refresh['credential_revision'] == previous[columns.index('revision')] and current_refresh['credential_revision'] == current[columns.index('revision')]
    assert current_refresh['observed_at_ms'] == max(event['occurred_at_ms'] for event in events)
    # Authenticate only this identified changed active material in its own exact AAD.
    master = bytearray((KEYS / 'master-key').read_bytes())
    envelope = current[columns.index('ciphertext')]
    context = bytearray(b'cpa-rust-gateway/control-plane/credential-aad/v1')
    for value in rotation:
        raw = value.encode(); context.extend(len(raw).to_bytes(4, 'big')); context.extend(raw)
    try:
        assert current[columns.index('key_version')] == 1 and envelope[0] == 1 and len(envelope) >= 41
        nonce = envelope[1:25]
        clear = bytearray(ChaCha20Poly1305(crypto.hchacha(bytes(master), nonce[:16])).decrypt(bytes(4) + nonce[16:], envelope[25:], bytes(context)))
        clear[:] = bytes(len(clear))
    finally:
        master[:] = bytes(len(master))
    assert live.execute('PRAGMA integrity_check').fetchone()[0] == 'ok' and not live.execute('PRAGMA foreign_key_check').fetchall()
    private_write(OPS / 'final-live-drift.private.json', (json.dumps({'rotation_identity': rotation, 'events': events, 'removed_refresh': removed, 'added_refresh': added_refresh, 'live_target_and_parent_context': exact_context(live)}, default=lambda blob: {'sqlite_blob_hex': blob.hex()}) + '\n').encode())
with db_ro(STATE / 'admin-auth.sqlite3') as admin:
    assert all(digest_rows(admin, table, prior['columns'], key) == {k: v for k, v in prior.items() if k != 'columns'} for table, prior in baseline['admin-auth.sqlite3'].items()), 'admin_auth_drift'
    assert admin.execute('PRAGMA integrity_check').fetchone()[0] == 'ok' and not admin.execute('PRAGMA foreign_key_check').fetchall()
assert runtime() == runtime_before, 'final_runtime_drift'
receipt('final-live-check.json', {'status': 'PASS', 'record_alias': 'AC-01', 'final_target_parent_all_columns_and_config_relationships_match_post_capture': True, 'final_target_ciphertext_repaired': True, 'only_identified_other_active_credential_rotated': True, 'rotation_after_post_capture_count': len(events), 'latest_independent_rotation_at_utc': datetime.datetime.fromtimestamp(max(event['occurred_at_ms'] for event in events) / 1000, datetime.timezone.utc).isoformat(), 'rotation_audit_actor_identity_revision_timestamp_and_refresh_match': True, 'rotated_material_exact_aad_authenticated': True, 'unexplained_live_deltas': 0, 'other_control_tables_preserved': len(old) - 3, 'admin_auth_tables_preserved': len(baseline['admin-auth.sqlite3']), 'all_active_configuration_graph_account_identity_binding_route_and_permission_fields_preserved': True, 'production_runtime': runtime_before, 'both_databases_integrity_and_foreign_keys': 'PASS', 'production_writes_in_this_script': 0, 'task_initiated_provider_calls': 0, 'completed_at_utc': utc()})
receipt('post-repair-validation.json', {'status': 'PASS', 'record_alias': 'AC-01', 'production_data_repair': 'PASS', 'operator_writer_transaction_changed_rows': 1, 'operator_writer_transaction_changed_columns': ['ciphertext'], 'other_control_tables_preserved_in_writer_transaction': 52, 'admin_auth_unchanged_by_operator': True, 'pre_post_snapshot_difference': {'operator_ac01_ciphertext_field': 1, 'independent_active_oauth_rotation_before_operator': 1, 'other_original_snapshot_tables_preserved': 51}, 'later_independent_active_rotations_reconciled': len(events), 'target_plaintext_status_kind_revisions_lifecycle_relationships_and_permissions_preserved': True, 'six_keys_old_binary_current_unit_pid_and_listeners_unchanged': True, 'crypto_post_capture': {'ordinary_authenticated': 41, 'ordinary_total': 41, 'archived_authenticated': 32, 'archived_total': 32, 'active_authenticated': 3, 'active_total': 3, 'draft_authenticated': 6, 'draft_total': 6, 'native_grok_authenticated': 5, 'native_grok_total': 5}, 'native_cases': 2, 'native_management_operations': 18, 'native_metadata_export_archived_validate': 'old and candidate 200', 'target_exports_equal_to_parent': True, 'archive_fork_and_publish_still_rejected': True, 'migration': {'status': 'PASS', 'schema_before': 31, 'schema_after': 34, 'original_tables_preserved': 54, 'store_migration_executed_in_retained_v2_attempt': True, 'native_migration_cli_exit': 1, 'cli_followup_limitation': 'missing_endpoint_capability_profile; complete CLI audit is not PASS', 'native_candidate_serve_with_persisted_views': 'PASS', 'ordinary_41_and_native_5_authenticated': True}, 'original_failure_baselines_and_full_fresh_pre_post_restore_packages_retained': True, 'single_field_rollback_rehearsed': 'PASS', 'rollback_executed_in_production': False, 'product_source_modified': False, 'signed_candidate_source': SHA, 'signed_candidate_binary_sha256': NEW_HASH, 'historical_credential_readability_gate': 'PASS', 'deployment': 'NOT_RUN', 'production_service_stop_start': 0, 'production_switches': 0, 'task_initiated_real_provider_calls': 0, 'github_issue_writes': 0, 'C_closed': False, 'issue_57_closed': False, 'parent_9_closed': False, 'completed_at_utc': utc()})
