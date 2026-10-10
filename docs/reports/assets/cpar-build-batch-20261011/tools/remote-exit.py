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
import time
import urllib.request

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/six-combinations-20261011'
unit = 'cpar-build-acceptance-20261010.service'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert not (epoch / 'final-receipt.json').exists()
lock = (epoch / 'execution.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
start = json.loads((epoch / 'start-receipt.json').read_text())
prepare = json.loads((epoch / 'prepare-receipt.json').read_text())
budget = json.loads((epoch / 'budget.json').read_text())
assert budget['historical_inference_requests'] == 9
assert budget['new_pin_intents'] <= 6 and budget['new_basic_intents'] <= 54
assert budget['new_pin_intents'] + budget['new_basic_intents'] <= 60
budget['stopped'] = True
(epoch / 'budget.json').write_text(json.dumps(budget, indent=2) + '\n')
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
audit = base / 'audit/six-combinations-20261011.jsonl'
audit_hash = digest(audit)
time.sleep(5)
assert digest(audit) == audit_hash
network_events = [json.loads(line)['event'] for line in audit.read_bytes().splitlines()]
assert all(event in ['dns_build_allowed', 'connect_build_allowed'] for event in network_events)
assert all(digest(base / name) == value for name, value in start['historical_file_hashes'].items())
source = ast.parse((base / 'private/bootstrap.py').read_text())
functions = ast.Module(body=[node for node in source.body if isinstance(node, ast.FunctionDef)
                            and node.name == 'production'], type_ignores=[])
exec(compile(functions, 'production-proof', 'exec'))
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
    assert len(account) == 1 and tuple(account[0]) == (134, None)
    assert delta.get('attempt', 0) <= budget['new_basic_intents']
    attempts = [json.loads(row[0])['attempt'] for row in db.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='attempt'")]
    billing = [dict(row) for row in db.execute('SELECT cost_microunits,cost_confidence,billing_status FROM billing_ledger_entries')]
pins = [json.loads(path.read_text()) for path in epoch.glob('*/pin-private.json')]
assert all(pin.get('attempt_count') == 1 and pin.get('upstream_sent') is True for pin in pins)
files = {}
for path in epoch.rglob('*'):
    assert not path.is_symlink()
    if path.is_dir():
        assert stat.S_IMODE(path.stat().st_mode) == 0o700
    elif path.is_file():
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
        files[str(path.relative_to(base))] = {'sha256': digest(path), 'bytes': path.stat().st_size, 'mode': '0600'}
receipt = {'status': 'PASS; exit and count reconciliation',
           'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
           'source_sha': start['source_sha'], 'binary_sha256': start['binary_sha256'],
           'process_pid_before_stop': pid, 'main_pid_after_stop': 0,
           'temporary_ports_listening': [], 'post_stop_flat_seconds': 5,
           'network_audit_sha256': audit_hash, 'network_audit_events': network_events,
           'new_pin_intents': budget['new_pin_intents'], 'new_basic_intents': budget['new_basic_intents'],
           'new_pin_actual_sends': len(pins), 'new_basic_attempts': delta.get('attempt', 0),
           'new_event_counts': delta, 'inherited_event_counts': prepare['inherited_event_counts'],
           'new_ledger_rows': ledger_count - prepare['inherited_ledger_count'],
           'all_attempt_outcomes_including_inherited': [row.get('outcome') for row in attempts],
           'all_attempt_retry_decisions_including_inherited': [row.get('retry_decision') for row in attempts],
           'billing_including_inherited': billing, 'budget': budget,
           'account_native_revision': 134, 'worker_claim': None,
           'production_after': production(), 'production_active_configuration': active,
           'historical_file_hashes_preserved': True, 'private_files': files,
           'provider_http_receive_count': 'UNKNOWN; independent Provider receive counter unavailable',
           'new_provider_calls_in_exit': 0, 'management_get_intents_in_exit': 1}
with (epoch / 'final-receipt.json').open('x') as output:
    json.dump(receipt, output, indent=2)
print(json.dumps(receipt))
