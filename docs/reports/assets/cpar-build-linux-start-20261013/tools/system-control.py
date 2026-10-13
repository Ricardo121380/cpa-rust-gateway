"""One durable synthetic Linux slot per command; never retries unknown results."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

from startup_probe import save

os.umask(0o077)
public = Path(__file__).resolve().parent.parent
private = public.parents[3] / 'output/cpar-build-linux-start-20261013'
number = int(sys.argv[1])
assert number in (1, 2, 3)
state_path = private / 'system-stage.json'
state = json.loads(state_path.read_text()) if state_path.exists() else {'runs': [], 'stopped': False}
assert number == len(state['runs']) + 1
if state['stopped']:
    # The preset run2 ended with a verified absent unit/PID and successful exit.
    # Allocate only the untouched third independent slot. Never reopen its state
    # or either stopped per-run zero-Provider budget; their FAIL/UNKNOWN remain.
    assert number == 3
    previous = json.loads((public / 'system-2.json').read_text())['result']
    observed = previous['startup_observations']['observations'][1]['systemd']
    assert previous['preset_negative'] and previous['scenario'] == 'exec_failure'
    assert previous['resource_exit_status'] == 'PASS' and previous['production_identity_preserved']
    assert previous['identity'] is None and observed['MainPID'] == '0' and observed['LoadState'] == 'not-found'
    assert previous['provider_intents'] == previous['provider_requests'] == 0
    assert previous['failure_reason'] == 'exec_handshake_failed' and previous['launch']['returncode'] != 0
    state_path = private / 'system-third-stage.json'
    assert not state_path.exists()
else:
    assert all(row.get('status') == 'PASS' for row in state['runs'])
files = {name: (public / 'tools' / name).read_text() for name in ['startup_probe.py', 'launch_recipe.py', 'linux-system-validation.py']}
files['recipe.json'] = (private / 'recipe-private.json').read_text()
freeze_path = public / ('system-freeze-third.json' if number == 3 else 'system-freeze.json')
freeze = json.loads(freeze_path.read_text())
assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == freeze['controller_sha256']
assert {name: hashlib.sha256(value.encode()).hexdigest() for name, value in files.items()} == freeze['files']
row = {'run': number, 'status': 'UNKNOWN'}
state['runs'].append(row)
save(state_path, state)
script = '''import pathlib,json,os,hashlib,sys
os.umask(0o077)
base=pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
assert json.loads((base/'owner.json').read_text())['task']=='cpar-build-20261010'
directory=base/'bin/SYSTEM_DIRECTORY'
files=FILES
if not directory.exists():
    directory.mkdir(mode=0o700)
    for name,value in files.items():
        with (directory/name).open('x') as output:output.write(value)
assert not directory.is_symlink() and (directory.stat().st_mode&0o777)==0o700
assert all(hashlib.sha256((directory/name).read_bytes()).hexdigest()==hashlib.sha256(value.encode()).hexdigest() for name,value in files.items())
sys.path.insert(0,str(directory));sys.argv=[str(directory/'linux-system-validation.py'),NUMBER]
exec(compile((directory/'linux-system-validation.py').read_text(),sys.argv[0],'exec'),{'__name__':'__main__','__file__':sys.argv[0]})
'''.replace('files=FILES', 'files=' + repr(files)).replace('NUMBER', repr(str(number))).replace(
    'SYSTEM_DIRECTORY', 'linux-start-system-20261013-third' if number == 3 else 'linux-start-system-20261013')
command = ['ssh', '-T', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', 'new-vps', 'sudo -n python3 -']
try:
    result = subprocess.run(command, input=script.encode(), capture_output=True, timeout=170)
    (private / ('system-' + str(number) + '.stdout')).write_bytes(result.stdout)
    (private / ('system-' + str(number) + '.stderr')).write_bytes(result.stderr)
    receipt = {'command': command, 'remote_script_sha256': hashlib.sha256(script.encode()).hexdigest(),
        'freeze_sha256': hashlib.sha256(freeze_path.read_bytes()).hexdigest(),
        'remote_exit_code': result.returncode, 'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
        'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
    if result.stdout: receipt['result'] = json.loads(result.stdout)
    save(public / ('system-' + str(number) + '.json'), receipt)
    assert result.returncode == 0 and receipt['result']['status'] == 'PASS'
    row['status'] = 'PASS'
except BaseException as error:
    state['stopped'] = True
    row['exception_type'] = type(error).__name__
    raise
finally:
    save(state_path, state)
print(json.dumps({'run': number, 'status': row['status'], 'remaining_system_slots': 3 - len(state['runs'])}))
