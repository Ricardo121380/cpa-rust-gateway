
import ast,datetime,fcntl,hashlib,json,os,pathlib,sqlite3,stat,subprocess,sys,time,urllib.request
os.umask(0o077)
base=pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch=base/'private/chat-json-502-20261011';unit='cpar-build-acceptance-20261010.service'
assert json.loads((base/'owner.json').read_text())['task']=='cpar-build-20261010'
lock=(epoch/'execution.lock').open('a');fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
sys.path.insert(0,str(epoch));import cpar_build_json_diagnostic as diagnostic
budget=json.loads((epoch/'budget.json').read_text());diagnostic.validate_budget(budget)
assert budget['new_pin_intents']==budget['new_basic_intents']==0
budget['stopped']=True;diagnostic.save_budget(epoch/'budget.json',budget)
prepare=json.loads((epoch/'prepare-receipt.json').read_text())
unit_state={name:subprocess.check_output(['systemctl','show',unit,'--value','--property='+name],text=True).strip() for name in ['MainPID','LoadState','ActiveState','SubState']}
assert unit_state['MainPID']=='0' and unit_state['LoadState']=='not-found'
assert not (epoch/'startup-process-binding.json').exists() and not (epoch/'start-receipt.json').exists() and not (epoch/'launch-v2.json').exists()
ports=[int(row.split()[1].rsplit(':',1)[1],16) for family in ['tcp','tcp6'] for row in pathlib.Path('/proc/net',family).read_text().splitlines()[1:] if row.split()[3]=='0A' and int(row.split()[1].rsplit(':',1)[1],16) in [19180,19181]]
assert not ports
competitors=[]
for proc in pathlib.Path('/proc').iterdir():
    if not proc.name.isdigit() or int(proc.name)==os.getpid():continue
    try:
        if str(epoch/'state').encode() in (proc/'cmdline').read_bytes() or any(str(fd.resolve()).startswith(str(epoch/'state/control.sqlite3')) for fd in (proc/'fd').iterdir()):competitors.append(int(proc.name))
    except (PermissionError,FileNotFoundError,ProcessLookupError):pass
assert not competitors
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
audit=base/'audit/chat-json-502-20261011.jsonl';audit_exists=audit.exists();audit_bytes=audit.read_bytes() if audit_exists else b''
time.sleep(5);assert audit_exists==audit.exists() and audit_bytes==(audit.read_bytes() if audit.exists() else b'') and not audit_bytes
assert not any((epoch/'native-capture').iterdir()) and not (epoch/'chat-json').exists()
assert all(sha(base/name)==value for name,value in prepare['historical_file_hashes'].items())
bootstrap=ast.parse((base/'private/bootstrap.py').read_text());functions=ast.Module(body=[node for node in bootstrap.body if isinstance(node,ast.FunctionDef) and node.name=='production'],type_ignores=[]);exec(compile(functions,'production-proof','exec'))
assert production()==prepare['production_before']
args=pathlib.Path('/proc',prepare['production_before']['MainPID'],'cmdline').read_bytes().decode().split('\0');creds=pathlib.Path(args[args.index('--credential-dir')+1])
client=urllib.request.build_opener(urllib.request.ProxyHandler({}))
with client.open(urllib.request.Request('http://127.0.0.1:18181/admin/config-versions',headers={'X-Management-Key':(creds/'management-key').read_text().strip()}),timeout=10) as response:
    active=[{key:row[key] for key in ['id','revision','status']} for row in json.load(response) if row['status']=='active']
assert active==prepare['production_active_configuration']
with sqlite3.connect('file:'+str(epoch/'state/control.sqlite3')+'?mode=ro',uri=True) as db:
    counts=dict(db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type'));ledger=db.execute('SELECT count(*) FROM billing_ledger_entries').fetchone()[0]
    assert counts==prepare['inherited_event_counts'] and ledger==prepare['inherited_ledger_count']
    accounts=db.execute('SELECT revision,worker_claim_kind FROM grok_accounts').fetchall();assert accounts==[(prepare['copied_native_revision'],None)]
files=[];directories=[]
for p in epoch.rglob('*'):
    assert not p.is_symlink()
    if p.is_dir():assert stat.S_IMODE(p.stat().st_mode)==0o700;directories.append(str(p.relative_to(epoch)))
    elif p.is_file():assert stat.S_IMODE(p.stat().st_mode)==0o600;files.append({'path':str(p.relative_to(epoch)),'sha256':sha(p),'bytes':p.stat().st_size})
receipt={'status':'PASS; terminated before launch; exit and zero-send reconciliation', 'resource_exit_status':'PASS','observed_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
'epoch':str(epoch),'source_sha':prepare['source_sha'],'binary_sha256':prepare['binary_sha256'],'unit_state':unit_state,
'launch_started':False,'owned_process_stop_status':'NOT_STARTED_VERIFIED','temporary_ports_listening':ports,'competing_processes':competitors,
'network_audit_file_exists':audit_exists,'network_audit_bytes':len(audit_bytes),'post_stop_flat_seconds':5,
'new_pin_intents':0,'new_chat_intents':0,'new_gateway_pin_confirmed_sends':0,'new_event_counts':{key:0 for key in counts},'new_ledger_rows':0,
'budget':budget,'native_capture_files':0,'source_writes':0,'proactive_refresh_reauthorization':0,
'production_after':production(),'production_active_configuration':active,'historical_file_hashes_preserved':True,'historical_file_count':len(prepare['historical_file_hashes']),
'private_file_count':len(files),'private_directory_count':len(directories),'private_modes':'0700 directories / 0600 files; no symlinks','private_inventory_sha256':hashlib.sha256(json.dumps(files,sort_keys=True).encode()).hexdigest(),
'provider_independent_receive_count':'UNKNOWN','actual_provider_cost':'UNKNOWN','management_get_intents_in_exit':1,'full_matrix':'NOT_RUN'}
with (epoch/'prelaunch-final-receipt.json').open('x') as output:json.dump(receipt,output,indent=2)
print(json.dumps(receipt))
