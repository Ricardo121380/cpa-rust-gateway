"""Read-only exit proof for a batch blocked before upload/epoch creation."""
import ast
import datetime
import hashlib
import json
import os
import pathlib
import stat
import subprocess
import sys
import time
import urllib.request

base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/chat-json-502-20261013'
uploaded = base / 'bin/chat-json-502-20261013'
audit = base / 'audit/chat-json-502-20261013.jsonl'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert not base.is_symlink() and base.stat().st_uid == 0
assert stat.S_IMODE(base.stat().st_mode) == 0o700
assert len(sys.argv) == 2
expected = json.loads(sys.argv[1])
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
historical = {str(path.relative_to(base)): sha(path)
              for path in base.rglob('*') if path.is_file() and not path.is_symlink()}
old = base / 'private/chat-json-502-20261011'
prepare = json.loads((old / 'prepare-receipt.json').read_text())
assert json.loads((old / 'budget.json').read_text())['stopped'] is True
assert all(sha(base / name) == digest for name, digest in prepare['historical_file_hashes'].items())
freeze = json.loads((base / 'bin/chat-json-502-20261011/freeze.json').read_text())
assert all(sha(base / 'bin/chat-json-502-20261011' / name) == digest
           for name, digest in freeze['files'].items())
unit = dict(row.split('=', 1) for row in subprocess.check_output([
    'systemctl', 'show', 'cpar-build-acceptance-20261010.service',
    '--property=MainPID,LoadState,ActiveState,SubState'], text=True).splitlines())
assert unit['MainPID'] == '0' and unit['LoadState'] == 'not-found'
ports = [int(row.split()[1].rsplit(':', 1)[1], 16)
         for family in ['tcp', 'tcp6']
         for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:]
         if row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181]]
assert not ports and not epoch.exists() and not uploaded.exists() and not audit.exists()
assert stat.S_IMODE((base / 'private').stat().st_mode) == 0o700
# Capture and owned epoch permissions retain their original strict boundary.
# The three earlier bootstrap receipts have pre-existing 0644 modes under the
# 0700 private parent; compare that observed baseline without modifying history.
legacy_modes = {'private/recovered-launch.json': 0o644,
                'private/guard-preflight-corrected.json': 0o644,
                'private/bootstrap-receipt.json': 0o644}
for name in legacy_modes:
    assert name in prepare['historical_file_hashes']
mode_inventory = {}
for path in (base / 'private').rglob('*'):
    assert not path.is_symlink() and path.stat().st_uid == 0
    name = str(path.relative_to(base))
    observed_mode = stat.S_IMODE(path.stat().st_mode)
    assert observed_mode == (0o700 if path.is_dir() else legacy_modes.get(name, 0o600))
    mode_inventory[name] = observed_mode
competitors = []
for proc in pathlib.Path('/proc').iterdir():
    if not proc.name.isdigit() or int(proc.name) == os.getpid():
        continue
    try:
        if str(base).encode() in (proc / 'cmdline').read_bytes() or any(
                str(fd.resolve()).startswith(str(base)) for fd in (proc / 'fd').iterdir()):
            competitors.append(int(proc.name))
    except (PermissionError, FileNotFoundError, ProcessLookupError):
        pass
assert not competitors
tree = ast.parse((base / 'private/bootstrap.py').read_text())
node = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'production')
exec(compile(ast.Module(body=[node], type_ignores=[]), 'production-proof', 'exec'))
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
time.sleep(5)
assert not epoch.exists() and not uploaded.exists() and not audit.exists()
assert all(sha(base / name) == digest for name, digest in historical.items())
assert all(stat.S_IMODE((base / name).stat().st_mode) == mode for name, mode in mode_inventory.items())
assert production() == expected['production']
print(json.dumps({'status': 'PASS', 'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  'owned_process_stop_status': 'NOT_CREATED_VERIFIED', 'temporary_unit': unit,
                  'temporary_ports': ports, 'competing_processes': competitors,
                  'new_epoch_upload_audit_absent': True, 'flat_observation_seconds': 5,
                  'old_stopped_epoch_unchanged': True, 'historical_frozen_files_verified': len(prepare['historical_file_hashes']),
                  'old_uploaded_payloads_verified': len(freeze['files']), 'protected_root_files_flat': len(historical),
                  'protected_root_inventory_sha256': hashlib.sha256(json.dumps(historical, sort_keys=True).encode()).hexdigest(),
                  'owned_epoch_and_capture_modes': '0700/0600; root owner; no symlinks',
                  'private_parent_mode': '0700', 'historical_bootstrap_modes_preserved': 'three existing 0644 receipts',
                  'production': production(), 'production_config': active,
                  'new_inference_intents': 0, 'new_gateway_pin_confirmed_sends': 0,
                  'provider_independent_receive_count': 'UNKNOWN', 'actual_provider_cost': 'UNKNOWN',
                  'management_get_intents': 1, 'production_writes': 0, 'remote_files_written': 0}))
