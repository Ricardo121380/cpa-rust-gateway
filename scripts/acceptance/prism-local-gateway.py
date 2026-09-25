"""Owned real gateway + loopback-only TLS mock. No real provider calls or production state."""
import os,pathlib,tempfile,secrets,socket,subprocess,time,json,ssl,threading,urllib.request,urllib.error,signal,sys
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
os.umask(0o077)
repo=pathlib.Path.cwd();target=pathlib.Path(os.environ.get('CARGO_TARGET_DIR', str(repo/'target'))).resolve();out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True);root=pathlib.Path(tempfile.mkdtemp(prefix='prism-complete-local-'));state=root/'state';creds=root/'credentials';state.mkdir();creds.mkdir()
previous=out/'local-preview.json'
if previous.exists():
 old=json.loads(previous.read_text())
 try:
  command=subprocess.check_output(['ps','-p',str(old.get('pid',0)),'-o','command='],text=True,stderr=subprocess.DEVNULL)
  if old.get('synthetic') and old.get('root','unused') in command and 'gateway serve' in command:raise RuntimeError('An owned preview is already running; reuse it or stop its controller first')
 except subprocess.CalledProcessError:pass
for name in ['master-key','backup-key','client-key-pepper','grok-build-cache-key']:(creds/name).write_bytes(secrets.token_bytes(32))
for name,prefix in [('management-key','mgmt_'),('management-csrf','csrf_')]:(creds/name).write_text(prefix+secrets.token_hex(32))
ca=root/'local-ca.pem';cakey=root/'local-ca.key';csr=root/'server.csr';ext=root/'server.ext';ext.write_text('subjectAltName=IP:127.0.0.1,DNS:localhost\nbasicConstraints=critical,CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n')
commands=[['openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','2','-keyout',str(cakey),'-out',str(ca),'-subj','/CN=Local acceptance CA','-addext','basicConstraints=critical,CA:TRUE','-addext','keyUsage=critical,keyCertSign,digitalSignature,keyEncipherment'],['openssl','req','-new','-newkey','rsa:2048','-nodes','-subj','/CN=localhost','-keyout',str(root/'tls.key'),'-out',str(csr)],['openssl','x509','-req','-in',str(csr),'-CA',str(ca),'-CAkey',str(cakey),'-CAcreateserial','-out',str(root/'tls.pem'),'-days','2','-extfile',str(ext)]]
for command in commands:subprocess.run(command,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
provider_keys=['local-'+secrets.token_hex(24) for _ in range(5)];provider_key=provider_keys[0]
models=['Exact/Model-v2','Model-Second','Model-Third']
class Provider(BaseHTTPRequestHandler):
 protocol_version='HTTP/1.1'
 def log_message(self,*args):pass
 def reply(self,status,body):
  raw=json.dumps(body).encode();self.send_response(status);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(raw)));self.end_headers();self.wfile.write(raw)
 def do_GET(self):
  if self.headers.get('Authorization') not in ['Bearer '+key for key in provider_keys]:return self.reply(401,{'error':{'message':'unauthorized'}})
  remaining=models[2:] if 'after_id=' in self.path else models[:2]
  self.reply(200,{'data':[{'id':model} for model in remaining],'has_more':'after_id=' not in self.path,'last_id':remaining[-1]})
  with (root/'metadata-calls.jsonl').open('a') as f:f.write(json.dumps({'path':self.path,'count':len(remaining)})+'\n')
 def do_POST(self):
  try:
   body=json.loads(self.rfile.read(int(self.headers.get('Content-Length',0))))
   if self.headers.get('Authorization') not in ['Bearer '+key for key in provider_keys]:return self.reply(401,{'error':{'message':'unauthorized'}})
   text=json.dumps(body);model=body.get('model');mode='fail' if 'fail-acceptance' in text else 'cancel' if 'cancel-acceptance' in text else 'stream-fail' if 'truncate-acceptance' in text else 'success'
   with (root/'inference-calls.jsonl').open('a') as f:f.write(json.dumps({'model':model,'mode':mode,'stream':bool(body.get('stream'))})+'\n')
   if mode=='fail':return self.reply(503,{'error':{'message':'local upstream unavailable','type':'server_error'}})
   if 'agent-roundtrip-acceptance' in text:
    history=body.get('input',[])
    turns=sum(item.get('type')=='function_call_output' for item in history if isinstance(item,dict)) if isinstance(history,list) else 0
    if turns:
     reasoning=[item for item in history if item.get('type')=='reasoning']
     calls=[item for item in history if item.get('type')=='function_call']
     results=[item for item in history if item.get('type')=='function_call_output']
     valid=len(reasoning)==turns and len(calls)==turns and all(call['call_id']==result['call_id'] and call['status']=='completed' for call,result in zip(calls,results))
     valid=valid and all(item.get('content')==[{'type':'reasoning_text','text':'synthetic reasoning'}] for item in reasoning)
     if not valid:return self.reply(400,{'error':{'message':'lost or reordered agent history'}})
    items=[{'id':f'rs_{turns}','type':'reasoning','status':'completed','summary':[{'type':'summary_text','text':'synthetic reasoning'}]}]
    if turns<2:items.append({'id':f'fc_{turns}','type':'function_call','status':'completed','call_id':f'call_{turns}','name':'read','arguments':'{"path":"proof.txt"}'})
    else:items.append({'id':'msg_done','type':'message','role':'assistant','status':'completed','content':[{'type':'output_text','text':'agent roundtrip complete','annotations':[]}]})
    response={'id':'resp_'+secrets.token_hex(8),'object':'response','status':'completed','model':model,'output':items,'usage':{'input_tokens':10+turns,'output_tokens':4,'total_tokens':14+turns}}
    with (root/'agent-roundtrip-calls.jsonl').open('a') as f:f.write(json.dumps({'turn':turns,'history_types':[item.get('type','message') for item in history],'reasoning':body.get('reasoning')})+'\n')
    if not body.get('stream'):return self.reply(200,response)
    self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Connection','close');self.end_headers();self.close_connection=True
    def agent_frame(kind,**payload):
     self.wfile.write(('event: '+kind+'\ndata: '+json.dumps({'type':kind,**payload})+'\n\n').encode());self.wfile.flush()
    agent_frame('response.created',response={**response,'status':'in_progress','output':[]})
    for index,item in enumerate(items):
     agent_frame('response.output_item.added',output_index=index,item={**item,'status':'in_progress','summary':[]} if item['type']=='reasoning' else {**item,'status':'in_progress'})
     if item['type']=='message':agent_frame('response.output_text.delta',output_index=index,item_id=item['id'],delta='agent roundtrip complete')
     agent_frame('response.output_item.done',output_index=index,item=item)
    agent_frame('response.completed',response=response)
    return
   item={'id':'msg_local','type':'message','role':'assistant','status':'completed','content':[{'type':'output_text','text':'local receipt','annotations':[]}]}
   response={'id':'resp_'+secrets.token_hex(8),'object':'response','created_at':int(time.time()),'status':'completed','model':model,'output':[item],'usage':{'input_tokens':10,'output_tokens':3,'total_tokens':13}}
   time.sleep(.03)
   if not body.get('stream'):return self.reply(200,response)
   self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Connection','close');self.end_headers();self.close_connection=True
   def frame(kind,payload):
    self.wfile.write(('event: '+kind+'\ndata: '+json.dumps({'type':kind,**payload})+'\n\n').encode());self.wfile.flush()
   frame('response.created',{'response':{**response,'status':'in_progress','output':[]}})
   frame('response.output_item.added',{'output_index':0,'item':{**item,'status':'in_progress','content':[]}})
   frame('response.content_part.added',{'item_id':item['id'],'output_index':0,'content_index':0,'part':{'type':'output_text','text':'','annotations':[]}})
   time.sleep(.05)
   frame('response.output_text.delta',{'item_id':item['id'],'output_index':0,'content_index':0,'delta':'local receipt'})
   if mode=='stream-fail':return
   if mode=='cancel':time.sleep(2)
   frame('response.output_text.done',{'item_id':item['id'],'output_index':0,'content_index':0,'text':'local receipt'})
   frame('response.content_part.done',{'item_id':item['id'],'output_index':0,'content_index':0,'part':item['content'][0]})
   frame('response.output_item.done',{'output_index':0,'item':item})
   frame('response.completed',{'response':response})
  except (BrokenPipeError,ConnectionResetError):pass
provider=ThreadingHTTPServer(('127.0.0.1',0),Provider);tls=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);tls.load_cert_chain(root/'tls.pem',root/'tls.key');provider.socket=tls.wrap_socket(provider.socket,server_side=True);threading.Thread(target=provider.serve_forever,daemon=True).start()
def port():
 with socket.socket() as s:s.bind(('127.0.0.1',0));return s.getsockname()[1]
a,d=port(),port();base=f'http://127.0.0.1:{a}';http=urllib.request.build_opener(urllib.request.ProxyHandler({}));log=(root/'gateway.log').open('ab')
subprocess.run([str(target/'debug/gateway'),'admin-login','init','--state-dir',str(state),'--password-file',str(root/'initial-password')],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
info={'root':str(root),'url':base+'/admin-ui/#/unlock','admin_port':a,'data_port':d,'mock_port':provider.server_port,'synthetic':True,'controller_pid':os.getpid()}
def start():
 p=subprocess.Popen([str(target/'debug/gateway'),'serve','--state-dir',str(state),'--credential-dir',str(creds),'--data-listen',f'127.0.0.1:{d}','--management-listen',f'127.0.0.1:{a}'],stdout=log,stderr=log,env={**os.environ,'SSL_CERT_FILE':str(ca)})
 info['pid']=p.pid;(out/'local-preview.json').write_text(json.dumps(info,indent=2));return p
p=start();scope=None;revision=None
headers={'Origin':base,'X-Management-Key':(creds/'management-key').read_text(),'X-Management-CSRF-Token':(creds/'management-csrf').read_text(),'Content-Type':'application/json'}
def api(method,path,body=None,extra=None):
 global revision
 h={**headers,**({'X-Config-Version':scope} if scope else {}),**({'If-Match':revision} if method!='GET' and revision else {}),**(extra or {})}
 try:r=http.open(urllib.request.Request(base+path,method=method,headers=h,data=None if body is None else json.dumps(body).encode()),timeout=40)
 except urllib.error.HTTPError as error:
  raw=error.read();code=json.loads(raw).get('error',{}).get('code','unknown');raise RuntimeError(f'{method} {path}: {error.code} {code}') from None
 with r:
  if method!='GET' and r.headers.get('ETag'):revision=r.headers['ETag']
  raw=r.read();return json.loads(raw) if raw else None
signal.signal(signal.SIGTERM,lambda *_:sys.exit(0))
try:
 for _ in range(200):
  if p.poll() is not None:raise RuntimeError('gateway_start_failed')
  try:api('GET','/admin/config-versions');break
  except OSError:time.sleep(.1)
 v=api('POST','/admin/config-versions',{'id':'local-accepted','description':'Isolated local workflows'});scope=v['id'];revision=v['revision']
 api('POST','/admin/egress-policies',{'id':'local-policy','name':'Local TLS','allowed_schemes':['https'],'allowed_hosts':['127.0.0.1'],'allowed_ports':[provider.server_port],'allowed_cidrs':['127.0.0.1/32'],'redirect_mode':'deny','max_redirects':0})
 api('POST','/admin/upstreams',{'id':'local-provider','name':'Local compatible provider','kind':'openai-compatible','enabled':True,'tags':[],'egress_policy_id':'local-policy'})
 api('POST','/admin/upstreams/local-provider/endpoints',{'id':'local-responses','adapter_id':'openai-compatible.responses','api_format':'openai/responses','base_url':f'https://127.0.0.1:{provider.server_port}/v1','inference_path':'/responses','models_path':'/models','transport':'https','enabled':True})
 for index,key in enumerate(provider_keys):
  account=api('POST','/admin/upstreams/local-provider/account-import',{'id':'local-account' if index==0 else f'local-account-{index}','channel':'openai-compatible','secret':key})
  api('POST','/admin/endpoints/local-responses/credential-bindings',{'credential_id':account['id'],'enabled':True,'priority':0,'weight':1,'concurrency':1})
 api('POST','/admin/public-models',{'id':'local-model','model_name':models[0],'display_name':models[0],'status':'active','capabilities':{}})
 api('POST','/admin/public-models/local-model/routes',{'id':'local-route','policy':'smooth_weighted_round_robin','max_attempts':1,'bootstrap_timeout_ms':10000})
 api('POST','/admin/routes/local-route/candidates',{'id':'local-candidate','endpoint_id':'local-responses','upstream_model':models[0],'credential_scope':'all_active','transform_mode':'canonical_bridge','enabled':True,'priority':0,'weight':1,'capability_override':{'allow_unlisted_model':True,'stored_responses':True}})
 api('POST','/admin/access-groups',{'id':'local-group','name':'Local client','status':'active','limits':{}})
 api('POST','/admin/access-groups/local-group/routes',{'route_id':'local-route','enabled':True})
 issued=api('POST','/admin/client-keys',{'id':'local-client','access_group_id':'local-group','status':'active','expires_at_ms':None});(root/'client-key').write_text(issued['key'])
 rates={key:0 for key in ['reasoning_microunits_per_million','cache_read_microunits_per_million','cache_creation_microunits_per_million','cached_microunits_per_million']}
 api('POST','/admin/billing/catalogs',{'catalog_version_id':'local-prices','effective_at_ms':0,'source':'operator','entries':[{'provider_id':'local-provider','channel_id':'local-responses','model':models[0],'input_microunits_per_million':1000000,'output_microunits_per_million':2000000,**rates}]})
 valid=api('POST',f'/admin/config-versions/{scope}/validate');assert valid['valid'],'configuration_invalid'
 api('POST',f'/admin/config-versions/{scope}/publish',extra={'X-Expected-Active-Version':'null','X-Expected-Lifecycle-Event':'0'})
 print(json.dumps({'ready':True,'url':info['url'],'root':str(root),'synthetic':True}),flush=True)
 while p.poll() is None:
  if (root/'restart-request').exists():
   (root/'restart-request').unlink();p.terminate();p.wait(timeout=40);p=start();print('restarted',flush=True)
  time.sleep(.2)
finally:
 if p.poll() is None:p.terminate();p.wait(timeout=40)
 provider.shutdown()
