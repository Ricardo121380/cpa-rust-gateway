"""Four isolated real Linux scenarios, zero Provider authority, one start each."""
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
import types

import launch_recipe
import startup_probe
from startup_probe import StartupError, StartupProbe, save
from system_exit_adapter import AuditTailClock, adapt_exit, production_identity, save_budget

os.umask(0o077)
base = Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert not base.is_symlink() and base.stat().st_uid == 0 and (base.stat().st_mode & 0o777) == 0o700
letter = sys.argv[1]
assert letter in ('A', 'B', 'C', 'D')
scenario = dict(A='normal', B='exec_failure', C='management_auth_failure', D='audit_tail_injection')[letter]
epoch = base / 'private' / ('linux-final-synthetic-20261013-' + letter)
unit = 'cpar-' + epoch.name + '.service'
uploaded = base / 'bin' / epoch.name
assert not epoch.exists() and not uploaded.exists()
assert subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=LoadState'], text=True).strip() == 'not-found'
epoch.mkdir(mode=0o700); uploaded.mkdir(mode=0o700)
for name in ['state', 'credentials', 'native-capture']:
    (epoch / name).mkdir(mode=0o700)
save(epoch / 'owner.json', {'task': 'cpar-build-linux-final-20261013', 'run': letter, 'scenario': scenario})
# Historic counters are receipt-derived, never allocated to this zero-Provider run.
old = json.loads((base / 'private/abd45431-six-20261011/final-receipt.json').read_text())
assert old['budget']['stopped']
assert old['budget']['historical_inference_requests'] + old['budget']['new_pin_intents'] + old['budget']['new_basic_intents'] == 35
assert old['budget']['historical_gateway_pin_confirmed_sends'] + old['new_pin_actual_sends'] + old['new_basic_attempts'] == 34
budget = {'provider_intent_limit': 0, 'provider_intents': 0, 'stopped': False,
    'historical_inference_requests': 35, 'historical_gateway_pin_confirmed_sends': 34,
    'new_pin_intents': 0, 'new_basic_intents': 0, 'new_inference_limit': 0, 'cumulative_inference_limit': 35}
save_budget(epoch / 'budget.json', budget)
production_before = production_identity()
host_namespace = os.readlink('/proc/1/ns/net')
source_binary = base / 'bin/chat-json-rehearsal-20261013/gateway-aarch64-unknown-linux-gnu'
binary_hash = 'cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e'
assert hashlib.sha256(source_binary.read_bytes()).hexdigest() == binary_hash
binary = uploaded / source_binary.name
shutil.copyfile(source_binary, binary); binary.chmod(0o700)
assert hashlib.sha256(binary.read_bytes()).hexdigest() == binary_hash
guard = base / 'bin/egress-guard-tcp.so'
guard_hash = '91c943c76884a2b674a0b24d5d4876ff40cf61b53c379ffe2d04a062a0dddf06'
assert hashlib.sha256(guard.read_bytes()).hexdigest() == guard_hash
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
launch.insert(launch.index(str(binary)), '--property=PrivateNetwork=yes')
assert '--collect' not in launch and not any('CollectMode=' in row for row in launch)
# Explicit default retention: failed state survives until this run's reset-failed.
launch.insert(launch.index(str(binary)), '--property=CollectMode=inactive')
fixture = None
if letter == 'B':
    fixture = uploaded / 'intentionally-invalid-gateway'
    fixture.write_bytes(b'SYNTHETIC INVALID EXECUTABLE FORMAT\n'); fixture.chmod(0o700)
    launch[launch.index(str(binary))] = str(fixture)
actual_binary = fixture or binary
actual_binary_hash = hashlib.sha256(actual_binary.read_bytes()).hexdigest()
save(epoch / 'launch-v2.json', {'command': launch})
for name in ['startup_probe.py', 'remote-exit.py', 'system_exit_adapter.py', 'artifact-manifest.json', 'gateway-build-metadata.json']:
    shutil.copyfile(Path(__file__).parent / name, uploaded / name)
    shutil.copyfile(uploaded / name, epoch / name)
freeze = {'source_sha': 'bf822b14a62c21cf582a7539e3635f69bd8b7677', 'epoch': epoch.name,
    'files': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in uploaded.iterdir()},
    'assembled_launch_sha256': hashlib.sha256(json.dumps(launch, separators=(',', ':')).encode()).hexdigest(),
    'authority': 'REAL_LINUX_SYSTEM_SYNTHETIC; Provider limit zero'}
save(uploaded / 'freeze.json', freeze); save(epoch / 'freeze.json', freeze)
save(epoch / 'prepare-receipt.json', {'inherited_event_counts': {}, 'inherited_ledger_count': 0,
    'selected_request_sha256': '0' * 64, 'synthetic': True})
probe = StartupProbe(epoch, unit, actual_binary, actual_binary_hash, launch, guard)
audit = base / 'audit' / (epoch.name + '.jsonl')
identity = failure = isolation = flat_seconds = injection = exec_native = exit_receipt = None
http_statuses = []; resource_exit = 'UNKNOWN'; startup_status = 'FAIL'
receipt = {'layer': 'REAL_LINUX_SYSTEM_SYNTHETIC', 'scenario': scenario, 'run': letter,
    'formal_account_qualification': 'NOT_RUN', 'provider_intents': 0, 'provider_requests': 0,
    'binary_sha256': binary_hash, 'actual_executable_sha256': actual_binary_hash, 'preparation': preparation,
    'unit': unit, 'epoch': str(epoch), 'freeze': freeze, 'systemd_version': subprocess.check_output(['systemctl', '--version'], text=True).splitlines()[0]}
try:
    identity = probe.launch(launch)
    proc = Path('/proc', str(identity['pid']))
    namespace = os.readlink(proc / 'ns/net'); assert namespace != host_namespace
    network_script = '''import socket,json
results=[]
for family,address in [(socket.AF_INET,('203.0.113.1',443)),(socket.AF_INET6,('2001:db8::1',443,0,0))]:
    with socket.socket(family,socket.SOCK_STREAM) as s:
        s.settimeout(1);results.append(s.connect_ex(address))
print(json.dumps({'interfaces':socket.if_nameindex(),'connect_errno':results}))
'''
    isolated = subprocess.run(['nsenter', '--target', str(identity['pid']), '--net', 'python3', '-c', network_script],
        capture_output=True, check=True, timeout=5)
    isolation = json.loads(isolated.stdout)
    assert isolation['interfaces'] == [[1, 'lo']] and isolation['connect_errno'] == [errno.ENETUNREACH, errno.ENETUNREACH]
    isolation.update(network_namespace=namespace, host_network_namespace=host_namespace,
        private_network=True, no_external_route_verified=True)
    save(epoch / 'isolation.json', isolation)
    probe.wait_listeners(identity); probe.record('application_check_started')
    http_script = '''import json,pathlib,urllib.request,urllib.error,sys
key=(pathlib.Path(sys.argv[1])/'credentials/management-key').read_text().strip()
if sys.argv[2]=='management_auth_failure':key='synthetic-wrong-management-key'
client=urllib.request.build_opener(urllib.request.ProxyHandler({}))
request=urllib.request.Request('http://127.0.0.1:19181/admin/config-versions',headers={'X-Management-Key':key})
try:
    with client.open(request,timeout=3) as r:print(json.dumps({'status':r.status,'body':json.load(r)}))
except urllib.error.HTTPError as e:print(json.dumps({'status':e.code}))
'''
    save(epoch / 'application-observation.json', {'http_statuses': [], 'request_intents': 1})
    result = subprocess.run(['nsenter', '--target', str(identity['pid']), '--net', 'python3', '-c', http_script, str(epoch), scenario],
        capture_output=True, check=True, timeout=6)
    response = json.loads(result.stdout); http_statuses.append(response['status'])
    save(epoch / 'application-observation.json', {'http_statuses': http_statuses, 'request_intents': 1})
    if response['status'] != 200:
        probe.trace['gates']['application'] = 'FAIL'; probe.record('application_rejected', http_status=response['status'])
        raise StartupError('first_management_request_rejected')
    active = [row for row in response['body'] if row['status'] == 'active']
    assert len(active) == 1 and active[0]['id'] == 'synthetic-start'
    probe.trace['gates']['application'] = 'PASS'; probe.record('synthetic_application_passed')
    clock = AuditTailClock(time, probe, audit, 'cpar-linux-final-D-tail') if letter == 'D' else None
    if clock: startup_probe.time = clock
    try:
        flat_seconds = probe.quiet_audit(identity, audit)
    finally:
        startup_probe.time = time
        if clock: injection = clock.injection
    assert all(value == 'PASS' for value in probe.trace['gates'].values())
    startup_status = 'PASS'
    save(epoch / 'start-receipt.json', {'status': 'PASS', 'synthetic': True, 'pid': identity['pid'],
        'process_start': identity['process_start'], 'binary_sha256': binary_hash, 'source_sha': freeze['source_sha'],
        'manifest_sha256': freeze['files']['artifact-manifest.json'],
        'startup_gates': dict(probe.trace['gates']), 'startup_observations_sha256': hashlib.sha256(probe.path.read_bytes()).hexdigest(),
        'argv_sha256': hashlib.sha256(json.dumps(probe.expected_args, separators=(',', ':')).encode()).hexdigest(),
        'assembled_launch_sha256': freeze['assembled_launch_sha256'], 'guard_sha256': guard_hash,
        'historical_file_hashes': {}, 'production_before': production_before, 'production_active_configuration': [], 'native_revision': 0})
except Exception as error:
    failure = str(error) if isinstance(error, StartupError) else 'system_validation_' + type(error).__name__
    probe.record('validation_failed', failure_reason=failure, exception_type=type(error).__name__)
    # Capture kernel/systemd failure before any cleanup or reset. Never infer 203.
    if letter == 'B':
        state = probe.systemd()
        show = subprocess.run(['systemctl', 'show', unit, '--property=Type,LoadState,ActiveState,SubState,MainPID,ExecMainPID,ExecMainCode,ExecMainStatus,Result,CollectMode,ExecStart,ControlGroup'], capture_output=True, timeout=5)
        journal = subprocess.run(['journalctl', '--unit=' + unit, '--no-pager', '--output=json'], capture_output=True, timeout=5)
        for name, raw in [('exec-systemd-native.txt', show.stdout), ('exec-journal-native.jsonl', journal.stdout)]:
            with (epoch / name).open('xb') as output:
                output.write(raw); output.flush(); os.fsync(output.fileno())
        messages = [json.loads(line).get('MESSAGE', '') for line in journal.stdout.splitlines()]
        exec_native = {'systemd': state, 'show_returncode': show.returncode, 'journal_returncode': journal.returncode,
            'show_sha256': hashlib.sha256(show.stdout).hexdigest(), 'journal_sha256': hashlib.sha256(journal.stdout).hexdigest(),
            'exec_format_error_observed': any('Exec format error' in m and ('execute' in m or 'EXEC' in m) for m in messages),
            'fixture_exists': actual_binary.exists(), 'fixture_mode': oct(actual_binary.stat().st_mode & 0o777)}
        save(epoch / 'exec-failure-evidence.json', exec_native)
finally:
    try:
        if startup_status == 'PASS':
            original = (epoch / 'remote-exit.py').read_text()
            adapted = adapt_exit(original, epoch.name, unit)
            (epoch / 'system-exit-executed.py').write_text(adapted)
            before = probe.path.read_bytes()
            wrapper = 'import sys\nsys.dont_write_bytecode=True\nsys.path.insert(0,' + repr(str(epoch)) + ')\n' + adapted
            save(epoch / 'exit-invocation.json', {'formal_source_sha256': hashlib.sha256(original.encode()).hexdigest(),
                'adapter_sha256': hashlib.sha256((epoch / 'system_exit_adapter.py').read_bytes()).hexdigest(),
                'adapted_source_sha256': hashlib.sha256(adapted.encode()).hexdigest(), 'executed_sha256': hashlib.sha256(wrapper.encode()).hexdigest(),
                'branch': 'complete remote-exit successful-start branch', 'provider_authority': 0})
            exited = subprocess.run([sys.executable, '-B', '-c', wrapper], capture_output=True, timeout=45)
            (epoch / 'exit-native.stdout').write_bytes(exited.stdout); (epoch / 'exit-native.stderr').write_bytes(exited.stderr)
            assert exited.returncode == 0, 'complete_exit_entry_failed'
            exit_receipt = json.loads(exited.stdout)
            assert exit_receipt['resource_exit_status'] == exit_receipt['count_reconciliation_status'] == 'PASS'
            assert probe.path.read_bytes() == before, 'startup_observations_changed_by_exit'
        else:
            state = probe.systemd()
            if int(state['MainPID']):
                current = probe.identity()
                if identity is not None: assert current == identity
                identity = current
                def stopped_budget():
                    current_budget = json.loads((epoch / 'budget.json').read_text()); current_budget['stopped'] = True
                    save_budget(epoch / 'budget.json', current_budget)
                probe.stop(identity, stopped_budget)
            else:
                current_budget = json.loads((epoch / 'budget.json').read_text()); current_budget['stopped'] = True
                save_budget(epoch / 'budget.json', current_budget)
                save(epoch / 'startup-stop-intent.json', {'stop_intent': True, 'no_live_pid': True, 'startup_validation_status': 'NOT_PASSED'})
        assert int(probe.systemd()['MainPID']) == 0
        if identity is not None: assert not Path('/proc', str(identity['pid'])).exists()
        if exec_native and int(exec_native['systemd'].get('ExecMainPID', '0')):
            assert not Path('/proc', exec_native['systemd']['ExecMainPID']).exists()
        before = audit.read_bytes() if audit.exists() else b''
        time.sleep(5)
        assert (audit.read_bytes() if audit.exists() else b'') == before
        synthetic_events = [json.loads(row) for row in before.splitlines()]
        if letter == 'D':
            assert injection is not None and len(synthetic_events) == 1
            assert synthetic_events[0]['event'] == 'synthetic_audit_tail' and synthetic_events[0]['synthetic'] is True
            assert synthetic_events[0]['marker'] == 'cpar-linux-final-D-tail'
            assert hashlib.sha256(before).hexdigest() == injection['sha256']
        else: assert before == b''
        assert production_identity() == production_before and os.readlink('/proc/1/ns/net') == host_namespace
        if probe.systemd().get('ActiveState') == 'failed':
            assert exec_native is not None
            save(epoch / 'before-reset-failed.json', {'captured_exec_native_sha256': hashlib.sha256((epoch / 'exec-failure-evidence.json').read_bytes()).hexdigest()})
            subprocess.run(['systemctl', 'reset-failed', unit], check=True, capture_output=True, timeout=5)
        final_state = probe.systemd()
        assert int(final_state['MainPID']) == 0 and final_state['LoadState'] == 'not-found'
        group = exec_native['systemd'].get('ControlGroup') if exec_native else '/system.slice/' + unit
        assert not group or not (Path('/sys/fs/cgroup') / group.lstrip('/')).exists()
        # Verify namespace/host listeners and state descriptors no longer belong to a surviving process.
        for proc in Path('/proc').iterdir():
            if not proc.name.isdigit() or int(proc.name) == os.getpid(): continue
            try:
                assert str(epoch / 'state').encode() not in (proc / 'cmdline').read_bytes()
                assert not any(str(fd.resolve()).startswith(str(epoch / 'state')) for fd in (proc / 'fd').iterdir())
            except (PermissionError, FileNotFoundError, ProcessLookupError): pass
        for family in ['tcp', 'tcp6']:
            assert not any(row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181]
                for row in Path('/proc/net', family).read_text().splitlines()[1:])
        with sqlite3.connect('file:' + str(database) + '?mode=ro', uri=True) as db:
            assert db.execute('SELECT * FROM grok_accounts').fetchall() == account_before
            assert db.execute('SELECT count(*) FROM gateway_event_log').fetchone()[0] == 0
            assert db.execute('SELECT count(*) FROM billing_ledger_entries').fetchone()[0] == 0
        assert json.loads((epoch / 'budget.json').read_text())['stopped']
        resource_exit = 'PASS'
    except Exception as cleanup_error:
        receipt['cleanup_exception_type'] = type(cleanup_error).__name__
        resource_exit = 'UNKNOWN'

exec_expected = bool(exec_native and exec_native['show_returncode'] == exec_native['journal_returncode'] == 0
    and exec_native['exec_format_error_observed'] and exec_native['fixture_mode'] == '0o700'
    and all(exec_native['systemd'].get(k) == v for k, v in {'Type': 'exec', 'LoadState': 'loaded', 'ActiveState': 'failed',
        'MainPID': '0', 'ExecMainCode': '1', 'ExecMainStatus': '203', 'Result': 'exit-code'}.items())
    and int(exec_native['systemd'].get('ExecMainPID', '0')) > 0)
expected = (letter == 'A' and failure is None and startup_status == 'PASS' and exit_receipt is not None) or \
    (letter == 'B' and failure == 'exec_handshake_failed' and probe.trace['launch'].get('returncode') != 0 and exec_expected) or \
    (letter == 'C' and failure == 'first_management_request_rejected' and http_statuses == [404] and probe.trace['gates']['quiet_audit'] == 'UNKNOWN') or \
    (letter == 'D' and failure == 'quiet_audit_not_flat' and injection is not None and probe.trace['gates']['quiet_audit'] != 'PASS'
        and not (epoch / 'start-receipt.json').exists())
try:
    events = [json.loads(row) for row in (audit.read_bytes() if audit.exists() else b'').splitlines()]
    marked = [event for event in events if event.get('event') == 'synthetic_audit_tail'
        and event.get('synthetic') is True and event.get('marker') == 'cpar-linux-final-D-tail']
    network_event_count, synthetic_event_count = len(events) - len(marked), len(marked)
except (ValueError, TypeError):
    network_event_count = synthetic_event_count = None
    expected = False
receipt.update(status='PASS' if expected and resource_exit == 'PASS' else 'FAIL', startup_validation_status=startup_status,
    startup_gates=probe.trace['gates'], launch=probe.trace['launch'], failure_reason=failure,
    preset_negative=letter != 'A', resource_exit_status=resource_exit, isolation=isolation,
    http_statuses=http_statuses, flat_observation_seconds=flat_seconds, post_stop_flat_seconds=5,
    production_identity_preserved=production_identity() == production_before, identity=identity,
    startup_observations=probe.trace, exec_native=exec_native, synthetic_injection=injection,
    complete_exit_receipt=exit_receipt, audit_sha256=hashlib.sha256(audit.read_bytes() if audit.exists() else b'').hexdigest(),
    real_network_events=network_event_count, synthetic_audit_events=synthetic_event_count)
save(epoch / 'system-result.json', receipt)
print(json.dumps(receipt))
if receipt['status'] != 'PASS': sys.exit(1)
