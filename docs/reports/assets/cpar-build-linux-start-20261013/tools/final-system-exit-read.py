"""Read-only fresh resource proof; synthetic state and nonsensitive budgets only."""
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import stat
import subprocess
import time

base = Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert stat.S_IMODE(base.stat().st_mode) == 0o700
results = []
audits = []
for number in (1, 2, 3):
    epoch = base / 'private' / ('linux-start-synthetic-20261013-' + str(number))
    assert json.loads((epoch / 'owner.json').read_text()) == {
        'task': 'cpar-build-linux-start-20261013', 'run': number,
        'scenario': {1: 'normal', 2: 'exec_failure', 3: 'management_auth_failure'}[number]}
    value = json.loads((epoch / 'system-result.json').read_text())
    unit = 'cpar-' + epoch.name + '.service'
    assert int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID'], timeout=5)) == 0
    assert subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=LoadState'], text=True, timeout=5).strip() == 'not-found'
    if value['identity'] is not None: assert not Path('/proc', str(value['identity']['pid'])).exists()
    budget = json.loads((epoch / 'budget.json').read_text())
    assert budget['stopped'] and budget['provider_intents'] == 0
    with sqlite3.connect('file:' + str(epoch / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
        assert db.execute('SELECT id,enabled,auth_status FROM grok_accounts').fetchall() == [('synthetic-disabled', 0, 'disabled')]
        events = dict(db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type'))
        ledger = db.execute('SELECT count(*) FROM billing_ledger_entries').fetchone()[0]
        assert not events and ledger == 0
    for path in epoch.rglob('*'):
        assert not path.is_symlink()
        expected = 0o700 if path.is_dir() or path.parent.name == 'bin' else 0o600
        assert stat.S_IMODE(path.stat().st_mode) == expected
    audit = base / 'audit' / (epoch.name + '.jsonl')
    assert not audit.exists() or audit.stat().st_size == 0
    audits.append(audit)
    results.append({'run': number, 'resource_exit_status': 'PASS', 'MainPID': 0, 'LoadState': 'not-found',
        'old_pid_absent': True, 'stopped_zero_provider_budget': True, 'new_event_counts': events,
        'new_ledger_rows': ledger, 'private_modes_verified': True,
        'native_launch_capture_present': (epoch / 'launch-native.stderr').exists()})
for family in ['tcp', 'tcp6']:
    assert not any(row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in (19180, 19181)
        for row in Path('/proc/net', family).read_text().splitlines()[1:])
time.sleep(5)
assert all(not audit.exists() or audit.stat().st_size == 0 for audit in audits)
budgets = []
for path in sorted((base / 'private').glob('*/budget.json')):
    budget = json.loads(path.read_text())
    assert budget['stopped'] is True
    budgets.append({'path': str(path.relative_to(base)), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'stopped': True})
raw = subprocess.check_output(['systemctl', 'show', 'cpa-rust-gateway.service',
    '--property=MainPID,NRestarts,ExecMainStartTimestampMonotonic,ActiveState,SubState,FragmentPath'], text=True, timeout=5)
production = dict(row.split('=', 1) for row in raw.splitlines() if '=' in row)
production.update(unit_sha256=hashlib.sha256(Path(production['FragmentPath']).read_bytes()).hexdigest(),
    binary_sha256=hashlib.sha256(Path('/proc', production['MainPID'], 'exe').read_bytes()).hexdigest(),
    current_target=str(Path('/opt/cpa-rust-gateway/current').resolve()),
    caddyfile_sha256=hashlib.sha256(Path('/etc/caddy/Caddyfile').read_bytes()).hexdigest())
assert not (base / 'private/chat-json-linux-20261013').exists()
assert not (base / 'bin/chat-json-linux-20261013').exists()
print(json.dumps({'status': 'PASS', 'runs': results, 'post_stop_flat_seconds': 5,
    'all_stopped_budgets': budgets, 'production': production,
    'systemd_version': subprocess.check_output(['systemd', '--version'], text=True).splitlines()[0],
    'kernel': subprocess.check_output(['uname', '-srm'], text=True).strip(),
    'formal_remote_epoch_status': 'NOT_CREATED', 'formal_upload_status': 'NOT_CREATED',
    'new_provider_calls': 0, 'provider_independent_receives': 'UNKNOWN', 'actual_provider_cost': 'UNKNOWN'}))
