import ast,datetime,fcntl,hashlib,json,os,pathlib,sqlite3,stat,subprocess,time,urllib.request
os.umask(0o077)
base=pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010');epoch=base/'private/chat-json-rehearsal-20261013';unit='cpar-build-chat-json-rehearsal-20261013.service';audit=base/'audit/chat-json-rehearsal-20261013.jsonl'
assert json.loads((base/'owner.json').read_text())['task']=='cpar-build-20261010'
lock=(epoch/'execution.lock').open('a');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
assert not (epoch/'identity-bound-cleanup-intent.json').exists()
budget=json.loads((epoch/'budget.json').read_text());assert budget['stopped'] is True and budget['new_pin_intents']==budget['new_basic_intents']==0
freeze=json.loads((epoch/'freeze.json').read_text());uploaded=base/'bin/chat-json-rehearsal-20261013/freeze.json';assert hashlib.sha256(uploaded.read_bytes()).hexdigest()=='53c1e3885231bd22a91d9c58d986c6638c6dd596c640afd0e9f6e6f92b88821a';assert freeze==json.loads(uploaded.read_text())
binary=base/'bin/chat-json-rehearsal-20261013/gateway-aarch64-unknown-linux-gnu';guard=base/'bin/egress-guard-tcp.so';launch=json.loads((epoch/'launch-v2.json').read_text())['command'];expected=launch[launch.index(str(binary)):]
pid=int(subprocess.check_output(['systemctl','show',unit,'--value','--property=MainPID']));assert pid==2026813
proc=pathlib.Path('/proc',str(pid));start=(proc/'stat').read_text().rsplit(')',1)[1].split()[19];assert start=='679814074'
h=lambda b:hashlib.sha256(b).hexdigest();args=(proc/'cmdline').read_bytes().decode().rstrip('\0').split('\0');assert args==expected
assert h(json.dumps(args,separators=(',',':')).encode())=='d021fe9417eb5a7f07be6d6e5c566cec0160056ef6abc2f491f743a861b89d9d'
assert str((proc/'exe').resolve())==str(binary) and h((proc/'exe').read_bytes())==freeze['files']['gateway-aarch64-unknown-linux-gnu']
assert args[args.index('--state-dir')+1]==str(epoch/'state') and args[args.index('--background-provider-tasks')+1]=='disabled'
env=[x for x in (proc/'environ').read_bytes().decode().split('\0') if '=' in x]
expected_environment={'LD_PRELOAD':str(guard),'CPAR_BUILD_EGRESS_AUDIT':str(audit),'CPAR_BUILD_DIAGNOSTIC_DIR':str(epoch/'native-capture'),'CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256':freeze['selected_request_sha256'],'CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION':freeze['source_sha']}
assert all(sum(x.split('=',1)[0]==n for x in env)==1 and n+'='+v in env for n,v in expected_environment.items())
assert str(guard) in (proc/'maps').read_text() and h(guard.read_bytes())=='91c943c76884a2b674a0b24d5d4876ff40cf61b53c379ffe2d04a062a0dddf06'
identity={'pid':pid,'process_start':start,'exe_sha256':h((proc/'exe').read_bytes()),'argv_sha256':h(json.dumps(args,separators=(',',':')).encode()),'state_dir':str(epoch/'state'),'guard_mapped':True,'environment_unique_and_exact':True,'provider_calls':0}
with (epoch/'identity-bound-cleanup-intent.json').open('x') as out:json.dump(identity,out,indent=2);out.flush();os.fsync(out.fileno())
d=os.open(epoch,os.O_RDONLY|os.O_DIRECTORY);os.fsync(d);os.close(d)
assert int(subprocess.check_output(['systemctl','show',unit,'--value','--property=MainPID']))==pid and (proc/'stat').read_text().rsplit(')',1)[1].split()[19]==start
subprocess.run(['systemctl','stop',unit],check=True,capture_output=True)
main=int(subprocess.check_output(['systemctl','show',unit,'--value','--property=MainPID']));ports=[int(x.split()[1].rsplit(':',1)[1],16) for family in ('tcp','tcp6') for x in pathlib.Path('/proc/net',family).read_text().splitlines()[1:] if x.split()[3]=='0A' and int(x.split()[1].rsplit(':',1)[1],16) in (19180,19181)]
audit_bytes=audit.read_bytes() if audit.exists() else b'';audit_sha=h(audit_bytes);time.sleep(5);flat=h(audit.read_bytes() if audit.exists() else b'')==audit_sha
source=ast.parse((base/'private/bootstrap.py').read_text());functions=ast.Module(body=[n for n in source.body if isinstance(n,ast.FunctionDef) and n.name=='production'],type_ignores=[]);exec(compile(functions,'production-proof','exec'));production_after=production();prepare=json.loads((epoch/'prepare-receipt.json').read_text());production_matches=production_after==prepare['production_before']
source_args=pathlib.Path('/proc',production_after['MainPID'],'cmdline').read_bytes().decode().split('\0');credentials=pathlib.Path(source_args[source_args.index('--credential-dir')+1]);client=urllib.request.build_opener(urllib.request.ProxyHandler({}))
with client.open(urllib.request.Request('http://127.0.0.1:18181/admin/config-versions',headers={'X-Management-Key':(credentials/'management-key').read_text().strip()}),timeout=10) as response:
 assert response.status==200
 active=[{k:r[k] for k in ('id','revision','status')} for r in json.load(response) if r['status']=='active']
expected_active=[{k:r[k] for k in ('id','revision','status')} for r in prepare['production_active_configuration']]
historical=prepare['historical_file_hashes'];history_mismatches=[n for n,v in historical.items() if h((base/n).read_bytes())!=v]
mode_failures=[];files={}
for p in epoch.rglob('*'):
 if p.is_symlink() or stat.S_IMODE(p.stat().st_mode)!=(0o700 if p.is_dir() else 0o600):mode_failures.append(str(p.relative_to(epoch)))
 if p.is_file():files[str(p.relative_to(base))]={'sha256':h(p.read_bytes()),'bytes':p.stat().st_size,'mode':format(stat.S_IMODE(p.stat().st_mode),'04o')}
with sqlite3.connect('file:'+str(epoch/'state/control.sqlite3')+'?mode=ro',uri=True) as db:
 counts=dict(db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type'));ledger=db.execute('SELECT count(*) FROM billing_ledger_entries').fetchone()[0];account=list(db.execute('SELECT revision,worker_claim_kind FROM grok_accounts'))
delta={k:counts.get(k,0)-prepare['inherited_event_counts'].get(k,0) for k in set(counts)|set(prepare['inherited_event_counts'])}
old_stopped=json.loads((base/'private/chat-json-502-20261011/budget.json').read_text())['stopped'] is True
checks={'main_pid_zero':main==0,'owned_pid_absent':not proc.exists(),'temporary_ports_released':not ports,'post_stop_audit_flat':flat,'history_bytes_preserved':not history_mismatches,'old_budget_still_stopped':old_stopped,'private_modes_preserved':not mode_failures,'production_process_and_files_match_fresh_prepare':production_matches,'production_active_config_matches_fresh_prepare':active==expected_active,'zero_new_event_rows':all(v==0 for v in delta.values()),'zero_new_ledger_rows':ledger==prepare['inherited_ledger_count'],'candidate_native_revision_preserved':account==[(prepare['copied_native_revision'],None)]}
receipt={'status':'PASS' if all(checks.values()) else 'PARTIAL','resource_exit_status':'PASS' if main==0 and not proc.exists() and not ports and flat else 'UNKNOWN','owned_process_stop_status':'STOPPED_VERIFIED' if main==0 and not proc.exists() and not ports else 'UNKNOWN','execution_status':'BLOCKED_AFTER_START_FAILURE; NEVER_RESUMED','remote_epoch_status':'CREATED','startup_validation_status':'FAIL; no successful startup receipt or 65-second gate','observed_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'identity':identity,'main_pid_after_stop':main,'temporary_ports_listening':ports,'post_stop_flat_seconds':5,'audit_sha256':audit_sha,'audit_bytes':len(audit_bytes),'checks':checks,'historical_files_verified':len(historical),'historical_mismatch_count':len(history_mismatches),'private_file_count':len(files),'private_mode_failure_count':len(mode_failures),'private_inventory_sha256':h(json.dumps(files,sort_keys=True).encode()),'production_after':production_after,'production_active_configuration':active,'budget_stopped':budget['stopped'],'new_pin_intents':budget['new_pin_intents'],'new_basic_intents':budget['new_basic_intents'],'new_gateway_pin_confirmed_sends':0 if all(v==0 for v in delta.values()) and len(audit_bytes)==0 else None,'new_event_counts':delta,'new_ledger_rows':ledger-prepare['inherited_ledger_count'],'provider_http_receive_count':'UNKNOWN','actual_provider_cost':'UNKNOWN','provider_calls_in_cleanup':0}
with (epoch/'identity-bound-cleanup-receipt.json').open('x') as out:json.dump({**receipt,'private_inventory':files},out,indent=2)
print(json.dumps(receipt))
