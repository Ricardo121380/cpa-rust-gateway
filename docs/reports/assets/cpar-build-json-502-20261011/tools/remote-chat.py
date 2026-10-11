"""One original Chat JSON request after a fresh scoped Pin; always latch and stop."""
import base64
import datetime
import fcntl
import gzip
import hashlib
import importlib.util
import io
import json
import os
import pathlib
import sqlite3
import subprocess
import sys
import time

os.umask(0o077)
base=pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch=base/'private/chat-json-502-20261011'
unit='cpar-build-acceptance-20261010.service'
assert json.loads((base/'owner.json').read_text())['task']=='cpar-build-20261010'
assert sys.argv[1:]==[]
lock=(epoch/'execution.lock').open('a')
fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
sys.path.insert(0,str(epoch))
import cpar_build_json_diagnostic as diagnostic
import cpar_new_build_acceptance as formal

budget_path=epoch/'budget.json'
budget=json.loads(budget_path.read_text())
diagnostic.validate_budget(budget)
assert not budget['stopped'] and budget['pin_status']=='PASS'
assert budget['new_pin_intents']==1 and budget['new_basic_intents']==0
private=epoch/'chat-json'
assert not (private/'chat-summary.json').exists()
start=json.loads((epoch/'start-receipt.json').read_text())
prepare=json.loads((epoch/'prepare-receipt.json').read_text())
freeze=json.loads((epoch/'freeze.json').read_text())
plan=formal.load_json((private/'frozen-plan.json').read_text())
summary={'status':'UNKNOWN','source_sha':start['source_sha'],'binary_sha256':start['binary_sha256'],
         'new_chat_intents':0,'full_matrix_status':'NOT_RUN','provider_receive_count':'UNKNOWN','actual_provider_cost':'UNKNOWN'}


def inspect():
    for name,checksum in freeze['files'].items():
        assert hashlib.sha256((base/'bin/chat-json-502-20261011'/name).read_bytes()).hexdigest()==checksum
        if name not in ['gateway-aarch64-unknown-linux-gnu','artifact-manifest.json','gateway-build-metadata.json']:
            assert hashlib.sha256((epoch/name).read_bytes()).hexdigest()==checksum
    script=formal.REMOTE_READ.replace('SERVICE',repr(unit)).replace('INVENTORY','True')
    observed=formal.load_json(subprocess.run([sys.executable,'-c',script],capture_output=True,text=True,check=True,timeout=90).stdout)
    runtime=observed['runtime']
    formal.bind_manifest(runtime,formal.load_json((epoch/'artifact-manifest.json').read_text()))
    assert runtime['pid']==start['pid'] and runtime['process_start']==start['process_start']
    assert runtime['artifact_sha256']==start['binary_sha256']
    diagnostic.verify_qualification(formal,plan,runtime,'http://127.0.0.1:19180/v1')
    proc=pathlib.Path('/proc',str(start['pid']))
    env=dict(row.split('=',1) for row in (proc/'environ').read_bytes().decode().split('\0') if '=' in row)
    assert env['CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION']==start['source_sha']
    assert env['CPAR_BUILD_DIAGNOSTIC_DIR']==str(epoch/'native-capture')
    assert env['CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256']==prepare['selected_request_sha256']
    assert env['LD_PRELOAD']==str(base/'bin/egress-guard-tcp.so')
    assert str(base/'bin/egress-guard-tcp.so') in (proc/'maps').read_text()
    args=(proc/'cmdline').read_bytes().decode().split('\0')
    assert args[args.index('--state-dir')+1]==str(epoch/'state')
    assert args[args.index('--background-provider-tasks')+1]=='disabled'
    assert not any(env.get(name) for name in ['HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','http_proxy','https_proxy','all_proxy'])
    with sqlite3.connect('file:'+str(epoch/'state/control.sqlite3')+'?mode=ro',uri=True) as db:
        account=db.execute('SELECT id,revision,enabled,auth_status,worker_claim_kind FROM grok_accounts').fetchall()
    assert len(account)==1 and formal.opaque(account[0][0])==start['account_evidence_id']
    assert account[0][1:]==(start['native_revision'],1,'active',None)
    pool=runtime['acceptance_inventory']['inventory']['native_pools']['items']
    assert len(pool)==1 and pool[0]['active_leases']==0 and pool[0]['expires_at_ms']==start['expires_at_ms']
    assert pool[0]['expires_at_ms']>int(time.time()*1000)+60000
    assert pool[0]['credential_revision']==plan['target']['credential_revision']
    assert all(hashlib.sha256((base/path).read_bytes()).hexdigest()==checksum for path,checksum in start['historical_file_hashes'].items())
    for line in (base/'audit/chat-json-502-20261011.jsonl').read_bytes().splitlines():
        assert json.loads(line)['event'] in ['dns_build_allowed','connect_build_allowed']
    (private/'chat-presend-runtime-private.json').write_text(json.dumps(observed,indent=2)+'\n')


try:
    inspect()
    payload=(epoch/'selected-request.json').read_bytes()
    assert hashlib.sha256(payload).hexdigest()==plan['public_request_sha256']==prepare['selected_request_sha256']
    body=formal.load_json(payload.decode())
    diagnostic.validate_request(body)
    assert json.dumps(body,separators=(',',':')).encode()==payload
    assert not any((epoch/'native-capture').iterdir())
    diagnostic.reserve_chat(budget)
    diagnostic.save_budget(budget_path,budget)
    summary['new_chat_intents']=1
    capture_path=private/'chat-http-private.json'
    assert not capture_path.exists()
    row={'request_wire_b64':base64.b64encode(payload).decode(),'wire_bytes_b64':'','sent_at_utc':formal.now()}
    capture_path.write_text(json.dumps(row,indent=2)+'\n')
    def progress(metadata,raw):
        row.update(metadata,wire_bytes_b64=base64.b64encode(raw).decode())
        capture_path.write_text(json.dumps(row,indent=2)+'\n')
    result=formal.http_exchange('http://127.0.0.1:19180/v1',(base/'private/client-key').read_text().strip(),
                                '/chat/completions',body,timeout=90,on_progress=progress)
    assert result['request_id'] and len(result['request_id'])<=512
    public=formal.load_json(result['wire'])
    error=public.get('error',{})
    safe_messages={
        'response projection cannot represent the reasoning metadata':'reasoning_unsupported',
        'response projection cannot represent the response metadata':'unknown_extensions',
        'response projection cannot represent the stop reason':'stop_reason_unsupported',
        'response projection cannot represent the stop sequence':'stop_sequence_unsupported',
        'response projection rejected an invalid canonical lifecycle':'invalid_canonical_lifecycle',
        'response projection rejected a tool constraint violation':'tool_constraint_violated',
    }
    summary.update(http_status=result['http_status'],request_id=result['request_id'],
                   wire_sha256=hashlib.sha256(result['wire'].encode()).hexdigest(),wire_bytes=len(result['wire'].encode()),
                   rejection=safe_messages.get(error.get('message')),
                   error_code='UpstreamProtocolError' if error.get('code')=='UpstreamProtocolError' else None)
    native_directory=epoch/'native-capture'
    selection=json.loads((native_directory/'selection.json').read_text())
    assert selection['request_id']==result['request_id'] and selection['public_request_sha256']==prepare['selected_request_sha256']
    receipt=json.loads((native_directory/'body-receipt.json').read_text())
    transport=json.loads((native_directory/'transport.json').read_text())
    native_bytes=(native_directory/'native-response.bin').read_bytes()
    assert len(native_bytes)==receipt['bytes']<=1048576 and hashlib.sha256(native_bytes).hexdigest()==receipt['sha256']
    assert receipt['complete'] is True
    if transport['content_encoding_class']=='Gzip':
        native_bytes=gzip.GzipFile(fileobj=io.BytesIO(native_bytes)).read(1048577)
    assert len(native_bytes)<=1048576
    native=formal.load_json(native_bytes.decode())
    # Only a bounded structure, fixed enum classes and counters cross this private boundary.
    known={'id','type','status','role','content','summary','encrypted_content','phase','name','call_id','arguments','signature'}
    output=native.get('output')
    assert isinstance(output,list) and len(output)<=256
    items=[]
    for item in output:
        assert isinstance(item,dict)
        shape={'known_fields':sorted(set(item)&known),'unknown_field_count':len(set(item)-known),
               'type_class':item.get('type') if item.get('type') in ['message','reasoning','function_call'] else 'other',
               'phase_class':item.get('phase') if item.get('phase') in ['commentary','final_answer'] else ('absent' if 'phase' not in item else 'other'),
               'content_parts':len(item['content']) if isinstance(item.get('content'),list) else None,
               'summary_parts':len(item['summary']) if isinstance(item.get('summary'),list) else None}
        items.append(shape)
    summary.update(native_transport=transport,native_body_receipt=receipt,native_output_structure=items)
    time.sleep(2)
    with sqlite3.connect('file:'+str(epoch/'state/control.sqlite3')+'?mode=ro',uri=True) as db:
        db.row_factory=sqlite3.Row
        selected=[]
        for event in db.execute('SELECT event_type,payload_json FROM gateway_event_log ORDER BY id LIMIT 64'):
            value=json.loads(event['payload_json'])
            data=next((child for child in value.values() if isinstance(child,dict)),{})
            if data.get('request_id')==result['request_id']:
                selected.append((event['event_type'],data))
        requests=[data for kind,data in selected if kind=='request']
        attempts=[data for kind,data in selected if kind=='attempt']
        terminals=[data for kind,data in selected if kind=='request_finished']
        usages=[data for kind,data in selected if kind=='usage']
        assert len(requests)==len(attempts)==len(terminals)==1
        assert requests[0]['protocol']=='openai_chat_completions' and requests[0]['streaming'] is False
        assert requests[0]['requested_model']=='grok-4.5'
        attempt=attempts[0]
        assert formal.opaque(attempt['credential_id'])==start['account_evidence_id']
        assert all(value['attempt_id']==attempt['attempt_id'] and value['response_id']==native['id'] for value in usages)
        ledger=[dict(value) for value in db.execute('SELECT source_event_id,request_id,response_id,cost_microunits FROM billing_ledger_entries WHERE request_id=?',(result['request_id'],))]
        assert len(ledger)==len(usages)<=1
        assert all(value['response_id']==native['id'] for value in ledger)
        summary.update(attempt_id=attempt['attempt_id'],attempt_outcome=attempt['outcome'],retry_decision=attempt['retry_decision'],
                       request_outcome=terminals[0]['outcome'],request_attempt_exact_match=True,
                       usage_rows=len(usages),ledger_rows=len(ledger),usage_sources=[value['usage'] for value in usages],
                       ledger_cost_observations=[value['cost_microunits'] for value in ledger])
    if result['http_status']==502 and error.get('code')=='UpstreamProtocolError':
        diagnostic.finish_chat(budget,'REPRODUCED')
        summary['status']='REPRODUCED'
    elif result['http_status']==200:
        diagnostic.finish_chat(budget,'NOT_REPRODUCED')
        summary['status']='NOT_REPRODUCED'
    else:
        diagnostic.finish_chat(budget,'UNKNOWN')
except BaseException as error:
    summary['category']=type(error).__name__
finally:
    budget['stopped']=True
    diagnostic.save_budget(budget_path,budget)
    summary['observed_at_utc']=datetime.datetime.now(datetime.timezone.utc).isoformat()
    summary['new_pin_intents']=budget['new_pin_intents']
    (private/'chat-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary))
