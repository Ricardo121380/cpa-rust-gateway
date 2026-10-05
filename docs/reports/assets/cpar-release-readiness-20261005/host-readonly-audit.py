"""Read-only, host-resident reconciliation. No material, raw row or key digest output.

Uses already rehearsed host helpers solely for reads and AEAD authentication.
No receipt() / private_write(), service command, Provider request or SQLite writer.
"""
import pathlib
import sys
sys.dont_write_bytecode = True
_task = pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884')
exec((_task / 'archived-credential-production-repair-20261002/repair-common.py').read_text())

def alias(values):
    return hashlib.sha256(('cpar-evidence:' + '|'.join(str(v) for v in values)).encode()).hexdigest()

def delta(old, new, columns, primary):
    positions = [columns.index(c) for c in primary]
    index = lambda rows: {tuple(row[i] for i in positions): row for row in rows}
    if not positions:
        return {'before': len(old), 'now': len(new), 'old_rows_missing': sum(row not in new for row in old), 'new_rows': sum(row not in old for row in new)}
    a, b = index(old), index(new)
    shared = a.keys() & b.keys()
    changed = [k for k in shared if a[k] != b[k]]
    groups = {}
    for k in changed:
        fields = tuple(c for i, c in enumerate(columns) if a[k][i] != b[k][i])
        groups[fields] = groups.get(fields, 0) + 1
    result = {'before': len(old), 'now': len(new), 'removed_primary_keys': len(a.keys() - b.keys()), 'added_primary_keys': len(b.keys() - a.keys()), 'changed_records': len(changed), 'changed_field_groups': [{'fields': list(f), 'records': n} for f, n in sorted(groups.items())]}
    if 'revision' in columns:
        i = columns.index('revision')
        result['revision_changes'] = [{'identity_evidence_id': alias(k), 'before': a[k][i], 'now': b[k][i]} for k in changed]
    return result

out = {'observed_at_utc': utc(), 'scope': 'read_only_host_resident_no_provider', 'production_writes': 0, 'service_changes': 0, 'task_initiated_provider_calls': 0, 'secret_output': False}
out['runtime'] = runtime()
out['crypto'] = crypto.authenticate(STATE / 'control.sqlite3', KEYS)
with db_ro(OPS / 'capture-post/state/control.sqlite3') as prior, db_ro(STATE / 'control.sqlite3') as live:
    prior.execute('BEGIN'); live.execute('BEGIN')
    out['ac01'] = {'record_alias': 'AC-01', 'context_all_columns_matches_post_repair': exact_context(prior) == exact_context(live)}
    columns = [r[1] for r in live.execute('PRAGMA table_info(upstream_credentials)')]
    target = exact_context(live)['target']
    out['ac01']['ciphertext_equals_retained_repaired_envelope'] = hmac.compare_digest(target[columns.index('ciphertext')], (OPS / 'repaired-envelope.private.bin').read_bytes())
    old, new = row_data(prior), row_data(live)
    changes, unchanged = {}, []
    for table in sorted(old):
        if old[table] == new.get(table):
            unchanged.append(table); continue
        schema = list(live.execute('PRAGMA table_info(' + q(table) + ')'))
        cols = [r[1] for r in schema]
        pk = [r[1] for r in sorted(schema, key=lambda r: r[5]) if r[5]]
        changes[table] = delta(old[table], new.get(table, []), cols, pk)
        if 'occurred_at_ms' in cols:
            i = cols.index('occurred_at_ms'); added = [r for r in new[table] if r not in old[table]]
            times = [r[i] for r in added if isinstance(r[i], int)]
            if times: changes[table]['added_time_bounds_utc'] = [datetime.datetime.fromtimestamp(t / 1000, datetime.timezone.utc).isoformat() for t in [min(times), max(times)]]
        if 'action' in cols:
            i = cols.index('action'); actions = {}
            for r in new[table]:
                if r not in old[table]: actions[r[i]] = actions.get(r[i], 0) + 1
            changes[table]['added_action_counts'] = actions
        if 'event_kind' in cols:
            i = cols.index('event_kind'); kinds = {}
            for r in new[table]:
                if r not in old[table]: kinds[r[i]] = kinds.get(r[i], 0) + 1
            changes[table]['added_event_kind_counts'] = kinds
    out['logical_comparison_to_20261002_post_repair'] = {'unchanged_tables': unchanged, 'changed_tables': changes, 'new_tables': sorted(new.keys() - old.keys())}
    def dictionaries(table, rows):
        names = [r[1] for r in live.execute('PRAGMA table_info(' + q(table) + ')')]
        return [dict(zip(names, row)) for row in rows]
    natives_before = dictionaries('grok_accounts', old['grok_accounts'])
    natives_after = dictionaries('grok_accounts', new['grok_accounts'])
    old_native = {r['id']: r for r in natives_before}
    new_native = {r['id']: r for r in natives_after}
    native_deltas = []
    for ident in old_native.keys() & new_native.keys():
        a,b = old_native[ident],new_native[ident]
        if a == b: continue
        native_deltas.append({'account_evidence_id': alias([ident]), 'provider': b['provider'], 'revision_before': a['revision'], 'revision_now': b['revision'], 'status_preserved': a['auth_status'] == b['auth_status'], 'enabled_preserved': a['enabled'] == b['enabled'], 'identity_and_key_version_preserved': a['identity_digest'] == b['identity_digest'] and a['credential_key_version'] == b['credential_key_version'], 'latest_refresh_at_utc': datetime.datetime.fromtimestamp(b['last_refresh_at_ms'] / 1000, datetime.timezone.utc).isoformat(), 'continuation_range_current_equals_revision': b['continuation_current_revision'] == b['revision'], 'timestamps_monotonic': b['last_refresh_at_ms'] >= a['last_refresh_at_ms'] and b['updated_at_ms'] >= a['updated_at_ms']})
    gen_before = dictionaries('native_account_inventory_generation', old['native_account_inventory_generation'])[0]['generation']
    gen_now = dictionaries('native_account_inventory_generation', new['native_account_inventory_generation'])[0]['generation']
    out['native_refresh_reconciliation'] = {'accounts': native_deltas, 'generation_before': gen_before, 'generation_now': gen_now, 'generation_delta_equals_native_revision_delta': gen_now - gen_before == sum(r['revision_now'] - r['revision_before'] for r in native_deltas)}
    refresh_before = dictionaries('credential_refresh_state', old['credential_refresh_state'])
    refresh_now = dictionaries('credential_refresh_state', new['credential_refresh_state'])
    index_refresh = lambda rows: {(r['config_version_id'], r['credential_id']): r for r in rows}
    ra,rb = index_refresh(refresh_before),index_refresh(refresh_now)
    out['refresh_bookkeeping'] = [{'account_evidence_id': alias([identity[1]]), 'credential_revision_before': ra[identity]['credential_revision'], 'credential_revision_now': rb[identity]['credential_revision'], 'failure_count_before': ra[identity]['failure_count'], 'failure_count_now': rb[identity]['failure_count'], 'error_category_unchanged': ra[identity]['last_error'] == rb[identity]['last_error'], 'no_claim': rb[identity]['claim_id'] is None, 'retry_after_monotonic': (rb[identity]['retry_after_ms'] or 0) >= (ra[identity]['retry_after_ms'] or 0), 'observed_at_utc': datetime.datetime.fromtimestamp(rb[identity]['observed_at_ms'] / 1000, datetime.timezone.utc).isoformat()} for identity in ra.keys() & rb.keys() if ra[identity] != rb[identity]]
    ca = dictionaries('model_catalog_targets', old['model_catalog_targets'])
    cb = dictionaries('model_catalog_targets', new['model_catalog_targets'])
    ix = lambda rows: {(r['config_version_id'], r['endpoint_id']): r for r in rows}
    a,b = ix(ca),ix(cb)
    out['catalog_refresh_reconciliation'] = [{'target_evidence_id': alias(identity), 'snapshot_before': a[identity]['snapshot_version'], 'snapshot_now': b[identity]['snapshot_version'], 'latest_observed_at_utc': datetime.datetime.fromtimestamp(b[identity]['observed_at_ms'] / 1000, datetime.timezone.utc).isoformat(), 'timestamps_monotonic': b[identity]['observed_at_ms'] >= a[identity]['observed_at_ms'], 'model_rows_and_failure_rows_unchanged': old['model_catalog_models'] == new['model_catalog_models'] and old['model_catalog_failures'] == new['model_catalog_failures']} for identity in a.keys() & b.keys() if a[identity] != b[identity]]
    if changes.get('upstream_credentials'):
        audit_cols = [r[1] for r in live.execute('PRAGMA table_info(management_resource_audit_events)')]
        events = [dict(zip(audit_cols, row)) for row in new['management_resource_audit_events'] if row not in old['management_resource_audit_events']]
        ident_fields = ['config_version_id', 'id', 'upstream_id']
        idx = [columns.index(c) for c in ident_fields]
        index = lambda rows: {tuple(row[i] for i in idx): row for row in rows}
        before, after = index(old['upstream_credentials']), index(new['upstream_credentials'])
        rotations = []
        for identity in before.keys() & after.keys():
            a,b = before[identity],after[identity]
            if a == b: continue
            relevant = [e for e in events if e.get('action') == 'credential_oauth_rotated' and e.get('config_version_id') == identity[0] and e.get('resource_id') == identity[1]]
            diff = [c for i,c in enumerate(columns) if a[i] != b[i]]
            rotations.append({'identity_evidence_id': alias(identity), 'changed_fields': diff, 'revision_before': a[columns.index('revision')], 'revision_now': b[columns.index('revision')], 'matching_rotation_audits': len(relevant), 'revision_delta_equals_audit_count': b[columns.index('revision')] - a[columns.index('revision')] == len(relevant), 'latest_rotation_at_utc': datetime.datetime.fromtimestamp(max(e['occurred_at_ms'] for e in relevant) / 1000, datetime.timezone.utc).isoformat() if relevant else None, 'target_or_parent': identity[1] == loc['credential_id']})
        out['ordinary_rotation_reconciliation'] = rotations
with db_ro(OPS / 'capture-post/state/admin-auth.sqlite3') as prior, db_ro(STATE / 'admin-auth.sqlite3') as live:
    prior.execute('BEGIN');live.execute('BEGIN')
    out['admin_auth_logical_unchanged'] = row_data(prior) == row_data(live)
out['unit_properties'] = dict(line.split('=',1) for line in subprocess.check_output(['systemctl','show',SERVICE,'-p','TimeoutStopUSec','-p','KillSignal','-p','KillMode','-p','Restart','-p','NRestarts','-p','ExecStart'],text=True).splitlines())
out['listener_addresses'] = [line.split()[3] for line in subprocess.check_output(['ss','-lntp'],text=True).splitlines()[1:] if 'pid=499070,' in line]
out['established_service_connections'] = sum('pid=499070,' in line for line in subprocess.check_output(['ss','-ntp','state','established'],text=True).splitlines()[1:])
out['artifact_files'] = [{'name': p.name, 'bytes': p.stat().st_size, 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted((TASK / 'artifact-542a3c30aae7').iterdir()) if p.is_file()]
out['retained_recovery_paths'] = {str(p.relative_to(TASK)): {'exists': p.exists(), 'mode': oct(stat.S_IMODE(p.stat().st_mode)) if p.exists() else None, 'uid': p.stat().st_uid if p.exists() else None, 'gid': p.stat().st_gid if p.exists() else None} for p in [CAP, TASK / 'restored-pristine', TASK / 'migration-entry-probe', OPS / 'capture-pre', OPS / 'capture-post', OPS / 'post-migration-pristine', TASK / 'operator-tools/target-rehearsal.py', TASK / 'operator-tools/private-crypto-audit.py']}
out['backup_root'] = {str(p): {'exists': p.exists(), 'mode': oct(stat.S_IMODE(p.stat().st_mode)) if p.exists() else None, 'free_bytes': shutil.disk_usage(p if p.exists() else p.parent).free} for p in [pathlib.Path('/var/backups'), pathlib.Path('/var/backups/cpa-rust-gateway'), TASK]}
out['completed_at_utc'] = utc()
print(json.dumps(out, indent=2))
