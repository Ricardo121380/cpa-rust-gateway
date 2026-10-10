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

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/revalidation-20261010'
unit = 'cpar-build-acceptance-20261010.service'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert not (epoch / 'seal.json').exists()
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
final = json.loads((epoch / 'final-receipt.json').read_text())
assert int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID'])) == 0
assert not pathlib.Path('/proc', str(final['process_pid_before_stop'])).exists()
for family in ['tcp', 'tcp6']:
    assert not any(row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181] for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:])
budget = json.loads((epoch / 'budget.json').read_text())
assert budget['stopped'] and budget['new_pin_intents'] == budget['new_basic_intents'] == 1
assert budget['new_catalog_intents'] == 0
audit = base / 'audit/revalidation-20261010.jsonl'
assert digest(audit) == final['network_audit_sha256']
time.sleep(5)
assert digest(audit) == final['network_audit_sha256']
assert all(digest(base / path) == value for path, value in final['historical_file_hashes'].items())
source = ast.parse((base / 'private/bootstrap.py').read_text())
functions = ast.Module(body=[node for node in source.body if isinstance(node, ast.FunctionDef) and node.name == 'production'], type_ignores=[])
exec(compile(functions, 'production-proof', 'exec'))
assert production() == final['production_after']
competing = []
for path in pathlib.Path('/proc').iterdir():
    if not path.name.isdigit() or int(path.name) == os.getpid():
        continue
    try:
        if str(base / 'state').encode() in (path / 'cmdline').read_bytes():
            competing.append(int(path.name))
        if any(str(fd.resolve()).startswith(str(base / 'state/control.sqlite3')) for fd in (path / 'fd').iterdir()):
            competing.append(int(path.name))
    except (PermissionError, FileNotFoundError, ProcessLookupError):
        pass
assert not competing
with sqlite3.connect('file:' + str(base / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    counts = dict(db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type'))
    assert counts == final['durable_event_counts']
files = {}
for path in epoch.rglob('*'):
    assert not path.is_symlink()
    if path.is_dir():
        assert stat.S_IMODE(path.stat().st_mode) == 0o700
    elif path.is_file():
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
        files[str(path.relative_to(base))] = {'sha256': digest(path), 'mode': '0600', 'bytes': path.stat().st_size}
assert stat.S_IMODE(epoch.stat().st_mode) == 0o700
receipt = {'status': 'PASS; evidence retention and exit only',
           'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
           'temporary_service': unit, 'main_pid': 0, 'temporary_ports_listening': [],
           'post_stop_network_audit_unchanged': True, 'flat_seconds': 5,
           'network_audit_sha256': final['network_audit_sha256'],
           'competing_task_state_processes': competing,
           'production_identity_unchanged': True, 'production_after': production(),
           'production_active_configuration_readback': final['production_active_configuration'],
           'production_active_configuration_observed_at_utc': final['observed_at_utc'],
           'new_pin_intents': 1, 'new_basic_intents': 1, 'new_catalog_intents': 0,
           'networked_inference_requests_in_seal': 0, 'management_requests_in_seal': 0,
           'durable_event_counts_unchanged': counts,
           'old_failed_evidence_preserved': True, 'private_root_mode': '0700',
           'private_files': files, 'provider_http_receive_count': 'UNKNOWN'}
with (epoch / 'seal.json').open('x') as output:
    json.dump(receipt, output, indent=2)
print(json.dumps(receipt))
