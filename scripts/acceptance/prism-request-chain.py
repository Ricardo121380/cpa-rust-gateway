import sys
"""Exercise only the owned local gateway/TLS mock; report metadata, never credentials."""
import pathlib,json,urllib.request,urllib.error,time,socket,struct
out=pathlib.Path(sys.argv[1]).resolve();info=json.loads((out/'local-preview.json').read_text());assert info['synthetic'];root=pathlib.Path(info['root']);assert root.name.startswith('prism-complete-local-')
http=urllib.request.build_opener(urllib.request.ProxyHandler({}));admin=f"http://127.0.0.1:{info['admin_port']}";base=f"http://127.0.0.1:{info['data_port']}"
management={'X-Management-Key':(root/'credentials/management-key').read_text(),'X-Config-Version':'local-accepted','Content-Type':'application/json'};client={'Authorization':'Bearer '+(root/'client-key').read_text(),'Content-Type':'application/json'}
def get(path,mgmt=True):
 with http.open(urllib.request.Request((admin if mgmt else base)+path,headers=management if mgmt else client),timeout=40) as r:return json.load(r)
def run(prompt,stream=False,cancel=False):
 for _ in range(200):
  availability=get('/admin/runtime/availability')
  if any(row['availability']=='available' for row in availability):break
  time.sleep(.1)
 else:raise RuntimeError('mock_endpoint_did_not_recover_from_cooldown')
 if cancel:
  payload=json.dumps({'model':'Exact/Model-v2','input':prompt,'stream':True}).encode()
  sock=socket.create_connection(('127.0.0.1',info['data_port']),timeout=10)
  headers='POST /v1/responses HTTP/1.1\r\nHost: localhost\r\n'+''.join(name+': '+value+'\r\n' for name,value in client.items())+'Content-Length: '+str(len(payload))+'\r\n\r\n'
  sock.sendall(headers.encode()+payload);raw=b''
  while b'local receipt' not in raw and len(raw)<100000:
   chunk=sock.recv(4096)
   if not chunk:break
   raw+=chunk
  sock.setsockopt(socket.SOL_SOCKET,socket.SO_LINGER,struct.pack('ii',1,0));sock.close()
  return int(raw.split(b' ',2)[1]),raw.decode()
 req=urllib.request.Request(base+'/v1/responses',headers=client,data=json.dumps({'model':'Exact/Model-v2','input':prompt,'stream':stream}).encode())
 try:r=http.open(req,timeout=20)
 except urllib.error.HTTPError as error:r=error
 with r:
  if cancel:
   lines=[]
   for line in r:
    lines.append(line.decode())
    if 'local receipt' in lines[-1]:break
   return r.status,''.join(lines)
  return r.status,r.read(100000).decode()
checks=[]
management['X-Config-Version']=next(row['id'] for row in get('/admin/config-versions') if row['status']=='active')
models_before=get('/v1/models',False)
req=urllib.request.Request(admin+'/admin/catalog/refresh',headers=management,data=json.dumps({'endpoint_id':'local-responses','credential_id':'local-account'}).encode())
with http.open(req,timeout=40) as r:refreshed=json.load(r)
assert refreshed['model_count']==3,refreshed
models_after=get('/v1/models',False)
assert {m['id'] for m in models_before['data']}=={'Exact/Model-v2'}=={m['id'] for m in models_after['data']}
checks.append('full two-page catalog refresh does not widen served models or key grants')
start=int(time.time()*1000)
for prompt,stream,expected in [('local-json-acceptance',False,200),('local-stream-acceptance',True,200),('fail-acceptance',False,503),('truncate-acceptance',True,200)]:
 status,body=run(prompt,stream);assert status==expected,(prompt,status,body[:120])
 if prompt=='local-stream-acceptance':assert 'response.completed' in body,body[:200]
 if prompt=='truncate-acceptance':assert 'response.failed' in body and 'response.completed' not in body,body[:200]
 checks.append(prompt)
status,_=run('cancel-acceptance',True,True);assert status==200
checks.append('client stream cancellation')
for _ in range(100):
 page=get('/admin/requests/summary?from_ms='+str(start-100)+'&limit=100')
 if page['summary']['requests']>=5 and all(row['outcome']!='unknown' for row in page['items']):break
 time.sleep(.1)
summary=page['summary'];print(json.dumps({'observed_summary':summary,'rows':[{'result':r['outcome'],'duration_ms':r['duration_ms'],'first_content_ms':r['first_content_ms'],'ledger_records':r['ledger_records']} for r in page['items']]}))
assert summary['requests']==5 and summary['succeeded']==2 and summary['failed']==2 and summary['cancelled']==1,summary
assert summary['attempts']==5,summary
assert all(row['duration_ms'] is not None for row in page['items'])
assert all(row['first_content_ms'] is None or row['first_content_ms']<=row['duration_ms'] for row in page['items'])
for _ in range(60):
 page=get('/admin/requests?from_ms='+str(start-100)+'&limit=100')
 if sum(row['ledger_records'] for row in page['items'] if row['outcome']=='succeeded')>=2:break
 time.sleep(.1)
assert sum(row['ledger_records'] for row in page['items'] if row['outcome']=='succeeded')>=2
report={'passed':True,'checks':checks,'summary':summary,'real_gateway':True,'provider':'owned loopback TLS mock','real_provider_inference_calls':0,'catalog_models':refreshed['model_count'],'opened_models':1,'successes_materialized':True}
(out/'request-chain-acceptance.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))
