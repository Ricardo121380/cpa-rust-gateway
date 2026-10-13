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
import sys
import time
import urllib.request

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/chat-json-linux-20261013'
unit = 'cpar-build-chat-json-linux-20261013.service'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert not base.is_symlink() and base.stat().st_uid == 0 and stat.S_IMODE(base.stat().st_mode) == 0o700
freeze = json.loads((base / 'bin/chat-json-linux-20261013/freeze.json').read_text())
assert freeze['kind'] == 'chat_json_502_diagnostic'
assert freeze['epoch'] == 'chat-json-linux-20261013'
assert len(freeze['source_sha']) == 40
for name, checksum in freeze['files'].items():
    assert '/' not in name and hashlib.sha256((base / 'bin/chat-json-linux-20261013' / name).read_bytes()).hexdigest() == checksum
sys.path.insert(0, str(base / 'bin/chat-json-linux-20261013'))
from qualification import validate_source_pool_revision
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
              for path in base.rglob('*') if path.is_file() and not path.is_symlink()}
binary = base / 'bin/chat-json-linux-20261013/gateway-aarch64-unknown-linux-gnu'
assert digest(binary) == freeze['files']['gateway-aarch64-unknown-linux-gnu']
assert digest(base / 'bin/egress-guard-tcp.so') == '91c943c76884a2b674a0b24d5d4876ff40cf61b53c379ffe2d04a062a0dddf06'
source = ast.parse((base / 'private/bootstrap.py').read_text())
functions = ast.Module(body=[node for node in source.body if isinstance(node, ast.FunctionDef)
                            and node.name == 'production'], type_ignores=[])
exec(compile(functions, 'production-proof', 'exec'))
production_before = production()
previous = json.loads((base / 'private/abd45431-six-20261011/final-receipt.json').read_text())
assert previous['budget']['stopped'] is True
assert previous['budget']['historical_inference_requests'] + previous['budget']['new_pin_intents'] + previous['budget']['new_basic_intents'] == 35
assert previous['budget']['historical_gateway_pin_confirmed_sends'] + previous['new_pin_actual_sends'] + previous['new_basic_attempts'] == 34
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
with sqlite3.connect('file:' + str(base / 'private/abd45431-six-20261011/state/control.sqlite3') + '?mode=ro', uri=True) as continuity_db:
    continuity_db.row_factory = sqlite3.Row
    authorized_account = dict(continuity_db.execute('SELECT * FROM grok_accounts').fetchone())
    assert authorized_account['revision'] >= authorized_account['revision']
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
        assert account['revision'] >= authorized_account['revision'] and account['worker_claim_kind'] is None
        assert account['identity_digest'] and account['identity_digest'] == authorized_account['identity_digest'] == old_account['identity_digest']
        assert account['last_refresh_at_ms'] >= authorized_account['last_refresh_at_ms']
        assert account['continuation_floor_revision'] == authorized_account['continuation_floor_revision'] == old_account['continuation_floor_revision']
        assert account['continuation_current_revision'] == account['revision'] + 1
        assert account['continuation_current_revision'] - authorized_account['continuation_current_revision'] == account['revision'] - authorized_account['revision']
        allowed_changes = {'credential_ciphertext', 'refresh_due_at_ms', 'last_refresh_at_ms', 'revision',
                           'import_batch_id', 'updated_at_ms', 'continuation_current_revision'}
        changed = [key for key in account if account[key] != authorized_account.get(key)]
        assert set(changed).issubset(allowed_changes)
        pools = [row for row in source_get('/admin/operations/provider-account-pools?limit=100')['items']
                 if row.get('account_id') == account['id']
                 and row.get('channel_id') == 'p12-12-production-grok-build-endpoint']
        assert len(pools) == 1 and pools[0]['runtime_status'] == 'available' and pools[0]['active_leases'] == 0
        validate_source_pool_revision(pools[0], account['continuation_current_revision'])
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
            assert json.loads(new_db.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='attempt'").fetchone()[0])['attempt']['outcome'] == 'succeeded'
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
    shutil.copyfile(base / 'bin/chat-json-linux-20261013' / name, epoch / name)
    (epoch / name).chmod(0o600)
assert digest(epoch / 'artifact-manifest.json') == freeze['files']['artifact-manifest.json']
for name in freeze['files']:
    if name in ['gateway-aarch64-unknown-linux-gnu', 'artifact-manifest.json', 'gateway-build-metadata.json']:
        continue
    with (epoch / name).open('xb') as output:
        output.write((base / 'bin/chat-json-linux-20261013' / name).read_bytes())
with (epoch / 'freeze.json').open('x') as output:
    json.dump(freeze, output, indent=2)
# Preserve the exact old synthetic request bytes, without exposing them outside this root.
old_http = json.loads((base / 'private/abd45431-six-20261011/chat-json/http-01-private.json').read_text())
import base64
payload = base64.b64decode(old_http['request_wire_b64'], validate=True)
import importlib.util
spec = importlib.util.spec_from_file_location('diagnostic', epoch / 'cpar_build_json_diagnostic.py')
diagnostic = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostic)
diagnostic.validate_request(json.loads(payload))
with (epoch / 'selected-request.json').open('xb') as output:
    output.write(payload)
(epoch / 'native-capture').mkdir(mode=0o700)
selected_request_sha256 = hashlib.sha256(payload).hexdigest()
assert production() == production_before
assert all(digest(base / name) == value for name, value in historical.items())
receipt = {'status': 'PASS', 'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
           'epoch': str(epoch), 'account_evidence_id': opaque(account['id']),
           'source_account_origin': str(source_state), 'source_schema': 31, 'candidate_schema': 34,
           'old_native_revision': authorized_account['revision'], 'initial_database_native_revision': old_account['revision'], 'copied_native_revision': account['revision'],
           'expected_runtime_revision': account['revision'] + 1, 'matching_key_version': version,
           'normal_refresh_fields_changed': changed, 'identity_digest_unchanged': True,
           'normal_refresh_range_floor': account['continuation_floor_revision'],
           'normal_refresh_range_current': account['continuation_current_revision'],
           'source_last_refresh_at_ms': account['last_refresh_at_ms'], 'expires_at_ms': expiry,
           'source_writes': 0, 'provider_calls': 0, 'oauth_refresh_by_this_task': 0,
           'copied_accounts': 1, 'copied_source_fields_match_except_owned_import_batch': True,
           'inherited_event_counts': counts, 'inherited_ledger_count': ledger_count,
           'historical_file_hashes': historical, 'selected_request_sha256': selected_request_sha256,
           'source_sha':freeze['source_sha'], 'binary_sha256':freeze['files']['gateway-aarch64-unknown-linux-gnu'], 'competing_task_processes': competing,
           'production_before': production_before, 'production_active_configuration': production_config}
(epoch / 'prepare-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
budget = diagnostic.new_budget()
assert not (epoch / 'budget.json').exists()
diagnostic.save_budget(epoch / 'budget.json',budget)
public = {key: value for key, value in receipt.items() if key != 'historical_file_hashes'}
public['preserved_historical_file_count'] = len(historical)
public['historical_inventory_sha256'] = hashlib.sha256(json.dumps(historical, sort_keys=True).encode()).hexdigest()
print(json.dumps(public))
