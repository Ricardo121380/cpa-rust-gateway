import ast
import datetime
import fcntl
import hashlib
import importlib.util
import json
import os
import pathlib
import sqlite3
import stat
import subprocess
import sys
import time
import urllib.request

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/chat-json-502-20261011'
unit = 'cpar-build-acceptance-20261010.service'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert stat.S_IMODE(base.stat().st_mode) == 0o700
lock = (epoch / 'execution.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
assert not (epoch / 'start-receipt.json').exists()
assert int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID'])) == 0
assert subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=LoadState'], text=True).strip() == 'not-found'
for family in ['tcp', 'tcp6']:
    assert not any(row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181]
                   for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:])
freeze = json.loads((epoch / 'freeze.json').read_text())
prepare = json.loads((epoch / 'prepare-receipt.json').read_text())
source_sha = freeze['source_sha']
binary_hash = freeze['files']['gateway-aarch64-unknown-linux-gnu']
binary = base / 'bin/chat-json-502-20261011/gateway-aarch64-unknown-linux-gnu'
guard = base / 'bin/egress-guard-tcp.so'
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
assert digest(binary) == binary_hash
assert digest(guard) == '91c943c76884a2b674a0b24d5d4876ff40cf61b53c379ffe2d04a062a0dddf06'
assert digest(epoch / 'artifact-manifest.json') == freeze['files']['artifact-manifest.json']
opaque = lambda value: hashlib.sha256(('cpar-evidence:' + str(value)).encode()).hexdigest()
old = json.loads((base / 'private/final-receipt.json').read_text())
historical = json.loads((epoch / 'prepare-receipt.json').read_text())['historical_file_hashes']
with sqlite3.connect('file:' + str(epoch / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    db.row_factory = sqlite3.Row
    accounts = [dict(row) for row in db.execute('SELECT * FROM grok_accounts')]
    assert len(accounts) == 1
    account = accounts[0]
    assert opaque(account['id']) == old['pin']['credential_evidence_id']
    assert account['provider'] == 'build' and account['revision'] == prepare['copied_native_revision']
    assert account['enabled'] == 1 and account['auth_status'] == 'active' and account['worker_claim_kind'] is None
    assert db.execute('SELECT count(*) FROM upstream_credentials').fetchone()[0] == 0
    assert dict(db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type')) == json.loads((epoch / 'prepare-receipt.json').read_text())['inherited_event_counts']
    management_audit_before = list(db.execute('SELECT id FROM management_resource_audit_events ORDER BY id'))
source = ast.parse((base / 'private/bootstrap.py').read_text())
functions = ast.Module(body=[node for node in source.body if isinstance(node, ast.FunctionDef) and node.name == 'production'], type_ignores=[])
exec(compile(functions, 'production-proof', 'exec'))
production_before = production()
assert production_before == json.loads((epoch / 'prepare-receipt.json').read_text())['production_before']
source_pid = int(production_before['MainPID'])
source_args = pathlib.Path('/proc', str(source_pid), 'cmdline').read_bytes().decode().split('\0')
source_credentials = pathlib.Path(source_args[source_args.index('--credential-dir') + 1])
with sqlite3.connect('file:/var/lib/cpa-rust-gateway/control.sqlite3?mode=ro', uri=True) as db:
    db.row_factory = sqlite3.Row
    original = dict(db.execute('SELECT * FROM grok_accounts WHERE id=?', (account['id'],)).fetchone())
    assert all(account.get(name) == value for name, value in original.items() if name != 'import_batch_id')
version = account['credential_key_version']
assert version == 1
source_key = (pathlib.Path(source_args[source_args.index('--master-key-dir') + 1]) / '1.key'
              if '--master-key-dir' in source_args else source_credentials / 'master-key')
assert source_key.read_bytes() == (epoch / 'credentials/master-key').read_bytes()
management_gets = 0
client = urllib.request.build_opener(urllib.request.ProxyHandler({}))

def get(path, production=False):
    global management_gets
    management_gets += 1
    credentials = source_credentials if production else epoch / 'credentials'
    port = 18181 if production else 19181
    headers = {'X-Management-Key': (credentials / 'management-key').read_text().strip()}
    with client.open(urllib.request.Request('http://127.0.0.1:' + str(port) + path, headers=headers), timeout=3) as response:
        assert response.status == 200
        return json.load(response)

production_config = [row for row in get('/admin/config-versions', True) if row['status'] == 'active']
source_pools = [row for row in get('/admin/operations/provider-account-pools?limit=100', True)['items']
                if row.get('account_id') == account['id'] and row.get('channel_id') == 'p12-12-production-grok-build-endpoint']
assert len(source_pools) == 1 and source_pools[0]['runtime_status'] == 'available'
assert source_pools[0]['expires_at_ms'] > int(time.time() * 1000) + 600000
launch = json.loads((base / 'private/recovered-launch.json').read_text())['command']
old_binary = str(base / 'bin/gateway-aarch64-unknown-linux-gnu')
assert launch.count(old_binary) == 1
audit = base / 'audit/chat-json-502-20261011.jsonl'
log = epoch / 'gateway-v2.log'
assert not audit.exists() and not log.exists()
launch = [str(binary) if value == old_binary else value.replace(str(base / 'private/gateway.log'), str(log)).replace(str(base / 'audit/candidate-egress.jsonl'), str(audit)) for value in launch]
launch = [value.replace(str(base / 'state'), str(epoch / 'state')).replace(str(base / 'credentials'), str(epoch / 'credentials')) for value in launch]
launch += json.loads((base / 'private/copy-runtime-options.json').read_text())
launch[launch.index('--setenv=LD_PRELOAD=' + str(guard)):launch.index('--setenv=LD_PRELOAD=' + str(guard))] = [
    '--setenv=CPAR_BUILD_DIAGNOSTIC_DIR=' + str(epoch / 'native-capture'),
    '--setenv=CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256=' + prepare['selected_request_sha256'],
    '--setenv=CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION=' + source_sha,
]
(epoch / 'launch-v2.json').write_text(json.dumps({'command': launch}, indent=2) + '\n')
def temporary_ports():
    return [int(row.split()[1].rsplit(':', 1)[1], 16)
            for family in ['tcp', 'tcp6']
            for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:]
            if row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181]]


def process_exists(pid):
    return pathlib.Path('/proc', str(pid)).exists()


def read_owned_identity():
    pid = int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
    if pid == 0:
        return None
    proc = pathlib.Path('/proc', str(pid))
    args = (proc / 'cmdline').read_bytes().decode().rstrip('\0').split('\0')
    assert args[0] == str(binary) and args[args.index('--state-dir') + 1] == str(epoch / 'state')
    assert digest(proc / 'exe') == binary_hash
    return {'pid': pid, 'process_start': (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19],
            'binary_sha256': binary_hash, 'state_dir': str(epoch / 'state')}


def shutdown_owned_start(identity):
    observed = read_owned_identity()
    if observed is not None:
        assert observed == identity
        subprocess.run(['systemctl', 'stop', unit], check=True)
    assert read_owned_identity() is None and not process_exists(identity['pid']) and not temporary_ports()
    return {'temporary_service_stopped': True, 'owned_process_stop_status': 'STOPPED_VERIFIED'}


started = False
launch_requested = False
startup_identity = None
try:
    launch_requested = True
    subprocess.run(launch, check=True, capture_output=True)
    started = True
    startup_identity = read_owned_identity()
    assert startup_identity is not None
    (epoch / 'startup-process-binding.json').write_text(json.dumps(startup_identity, indent=2) + '\n')
    deadline = time.monotonic() + 30
    while True:
        try:
            versions = get('/admin/config-versions')
            break
        except urllib.error.URLError:
            assert time.monotonic() < deadline
            time.sleep(.2)
    active = [row for row in versions if row['status'] == 'active']
    assert len(active) == 1 and active[0]['id'] == 'cpar-build-config-20261010'
    assert active[0]['revision'].strip('"') == 'rev-9'
    pid = int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
    proc = pathlib.Path('/proc', str(pid))
    args = proc.joinpath('cmdline').read_bytes().decode().rstrip('\0').split('\0')
    assert args[0] == str(binary) and args[args.index('--background-provider-tasks') + 1] == 'disabled'
    assert args[args.index('--data-listen') + 1] == '127.0.0.1:19180'
    assert args[args.index('--management-listen') + 1] == '127.0.0.1:19181'
    assert digest(proc / 'exe') == binary_hash and str(guard) in (proc / 'maps').read_text()
    env = dict(row.split('=', 1) for row in (proc / 'environ').read_bytes().decode().split('\0') if '=' in row)
    assert env['LD_PRELOAD'] == str(guard) and env['CPAR_BUILD_EGRESS_AUDIT'] == str(audit)
    assert env['CPAR_BUILD_DIAGNOSTIC_DIR'] == str(epoch / 'native-capture')
    assert env['CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256'] == prepare['selected_request_sha256']
    assert env['CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION'] == source_sha
    assert not any(env.get(name) for name in ['HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'http_proxy', 'https_proxy', 'all_proxy'])
    status = dict(row.split(':', 1) for row in (proc / 'status').read_text().splitlines() if ':' in row)
    assert int(status['CapEff'].strip(), 16) == 4
    begin = time.monotonic()
    while time.monotonic() - begin < 65:
        assert int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID'])) == pid
        assert not audit.exists() or audit.stat().st_size == 0
        time.sleep(.5)
    pools = get('/admin/operations/provider-account-pools?limit=100')['items']
    assert len(pools) == 1 and pools[0]['credential_revision'] == prepare['expected_runtime_revision']
    assert pools[0]['runtime_status'] == 'available' and pools[0]['active_leases'] == 0
    assert pools[0]['expires_at_ms'] == source_pools[0]['expires_at_ms']
    with sqlite3.connect('file:' + str(epoch / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
        db.row_factory = sqlite3.Row
        saved = dict(db.execute('SELECT * FROM grok_accounts').fetchone())
        assert saved == account
        assert list(db.execute('SELECT id FROM management_resource_audit_events ORDER BY id')) == management_audit_before
    competing = []
    for path in pathlib.Path('/proc').iterdir():
        if not path.name.isdigit() or int(path.name) in [pid, os.getpid()]:
            continue
        try:
            if str(epoch / 'state').encode() in (path / 'cmdline').read_bytes():
                competing.append(int(path.name))
            for fd in (path / 'fd').iterdir():
                if str(fd.resolve()).startswith(str(epoch / 'state/control.sqlite3')):
                    competing.append(int(path.name))
        except (PermissionError, FileNotFoundError, ProcessLookupError):
            pass
    assert not competing
    assert production() == production_before
    assert [row for row in get('/admin/config-versions', True) if row['status'] == 'active'] == production_config
    assert all(digest(base / path) == value for path, value in historical.items())
    receipt = {
        'status': 'PASS', 'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'source_sha': source_sha, 'binary_sha256': binary_hash,
        'manifest_sha256': digest(epoch / 'artifact-manifest.json'), 'pid': pid,
        'process_start': (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19],
        'service': unit, 'configuration_id': 'cpar-build-config-20261010', 'configuration_revision': 9,
        'account_evidence_id': opaque(account['id']), 'native_revision': prepare['copied_native_revision'], 'runtime_projection_revision': pools[0]['credential_revision'],
        'source_ciphertext_and_all_account_fields_match_except_import_batch': True,
        'source_key_version_match': version, 'expires_at_ms': pools[0]['expires_at_ms'],
        'worker_claim': None, 'guard_sha256': digest(guard), 'guard_mapped': True,
        'background_provider_tasks': 'disabled', 'flat_observation_seconds': time.monotonic() - begin,
        'new_network_audit_events': 0, 'competing_state_processes': competing,
        'management_audit_unchanged': True, 'management_get_requests': management_gets,
        'historical_file_hashes': historical, 'production_before': production_before,
        'production_active_configuration': [{key: row[key] for key in ['id', 'revision', 'status']} for row in production_config],
        'production_after': production(), 'provider_inference_requests': 0, 'catalog_requests': 0,
    }
    (epoch / 'start-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    budget = json.loads((epoch / 'budget.json').read_text())
    assert budget['historical_inference_requests'] == 35 and budget['historical_gateway_pin_confirmed_sends'] == 34 and budget['new_inference_limit'] == 2 and budget['cumulative_inference_limit'] == 37
    public = {key: value for key, value in receipt.items() if key != 'historical_file_hashes'}
    public['preserved_historical_file_count'] = len(historical)
    public['historical_inventory_sha256'] = hashlib.sha256(json.dumps(historical, sort_keys=True).encode()).hexdigest()
    print(json.dumps(public))
except BaseException as error:
    cleanup = {'temporary_service_stopped': None, 'owned_process_stop_status': 'NOT_STARTED'}
    try:
        if launch_requested and startup_identity is None:
            startup_identity = read_owned_identity()
        if startup_identity is not None:
            cleanup = shutdown_owned_start(startup_identity)
        elif launch_requested:
            cleanup['owned_process_stop_status'] = 'NO_OWNED_PROCESS_OBSERVED'
    except Exception as cleanup_error:
        cleanup = {'temporary_service_stopped': None, 'owned_process_stop_status': 'IDENTITY_OR_EXIT_UNKNOWN',
                   'cleanup_exception_type': type(cleanup_error).__name__}
    receipt = {'status': 'BLOCKED', 'category': 'startup_precondition_failed', 'exception_type': type(error).__name__,
               'provider_inference_requests': 0, 'catalog_requests': 0, **cleanup}
    (epoch / 'start-failure-v2.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))
    raise
