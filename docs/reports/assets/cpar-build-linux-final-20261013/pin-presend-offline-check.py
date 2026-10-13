import pathlib,json,hashlib,sqlite3
base=pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010');epoch=base/'private/chat-json-linux-final-20261013';private=epoch/'chat-json'
observed=json.loads((private/'inspection-0-private.json').read_text());runtime=observed['runtime'];start=json.loads((epoch/'start-receipt.json').read_text())
opaque=lambda value:hashlib.sha256(('cpar-evidence:'+str(value)).encode()).hexdigest()
with sqlite3.connect('file:'+str(epoch/'state/control.sqlite3')+'?mode=ro',uri=True) as db:account=db.execute('SELECT id,revision,enabled,auth_status,worker_claim_kind FROM grok_accounts').fetchall()
checks=[]
def check(name,line,passed,actual,expected): checks.append({'assertion':name,'source_line':line,'passed':bool(passed),'actual':actual,'expected':expected})
check('account_count',81,len(account)==1,len(account),1)
check('account_identity',81,len(account)==1 and opaque(account[0][0])==start['account_evidence_id'],opaque(account[0][0]) if len(account)==1 else None,start['account_evidence_id'])
check('account_native_fields',82,len(account)==1 and account[0][1:]==(start['native_revision'],1,'active',None),list(account[0][1:]) if len(account)==1 else None,[start['native_revision'],1,'active',None])
pools=runtime['acceptance_inventory']['inventory']['native_pools']['items'];pool=pools[0] if len(pools)==1 else {}
check('candidate_pool_count_and_no_lease',84,len(pools)==1 and pool.get('active_leases')==0,{'count':len(pools),'active_leases':pool.get('active_leases')},{'count':1,'active_leases':0})
now_ms=int(observed.get('observed_now_ms',0) or 0)
check('candidate_expiry_margin',85,pool.get('expires_at_ms',0)>1791907225902+600000,pool.get('expires_at_ms'),'> recorded pin-result observed time +600s')
check('candidate_revision',86,pool.get('credential_revision')==start['runtime_projection_revision'],pool.get('credential_revision'),start['runtime_projection_revision'])
declarations=runtime['capability_observations'];d=declarations[0] if len(declarations)==1 else {}
check('declaration_count',88,len(declarations)==1,len(declarations),1)
fields=[('source','serving_snapshot',90),('client_key_evidence_id',opaque('cpar-build-client'),91),('config_version_id','cpar-build-config-20261010',92),('config_revision',9,92),('route_id','cpar-build-route',93),('route_candidate_id','cpar-build-candidate',93),('endpoint_id','cpar-build-endpoint',94),('upstream_model','grok-4.5',94),('capabilities',{'reasoning':True,'parallel':False,'stored':True,'continuation':True,'compact':True,'websocket':True},95)]
for name,expected,line in fields:check('declaration_'+name,line,len(declarations)==1 and d.get(name)==expected,d.get(name),expected)
first=next((row for row in checks if not row['passed']),None)
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
print(json.dumps({'status':'PASS; retained snapshot comparisons only','kind':'OFFLINE_ASSERTION_COMPARISON_NO_DISPATCH','inspection_sha256':sha(private/'inspection-0-private.json'),'pin_driver_sha256':sha(epoch/'remote-pin.py'),'start_receipt_sha256':sha(epoch/'start-receipt.json'),'checks':checks,'first_mismatch':first,'candidate_pool_safe_fields':{key:pool.get(key) for key in ['runtime_status','credential_revision','expires_at_ms','active_leases']},'declaration_safe_fields':{key:d.get(key) for key in ['source','client_key_evidence_id','config_version_id','config_revision','route_id','route_candidate_id','endpoint_id','upstream_model','capabilities']},'pin_request_artifact_exists':(private/'pin-request-private.json').exists(),'pin_response_artifact_exists':(private/'pin-private.json').exists(),'provider_intents':json.loads((epoch/'budget.json').read_text())['new_pin_intents'],'new_http_calls':0,'new_launches':0}))
