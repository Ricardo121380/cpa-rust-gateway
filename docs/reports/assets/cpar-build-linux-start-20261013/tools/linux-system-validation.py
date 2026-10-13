"""Three exclusive synthetic runs on new-vps; never loads a real credential."""
import errno
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import time

import launch_recipe
from startup_probe import StartupError, StartupProbe, save

os.umask(0o077)
base = Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert not base.is_symlink() and base.stat().st_uid == 0 and (base.stat().st_mode & 0o777) == 0o700
number = int(sys.argv[1])
assert number in (1, 2, 3)
scenario = {1: 'normal', 2: 'exec_failure', 3: 'management_auth_failure'}[number]
epoch = base / 'private' / ('linux-start-synthetic-20261013-' + str(number))
unit = 'cpar-' + epoch.name + '.service'
assert not epoch.exists()
assert subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=LoadState'], text=True).strip() == 'not-found'
epoch.mkdir(mode=0o700)
for name in ['state', 'credentials', 'native-capture', 'bin']:
    (epoch / name).mkdir(mode=0o700)
save(epoch / 'owner.json', {'task': 'cpar-build-linux-start-20261013', 'run': number, 'scenario': scenario})
save(epoch / 'budget.json', {'provider_intent_limit': 0, 'provider_intents': 0, 'stopped': False})


def production_identity():
    raw = subprocess.check_output(['systemctl', 'show', 'cpa-rust-gateway.service',
        '--property=MainPID,ActiveState,SubState'], text=True, timeout=5)
    value = dict(row.split('=', 1) for row in raw.splitlines() if '=' in row)
    proc = Path('/proc', value['MainPID'])
    value.update(process_start=(proc / 'stat').read_text().rsplit(')', 1)[1].split()[19],
        argv_sha256=hashlib.sha256((proc / 'cmdline').read_bytes()).hexdigest(),
        exe_sha256=hashlib.sha256((proc / 'exe').read_bytes()).hexdigest())
    return value


production_before = production_identity()
host_namespace = os.readlink('/proc/1/ns/net')
source_binary = base / 'bin/chat-json-rehearsal-20261013/gateway-aarch64-unknown-linux-gnu'
binary_hash = 'cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e'
assert hashlib.sha256(source_binary.read_bytes()).hexdigest() == binary_hash
binary = epoch / 'bin/gateway-aarch64-unknown-linux-gnu'
shutil.copyfile(source_binary, binary)
binary.chmod(0o700)
assert hashlib.sha256(binary.read_bytes()).hexdigest() == binary_hash
guard = base / 'bin/egress-guard-tcp.so'
assert hashlib.sha256(guard.read_bytes()).hexdigest() == '91c943c76884a2b674a0b24d5d4876ff40cf61b53c379ffe2d04a062a0dddf06'
# Fresh synthetic keys only; never reads base/credentials or any production DB.
for name in ['master-key', 'backup-key', 'client-key-pepper', 'grok-build-cache-key']:
    (epoch / 'credentials' / name).write_bytes(os.urandom(32))
for name, prefix in [('management-key', 'mgmt_'), ('management-csrf', 'csrf_')]:
    (epoch / 'credentials' / name).write_text(prefix + os.urandom(24).hex())
database = epoch / 'state/control.sqlite3'
preparation = []
for action in ['create', 'validate', 'publish']:
    command = [str(binary), 'admin', action, '--db', str(database), '--version', 'synthetic-start', '--actor', 'synthetic-system-validation']
    if action == 'create': command += ['--description', 'Synthetic Linux startup; no Provider route']
    result = subprocess.run(command, capture_output=True, timeout=20)
    preparation.append({'action': action, 'returncode': result.returncode, 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()})
    save(epoch / 'preparation.json', preparation)
    assert result.returncode == 0, 'synthetic_cli_preparation_failed'
with sqlite3.connect(database) as db:
    db.execute("INSERT INTO grok_account_import_batches(id,status,created_count,unchanged_count,created_at_ms) VALUES ('synthetic','applied',1,0,1)")
    db.execute("INSERT INTO grok_accounts(id,provider,identity_digest,credential_ciphertext,credential_key_version,auth_status,enabled,priority,weight,max_concurrency,revision,import_batch_id,created_at_ms,updated_at_ms) VALUES ('synthetic-disabled','build',?,?,1,'disabled',0,0,1,1,0,'synthetic',1,1)",
        (hashlib.sha256(b'synthetic-disabled-account').digest(), b'synthetic-disabled-not-a-provider-credential'))
    db.commit()
    assert db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0] == 34
    account_before = db.execute('SELECT * FROM grok_accounts').fetchall()
    assert len(account_before) == 1

# Derive the same startup recipe, then add only per-unit network isolation.
import types
def with_environment(command, binary, guard, audit, capture, request_sha, source_sha):
    for i, value in enumerate(command):
        if value.startswith('--property=Environment='):
            command[i] = '--property=Environment=LD_PRELOAD=' + str(guard) + ' CPAR_BUILD_EGRESS_AUDIT=' + str(audit) + \
                ' CPAR_BUILD_DIAGNOSTIC_DIR=' + str(capture) + ' CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256=' + request_sha + \
                ' CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION=' + source_sha
    return command
recipe = json.loads((Path(__file__).parent / 'recipe.json').read_text())
launch = launch_recipe.assemble(recipe['approved_launch'], recipe['runtime_options'], base, epoch, unit,
    'bf822b14a62c21cf582a7539e3635f69bd8b7677', '0' * 64, types.SimpleNamespace(with_diagnostic_environment=with_environment))
old_binary = base / 'bin' / epoch.name / binary.name
launch[launch.index(str(old_binary))] = str(binary)
launch.insert(launch.index(str(binary)), '--property=PrivateNetwork=yes')
if number == 2:
    # An existing executable with an invalid ELF reaches the kernel execve step;
    # a missing path can instead be rejected by systemd-run before unit creation.
    # This repaired fixture has NOT been run under this batch's exhausted cap.
    binary = epoch / 'bin/intentionally-invalid-gateway'
    binary.write_bytes(b'SYNTHETIC INVALID EXECUTABLE FORMAT\n')
    binary.chmod(0o700)
    launch[launch.index(str(epoch / 'bin/gateway-aarch64-unknown-linux-gnu'))] = str(binary)
save(epoch / 'launch.json', {'command': launch})
probe = StartupProbe(epoch, unit, binary, binary_hash, launch, guard)
audit = base / 'audit' / (epoch.name + '.jsonl')
identity = None
failure = None
isolation = None
http_statuses = []
flat_seconds = None
resource_exit = 'UNKNOWN'
receipt = {'layer': 'REAL_LINUX_SYSTEM_SYNTHETIC', 'scenario': scenario, 'run': number,
           'formal_account_qualification': 'NOT_RUN', 'provider_intents': 0, 'provider_requests': 0,
           'binary_sha256': binary_hash, 'preparation': preparation}
try:
    identity = probe.launch(launch)
    proc = Path('/proc', str(identity['pid']))
    namespace = os.readlink(proc / 'ns/net')
    assert namespace != host_namespace, 'private_network_namespace_missing'
    # Non-Provider TEST-NET destinations. Connects cannot emit packets without a route.
    network_script = '''import socket,json,os
interfaces=socket.if_nameindex()
results=[]
for family,address in [(socket.AF_INET,('203.0.113.1',443)),(socket.AF_INET6,('2001:db8::1',443,0,0))]:
    with socket.socket(family,socket.SOCK_STREAM) as s:
        s.settimeout(1);results.append(s.connect_ex(address))
print(json.dumps({'interfaces':interfaces,'connect_errno':results}))
'''
    isolated = subprocess.run(['nsenter', '--target', str(identity['pid']), '--net', 'python3', '-c', network_script],
        capture_output=True, check=True, timeout=5)
    isolation = json.loads(isolated.stdout)
    assert isolation['interfaces'] == [[1, 'lo']]
    assert isolation['connect_errno'] == [errno.ENETUNREACH, errno.ENETUNREACH]
    isolation.update(network_namespace=namespace, host_network_namespace=host_namespace,
                     private_network=True, no_external_route_verified=True)
    save(epoch / 'isolation.json', isolation)
    probe.wait_listeners(identity)
    probe.record('application_check_started')
    http_script = '''import json,pathlib,urllib.request,urllib.error,sys
key=(pathlib.Path(sys.argv[1])/'credentials/management-key').read_text().strip()
if sys.argv[2]=='management_auth_failure':key='synthetic-wrong-management-key'
client=urllib.request.build_opener(urllib.request.ProxyHandler({}))
request=urllib.request.Request('http://127.0.0.1:19181/admin/config-versions',headers={'X-Management-Key':key})
try:
    with client.open(request,timeout=3) as r:print(json.dumps({'status':r.status,'body':json.load(r)}))
except urllib.error.HTTPError as e:print(json.dumps({'status':e.code}))
'''
    result = subprocess.run(['nsenter', '--target', str(identity['pid']), '--net', 'python3', '-c', http_script,
        str(epoch), scenario], capture_output=True, check=True, timeout=6)
    response = json.loads(result.stdout)
    http_statuses.append(response['status'])
    save(epoch / 'application-observation.json', {'http_statuses': http_statuses, 'request_intents': 1})
    if response['status'] != 200:
        probe.trace['gates']['application'] = 'FAIL'
        probe.record('application_rejected', http_status=response['status'])
        raise StartupError('first_management_request_rejected')
    active = [row for row in response['body'] if row['status'] == 'active']
    assert len(active) == 1 and active[0]['id'] == 'synthetic-start'
    probe.trace['gates']['application'] = 'PASS'
    probe.record('synthetic_application_passed')
    flat_seconds = probe.quiet_audit(identity, audit)
    with sqlite3.connect(database) as db:
        assert db.execute('SELECT * FROM grok_accounts').fetchall() == account_before
        assert db.execute('SELECT count(*) FROM gateway_event_log').fetchone()[0] == 0
        assert db.execute('SELECT count(*) FROM billing_ledger_entries').fetchone()[0] == 0
    assert production_identity() == production_before
except Exception as error:
    failure = str(error) if isinstance(error, StartupError) else 'system_validation_' + type(error).__name__
    probe.record('validation_failed', failure_reason=failure, exception_type=type(error).__name__)
finally:
    try:
        state = probe.systemd()
        if int(state['MainPID']):
            current = probe.identity()
            if identity is not None: assert current == identity
            identity = current
            def stopped_budget():
                budget = json.loads((epoch / 'budget.json').read_text())
                assert budget['provider_intents'] == 0
                budget['stopped'] = True
                save(epoch / 'budget.json', budget)
            probe.stop(identity, stopped_budget)
        else:
            budget = json.loads((epoch / 'budget.json').read_text()); budget['stopped'] = True
            save(epoch / 'budget.json', budget)
            save(epoch / 'startup-stop-intent.json', {'stop_intent': True, 'no_live_pid': True})
        assert int(probe.systemd()['MainPID']) == 0
        if identity is not None: assert not Path('/proc', str(identity['pid'])).exists()
        before = audit.read_bytes() if audit.exists() else b''
        time.sleep(5)
        assert (audit.read_bytes() if audit.exists() else b'') == before == b''
        assert production_identity() == production_before and os.readlink('/proc/1/ns/net') == host_namespace
        if probe.systemd().get('ActiveState') == 'failed':
            subprocess.run(['systemctl', 'reset-failed', unit], check=True, capture_output=True, timeout=5)
        assert int(probe.systemd()['MainPID']) == 0
        resource_exit = 'PASS'
    except Exception as cleanup_error:
        receipt['cleanup_exception_type'] = type(cleanup_error).__name__
        resource_exit = 'UNKNOWN'

expected = (number == 1 and failure is None and all(value == 'PASS' for value in probe.trace['gates'].values())) or \
    (number == 2 and failure == 'exec_handshake_failed' and probe.trace['launch'].get('returncode') != 0
        and probe.trace['observations'][1]['systemd'].get('ExecMainStatus') == '203') or \
    (number == 3 and failure == 'first_management_request_rejected' and http_statuses == [404]
        and probe.trace['gates']['quiet_audit'] == 'UNKNOWN')
receipt.update(status='PASS' if expected and resource_exit == 'PASS' else 'FAIL',
    startup_gates=probe.trace['gates'], launch=probe.trace['launch'], failure_reason=failure,
    preset_negative=number != 1, resource_exit_status=resource_exit, isolation=isolation,
    http_statuses=http_statuses, flat_observation_seconds=flat_seconds, post_stop_flat_seconds=5,
    production_identity_preserved=production_identity() == production_before,
    identity=identity, startup_observations=probe.trace)
save(epoch / 'system-result.json', receipt)
print(json.dumps(receipt))
if receipt['status'] != 'PASS': sys.exit(1)
