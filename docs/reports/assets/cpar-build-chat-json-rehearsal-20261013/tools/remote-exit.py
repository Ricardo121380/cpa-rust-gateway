"""Stop only the bound epoch, reconcile durable counts and retain all evidence."""
import ast
import datetime
import fcntl
import hashlib
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
epoch = base / 'private/chat-json-rehearsal-20261013'
unit = 'cpar-build-chat-json-rehearsal-20261013.service'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert not (epoch / 'final-receipt.json').exists()
source = ast.parse((base / 'private/bootstrap.py').read_text())
functions = ast.Module(body=[node for node in source.body if isinstance(node, ast.FunctionDef)
                            and node.name == 'production'], type_ignores=[])
exec(compile(functions, 'production-proof', 'exec'))
if not (epoch / 'start-receipt.json').exists():
    # Startup may fail before its successful binding receipt. A current process
    # must be fully bound to the frozen launch before any stop; this is cleanup
    # evidence only and never restores startup or inference qualification.
    expected = exit_expected_baseline
    uploaded = base / 'bin/chat-json-rehearsal-20261013'
    freeze = json.loads((uploaded / 'freeze.json').read_text())
    assert all(hashlib.sha256((uploaded / name).read_bytes()).hexdigest() == value
               for name, value in freeze['files'].items())
    owned_process_stop_status = None
    if epoch.exists():
        lock = (epoch / 'execution.lock').open('a')
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if (epoch / 'budget.json').exists():
            sys.path.insert(0, str(epoch))
            import cpar_build_json_diagnostic as diagnostic
            budget = json.loads((epoch / 'budget.json').read_text())
            assert budget['new_pin_intents'] == budget['new_basic_intents'] == 0
            budget['stopped'] = True
            diagnostic.save_budget(epoch / 'budget.json', budget)
    pid = int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
    if pid:
        assert json.loads((epoch / 'freeze.json').read_text()) == freeze
        launch = json.loads((epoch / 'launch-v2.json').read_text())['command']
        assert hashlib.sha256(json.dumps(launch, separators=(',', ':')).encode()).hexdigest() == freeze['assembled_launch_sha256']
        binary = uploaded / 'gateway-aarch64-unknown-linux-gnu'
        proc = pathlib.Path('/proc', str(pid))
        args = (proc / 'cmdline').read_bytes().decode().rstrip('\0').split('\0')
        assert args == launch[launch.index(str(binary)):]
        assert args[args.index('--state-dir') + 1] == str(epoch / 'state')
        assert args[args.index('--background-provider-tasks') + 1] == 'disabled'
        assert str((proc / 'exe').resolve()) == str(binary)
        assert hashlib.sha256((proc / 'exe').read_bytes()).hexdigest() == freeze['files'][binary.name]
        guard = base / 'bin/egress-guard-tcp.so'
        assert hashlib.sha256(guard.read_bytes()).hexdigest() == '91c943c76884a2b674a0b24d5d4876ff40cf61b53c379ffe2d04a062a0dddf06'
        assert str(guard) in (proc / 'maps').read_text()
        environment = [row for row in (proc / 'environ').read_bytes().decode().split('\0') if '=' in row]
        prepare = json.loads((epoch / 'prepare-receipt.json').read_text())
        required_environment = {'LD_PRELOAD': str(guard), 'CPAR_BUILD_EGRESS_AUDIT': str(base / 'audit/chat-json-rehearsal-20261013.jsonl'),
            'CPAR_BUILD_DIAGNOSTIC_DIR': str(epoch / 'native-capture'),
            'CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256': prepare['selected_request_sha256'],
            'CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION': freeze['source_sha']}
        assert all(sum(row.split('=', 1)[0] == name for row in environment) == 1
                   and name + '=' + value in environment for name, value in required_environment.items())
        process_start = (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19]
        binding = {'pid': pid, 'process_start': process_start, 'binary_sha256': freeze['files'][binary.name],
                   'argv_sha256': hashlib.sha256(json.dumps(args, separators=(',', ':')).encode()).hexdigest(),
                   'state_dir': str(epoch / 'state'), 'startup_validation_status': 'NOT_PASSED'}
        with (epoch / 'exit-process-binding.json').open('x') as output:
            json.dump(binding, output, indent=2)
            output.flush()
            os.fsync(output.fileno())
        directory = os.open(epoch, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
        assert int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID'])) == pid
        assert (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19] == process_start
        subprocess.run(['systemctl', 'stop', unit], check=True)
        assert not proc.exists()
        owned_process_stop_status = 'STOPPED_VERIFIED'
    assert int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID'])) == 0
    for proc in pathlib.Path('/proc').iterdir():
        if not proc.name.isdigit() or int(proc.name) == os.getpid(): continue
        try:
            assert str(epoch / 'state').encode() not in (proc / 'cmdline').read_bytes()
            assert not any(str(fd.resolve()).startswith(str(epoch / 'state')) for fd in (proc / 'fd').iterdir())
        except (PermissionError, FileNotFoundError, ProcessLookupError): pass
    for family in ['tcp', 'tcp6']:
        assert not any(row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181]
                       for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:])
    assert production() == expected['production']
    args = pathlib.Path('/proc', expected['production']['MainPID'], 'cmdline').read_bytes().decode().split('\0')
    credentials = pathlib.Path(args[args.index('--credential-dir') + 1])
    client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with client.open(urllib.request.Request('http://127.0.0.1:18181/admin/config-versions', headers={
            'X-Management-Key': (credentials / 'management-key').read_text().strip()}), timeout=10) as response:
        assert response.status == 200
        active = [{key: row[key] for key in ['id', 'revision', 'status']}
                  for row in json.load(response) if row['status'] == 'active']
    assert active == expected['production_config']
    prior = json.loads((base / 'private/chat-json-502-20261011/prepare-receipt.json').read_text())
    assert json.loads((base / 'private/chat-json-502-20261011/budget.json').read_text())['stopped'] is True
    assert all(hashlib.sha256((base / name).read_bytes()).hexdigest() == value
               for name, value in prior['historical_file_hashes'].items())
    if epoch.exists():
        for path in epoch.rglob('*'):
            assert not path.is_symlink()
            assert stat.S_IMODE(path.stat().st_mode) == (0o700 if path.is_dir() else 0o600)
    audit = base / 'audit/chat-json-rehearsal-20261013.jsonl'
    audit_hash = hashlib.sha256(audit.read_bytes() if audit.exists() else b'').hexdigest()
    time.sleep(5)
    assert hashlib.sha256(audit.read_bytes() if audit.exists() else b'').hexdigest() == audit_hash
    assert production() == expected['production']
    start_failure = (json.loads((epoch / 'start-failure-v2.json').read_text())
                     if (epoch / 'start-failure-v2.json').exists() else {})
    receipt = {'status': 'PASS; resource exit before successful start', 'resource_exit_status': 'PASS',
               'owned_process_stop_status': owned_process_stop_status or start_failure.get('owned_process_stop_status', 'NOT_STARTED'),
               'startup_validation_status': 'NOT_PASSED',
               'remote_epoch_status': 'CREATED' if epoch.exists() else 'NOT_CREATED',
               'temporary_ports_listening': [], 'post_stop_flat_seconds': 5, 'production_after': production(),
               'production_active_configuration': active, 'historical_file_hashes_preserved': True,
               'uploaded_files_verified': len(freeze['files']), 'network_audit_sha256': audit_hash,
               'new_provider_calls_in_exit': 0, 'provider_http_receive_count': 'UNKNOWN', 'actual_provider_cost': 'UNKNOWN',
               'new_pin_intents': 0, 'new_basic_intents': 0}
    if epoch.exists():
        with (epoch / 'final-receipt.json').open('x') as output: json.dump(receipt, output, indent=2)
    print(json.dumps(receipt))
    sys.exit(0)
lock = (epoch / 'execution.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
start = json.loads((epoch / 'start-receipt.json').read_text())
prepare = json.loads((epoch / 'prepare-receipt.json').read_text())
budget = json.loads((epoch / 'budget.json').read_text())
assert budget['historical_inference_requests'] == 35 and budget['historical_gateway_pin_confirmed_sends'] == 34
assert 0 <= budget['new_pin_intents'] <= 1 and 0 <= budget['new_basic_intents'] <= 1
assert budget['new_pin_intents'] + budget['new_basic_intents'] <= 2
assert budget['new_inference_limit'] == 2 and budget['cumulative_inference_limit'] == 37
budget['stopped'] = True
import sys
sys.path.insert(0,str(epoch))
import cpar_build_json_diagnostic as diagnostic
diagnostic.save_budget(epoch / 'budget.json',budget)
pid = int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
assert pid == start['pid']
proc = pathlib.Path('/proc', str(pid))
assert (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19] == start['process_start']
args = (proc / 'cmdline').read_bytes().decode().split('\0')
assert args[args.index('--state-dir') + 1] == str(epoch / 'state')
assert hashlib.sha256((proc / 'exe').read_bytes()).hexdigest() == start['binary_sha256']
subprocess.run(['systemctl', 'stop', unit], check=True)
assert int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID'])) == 0
assert not proc.exists()
for family in ['tcp', 'tcp6']:
    assert not any(row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181]
                   for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:])
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
audit = base / 'audit/chat-json-rehearsal-20261013.jsonl'
audit_hash = hashlib.sha256(audit.read_bytes() if audit.exists() else b'').hexdigest()
time.sleep(5)
assert hashlib.sha256(audit.read_bytes() if audit.exists() else b'').hexdigest() == audit_hash
network_events = [json.loads(line)['event'] for line in (audit.read_bytes() if audit.exists() else b'').splitlines()]
assert all(event in ['dns_build_allowed', 'connect_build_allowed'] for event in network_events)
assert all(digest(base / name) == value for name, value in start['historical_file_hashes'].items())
assert production() == start['production_before']
source_args = pathlib.Path('/proc', start['production_before']['MainPID'], 'cmdline').read_bytes().decode().split('\0')
credentials = pathlib.Path(source_args[source_args.index('--credential-dir') + 1])
client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
headers = {'X-Management-Key': (credentials / 'management-key').read_text().strip()}
with client.open(urllib.request.Request('http://127.0.0.1:18181/admin/config-versions', headers=headers), timeout=10) as response:
    active = [{key: row[key] for key in ['id', 'revision', 'status']}
              for row in json.load(response) if row['status'] == 'active']
assert active == start['production_active_configuration']
with sqlite3.connect('file:' + str(epoch / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    db.row_factory = sqlite3.Row
    counts = dict(db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type'))
    delta = {key: value - prepare['inherited_event_counts'].get(key, 0) for key, value in counts.items()}
    ledger_count = db.execute('SELECT count(*) FROM billing_ledger_entries').fetchone()[0]
    account = db.execute('SELECT revision,worker_claim_kind FROM grok_accounts').fetchall()
    assert len(account) == 1 and tuple(account[0]) == (start['native_revision'], None)
    assert delta.get('attempt', 0) <= budget['new_basic_intents']
    attempts = [json.loads(row[0])['attempt'] for row in db.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='attempt'")]
    billing = [dict(row) for row in db.execute('SELECT cost_microunits,cost_confidence,billing_status FROM billing_ledger_entries')]
pins = [json.loads(path.read_text()) for path in epoch.glob('*/pin-private.json')]
pin_send_accounting_complete = len(pins) == budget['new_pin_intents'] and all((pin.get('attempt_count') == 1 and pin.get('upstream_sent') is True) or (pin.get('attempt_count') == 0 and pin.get('upstream_sent') is False) for pin in pins)
pin_known_sends = sum(pin.get('attempt_count') == 1 and pin.get('upstream_sent') is True for pin in pins)
pin_actual_sends = pin_known_sends if pin_send_accounting_complete else None
# A completed successful Attempt confirms a send. Missing/failed attribution does
# not establish zero sends, even when shutdown itself is fully verified.
basic_confirmed_sends = sum(row.get('outcome') == 'succeeded' for row in attempts) - prepare['inherited_event_counts'].get('attempt', 0)
basic_send_accounting_complete = (
    delta.get('request', 0) == delta.get('request_finished', 0)
    == delta.get('attempt', 0) == basic_confirmed_sends == budget['new_basic_intents']
)
send_accounting_complete = pin_send_accounting_complete and basic_send_accounting_complete
files = {}
for path in epoch.rglob('*'):
    assert not path.is_symlink()
    if path.is_dir():
        assert stat.S_IMODE(path.stat().st_mode) == 0o700
    elif path.is_file():
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
        files[str(path.relative_to(base))] = {'sha256': digest(path), 'bytes': path.stat().st_size, 'mode': '0600'}
receipt = {'status': 'PASS; resource exit and count reconciliation' if send_accounting_complete else 'PARTIAL; resource exit PASS; send count UNKNOWN',
           'resource_exit_status': 'PASS', 'count_reconciliation_status': 'PASS' if send_accounting_complete else 'UNKNOWN',
           'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
           'source_sha': start['source_sha'], 'binary_sha256': start['binary_sha256'],
           'process_pid_before_stop': pid, 'main_pid_after_stop': 0,
           'temporary_ports_listening': [], 'post_stop_flat_seconds': 5,
           'network_audit_sha256': audit_hash, 'network_audit_events': network_events,
           'new_pin_intents': budget['new_pin_intents'], 'new_basic_intents': budget['new_basic_intents'],
           'new_pin_actual_sends': pin_actual_sends, 'new_pin_confirmed_sends_lower_bound': pin_known_sends,
           'pin_send_accounting_complete': pin_send_accounting_complete, 'new_basic_attempts': delta.get('attempt', 0),
           'basic_send_accounting_complete': basic_send_accounting_complete,
           'new_basic_confirmed_sends_lower_bound': basic_confirmed_sends,
           'new_gateway_pin_confirmed_sends_lower_bound': basic_confirmed_sends + pin_known_sends,
           'new_gateway_pin_actual_sends': basic_confirmed_sends + pin_known_sends if send_accounting_complete else None,
           'new_event_counts': delta, 'inherited_event_counts': prepare['inherited_event_counts'],
           'new_ledger_rows': ledger_count - prepare['inherited_ledger_count'],
           'all_attempt_outcomes_including_inherited': [row.get('outcome') for row in attempts],
           'all_attempt_retry_decisions_including_inherited': [row.get('retry_decision') for row in attempts],
           'billing_including_inherited': billing, 'budget': budget,
           'account_native_revision': start['native_revision'], 'worker_claim': None,
           'production_after': production(), 'production_active_configuration': active,
           'historical_file_hashes_preserved': True, 'private_files': files,
           'provider_http_receive_count': 'UNKNOWN; independent Provider receive counter unavailable',
           'new_provider_calls_in_exit': 0, 'management_get_intents_in_exit': 1}
with (epoch / 'final-receipt.json').open('x') as output:
    json.dump(receipt, output, indent=2)
public = {key: value for key, value in receipt.items() if key != 'private_files'}
public['private_file_count'] = len(files)
public['private_inventory_sha256'] = hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()
print(json.dumps(public))
