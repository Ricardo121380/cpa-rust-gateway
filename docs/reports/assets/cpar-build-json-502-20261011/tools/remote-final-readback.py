import ast
import datetime
import hashlib
import json
import os
import pathlib
import sqlite3
import stat
import subprocess
import time

base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/chat-json-502-20261011'
uploaded = base / 'bin/chat-json-502-20261011'
prepare = json.loads((epoch / 'prepare-receipt.json').read_text())
exit_receipt = json.loads((epoch / 'prelaunch-final-receipt.json').read_text())
budget = json.loads((epoch / 'budget.json').read_text())
assert budget == exit_receipt['budget'] and budget['stopped']
assert budget['new_pin_intents'] == budget['new_basic_intents'] == 0
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
unit = {key: subprocess.check_output(['systemctl', 'show', 'cpar-build-acceptance-20261010.service', '--value', '--property=' + key], text=True).strip() for key in exit_receipt['unit_state']}
assert unit == exit_receipt['unit_state']
ports = [int(row.split()[1].rsplit(':', 1)[1], 16) for family in ['tcp', 'tcp6'] for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:] if row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181]]
assert not ports
competitors = []
for proc in pathlib.Path('/proc').iterdir():
    if not proc.name.isdigit() or int(proc.name) == os.getpid():
        continue
    try:
        if str(epoch / 'state').encode() in (proc / 'cmdline').read_bytes() or any(str(fd.resolve()).startswith(str(epoch / 'state/control.sqlite3')) for fd in (proc / 'fd').iterdir()):
            competitors.append(int(proc.name))
    except (PermissionError, FileNotFoundError, ProcessLookupError):
        pass
assert not competitors
audit = base / 'audit/chat-json-502-20261011.jsonl'
assert not audit.exists()
time.sleep(5)
assert not audit.exists() and not any((epoch / 'native-capture').iterdir())
assert all(sha(base / name) == value for name, value in prepare['historical_file_hashes'].items())
bootstrap = ast.parse((base / 'private/bootstrap.py').read_text())
functions = ast.Module(body=[node for node in bootstrap.body if isinstance(node, ast.FunctionDef) and node.name == 'production'], type_ignores=[])
exec(compile(functions, 'production-proof', 'exec'))
assert production() == prepare['production_before']
with sqlite3.connect('file:' + str(epoch / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    counts = dict(db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type'))
    assert counts == prepare['inherited_event_counts']
    assert db.execute('SELECT count(*) FROM billing_ledger_entries').fetchone()[0] == prepare['inherited_ledger_count']
    assert db.execute('SELECT revision,worker_claim_kind FROM grok_accounts').fetchall() == [(prepare['copied_native_revision'], None)]
private_files = private_directories = 0
assert stat.S_IMODE(epoch.stat().st_mode) == 0o700
for path in epoch.rglob('*'):
    assert not path.is_symlink()
    if path.is_dir():
        assert stat.S_IMODE(path.stat().st_mode) == 0o700
        private_directories += 1
    elif path.is_file():
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
        private_files += 1
freeze = json.loads((uploaded / 'freeze.json').read_text())
assert freeze['driver_source_sha'] == '38504a1eabeaacfcb548e23de7a61a545351b35b'
assert stat.S_IMODE(uploaded.stat().st_mode) == 0o700 and not uploaded.is_symlink()
for name, digest in freeze['files'].items():
    path = uploaded / name
    assert not path.is_symlink() and sha(path) == digest
    assert stat.S_IMODE(path.stat().st_mode) == (0o700 if name == 'gateway-aarch64-unknown-linux-gnu' else 0o600)
assert stat.S_IMODE((uploaded / 'freeze.json').stat().st_mode) == 0o600
print(json.dumps({'status': 'PASS; read-only final exit readback', 'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'unit_state': unit, 'temporary_ports': ports, 'competing_processes': competitors, 'audit_absent_and_flat_seconds': 5, 'native_capture_files': 0, 'new_pin_intents': 0, 'new_chat_intents': 0, 'budget_stopped': True, 'new_event_and_ledger_rows': 0, 'source_writes': 0, 'private_file_count': private_files, 'private_directory_count': private_directories, 'private_modes': '0700/0600; no symlinks', 'historical_frozen_files_unchanged': len(prepare['historical_file_hashes']), 'uploaded_payloads_unchanged': len(freeze['files']), 'uploaded_driver_source_sha': freeze['driver_source_sha'], 'repaired_driver_uploaded': False, 'production_runtime_identity_unchanged': True, 'production_active_config_verified_at_exit': exit_receipt['production_active_configuration'], 'provider_inference_intents': 0, 'provider_independent_receive_count': 'UNKNOWN', 'actual_provider_cost': 'UNKNOWN'}))
