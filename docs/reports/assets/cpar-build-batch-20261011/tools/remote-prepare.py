"""Prepare one owned epoch; no Provider operation or production write."""
import ast
import datetime
import fcntl
import hashlib
import json
import os
import pathlib
import shutil
import sqlite3
import stat
import subprocess
import time
import urllib.request

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/six-combinations-20261011'
unit = 'cpar-build-acceptance-20261010.service'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert stat.S_IMODE(base.stat().st_mode) == 0o700
assert not epoch.exists()
assert int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID'])) == 0
assert subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=LoadState'], text=True).strip() == 'not-found'
for family in ['tcp', 'tcp6']:
    assert not any(row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181]
                   for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:])
competing = []
for proc in pathlib.Path('/proc').iterdir():
    if not proc.name.isdigit() or int(proc.name) == os.getpid():
        continue
    try:
        if str(base).encode() in (proc / 'cmdline').read_bytes():
            competing.append(int(proc.name))
        if any(str(fd.resolve()).startswith(str(base)) for fd in (proc / 'fd').iterdir()):
            competing.append(int(proc.name))
    except (PermissionError, FileNotFoundError, ProcessLookupError):
        pass
assert not competing
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
opaque = lambda value: hashlib.sha256(('cpar-evidence:' + str(value)).encode()).hexdigest()
historical = {str(path.relative_to(base)): digest(path)
              for root in [base / 'private/revalidation-20261010', base / 'private/phase2', base / 'private/six-combinations-20261010']
              for path in root.rglob('*') if path.is_file()}
for name in ['private/batch-budget.json', 'private/batch-budget-v2.json', 'private/gateway.log',
             'audit/candidate-egress.jsonl', 'audit/revalidation-20261010.jsonl', 'audit/six-combinations-20261010.jsonl']:
    historical[name] = digest(base / name)
binary = base / 'bin/1b6023f5/gateway-aarch64-unknown-linux-gnu'
assert digest(binary) == 'ea89d5c72628a871f7683f32d0b51a76dc9c588f7172c86345d1c989f1bd9063'
assert digest(base / 'bin/egress-guard-tcp.so') == '91c943c76884a2b674a0b24d5d4876ff40cf61b53c379ffe2d04a062a0dddf06'
source = ast.parse((base / 'private/bootstrap.py').read_text())
functions = ast.Module(body=[node for node in source.body if isinstance(node, ast.FunctionDef)
                            and node.name == 'production'], type_ignores=[])
exec(compile(functions, 'production-proof', 'exec'))
production_before = production()
previous = json.loads((base / 'private/six-combinations-20261010/final-receipt.json').read_text())
assert previous['budget']['stopped'] is True
assert previous['budget']['historical_inference_requests'] + previous['new_pin_actual_sends'] + previous['new_basic_attempts'] == 9
source_pid = int(production_before['MainPID'])
source_args = pathlib.Path('/proc', str(source_pid), 'cmdline').read_bytes().decode().split('\0')
source_state = pathlib.Path(source_args[source_args.index('--state-dir') + 1])
source_credentials = pathlib.Path(source_args[source_args.index('--credential-dir') + 1])
assert str(source_state) == '/var/lib/cpa-rust-gateway'
assert source_args[source_args.index('--management-listen') + 1] == '127.0.0.1:18181'
client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
headers = {'X-Management-Key': (source_credentials / 'management-key').read_text().strip()}
def source_get(path):
    with client.open(urllib.request.Request('http://127.0.0.1:18181' + path, headers=headers), timeout=10) as response:
        assert response.status == 200
        return json.load(response)
production_config = [{key: row[key] for key in ['id', 'revision', 'status']}
                     for row in source_get('/admin/config-versions') if row['status'] == 'active']
with sqlite3.connect('file:' + str(base / 'state/control.sqlite3') + '?mode=ro', uri=True) as old_db:
    old_db.row_factory = sqlite3.Row
    rows = [dict(row) for row in old_db.execute('SELECT * FROM grok_accounts')]
    assert len(rows) == 1
    old_account = rows[0]
    assert opaque(old_account['id']) == '5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54'
    assert old_account['revision'] == 133 and old_account['worker_claim_kind'] is None
    with sqlite3.connect('file:' + str(source_state / 'control.sqlite3') + '?mode=ro', uri=True) as source_db:
        source_db.row_factory = sqlite3.Row
        source_db.execute('BEGIN')
        assert source_db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0] == 31
        account = dict(source_db.execute('SELECT * FROM grok_accounts WHERE id=?', (old_account['id'],)).fetchone())
        assert account['provider'] == 'build' and account['enabled'] == 1 and account['auth_status'] == 'active'
        assert account['revision'] == 134 and account['worker_claim_kind'] is None
        assert account['identity_digest'] == old_account['identity_digest']
        assert account['last_refresh_at_ms'] > old_account['last_refresh_at_ms']
        assert account['continuation_floor_revision'] == old_account['continuation_floor_revision']
        assert account['continuation_current_revision'] == account['revision'] + 1
        assert account['continuation_current_revision'] == old_account['continuation_current_revision'] + 1
        allowed_changes = {'credential_ciphertext', 'refresh_due_at_ms', 'last_refresh_at_ms', 'revision',
                           'import_batch_id', 'updated_at_ms', 'continuation_current_revision'}
        changed = [key for key in account if account[key] != old_account.get(key)]
        assert set(changed).issubset(allowed_changes)
        pools = [row for row in source_get('/admin/operations/provider-account-pools?limit=100')['items']
                 if row.get('account_id') == account['id']
                 and row.get('channel_id') == 'p12-12-production-grok-build-endpoint']
        assert len(pools) == 1 and pools[0]['runtime_status'] == 'available' and pools[0]['active_leases'] == 0
        expiry = pools[0]['expires_at_ms']
        assert expiry > int(time.time() * 1000) + 3600000
        version = account['credential_key_version']
        assert version == old_account['credential_key_version'] == 1
        source_key = (pathlib.Path(source_args[source_args.index('--master-key-dir') + 1]) / '1.key'
                      if '--master-key-dir' in source_args else source_credentials / 'master-key')
        assert source_key.read_bytes() == (base / 'credentials/master-key').read_bytes()
        epoch.mkdir(mode=0o700)
        (epoch / 'state').mkdir(mode=0o700)
        (epoch / 'credentials').mkdir(mode=0o700)
        lock = (epoch / 'execution.lock').open('x')
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        with sqlite3.connect(str(epoch / 'state/control.sqlite3')) as new_db:
            old_db.backup(new_db)
            assert new_db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0] == 34
            columns = {row[1] for row in new_db.execute('PRAGMA table_info(grok_accounts)')}
            assert set(account).issubset(columns)
            fields = [key for key in account if key not in {'id', 'import_batch_id'}]
            new_db.execute('UPDATE grok_accounts SET ' + ','.join(key + '=?' for key in fields) + ' WHERE id=?',
                           [account[key] for key in fields] + [account['id']])
            new_db.commit()
            copied = dict(zip([row[1] for row in new_db.execute('PRAGMA table_info(grok_accounts)')],
                              new_db.execute('SELECT * FROM grok_accounts').fetchone()))
            assert all(copied[key] == value for key, value in account.items() if key != 'import_batch_id')
            counts = dict(new_db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type'))
            assert counts == {'attempt': 1, 'request': 1, 'request_finished': 1, 'usage': 1}
            assert new_db.execute('SELECT count(*) FROM upstream_credentials').fetchone()[0] == 0
            ledger_count = new_db.execute('SELECT count(*) FROM billing_ledger_entries').fetchone()[0]
            assert ledger_count == 1
        source_db.rollback()
    assert [dict(row) for row in old_db.execute('SELECT * FROM grok_accounts')] == rows
for name in ['master-key', 'management-key', 'management-csrf', 'backup-key',
             'client-key-pepper', 'grok-build-cache-key']:
    with (epoch / 'credentials' / name).open('xb') as output:
        output.write((base / 'credentials' / name).read_bytes())
for name in ['artifact-manifest.json', 'gateway-build-metadata.json']:
    shutil.copyfile(base / 'private/revalidation-20261010' / name, epoch / name)
    (epoch / name).chmod(0o600)
assert digest(epoch / 'artifact-manifest.json') == '20ef11961ca40d4dd659ae278c1e7ee629f9f70319f4a4b006d2d8685a5c9777'
assert production() == production_before
assert all(digest(base / name) == value for name, value in historical.items())
receipt = {'status': 'PASS', 'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
           'epoch': str(epoch), 'account_evidence_id': opaque(account['id']),
           'source_account_origin': str(source_state), 'source_schema': 31, 'candidate_schema': 34,
           'old_native_revision': 133, 'copied_native_revision': account['revision'],
           'expected_runtime_revision': account['revision'] + 1, 'matching_key_version': version,
           'normal_refresh_fields_changed': changed, 'identity_digest_unchanged': True,
           'normal_refresh_range_floor': account['continuation_floor_revision'],
           'normal_refresh_range_current': account['continuation_current_revision'],
           'source_last_refresh_at_ms': account['last_refresh_at_ms'], 'expires_at_ms': expiry,
           'source_writes': 0, 'provider_calls': 0, 'oauth_refresh_by_this_task': 0,
           'copied_accounts': 1, 'copied_source_fields_match_except_owned_import_batch': True,
           'inherited_event_counts': counts, 'inherited_ledger_count': ledger_count,
           'historical_file_hashes': historical, 'competing_task_processes': competing,
           'production_before': production_before, 'production_active_configuration': production_config}
(epoch / 'prepare-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
budget = {'schema_version': 4, 'historical_provider_pin_requests': 3, 'historical_basic_requests': 6,
          'historical_inference_requests': 9, 'historical_catalog_requests': 1,
          'new_pin_intents': 0, 'new_basic_intents': 0, 'new_catalog_intents': 0,
          'new_inference_limit': 60, 'cumulative_inference_limit': 69,
          'initial_responses_json_pin_passed': False, 'stopped': False, 'combinations': {},
          'oauth_refresh_reauthorization': 0, 'quota_queries': 0, 'extensions': 0,
          'retries': 0, 'fallbacks': 0, 'reruns_after_failure': 0,
          'authorization': 'new six-combination batch; every Pin and basic request counts; failure/unknown stops all'}
with (epoch / 'budget.json').open('x') as output:
    json.dump(budget, output, indent=2)
print(json.dumps(receipt))
