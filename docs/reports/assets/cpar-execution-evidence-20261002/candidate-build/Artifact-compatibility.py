import hashlib,json,os,pathlib,shutil,socket,sqlite3,subprocess,time,urllib.request,tempfile
source=pathlib.Path('/workspace'); binary=pathlib.Path('/candidate/gateway-aarch64-unknown-linux-gnu')
root=pathlib.Path(json.loads(pathlib.Path('/evidence/native-http/receipt.json').read_text())['private_state'])
sandbox=pathlib.Path(tempfile.mkdtemp(prefix='cpar-schema-',dir='/private')); state=sandbox/'upgrade-31-state'; shutil.copytree(root/'state',state)
def connect():
 c=sqlite3.connect(state/'control.sqlite3'); c.execute('PRAGMA foreign_keys=ON'); return c
with connect() as db:
 for n in [34,33,32]:
  path=next((source/'crates/gateway-store/migrations').glob(f'{n:04d}_*.down.sql'))
  db.executescript(path.read_text()); db.execute('DELETE FROM schema_migrations WHERE version=?',(n,))
 db.commit()
 assert db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0]==31
 # Only pre-existing columns are compared across the candidate's migration; raw rows stay private.
 tables=['config_versions','upstreams','upstream_endpoints','upstream_credentials','endpoint_credential_bindings','public_models','model_routes','route_candidates','access_groups','access_group_routes','client_keys','gateway_event_log','billing_ledger_entries','grok_accounts']
 snapshots={}
 def freeze(rows):
  encoded=[json.dumps(list(row),default=lambda b:b.hex(),sort_keys=True,separators=(',',':')) for row in rows]
  return sorted(encoded)
 for table in tables:
  columns=[r[1] for r in db.execute('PRAGMA table_info('+table+')')]
  snapshots[table]=(columns,freeze(db.execute('SELECT '+','.join(columns)+' FROM '+table).fetchall()))
 db.execute('PRAGMA wal_checkpoint(TRUNCATE)')
backup=sandbox/'pre-upgrade-31-backup'; shutil.copytree(state,backup)
def port():
 with socket.socket() as s:s.bind(('127.0.0.1',0)); return s.getsockname()[1]
data_port,admin_port=port(),port()
log=open('/evidence/native-compatibility-private.log','wb')
p=subprocess.Popen([str(binary),'serve','--state-dir',str(state),'--credential-dir',str(root/'credentials'),'--data-listen',f'127.0.0.1:{data_port}','--management-listen',f'127.0.0.1:{admin_port}'],stdout=log,stderr=log)
try:
 deadline=time.monotonic()+20
 while True:
  try:
   with urllib.request.urlopen(f'http://127.0.0.1:{data_port}/healthz',timeout=1) as reply:
    assert reply.status==200; break
  except Exception:
   assert p.poll() is None and time.monotonic()<deadline, 'candidate migration/startup failed'
   time.sleep(.1)
 request=urllib.request.Request(f'http://127.0.0.1:{admin_port}/admin/system',headers={'X-Management-Key':(root/'credentials/management-key').read_text()})
 with urllib.request.urlopen(request,timeout=5) as reply:
  system_information=json.loads(reply.read(16384)); build=system_information['build']
 expected_build={'version':'0.1.0','build_revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'build_target':'aarch64-unknown-linux-gnu','rust_version':'1.97.1','schema_version':34}
 assert build==expected_build, 'actual binary build metadata mismatch'
 with connect() as db:
  db.execute('BEGIN')
  assert db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0]==34
  assert db.execute('PRAGMA quick_check').fetchone()[0]=='ok'
  for table,(columns,old) in snapshots.items():
   assert freeze(db.execute('SELECT '+','.join(columns)+' FROM '+table).fetchall())==old, table+' changed pre-31 facts'
  assert db.execute('SELECT count(*) FROM gateway_attempt_execution').fetchone()[0]==0
  assert db.execute('SELECT count(*) FROM gateway_event_log WHERE event_type=\'attempt\'').fetchone()[0]>0
  metadata=db.execute('SELECT DISTINCT usage_evidence_json FROM billing_ledger_entries').fetchall()
  assert metadata and all(json.loads(r[0])=={'provenance':'unknown','input_accounting':'unknown'} for r in metadata)
 restored=sandbox/'restored-31-state'; shutil.copytree(backup,restored)
 with sqlite3.connect('file:'+str(restored/'control.sqlite3')+'?mode=ro',uri=True) as rollback_db:
  assert rollback_db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0]==31
  assert rollback_db.execute('PRAGMA quick_check').fetchone()[0]=='ok'
  for table,(columns,old) in snapshots.items():
   assert freeze(rollback_db.execute('SELECT '+','.join(columns)+' FROM '+table).fetchall())==old, table+' not restored'
 assert hashlib.sha256((backup/'control.sqlite3').read_bytes()).hexdigest()==hashlib.sha256((restored/'control.sqlite3').read_bytes()).hexdigest()
 result={'status':'PASS','layer':'LOCAL_SYNTHETIC_LINUX_ARM64','source_sha':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'schema_before':31,'schema_after':34,'startup_health':'PASS','actual_binary_build':build,'integrity':'ok','preserved_tables':{table:len(old) for table,(_,old) in snapshots.items()},'legacy_execution':'unknown_no_backfill','legacy_usage_evidence':'unknown_no_invented_measurement','rollback_backup':'synthetic_full_state_restored_and_all_14_table_snapshots_exact','rollback_schema':31,'rollback_integrity':'ok','old_binary_on_restored_state':'NOT_RUN','production_copy_rehearsal':'NOT_RUN','real_provider_calls':0}
 pathlib.Path('/evidence/native-compatibility.json').write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps(result))
finally:
 p.terminate(); p.wait(timeout=30); log.close()
