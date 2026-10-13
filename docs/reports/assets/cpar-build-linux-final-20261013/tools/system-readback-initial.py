"""Read-only independent resource/raw-evidence readback; no validation slot or call."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

from startup_probe import save

os.umask(0o077)
public = Path(__file__).resolve().parent.parent
private = public.parents[3] / 'output/cpar-build-linux-final-20261013'
assert not (public / 'system-readback.json').exists()
expected = {letter: json.loads((public / ('system-' + letter + '.json')).read_text())['result'] for letter in 'ABCD'}
assert all(row['status'] == row['resource_exit_status'] == 'PASS' for row in expected.values())
freeze = json.loads((public / 'system-freeze.json').read_text())
script = '''import hashlib,json,os,pathlib,subprocess,sys
sys.dont_write_bytecode=True
base=pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
expected=EXPECTED
system_freeze=SYSTEM_FREEZE
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
sys.path.insert(0,str(base/'bin/linux-final-tools-20261013-A'))
assert sha(base/'bin/linux-final-tools-20261013-A/system_exit_adapter.py')==system_freeze['files']['system_exit_adapter.py']
from system_exit_adapter import production_identity
rows=[]
for letter,record in expected.items():
    epoch=pathlib.Path(record['epoch']);unit=record['unit'];uploaded=base/'bin'/epoch.name
    result_path=epoch/'system-result.json';assert json.loads(result_path.read_text())==record
    raw=subprocess.check_output(['systemctl','show',unit,'--property=LoadState,MainPID,ActiveState,SubState'],text=True,timeout=5)
    state=dict(line.split('=',1) for line in raw.splitlines() if '=' in line)
    assert state['LoadState']=='not-found' and state['MainPID']=='0'
    assert not (pathlib.Path('/sys/fs/cgroup/system.slice')/unit).exists()
    budget=json.loads((epoch/'budget.json').read_text())
    assert budget['stopped'] and budget['provider_intent_limit']==budget['provider_intents']==budget['new_pin_intents']==budget['new_basic_intents']==0
    frozen=json.loads((epoch/'freeze.json').read_text());assert frozen==record['freeze']==json.loads((uploaded/'freeze.json').read_text())
    for name,digest in frozen['files'].items():assert sha(uploaded/name)==digest
    stop=json.loads((epoch/'startup-stop-intent.json').read_text());assert stop['stop_intent']
    identity=record['identity']
    if identity:
        assert stop['pid']==identity['pid'] and stop['process_start']==identity['process_start']
        assert not pathlib.Path('/proc',str(identity['pid'])).exists()
    observed=json.loads((epoch/'startup-observations.json').read_text())
    assert observed==record['startup_observations']
    audit=base/'audit'/(epoch.name+'.jsonl');raw_audit=audit.read_bytes() if audit.exists() else b''
    assert hashlib.sha256(raw_audit).hexdigest()==record['audit_sha256']
    detail={'unit_state':state,'result_sha256':sha(result_path),'stop_intent_sha256':sha(epoch/'startup-stop-intent.json'),
        'audit_exists':audit.exists(),'audit_sha256':record['audit_sha256'],'audit_bytes':len(raw_audit)}
    if letter=='A':
        start=json.loads((epoch/'start-receipt.json').read_text());invocation=json.loads((epoch/'exit-invocation.json').read_text())
        assert sha(epoch/'startup-observations.json')==start['startup_observations_sha256']
        assert invocation['formal_source_sha256']==system_freeze['files']['remote-exit.py']
        assert invocation['adapter_sha256']==system_freeze['files']['system_exit_adapter.py']
        assert invocation['adapted_source_sha256']==system_freeze['adapted_exit_A_sha256']==sha(epoch/'system-exit-executed.py')
        final=json.loads((epoch/'final-receipt.json').read_text());assert final==record['complete_exit_receipt']
        assert sha(epoch/'exit-observations.json')==final['exit_observations_sha256']
        assert sha(epoch/'startup-stop-intent.json')==final['stop_intent_sha256']
        detail.update(exit_invocation=invocation,startup_observations_preserved=True)
    elif letter=='B':
        native=record['exec_native'];assert sha(epoch/'exec-systemd-native.txt')==native['show_sha256']
        assert sha(epoch/'exec-journal-native.jsonl')==native['journal_sha256']
        assert sha(epoch/'exec-failure-evidence.json')==json.loads((epoch/'before-reset-failed.json').read_text())['captured_exec_native_sha256']
        assert not pathlib.Path('/proc',native['systemd']['ExecMainPID']).exists()
        messages=[json.loads(line).get('MESSAGE','') for line in (epoch/'exec-journal-native.jsonl').read_bytes().splitlines()]
        detail['native_exec_format_messages']=[message for message in messages if 'Exec format error' in message]
        assert detail['native_exec_format_messages']
    elif letter=='C':
        application=json.loads((epoch/'application-observation.json').read_text())
        assert application['request_intents']==1 and application['http_statuses']==[404]
        detail['application']=application
    else:
        assert audit.exists() and not (epoch/'start-receipt.json').exists()
        injection=json.loads((epoch/'synthetic-injection.json').read_text());assert injection==record['synthetic_injection']
        assert injection['elapsed_seconds']>=65 and len(raw_audit.splitlines())==1
        assert observed['gates']['quiet_audit']!='PASS'
        detail.update(synthetic_event=json.loads(raw_audit),injection=injection,no_start_receipt=True)
    assert all((path.stat().st_mode&0o777)==(0o700 if path.is_dir() else 0o600) for path in epoch.rglob('*'))
    rows.append({'run':letter,'status':'PASS','resource_exit_status':'PASS',**detail})
production=production_identity();assert production==expected['A']['complete_exit_receipt']['production_after']
for family in ['tcp','tcp6']:
    assert not any(line.split()[3]=='0A' and int(line.split()[1].rsplit(':',1)[1],16) in [19180,19181] for line in pathlib.Path('/proc/net',family).read_text().splitlines()[1:])
print(json.dumps({'status':'PASS','kind':'READ_ONLY_INDEPENDENT_READBACK','rows':rows,'production_identity_preserved':True,'provider_intents':0,'new_system_launches':0}))
'''.replace('expected=EXPECTED', 'expected=' + repr(expected)).replace('system_freeze=SYSTEM_FREEZE', 'system_freeze=' + repr(freeze))
(private / 'system-readback-executed.py').write_text(script)
command = ['ssh', '-T', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', 'new-vps', 'sudo', '-n', 'python3', '-']
result = subprocess.run(command, input=script.encode(), capture_output=True, timeout=45)
for name, raw in [('stdout', result.stdout), ('stderr', result.stderr)]:
    (private / ('system-readback.' + name)).write_bytes(raw)
receipt = {'command': command, 'remote_exit_code': result.returncode,
    'script_sha256': hashlib.sha256(script.encode()).hexdigest(), 'tool_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(), 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}
if result.stdout: receipt['result'] = json.loads(result.stdout)
save(public / 'system-readback.json', receipt)
assert result.returncode == 0 and receipt['result']['status'] == 'PASS'
print(json.dumps({'readback': 'PASS', 'runs': len(receipt['result']['rows']), 'new_system_launches': 0, 'provider_intents': 0}))
