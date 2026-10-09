"""Build refresh -> actual stock-process restart -> public JSON/SSE continuation.

One owned temporary state, fixed-origin TLS loopback, OS outbound deny, synthetic
grants only. The real startup worker creates the proof; this driver never sets it.
"""
import json
import os
import pathlib
import sqlite3
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

os.umask(0o077)
out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
server_out=out/"server";server_out.mkdir()
log=(out/"server-controller.log").open("wb")
server=subprocess.Popen([sys.executable,"scripts/acceptance/cpar_seo_ui_http.py",str(server_out)],stdout=log,stderr=log,env={**os.environ,"CPAR_UI_OPERATIONS":"1"})
client=urllib.request.build_opener(urllib.request.ProxyHandler({}))
result={"result":"FAIL","scope":"stock-process / public HTTP / fixed TLS loopback synthetic only","real_provider_calls":0,"checks":[]}
scope=revision=None

def wait_for(predicate,timeout=30):
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
        if server.poll() is not None:raise RuntimeError("owned controller exited")
        try:
            value=predicate()
            if value:return value
        except (OSError,ValueError,sqlite3.OperationalError):pass
        time.sleep(.1)
    raise AssertionError("controlled boundary did not reach expected state")

def descriptor():return json.loads((server_out/"descriptor.json").read_text())

def api(method,path,body=None,expected=200,extra=None):
    global revision
    headers={"X-Management-Key":management,"Content-Type":"application/json",**({"X-Config-Version":scope} if scope else {}),**({"If-Match":revision} if revision and method!="GET" else {}),**(extra or {})}
    try:response=client.open(urllib.request.Request(d["origin"]+path,method=method,headers=headers,data=None if body is None else json.dumps(body).encode()),timeout=20)
    except urllib.error.HTTPError as error:response=error
    with response:
        value=json.loads(response.read() or "null")
        assert response.status==expected,(method,path,response.status,value.get("error",{}).get("code") if isinstance(value,dict) else None)
        if method!="GET" and response.headers.get("ETag"):revision=response.headers["ETag"]
        return value

def data(body,key=None):
    request=urllib.request.Request(d["data_origin"]+"/v1/responses",method="POST",headers={"Authorization":"Bearer "+(key or presented),"Content-Type":"application/json"},data=json.dumps(body).encode())
    try:response=client.open(request,timeout=20)
    except urllib.error.HTTPError as error:response=error
    with response:return response.status,response.read()

def account_facts():
    with sqlite3.connect(pathlib.Path(d["state"])/"control.sqlite3") as db:
        return [{"id":row[0],"revision":row[1],"floor":row[2],"current":row[3],"enabled":row[4],"claim":row[5]} for row in db.execute("SELECT id,revision,continuation_floor_revision,continuation_current_revision,enabled,worker_claim_kind FROM grok_accounts WHERE provider='build' ORDER BY id")]

def stored_root_count():
    with sqlite3.connect(pathlib.Path(d["state"])/"control.sqlite3") as db:
        return db.execute("SELECT count(*) FROM stored_responses WHERE client_key_id='e05-owner'").fetchone()[0]

def historical_rows():
    with sqlite3.connect(pathlib.Path(d["state"])/"control.sqlite3") as db:
        return {"request":set(row[0] for row in db.execute("SELECT event_ordinal FROM gateway_event_log WHERE event_type='request'")),
                "usage":set(row[0] for row in db.execute("SELECT event_ordinal FROM gateway_event_log WHERE event_type='usage'")),
                "ledger":set(row[0] for row in db.execute("SELECT ledger_id FROM billing_ledger_entries"))}

def wait_for_idle_native():
    # Startup catalog/entitlement workers share the real native pool's single
    # lease. Observe their release before an inference; never retry a write.
    return wait_for(lambda: next((row for row in api("GET","/admin/operations/provider-account-pools?limit=100")["items"] if row.get("account_id")==saved["account_id"] and row["active_leases"]==0),None),30)

def authorize(target=None):
    started=api("POST","/admin/native-account-authorizations",{"name":"e05-controlled",**({"target":target} if target else {})})
    session=started["session_id"]
    def terminal():
        view=api("POST",f"/admin/native-account-authorizations/{session}/poll")
        return view if view["state"]!="pending" else None
    view=wait_for(terminal,20);assert view["state"]=="complete" and view["runtime_applied"]
    return {key:view[key] for key in ["account_id","credential_revision","identity_state","runtime_applied"]}

def root_id(raw,stream):
    if not stream:return json.loads(raw)["id"]
    frames=[json.loads(line[6:]) for line in raw.decode().splitlines() if line.startswith("data: ")]
    assert any(frame.get("type")=="response.completed" for frame in frames)
    return next(frame["response"]["id"] for frame in frames if frame.get("type")=="response.created")

try:
    d=wait_for(lambda:descriptor());result.update(source_sha=d["source_sha"],source_dirty=d["source_dirty"],binary_sha256=d["binary_sha256"],original_pid=d["gateway_pid"],sandbox=d["outside_loopback_denied_by_os_sandbox"])
    management=(pathlib.Path(d["root"])/"credentials/management-key").read_text().strip()
    saved=authorize();result["initial_save"]=saved
    active=next(row for row in api("GET","/admin/config-versions") if row["status"]=="active");scope,revision=active["id"],active["revision"]
    draft=api("POST",f"/admin/config-versions/{scope}/fork",{"id":"e05-active","description":"Owned Build continuation process acceptance"},201);scope,revision=draft["id"],draft["revision"]
    api("POST","/admin/egress-policies",{"id":"e05-fixed","name":"Fixed Build host under outbound deny","allowed_schemes":["https"],"allowed_hosts":["cli-chat-proxy.grok.com"],"allowed_ports":[443],"allowed_cidrs":["127.0.0.1/32"],"redirect_mode":"deny","max_redirects":0},201)
    api("POST","/admin/upstreams",{"id":"e05-build","name":"Controlled native Build","kind":"grok-build-native","enabled":True,"tags":[],"egress_policy_id":"e05-fixed"},201)
    api("POST","/admin/upstreams/e05-build/endpoints",{"id":"e05-native-endpoint","adapter_id":"grok.build.responses","api_format":"openai/responses","base_url":"https://cli-chat-proxy.grok.com/v1","inference_path":"/responses","models_path":None,"transport":"https","enabled":True},201)
    # An actual eligible ordinary sibling makes a zero-fallback assertion meaningful.
    material=json.loads(pathlib.Path(d["api_material_file"]).read_text())["responses"]
    api("POST","/admin/upstreams",{"id":"e05-sibling","name":"Controlled fallback","kind":"openai-compatible","enabled":True,"tags":[],"egress_policy_id":"ui-local-policy"},201)
    api("POST","/admin/upstreams/e05-sibling/endpoints",{"id":"e05-sibling-endpoint","adapter_id":"openai-compatible.responses","api_format":"openai/responses","base_url":d["targets"]["responses"]["base"],"inference_path":"/responses","models_path":None,"transport":"https","enabled":True},201)
    api("POST","/admin/upstreams/e05-sibling/account-import",{"id":"e05-sibling-account","channel":"openai-compatible","secret":material},201)
    api("POST","/admin/endpoints/e05-sibling-endpoint/credential-bindings",{"credential_id":"e05-sibling-account","enabled":True,"priority":0,"weight":1,"concurrency":1},201)
    api("POST","/admin/public-models",{"id":"e05-model","model_name":"grok-model","display_name":"Controlled Build","status":"active","capabilities":{}},201)
    api("POST","/admin/public-models/e05-model/routes",{"id":"e05-route","policy":"smooth_weighted_round_robin","max_attempts":2,"bootstrap_timeout_ms":10000},201)
    for endpoint,priority,mode in [("e05-native-endpoint",0,"canonical"),("e05-sibling-endpoint",10,"canonical_bridge")]:
        api("POST","/admin/routes/e05-route/candidates",{"id":endpoint+"-candidate","endpoint_id":endpoint,"upstream_model":"grok-model","credential_scope":"all_active","transform_mode":mode,"enabled":True,"priority":priority,"weight":1,"capability_override":{"allow_unlisted_model":True}},201)
    api("POST","/admin/access-groups",{"id":"e05-group","name":"Controlled owner","status":"active","limits":{}},201)
    api("POST","/admin/access-groups/e05-group/routes",{"route_id":"e05-route","enabled":True},201)
    presented=api("POST","/admin/client-keys",{"id":"e05-owner","access_group_id":"e05-group","status":"active","expires_at_ms":None},201)["key"]
    foreign=api("POST","/admin/client-keys",{"id":"e05-foreign","access_group_id":"e05-group","status":"active","expires_at_ms":None},201)["key"]
    assert api("POST",f"/admin/config-versions/{scope}/validate")["valid"]
    event=max(row["id"] for row in api("GET","/admin/audit-events") if row["action"]=="config_published")
    api("POST",f"/admin/config-versions/{scope}/publish",extra={"X-Expected-Active-Version":json.dumps(active["id"]),"X-Expected-Lifecycle-Event":str(event)})
    result["runtime_pools"]=api("GET","/admin/operations/provider-account-pools?limit=100")["items"]
    result["effective_models"]=api("GET","/admin/models/effective?client_key_id=e05-owner")
    result["idle_native_before_inference"]=wait_for_idle_native()
    roots=[]
    for stream in [False,True]:
        body={"model":"grok-model","input":"controlled first turn","store":True,"stream":stream,"parallel_tool_calls":False,"reasoning":{"effort":"low","summary":"auto"},"tools":[{"type":"function","name":"local_tool","parameters":{"type":"object"}}]}
        status,raw=data(body)
        if status!=200:
            result["failed_requests"]=api("GET","/admin/requests?limit=20")
            result["configured_candidates"]=api("GET","/admin/route-candidates")
        assert status==200,("first",stream,status,json.loads(raw).get("error",{}).get("code") if status!=200 else None)
        roots.append((stream,root_id(raw,stream)))
    result["before_restart"]=account_facts();(server_out/"e05-restart").touch()
    wait_for(lambda:descriptor()["restart_count"]==1)
    wait_for(lambda:account_facts()[0]["current"]==2)
    current=descriptor();assert current["gateway_pid"]!=result["original_pid"] and current["prior_process"]["exit_code"]==0
    result.update(restarted_pid=current["gateway_pid"],prior_exit_code=current["prior_process"]["exit_code"],after_startup_worker=account_facts())
    assert result["after_startup_worker"][0]["revision"]==1 and result["after_startup_worker"][0]["floor"]==1
    result["idle_native_after_restart"]=wait_for_idle_native()
    for stream,root in roots:
        body={"model":"grok-model","input":[{"type":"function_call_output","call_id":"e05-tool-call","output":"controlled tool result"}],"previous_response_id":root,"store":True,"stream":stream,"parallel_tool_calls":False,"reasoning":{"effort":"low","summary":"auto"}}
        status,raw=data(body);assert status==200,("refreshed continuation",stream,status,json.loads(raw).get("error",{}).get("code") if status!=200 else None);root_id(raw,stream)
        assert data(body,foreign)[0]>=400
        result["checks"].append({"case":"startup-worker-process-restart-continuation","stream":stream,"result":"PASS","upstream_count":1})
    manual=authorize({"account_id":saved["account_id"],"revision":1});result["manual_reauthorization"]=manual
    result["after_manual_reauthorization"]=account_facts()
    assert all(row["current"]!=row["revision"]+1 for row in account_facts()),"stale proof must not match the current runtime revision"
    prior_roots=stored_root_count();assert prior_roots==4
    for stream,root in roots:
        status,raw=data({"model":"grok-model","input":"must reject","previous_response_id":root,"store":True,"stream":stream,"parallel_tool_calls":False});assert status>=400 and b"response.completed" not in raw
        result["checks"].append({"case":"manual-reauthorization-rejects-old-root","stream":stream,"result":"PASS","upstream_count":0,"fallback_count":0})
    device=wait_for(lambda:json.loads((server_out/"device-receives.json").read_text()) if (server_out/"device-receives.json").exists() else None)
    assert device["refresh_requests"]==1
    receives=wait_for(lambda:json.loads((server_out/"provider-receives.json").read_text()))
    native=[row for row in receives if row.get("protocol")=="grok-build-native"]
    assert len(native)==4 and sum(row["has_tool_output"] for row in native)==2
    assert not [row for row in receives if row["method"]=="POST" and row.get("protocol")!="grok-build-native"],"no ordinary fallback or retry"
    assert stored_root_count()==prior_roots,"rejected requests must not produce successful history roots"
    history=wait_for(lambda:historical_rows() if all(historical_rows().values()) else None)
    account=account_facts()[0]
    impact=api("GET",f"/admin/native-accounts/{saved['account_id']}/deletion-impact?revision={account['revision']}")
    assert impact["connections"][0]["endpoint_id"]=="e05-native-endpoint"
    api("DELETE",f"/admin/native-accounts/{saved['account_id']}?revision={account['revision']}",extra={"X-Deletion-Impact-Review":impact["review_token"]})
    assert not account_facts() and stored_root_count()==prior_roots
    retained=historical_rows();assert all(ids.issubset(retained[kind]) for kind,ids in history.items())
    assert api("GET","/admin/endpoints/e05-native-endpoint")["id"]=="e05-native-endpoint"
    assert api("GET","/admin/public-models/e05-model")["id"]=="e05-model"
    result["native_deletion_history"]={kind:{"before":len(ids),"after":len(retained[kind]),"all_original_ids_retained":True} for kind,ids in history.items()}
    result.update(result="PASS",provider_calls=native,refresh_requests=device["refresh_requests"],device_exchanges=sum(row["token_requests"] for row in device["sessions"]),fallback_calls=0,stored_successful_roots=prior_roots)
    print(json.dumps({"result":"PASS","checks":len(result["checks"]),"provider_calls":len(native),"refresh_requests":1,"fallback_calls":0,"actual_process_restart":True}))
except Exception as error:
    result["error"]=str(error);raise
finally:
    (out/"e05-process-http.json").write_text(json.dumps(result,indent=2)+"\n")
    (server_out/"stop").touch()
    if server.poll() is None:server.wait(timeout=40)
    log.close()
