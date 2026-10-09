import datetime,hashlib,json,os,pathlib,re,subprocess,sys,time
root=pathlib.Path.cwd(); name=sys.argv[1]; command=sys.argv[2:]
e=pathlib.Path(os.environ.get('CPAR_SEO_EVIDENCE_ROOT',str(root/'docs/reports/assets/cpar-seo-closeout-20261009'))); out=pathlib.Path(os.environ.get('CPAR_SEO_OUTPUT_ROOT',str(root/'output/cpar-seo-closeout-20261009'))); out.mkdir(parents=True,exist_ok=True);e.mkdir(parents=True,exist_ok=True)
def source():
 files=subprocess.check_output(['git','ls-files','-z']).decode().split('\0'); hashes={}
 owned=['apps/gateway/src/runtime/batch_c/http/responses_history/phase_admission.rs','crates/gateway-http-actix/tests/managed_resource_inventory/device_commit_loss.rs','web/prism/src/api/seo-fixtures.test.ts']
 owned += [str(p.relative_to(root)) for pattern in ('scripts/acceptance/cpar_*.py','scripts/acceptance/cpar-*.mjs') for p in root.glob(pattern)]
 owned += ['scripts/acceptance/cpar-device-loopback.c']
 files=sorted(set(files+owned))
 for rel in files:
  p=root/rel
  if p.is_file() and not rel.startswith('docs/reports/assets/cpar-seo-closeout-20261009/'): hashes[rel]=hashlib.sha256(p.read_bytes()).hexdigest()
 return {'head':subprocess.check_output(['git','rev-parse','HEAD']).decode().strip(),'dirty':subprocess.check_output(['git','status','--porcelain=v1']).decode().splitlines(),'tracked_files_sha256':hashlib.sha256(json.dumps(hashes,sort_keys=True).encode()).hexdigest(),'changed_files_sha256':{r:hashes.get(r) for r in subprocess.check_output(['git','diff','--name-only']).decode().splitlines()}}
before=source(); start=datetime.datetime.now(datetime.timezone.utc).isoformat(); clock=time.monotonic();log=out/(name+'.log')
with log.open('wb') as stream:
 result=subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT)
raw=log.read_bytes(); text=raw.decode(errors='replace'); counts=[dict(zip(('passed','failed','ignored'),map(int,m))) for m in re.findall(r'test result: .*?([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored',text)]
frontend={}
frontend_match=re.search(r'^\s*Tests\s+(.+)$',text,re.M)
if_frontend=frontend_match is not None
if if_frontend:
 frontend={k:int(m.group(1)) if (m:=re.search(r'([0-9]+) '+k,frontend_match.group(1))) else 0 for k in ('passed','failed','skipped')}
receipt={'name':name,'command':command,'cwd':str(root),'started_utc':start,'finished_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'duration_seconds':round(time.monotonic()-clock,3),'exit_code':result.returncode,'result':'PASS' if result.returncode==0 else 'FAIL','source_before':before,'source_after':source(),'test_totals':{k:sum(c[k] for c in counts) for k in ('passed','failed','ignored')},'frontend_test_totals':frontend,'hash_objects':{'raw_log':{'path':str(log),'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw)}}}
(e/(name+'-receipt.json')).write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({k:receipt[k] for k in ('name','exit_code','result','duration_seconds','test_totals')}));print(text[-5000:]);sys.exit(result.returncode)
