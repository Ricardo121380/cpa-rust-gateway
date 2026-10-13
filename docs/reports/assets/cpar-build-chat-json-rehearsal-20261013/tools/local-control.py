"""One explicitly selected batch action per invocation; no loop or implicit retry."""
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

sys.dont_write_bytecode = True
os.umask(0o077)
ROOT = Path(__file__).resolve().parents[5]
PUBLIC = Path(__file__).resolve().parent.parent
PRIVATE = ROOT / 'output/cpar-build-chat-json-rehearsal-20261013'
BASE = '/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010'
EPOCH = 'chat-json-rehearsal-20261013'
SSH = ['ssh', '-T', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', 'new-vps']


def save(path, value):
    temporary = path.with_suffix('.tmp')
    with temporary.open('w') as output:
        json.dump(value, output, indent=2)
        output.flush()
        os.fsync(output.fileno())
    temporary.replace(path)
    descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def load_state():
    path = PRIVATE / 'stage.json'
    if path.exists():
        return json.loads(path.read_text())
    return {'phase': 'PREPARED_LOCAL', 'stopped': False, 'observations': [], 'formal_actions': []}


def remote(script, name, timeout=150, stream=None, arguments=()):
    if stream is None:
        command = SSH + ['sudo -n python3 -']
        data = script.encode()
    else:
        # shlex.quote is shell quoting; credentials/native bytes are never arguments.
        import shlex
        command = SSH + ['sudo -n python3 -c ' + shlex.quote(script) + ' ' +
                         ' '.join(shlex.quote(value) for value in arguments)]
        data = stream
    result = subprocess.run(command, input=data, capture_output=True, timeout=timeout)
    (PRIVATE / (name + '.stdout')).write_bytes(result.stdout)
    (PRIVATE / (name + '.stderr')).write_bytes(result.stderr)
    receipt = {'exit_code': result.returncode,
               'executed_script_sha256': hashlib.sha256(script.encode()).hexdigest(),
               'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
               'stderr_sha256': hashlib.sha256(result.stderr).hexdigest(),
               'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    if result.returncode == 0:
        receipt['result'] = json.loads(result.stdout)
    save(PUBLIC / (name + '.json'), receipt)
    assert result.returncode == 0, 'remote_action_failed'
    return receipt['result']


def latch_remote():
    freeze_hash = hashlib.sha256((PRIVATE / 'payload/freeze.json').read_bytes()).hexdigest()
    script = f'''import pathlib,json,sys,os
os.umask(0o077)
base=pathlib.Path({BASE!r});epoch=base/'private/{EPOCH}';uploaded=base/'bin/{EPOCH}'
assert json.loads((base/'owner.json').read_text())['task']=='cpar-build-20261010'
if uploaded.exists():
    import hashlib
    assert hashlib.sha256((uploaded/'freeze.json').read_bytes()).hexdigest()=={freeze_hash!r}
if (epoch/'budget.json').exists():
    import fcntl
    lock=(epoch/'execution.lock').open('a');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    sys.path.insert(0,str(epoch));import cpar_build_json_diagnostic as diagnostic
    budget=json.loads((epoch/'budget.json').read_text());budget['stopped']=True
    diagnostic.save_budget(epoch/'budget.json',budget)
print(json.dumps({{'status':'PASS; stop latched','epoch_created':epoch.exists(),'upload_created':uploaded.exists(),'provider_calls':0}}))
'''
    return remote(script, 'stop-latch', timeout=40)


def main(action):
    frozen = json.loads((PRIVATE / 'payload/freeze.json').read_text())
    assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == frozen['local_controller_sha256']
    upload_script = (PUBLIC / 'tools/remote-upload.py').read_bytes()
    assert hashlib.sha256(upload_script).hexdigest() == frozen['upload_tool_sha256']
    no_launch_exit = (PUBLIC / 'tools/remote-no-launch-exit.py').read_bytes()
    assert hashlib.sha256(no_launch_exit).hexdigest() == frozen['no_launch_exit_tool_sha256']
    verified_payload = {}
    for name, expected_hash in frozen['files'].items():
        content = (PRIVATE / 'payload' / name).read_bytes()
        assert hashlib.sha256(content).hexdigest() == expected_hash
        verified_payload[name] = content
    qualification = {}
    exec(compile(verified_payload['qualification.py'], 'frozen-qualification.py', 'exec'), qualification)
    review = json.loads((PUBLIC / 'review-pre-execution.json').read_text())
    assert review['axes']['Standards']['status'] == review['axes']['Spec']['status'] == 'PASS'
    assert review['payload_freeze_sha256'] == hashlib.sha256((PRIVATE / 'payload/freeze.json').read_bytes()).hexdigest()
    state = load_state()
    if action == 'observe':
        assert not state['stopped'] and state['phase'] in ('PREPARED_LOCAL', 'WAITING_FOR_QUALIFICATION')
        now_ms = int(time.time() * 1000)
        qualification['may_observe'](state['observations'], now_ms)
        name = 'observation-' + str(len(state['observations']) + 1)
        # Persist the observation slot before any read, including a failed read.
        row = {'local_started_at_ms': now_ms, 'classification': 'UNKNOWN'}
        state['observations'].append(row)
        save(PRIVATE / 'stage.json', state)
        script = verified_payload['qualification.py'].decode() + '\n' + (
            verified_payload['remote-readiness.py'].decode().replace('from qualification import classify', ''))
        result = remote(script, name, timeout=40)
        if all(key in result for key in ('production', 'production_config')):
            state['qualification_baseline'] = {key: result[key] for key in ('production', 'production_config')}
            save(PRIVATE / 'stage.json', state)
        row['classification'] = qualification['classify'](result)
        state['phase'] = row['classification']
        save(PRIVATE / 'stage.json', state)
        print(json.dumps({'phase': state['phase'], 'observation': len(state['observations']),
                          'remaining_material_seconds': result['remaining_material_seconds']}))
        return
    assert action in ('upload', 'prepare', 'start', 'pin', 'chat', 'exit')
    expected = {'upload': 'PREPARE_WINDOW_AVAILABLE', 'prepare': 'UPLOADED',
                'start': 'EPOCH_PREPARED', 'pin': 'STARTED', 'chat': 'PIN_PASS'}
    if action != 'exit':
        assert not state['stopped'] and state['phase'] == expected[action]
    assert action not in state['formal_actions']
    if action == 'upload':
        result = json.loads((PUBLIC / ('observation-' + str(len(state['observations'])) + '.json')).read_text())['result']
        assert result['source_pool']['expires_at_ms'] > int(time.time() * 1000) + 3600000
        state['formal_epoch_started_at_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    # Reserve each formal operation before transport. Failure is never replayable.
    state['formal_actions'].append(action)
    state['phase'] = 'EXECUTING_' + action.upper()
    save(PRIVATE / 'stage.json', state)
    if action == 'upload':
        result = remote(upload_script.decode(), 'upload', stream=(PRIVATE / 'upload.tar').read_bytes(),
                        arguments=(hashlib.sha256((PRIVATE / 'payload/freeze.json').read_bytes()).hexdigest(),))
    elif action == 'exit' and 'prepare' not in state['formal_actions']:
        proof = {'baseline': state.get('qualification_baseline', {}),
                 'payload': {'freeze_sha256': hashlib.sha256((PRIVATE / 'payload/freeze.json').read_bytes()).hexdigest(),
                             'files': frozen['files']}}
        wrapper = 'import sys\nsys.argv = ' + repr(['frozen-no-launch-exit', json.dumps(proof)]) + '\n'
        result = remote(wrapper + no_launch_exit.decode(), 'exit', timeout=60)
    else:
        # Execute the reviewed uploaded bytes; no inline unreviewed remote edit.
        frozen = json.loads((PRIVATE / 'payload/freeze.json').read_text())
        expected_hash = frozen['files']['remote-' + action + '.py']
        wrapper = f'''import pathlib,hashlib
path=pathlib.Path({BASE!r})/'bin/{EPOCH}/remote-{action}.py'
assert hashlib.sha256(path.read_bytes()).hexdigest()=={expected_hash!r}
exec(compile(path.read_text(),str(path),'exec'))
'''
        if action == 'exit':
            wrapper = 'exit_expected_baseline = ' + repr(state['qualification_baseline']) + '\n' + wrapper
        result = remote(wrapper, action, timeout=180)
    if action in ('upload', 'prepare', 'start', 'pin'):
        assert result['status'] == 'PASS', 'formal_result_not_pass'
    state['phase'] = {'upload': 'UPLOADED', 'prepare': 'EPOCH_PREPARED', 'start': 'STARTED',
                      'pin': 'PIN_PASS', 'chat': 'CHAT_FINISHED', 'exit': 'EXITED'}[action]
    if action in ('chat', 'exit'):
        state['stopped'] = True
    save(PRIVATE / 'stage.json', state)
    print(json.dumps({'phase': state['phase'], 'stopped': state['stopped'], 'result_status': result['status']}))


if __name__ == '__main__':
    assert len(sys.argv) == 2
    execution_lock = (PRIVATE / 'local-execution.lock').open('a')
    try:
        fcntl.flock(execution_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        print(json.dumps({'status': 'BLOCKED; another local operation owns execution'}))
        sys.exit(2)
    try:
        main(sys.argv[1])
    except BaseException as error:
        state = load_state()
        state.update(stopped=True, failure_type=type(error).__name__)
        save(PRIVATE / 'stage.json', state)
        if 'formal_epoch_started_at_utc' in state:
            try:
                latch_remote()
            except BaseException as latch_error:
                state['remote_latch_status'] = 'UNKNOWN:' + type(latch_error).__name__
                save(PRIVATE / 'stage.json', state)
        print(json.dumps({'phase': state['phase'], 'stopped': True, 'failure_type': type(error).__name__}))
        sys.exit(1)
