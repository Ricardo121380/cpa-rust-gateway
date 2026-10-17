"""Read one stopped epoch; private bytes go directly to local 0700/0600 files.

No runtime start, inference HTTP, credential read or remote mutation is performed.
Never print the SSH payload or stderr: the payload contains private native bytes.
"""
import base64
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[5]
PUBLIC = Path(__file__).resolve().parent.parent
PRIVATE = ROOT / 'output/cpar-native-chat-offline-20261017'
os.umask(0o077)
REMOTE = r'''
import base64,hashlib,json,pathlib,sqlite3,subprocess
base=pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch=base/'private/chat-json-serving-20261015'
assert json.loads((base/'owner.json').read_text())['task']=='cpar-build-20261010'
state=dict(row.split('=',1) for row in subprocess.check_output(
 ['systemctl','show','cpar-build-chat-json-serving-20261015.service',
 '--property=MainPID,LoadState,ActiveState,SubState'],text=True).splitlines())
assert state['MainPID']=='0' and state['LoadState']=='not-found'
budget=json.loads((epoch/'budget.json').read_text())
assert budget['stopped'] and budget['new_pin_intents']==budget['new_basic_intents']==1
assert budget['retries']==budget['fallbacks']==budget['refresh_reauthorization']==0
final=json.loads((epoch/'final-receipt.json').read_text())
names=['native-capture/native-response.bin','native-capture/selection.json',
 'native-capture/body-receipt.json','native-capture/transport.json',
 'selected-request.json','start-receipt.json','freeze.json',
 'chat-json/chat-summary.json','chat-json/pin-private.json']
files={}
for name in names:
 path=epoch/name
 assert not path.is_symlink() and path.stat().st_mode & 0o777==0o600
 assert path.stat().st_nlink==1 and path.stat().st_size<=1048576
 data=path.read_bytes()
 facts=final['private_files'][str(path.relative_to(base))]
 assert len(data)==facts['bytes'] and hashlib.sha256(data).hexdigest()==facts['sha256']
 files[name]=base64.b64encode(data).decode()
assert epoch.stat().st_mode & 0o777==0o700
assert (epoch/'native-capture').stat().st_mode & 0o777==0o700
request_id=json.loads((epoch/'chat-json/chat-summary.json').read_text())['request_id']
with sqlite3.connect('file:'+str(epoch/'state/control.sqlite3')+'?mode=ro',uri=True) as db:
 events=db.execute('SELECT event_type,event_id,payload_json FROM gateway_event_log WHERE request_id=? ORDER BY event_ordinal',(request_id,)).fetchall()
 sidecars=db.execute('SELECT request_id,attempt_payload_sha256,evidence_json FROM gateway_attempt_execution WHERE request_id=?',(request_id,)).fetchall()
 ledger=db.execute('SELECT source_event_id,request_id,cost_microunits FROM billing_ledger_entries WHERE request_id=?',(request_id,)).fetchall()
print(json.dumps({'files':files,'events':events,'sidecars':sidecars,'ledger':ledger,
 'state':state,'budget_stopped':True,'remote_provider_calls':0,'remote_launches':0}))
'''


def main():
    target = PRIVATE / 'capture'
    target.mkdir(mode=0o700, exist_ok=False)
    result = subprocess.run(
        ['ssh', '-T', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10',
         'new-vps', 'sudo -n python3 -'], input=REMOTE.encode(),
        capture_output=True, timeout=55)
    receipt = {
        'observed_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'script_sha256': hashlib.sha256(REMOTE.encode()).hexdigest(),
        'returncode': result.returncode,
        'stderr_sha256': hashlib.sha256(result.stderr).hexdigest(),
        'provider_calls': 0, 'runtime_launches': 0,
    }
    (PRIVATE / 'acquire-stderr.txt').write_bytes(result.stderr)
    if result.returncode:
        (PUBLIC / 'acquire-failed.json').write_text(json.dumps(receipt, indent=2)+'\n')
        raise SystemExit('read-only acquisition failed; private stderr retained')
    payload = json.loads(result.stdout)
    for name, encoded in payload.pop('files').items():
        path = target / name
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        path.write_bytes(base64.b64decode(encoded, validate=True))
        os.chmod(path, 0o600)
    (target / 'lineage-private.json').write_text(json.dumps(payload))
    raw = (target/'native-capture/native-response.bin').read_bytes()
    request = (target/'selected-request.json').read_bytes()
    assert len(raw)==673 and hashlib.sha256(raw).hexdigest()=='7b5a3c2766766397af638665aaa7160f351780232a2c06f11d8f784ebbfdaa96'
    assert hashlib.sha256(request).hexdigest()=='245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062'
    receipt.update(status='PASS', private_files=10, remote_state=payload['state'],
                   budget_stopped=payload['budget_stopped'], native_bytes=len(raw),
                   native_sha256=hashlib.sha256(raw).hexdigest(),
                   request_sha256=hashlib.sha256(request).hexdigest())
    (PUBLIC/'acquire.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps(receipt,indent=2))


if __name__ == '__main__':
    main()
