"""Four exclusive durable zero-Provider slots. No restart, retry or fifth slot."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

from startup_probe import save

os.umask(0o077)
sys.dont_write_bytecode = True
public = Path(__file__).resolve().parent.parent
private = public.parents[3] / 'output/cpar-build-linux-final-20261013'
letter = sys.argv[1]
assert letter in ('A', 'B', 'C', 'D')
lock = (private / 'system-execution.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
state_path = private / 'system-stage.json'
state = json.loads(state_path.read_text()) if state_path.exists() else {'runs': [], 'stopped': False, 'max_runs': 4, 'provider_intent_limit': 0}
assert not state['stopped'] and len(state['runs']) < 4
assert letter == ('A', 'B', 'C', 'D')[len(state['runs'])]
assert all(row['status'] == 'PASS' for row in state['runs'])
freeze_path = public / 'system-freeze.json'
freeze = json.loads(freeze_path.read_text())
assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == freeze['controller_sha256']
files = {name: (public / 'tools' / name).read_text() for name in ['startup_probe.py', 'launch_recipe.py',
    'linux-system-validation.py', 'remote-exit.py', 'system_exit_adapter.py']}
files['recipe.json'] = (private / 'recipe-private.json').read_text()
for name in ['artifact-manifest.json', 'gateway-build-metadata.json']:
    files[name] = (private / 'payload' / name).read_text()
assert {name: hashlib.sha256(raw.encode()).hexdigest() for name, raw in files.items()} == freeze['files']
review = json.loads((public / 'review-system-source.json').read_text())
assert review['axes']['Standards']['status'] == review['axes']['Spec']['status'] == 'PASS'
assert review['system_freeze_sha256'] == hashlib.sha256(freeze_path.read_bytes()).hexdigest()
row = {'run': letter, 'status': 'UNKNOWN', 'freeze_sha256': hashlib.sha256(freeze_path.read_bytes()).hexdigest()}
state['runs'].append(row)
save(state_path, state)  # The slot is consumed before physical dispatch.
script = '''import pathlib,json,os,hashlib,sys
os.umask(0o077);sys.dont_write_bytecode=True
base=pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
assert json.loads((base/'owner.json').read_text())['task']=='cpar-build-20261010'
directory=base/'bin/linux-final-tools-20261013-LETTER'
files=FILES
assert not directory.exists()
directory.mkdir(mode=0o700)
for name,value in files.items():
    with (directory/name).open('x') as output:output.write(value)
assert not directory.is_symlink() and (directory.stat().st_mode&0o777)==0o700
assert all(hashlib.sha256((directory/name).read_bytes()).hexdigest()==hashlib.sha256(value.encode()).hexdigest() for name,value in files.items())
sys.path.insert(0,str(directory));sys.argv=[str(directory/'linux-system-validation.py'),'LETTER']
exec(compile((directory/'linux-system-validation.py').read_text(),sys.argv[0],'exec'),{'__name__':'__main__','__file__':sys.argv[0]})
'''.replace('files=FILES', 'files=' + repr(files)).replace('LETTER', letter)
(private / ('system-' + letter + '-executed.py')).write_text(script)
command = ['ssh', '-T', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', 'new-vps', 'sudo -n python3 -']
receipt = {'command': command, 'remote_script_sha256': hashlib.sha256(script.encode()).hexdigest(),
    'freeze_sha256': row['freeze_sha256'], 'run': letter}
try:
    result = subprocess.run(command, input=script.encode(), capture_output=True, timeout=220)
    receipt['remote_exit_code'] = result.returncode
    stdout, stderr = result.stdout, result.stderr
    (private / ('system-' + letter + '.stdout')).write_bytes(stdout)
    (private / ('system-' + letter + '.stderr')).write_bytes(stderr)
    receipt.update(stdout_sha256=hashlib.sha256(stdout).hexdigest(), stderr_sha256=hashlib.sha256(stderr).hexdigest())
    if stdout: receipt['result'] = json.loads(stdout)
    save(public / ('system-' + letter + '.json'), receipt)
    assert result.returncode == 0 and receipt['result']['status'] == receipt['result']['resource_exit_status'] == 'PASS'
    row['status'] = 'PASS'
except BaseException as error:
    state['stopped'] = True
    row['exception_type'] = type(error).__name__
    row['status'] = 'FAIL' if receipt.get('result', {}).get('status') == 'FAIL' else 'UNKNOWN'
    if isinstance(error, subprocess.TimeoutExpired):
        for name in ['stdout', 'stderr']:
            raw = getattr(error, name, None) or b''
            (private / ('system-' + letter + '.' + name)).write_bytes(raw)
            receipt[name + '_sha256'] = hashlib.sha256(raw).hexdigest()
        receipt.update(remote_exit_code=None, timeout_seconds=220)
    receipt['controller_exception_type'] = type(error).__name__
    save(public / ('system-' + letter + '.json'), receipt)
    raise
finally:
    if len(state['runs']) == 4: state['stopped'] = True
    save(state_path, state)
print(json.dumps({'run': letter, 'status': row['status'], 'remaining_system_slots': 4 - len(state['runs'])}))
