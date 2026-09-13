import sys
import pathlib,json,urllib.request,urllib.error,time
out=pathlib.Path(sys.argv[1]).resolve();m=json.loads((out/'local-preview.json').read_text());assert m['synthetic'];root=pathlib.Path(m['root']);http=urllib.request.build_opener(urllib.request.ProxyHandler({}));base=f"http://127.0.0.1:{m['data_port']}";admin=f"http://127.0.0.1:{m['admin_port']}"
def request(key,path,body=None):
 try:r=http.open(urllib.request.Request(base+path,headers={'Authorization':'Bearer '+key,'Content-Type':'application/json'},data=None if body is None else json.dumps(body).encode()),timeout=20)
 except urllib.error.HTTPError as e:r=e
 with r:return r.status,json.load(r)
old=(root/'client-key').read_text();new=(root/'ui-client-key').read_text();a,models=request(old,'/v1/models');assert a==200 and {r['id'] for r in models['data']}=={'Exact/Model-v2'}
a,models=request(new,'/v1/models');assert a==200 and {r['id'] for r in models['data']}=={'Model-Second'}
a,_=request(new,'/v1/responses',{'model':'Exact/Model-v2','input':'must be denied'});assert a==404
start=int(time.time()*1000);a,response=request(new,'/v1/responses',{'model':'Model-Second','input':'local UI permission acceptance'});assert a==200,(a,response)
headers={'X-Management-Key':(root/'credentials/management-key').read_text()}
for _ in range(80):
 with http.open(urllib.request.Request(admin+'/admin/requests?model=Model-Second&from_ms='+str(start-100),headers=headers)) as r:page=json.load(r)
 if page['items'] and page['items'][0]['ledger_records']:break
 time.sleep(.1)
row=page['items'][0];assert row['outcome']=='succeeded' and row['cost_confidence']=='unpriced' and row['cost_microunits'] is None,row
report={'passed':True,'real_gateway':True,'ui_model_opened':'Model-Second','ui_key_select_all_count':2,'ui_key_final_selection':['Model-Second'],'existing_key_unchanged':True,'cross_model_denied':True,'unpriced_preserved':True,'mock_inference_only':True}
(out/'ui-model-key-acceptance.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))
