import hashlib,json,pathlib,sqlite3
receipt=json.loads(pathlib.Path('/evidence/native-http/receipt.json').read_text());state=pathlib.Path(receipt['private_state'])/'state/control.sqlite3'
assert not pathlib.Path(str(state)+'-wal').exists(), 'export requires closed, checkpointed fixture'
db=sqlite3.connect('file:'+str(state)+'?mode=ro&immutable=1',uri=True);db.row_factory=sqlite3.Row
def opaque(v):return hashlib.sha256(('cpar-evidence:'+str(v)).encode()).hexdigest()
rows=db.execute("SELECT a.event_id,a.request_id,a.payload_json,e.attempt_payload_sha256,e.evidence_json FROM gateway_event_log a JOIN gateway_attempt_execution e ON e.attempt_id=a.event_id WHERE a.event_type='attempt' ORDER BY a.event_ordinal").fetchall()
selected=list(rows[:6])+[r for r in rows if json.loads(r['evidence_json'])['config_version_id'] in ['execution-before','execution-after']]
assert len(selected)==8
observations=[]
for row in selected:
 attempt=json.loads(row['payload_json'])['attempt'];identity=json.loads(row['evidence_json']);assert hashlib.sha256(row['payload_json'].encode()).hexdigest()==row['attempt_payload_sha256']
 requests=db.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='request' AND request_id=?",(row['request_id'],)).fetchall();assert len(requests)==1
 request=json.loads(requests[0][0])['request'];finished=db.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='request_finished' AND request_id=?",(row['request_id'],)).fetchall();assert len(finished)==1
 usages=db.execute("SELECT event_id,payload_json FROM gateway_event_log WHERE event_type='usage' AND request_id=?",(row['request_id'],)).fetchall();assert len(usages)==1
 usage=json.loads(usages[0]['payload_json'])['usage'];assert usage['attempt_id']==row['event_id']
 ledger=db.execute('SELECT source_event_id,request_id,response_id FROM billing_ledger_entries WHERE source_event_id=?',(usages[0]['event_id'],)).fetchall();assert len(ledger)==1 and ledger[0]['request_id']==row['request_id'] and ledger[0]['response_id']==usage['response_id']
 observations.append({'request_evidence_id':opaque(row['request_id']),'attempt_evidence_id':opaque(row['event_id']),'client_key_evidence_id':opaque(request['client_key_id']),'protocol':request['protocol'],'mode':'sse' if request['streaming'] else 'json','request_outcome':json.loads(finished[0][0])['request_finished']['outcome'],'attempt_outcome':attempt['outcome'],'endpoint_id':attempt['endpoint_id'],'route_id':attempt['route_id'],'route_candidate_id':attempt['route_candidate_id'],'upstream_model':attempt['upstream_model'],'credential_evidence_id':opaque(attempt['credential_id']),'actual_execution':identity,'original_attempt_sha256':row['attempt_payload_sha256'],'execution_record_sha256':hashlib.sha256(row['evidence_json'].encode()).hexdigest(),'usage_source_event_evidence_id':opaque(usages[0]['event_id']),'usage_attempt_matches':True,'ledger_exact_usage_source_matches':True})
result={'schema_version':1,'layer':'LOCAL_SYNTHETIC_HISTORICAL_FIXTURE_EXPORT','source_sha':receipt['source_sha'],'binary_sha256':receipt['binary_sha256'],'real_provider_calls':0,'origin':'closed checkpointed SQLite from passing native 410-check harness; not a live process attestation','observations':observations,'unknown_in_production':['new_runtime_not_deployed','actual_lease_revision','loaded_configuration_revision','actual_egress_identity_and_revision','scoped_serving_capability_evidence']}
pathlib.Path('/evidence/execution-examples.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'observations':len(observations),'scope':result['layer'],'source_sha':receipt['source_sha']}))
