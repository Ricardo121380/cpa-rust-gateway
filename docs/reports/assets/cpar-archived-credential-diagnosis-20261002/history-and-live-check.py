"""Inspect only AC-01/direct parent in retained historical sources and current read-only DB."""
import pathlib
TASK = pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884')
exec((TASK / 'operator-tools/target-rehearsal.py').read_text().split('assert ROOT.lstat()')[0])
from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
DIAG = TASK / 'archived-credential-diagnosis-20261002'
CAP = TASK / 'capture-2'
target = json.loads((DIAG / 'target.private.json').read_text())
module_spec = importlib.util.spec_from_file_location('crypto', TASK / 'operator-tools/private-crypto-audit.py')
crypto = importlib.util.module_from_spec(module_spec)
module_spec.loader.exec_module(crypto)
master = bytearray((CAP / 'credentials/master-key').read_bytes())

def db_ro(path, immutable=False):
    connection = sqlite3.connect(f'file:{path}?mode=ro' + ('&immutable=1' if immutable else ''), uri=True)
    connection.row_factory = sqlite3.Row
    connection.execute('BEGIN')
    return connection

def aad(config):
    data = bytearray(b'cpa-rust-gateway/control-plane/credential-aad/v1')
    for field in (config, target['credential_id'], target['upstream_id']):
        raw = field.encode()
        data.extend(len(raw).to_bytes(4, 'big'))
        data.extend(raw)
    return bytes(data)

def check_envelope(envelope, config):
    nonce = envelope[1:25]
    try:
        plain = bytearray(ChaCha20Poly1305(crypto.hchacha(bytes(master), nonce[:16])).decrypt(b'\0' * 4 + nonce[16:], envelope[25:], aad(config)))
        plain[:] = b'\0' * len(plain)
        return 'PASS'
    except InvalidTag:
        return 'AEAD_INVALID_TAG'

def utc(ms):
    return datetime.datetime.fromtimestamp(ms / 1000, datetime.timezone.utc).isoformat() if ms is not None else None

def exact_row(db, config):
    return db.execute('SELECT * FROM upstream_credentials WHERE config_version_id=? AND id=? AND upstream_id=?', (config, target['credential_id'], target['upstream_id'])).fetchone()

try:
    with db_ro(CAP / 'state/control.sqlite3', True) as captured:
        parent_id = captured.execute('SELECT parent_id FROM config_versions WHERE id=?', (target['config_version_id'],)).fetchone()[0]
        original = exact_row(captured, target['config_version_id'])
        parent = exact_row(captured, parent_id)
        history = []
        for item in json.loads((DIAG / 'backup-index.private.json').read_text()):
            if item['alias'] not in ('backup-10', 'backup-4'):
                continue
            file = pathlib.Path(item['path'])
            with db_ro(file, True) as backup:
                target_row, parent_row = exact_row(backup, target['config_version_id']), exact_row(backup, parent_id)
                entry = {'alias': item['alias'], 'file_mtime_utc': datetime.datetime.fromtimestamp(file.stat().st_mtime, datetime.timezone.utc).isoformat(), 'source_schema_version': backup.execute('SELECT max(version) FROM schema_migrations').fetchone()[0], 'latest_lifecycle_audit_utc': utc(backup.execute('SELECT max(occurred_at_ms) FROM management_audit_events').fetchone()[0]), 'target_present': target_row is not None, 'parent_present': parent_row is not None}
                if target_row:
                    entry['target_envelope_identical_to_current_capture'] = target_row['ciphertext'] == original['ciphertext']
                    entry['target_exact_context'] = check_envelope(target_row['ciphertext'], target['config_version_id'])
                if parent_row:
                    entry['parent_envelope_identical_to_current_capture'] = parent_row['ciphertext'] == parent['ciphertext']
                    entry['parent_kind_status_revision_key_version_match'] = all(parent_row[c] == parent[c] for c in ('kind', 'status', 'revision', 'key_version'))
                    entry['parent_exact_context'] = check_envelope(parent_row['ciphertext'], parent_id)
                    entry['failed_envelope_with_parent_context'] = check_envelope(original['ciphertext'], parent_id)
                history.append(entry)
        baseline = json.loads((CAP / 'logical-before.private.json').read_text())
        original_states = []
        for label, directory, migrated in [('capture-2', CAP, False), ('migration-entry-probe', TASK / 'migration-entry-probe', True), ('restored-pristine', TASK / 'restored-pristine', False)]:
            preservation = preserved(directory / 'state', baseline, (CAP / 'integrity-hmac-key').read_bytes(), migrated)
            assert all(r['preserved'] for tables in preservation.values() for r in tables.values())
            with db_ro(directory / 'state/control.sqlite3', True) as prior:
                assert exact_row(prior, target['config_version_id'])['ciphertext'] == original['ciphertext']
            original_states.append({'label': label, 'all_54_original_tables_preserved': True, 'failed_envelope_identical': True})
        assert hashlib.sha256((CAP / 'state/control.sqlite3').read_bytes()).hexdigest() == json.loads((DIAG / 'inspection.json').read_text())['source_db_sha256']

    pid = int(subprocess.check_output(['systemctl', 'show', 'cpa-rust-gateway.service', '--value', '-p', 'MainPID']))
    key_directory = pathlib.Path('/etc/cpa-rust-gateway/credentials')
    loaded = pathlib.Path(f'/proc/{pid}/root/run/credentials/cpa-rust-gateway.service')
    all_keys_match = all(hmac.compare_digest((key_directory / n).read_bytes(), (CAP / 'credentials' / n).read_bytes()) and hmac.compare_digest((key_directory / n).read_bytes(), (loaded / n).read_bytes()) for n in KEY_NAMES)
    binary_matches = hashlib.sha256(pathlib.Path(f'/proc/{pid}/exe').read_bytes()).hexdigest() == OLD_HASH
    current_matches = str(pathlib.Path('/opt/cpa-rust-gateway/current').resolve()) == (CAP / 'old-current.private').read_text().strip()
    unit_matches = subprocess.check_output(['systemctl', 'cat', 'cpa-rust-gateway.service']) == (CAP / 'unit.private').read_bytes()
    assert all_keys_match and binary_matches and current_matches and unit_matches
    with db_ro('/var/lib/cpa-rust-gateway/control.sqlite3') as live:
        live_target, live_parent = exact_row(live, target['config_version_id']), exact_row(live, parent_id)
        live_config = live.execute('SELECT parent_id,status,revision FROM config_versions WHERE id=?', (target['config_version_id'],)).fetchone()
        active = live.execute("SELECT id FROM config_versions WHERE status='active'").fetchone()[0]
        latest = live.execute("SELECT config_version_id,replaced_config_version_id FROM management_audit_events WHERE action IN ('config_published','config_rolled_back') ORDER BY id DESC LIMIT 1").fetchone()
        live_result = {'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'production_pid': pid, 'old_binary_current_unit_unchanged': True, 'six_persistent_loaded_and_capture_keys_match': True, 'master_key_version': live_target['key_version'], 'target_all_columns_identical_to_capture': tuple(live_target) == tuple(original), 'parent_all_columns_identical_to_capture': tuple(live_parent) == tuple(parent), 'target_configuration_lifecycle': live_config['status'], 'target_configuration_revision': live_config['revision'], 'direct_parent_unchanged': live_config['parent_id'] == parent_id, 'target_exact_context': check_envelope(live_target['ciphertext'], target['config_version_id']), 'target_envelope_with_parent_context': check_envelope(live_target['ciphertext'], parent_id), 'target_in_active_configuration': active == target['config_version_id'], 'same_credential_id_in_active_configuration': live.execute('SELECT count(*) FROM upstream_credentials WHERE config_version_id=? AND id=?', (active, target['credential_id'])).fetchone()[0], 'target_is_current_rollback_predecessor': latest['replaced_config_version_id'] == target['config_version_id'], 'associated_route_candidates_in_archived_configuration': live.execute('SELECT count(*) FROM route_candidates r JOIN endpoint_credential_bindings b ON b.config_version_id=r.config_version_id AND b.endpoint_id=r.endpoint_id WHERE b.config_version_id=? AND b.credential_id=?', (target['config_version_id'], target['credential_id'])).fetchone()[0], 'target_refresh_state_count': live.execute('SELECT count(*) FROM credential_refresh_state WHERE config_version_id=? AND credential_id=?', (target['config_version_id'], target['credential_id'])).fetchone()[0]}
        assert live_result['target_all_columns_identical_to_capture'] and live_result['parent_all_columns_identical_to_capture']
    listeners = subprocess.check_output(['ss', '-H', '-ltnp'], text=True)
    assert not any(':' + str(port) + ' ' in line for port in (28180, 28181) for line in listeners.splitlines())
    output = {'status': 'PASS', 'record_alias': 'AC-01', 'historical_cross_checks': history, 'original_pristine_states': original_states, 'capture_database_byte_unchanged': True, 'current_read_only_check': live_result, 'isolated_listeners_remaining': 0, 'production_writes': 0, 'plaintext_and_raw_ids_exported': False}
    (DIAG / 'history-and-live-check.json').write_text(json.dumps(output, indent=2) + '\n')
    print(json.dumps(output))
finally:
    master[:] = b'\0' * len(master)
