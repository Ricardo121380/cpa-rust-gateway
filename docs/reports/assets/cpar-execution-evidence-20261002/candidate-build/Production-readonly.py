import datetime,hashlib,json,pathlib,re,sqlite3,subprocess,os,shutil
service='cpa-rust-gateway.service'
def unit():
 raw=subprocess.check_output(['systemctl','show',service,'-p','MainPID','-p','ActiveState','-p','SubState','-p','ExecMainStartTimestamp','-p','WorkingDirectory','-p','User','-p','Group','-p','ExecStart','-p','FragmentPath'],text=True)
 return dict(line.split('=',1) for line in raw.splitlines() if '=' in line)
before=unit();pid=int(before['MainPID']);process_start=pathlib.Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()[19]
exe=pathlib.Path(f'/proc/{pid}/exe');data=exe.read_bytes()
embedded={k.decode():v.decode() for k,v in re.findall(rb'gateway-release-(revision|rust-version|target)=([^\n]{1,100})\n',data)}
exec_start=before.pop('ExecStart');match=re.search(r'path=([^ ;]+)',exec_start)
state=pathlib.Path('/var/lib/cpa-rust-gateway')
with sqlite3.connect('file:'+str(state/'control.sqlite3')+'?mode=ro',uri=True) as db:
 db.execute('BEGIN');schema=db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0]
 integrity=db.execute('PRAGMA quick_check').fetchone()[0]
 sidecar=bool(db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='gateway_attempt_execution'").fetchone())
after=unit();after.pop('ExecStart');start_after=pathlib.Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()[19]
assert before==after and process_start==start_after
result={'observed_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'scope':'read_only_no_provider_no_service_change','service':service,'unit':before,'exec_start_path':match.group(1) if match else None,'executable':os.readlink(exe),'binary_sha256':hashlib.sha256(data).hexdigest(),'embedded':embedded,'architecture':os.uname().machine,'libc':os.confstr('CS_GNU_LIBC_VERSION'),'current_symlink':str(pathlib.Path('/opt/cpa-rust-gateway/current').resolve()),'control_database':{'schema_version':schema,'quick_check':integrity,'execution_sidecar_exists':sidecar},'state_files':[{'name':p.name,'bytes':p.stat().st_size} for p in sorted(state.iterdir()) if p.is_file()],'free_bytes':{p:shutil.disk_usage(p).free for p in [str(state),'/opt/cpa-rust-gateway']},'stable_process':True}
print(json.dumps(result,indent=2))
