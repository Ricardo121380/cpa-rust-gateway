"""Own an isolated stock serve + embedded UI + loopback TLS until the stop file exists."""
import hashlib
import json
import os
import pathlib
import secrets
import socket
import sqlite3
import ssl
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

os.umask(0o077)
out = pathlib.Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=True)
root = pathlib.Path(tempfile.mkdtemp(prefix="cpar-seo-ui-"))
state, credentials = root / "state", root / "credentials"
state.mkdir(); credentials.mkdir()
for name in ("master-key", "backup-key", "client-key-pepper", "grok-build-cache-key"):
    (credentials / name).write_bytes(secrets.token_bytes(32))
for name, prefix in (("management-key", "mgmt_"), ("management-csrf", "csrf_")):
    (credentials / name).write_text(prefix + secrets.token_hex(32))
materials = {name: "synthetic-" + secrets.token_hex(24) for name in ("responses", "chat", "messages", "kimi", "grok-api", "new", "lost")}
(root / "api-material.json").write_text(json.dumps(materials))
(root / "kiro-material.json").write_text(json.dumps({"kind":"social","access_token":"synthetic-"+secrets.token_hex(24),"refresh_token":"synthetic-"+secrets.token_hex(24),"expires_at_ms":int(time.time()*1000)+86400000}))
(root / "current-password").write_text("Synthetic-" + secrets.token_hex(16))
extension = root / "tls.ext"
extension.write_text("subjectAltName=IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\n")
commands = [
    ["openssl","req","-x509","-newkey","rsa:2048","-nodes","-days","2","-keyout",str(root/"ca.key"),"-out",str(root/"ca.pem"),"-subj","/CN=CPAR UI local CA","-addext","basicConstraints=critical,CA:TRUE"],
    ["openssl","req","-new","-newkey","rsa:2048","-nodes","-keyout",str(root/"tls.key"),"-out",str(root/"tls.csr"),"-subj","/CN=127.0.0.1"],
    ["openssl","x509","-req","-in",str(root/"tls.csr"),"-CA",str(root/"ca.pem"),"-CAkey",str(root/"ca.key"),"-CAcreateserial","-out",str(root/"tls.pem"),"-days","2","-extfile",str(extension)],
]
for command in commands:
    subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
receives = []
lock = threading.Lock()

class Provider(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass
    def reply(self, status, body):
        raw=json.dumps(body).encode();self.send_response(status)
        self.send_header("Content-Type","application/json");self.send_header("Content-Length",str(len(raw)))
        self.end_headers();self.wfile.write(raw)
    def do_GET(self):
        with lock:receives.append({"method":"GET","path":self.path})
        self.reply(200,{"data":[],"has_more":False})
    def do_POST(self):
        record={"method":"POST","path":self.path}
        with lock:receives.append(record)
        body=json.loads(self.rfile.read(int(self.headers.get("Content-Length","0"))))
        token=self.headers.get("Authorization","").removeprefix("Bearer ") or self.headers.get("x-api-key","")
        channel=next((name for name,value in materials.items() if value==token),None)
        if channel is None:return self.reply(401,{"error":{"type":"invalid_api_key"}})
        protocol="chat" if self.path.endswith("/chat/completions") else "messages" if self.path.endswith("/messages") else "responses"
        with lock:record.update(protocol=protocol,material_channel=channel,model=body.get("model"),stream=bool(body.get("stream")))
        if body.get("stream"):return self.reply(400,{"error":{"type":"fixture_json_only"}})
        response_id="local_"+secrets.token_hex(12)
        if protocol=="chat":
            return self.reply(200,{"id":response_id,"object":"chat.completion","model":body["model"],"choices":[{"index":0,"message":{"role":"assistant","content":"local UI reply"},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}})
        if protocol=="messages":
            return self.reply(200,{"id":response_id,"type":"message","role":"assistant","model":body["model"],"content":[{"type":"text","text":"local UI reply"}],"stop_reason":"end_turn","stop_sequence":None,"usage":{"input_tokens":3,"output_tokens":2}})
        self.reply(200,{"id":response_id,"object":"response","status":"completed","model":body["model"],"output":[{"id":"msg_"+response_id,"type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"local UI reply","annotations":[]}]}],"usage":{"input_tokens":3,"output_tokens":2,"total_tokens":5}})

provider=ThreadingHTTPServer(("127.0.0.1",0),Provider)
tls=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);tls.load_cert_chain(root/"tls.pem",root/"tls.key")
provider.socket=tls.wrap_socket(provider.socket,server_side=True)
threading.Thread(target=provider.serve_forever,daemon=True).start()
def port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1",0));return listener.getsockname()[1]
admin_port,data_port=port(),port()
base=f"http://127.0.0.1:{admin_port}"
binary=pathlib.Path(os.environ.get("CPAR_GATEWAY_BINARY","target/debug/gateway")).resolve()
subprocess.run([str(binary),"admin-login","init","--state-dir",str(state),"--password-file",str(root/"initial-password")],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
log=(out/"gateway.log").open("ab")
network_profile='(version 1) (allow default) (deny network-outbound) (allow network-outbound (remote ip "localhost:*"))'
preflight_code='''import json,socket,sys
with socket.create_connection(("127.0.0.1",int(sys.argv[1])),timeout=2):pass
try:socket.create_connection(("192.0.2.1",443),timeout=.5)
except PermissionError as error:assert error.errno==1
else:raise AssertionError("reserved remote address was not denied")
print(json.dumps({"localhost_connect":True,"test_net_outbound_denied":True,"real_provider_requests":0}))
'''
preflight=subprocess.check_output(["/usr/bin/sandbox-exec","-p",network_profile,sys.executable,"-c",preflight_code,str(provider.server_port)],text=True)
(out/"network-sandbox.json").write_text(preflight)
command=["/usr/bin/sandbox-exec","-p",network_profile,str(binary),"serve","--state-dir",str(state),"--credential-dir",str(credentials),"--data-listen",f"127.0.0.1:{data_port}","--management-listen",f"127.0.0.1:{admin_port}"]
gateway=subprocess.Popen(command,stdout=log,stderr=log,env={**os.environ,"SSL_CERT_FILE":str(root/"ca.pem")})
client=urllib.request.build_opener(urllib.request.ProxyHandler({}))
scope,revision=None,None
def api(method,path,body=None,expected=200,extra=None):
    global revision
    headers={"X-Management-Key":(credentials/"management-key").read_text(),"Content-Type":"application/json",**({"X-Config-Version":scope} if scope else {}),**({"If-Match":revision} if revision and method!="GET" else {}),**(extra or {})}
    try:response=client.open(urllib.request.Request(base+path,method=method,headers=headers,data=None if body is None else json.dumps(body).encode()),timeout=20)
    except urllib.error.HTTPError as error:response=error
    with response:
        value=json.loads(response.read() or "null");assert response.status==expected,(path,response.status,value.get("error") if isinstance(value,dict) else None)
        if method!="GET" and response.headers.get("ETag"):revision=response.headers["ETag"]
        return value
try:
    for _ in range(200):
        if gateway.poll() is not None:raise RuntimeError("owned gateway exited")
        try:api("GET","/admin/config-versions");break
        except urllib.error.URLError:time.sleep(.05)
    draft=api("POST","/admin/config-versions",{"id":"ui-baseline","description":"Isolated embedded UI baseline"},201)
    scope,revision=draft["id"],draft["revision"]
    api("POST","/admin/egress-policies",{"id":"ui-local-policy","name":"Only this loopback fixture","allowed_schemes":["https"],"allowed_hosts":["127.0.0.1"],"allowed_ports":[provider.server_port],"allowed_cidrs":["127.0.0.1/32"],"redirect_mode":"deny","max_redirects":0},201)
    # Stock Kiro preparation checks its canonical policy. Its owner stays disabled,
    # and the child-process OS sandbox denies all non-loopback socket connections.
    api("POST","/admin/egress-policies",{"id":"kiro-us-east-1-egress","name":"Canonical Kiro under OS outbound deny","allowed_schemes":["https"],"allowed_hosts":["runtime.us-east-1.kiro.dev","q.us-east-1.amazonaws.com","management.us-east-1.kiro.dev"],"allowed_ports":[443],"allowed_cidrs":[],"redirect_mode":"deny","max_redirects":0},201)
    presets={"responses":("openai-compatible","openai-compatible.responses","openai/responses","/responses"),"chat":("openai-compatible","openai-compatible.chat-completions","openai/chat-completions","/chat/completions"),"messages":("anthropic-compatible","anthropic-compatible.messages","anthropic/messages","/v1/messages"),"kimi":("kimi","openai-compatible.chat-completions","openai/chat-completions","/chat/completions"),"grok-api":("grok.official","grok.official.responses","openai/responses","/v1/responses"),"kiro":("kiro","kiro.messages","anthropic/messages","/")}
    targets={}
    for preset,(kind,adapter,api_format,path) in presets.items():
        owner="kiro-us-east-1" if preset=="kiro" else "ui-"+preset
        target=owner+"-messages" if preset=="kiro" else owner+"-connection"
        enabled=preset not in ("grok-api","kiro")
        address="https://api.x.ai" if preset=="grok-api" else "https://runtime.us-east-1.kiro.dev" if preset=="kiro" else f"https://127.0.0.1:{provider.server_port}/v1"
        endpoint_enabled=preset!="grok-api"
        api("POST","/admin/upstreams",{"id":owner,"name":"Isolated "+preset,"kind":kind,"enabled":enabled,"tags":[],"egress_policy_id":"kiro-us-east-1-egress" if preset=="kiro" else "ui-local-policy"},201)
        api("POST",f"/admin/upstreams/{owner}/endpoints",{"id":target,"adapter_id":adapter,"api_format":api_format,"base_url":address,"inference_path":path,"models_path":None,"transport":"https","enabled":endpoint_enabled},201)
        targets[preset]={"upstream_id":owner,"endpoint_id":target,"base":address,"path":path,"adapter":adapter,"format":api_format,"enabled":enabled,"endpoint_enabled":endpoint_enabled}
    assert api("POST","/admin/config-versions/ui-baseline/validate")["valid"]
    api("POST","/admin/config-versions/ui-baseline/publish",extra={"X-Expected-Active-Version":"null","X-Expected-Lifecycle-Event":"0"})
    descriptor={"origin":base,"data_origin":f"http://127.0.0.1:{data_port}","root":str(root),"state":str(state),"command":command,"gateway_pid":gateway.pid,"binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"source_sha":subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(),"source_dirty":subprocess.check_output(["git","status","--porcelain=v1"],text=True).splitlines(),"initial_password_file":str(root/"initial-password"),"password_file":str(root/"current-password"),"api_material_file":str(root/"api-material.json"),"kiro_material_file":str(root/"kiro-material.json"),"targets":targets,"real_provider_calls":0,"outside_loopback_denied_by_os_sandbox":True,"api_targets_use_loopback_egress_policy":True}
    (out/"descriptor.json").write_text(json.dumps(descriptor,indent=2)+"\n")
    print(json.dumps({"ready":True,"origin":base,"gateway_pid":gateway.pid,"descriptor":str(out/"descriptor.json")}),flush=True)
    while not (out/"stop").exists():
        if gateway.poll() is not None:raise RuntimeError("owned gateway exited during UI acceptance")
        (out/"provider-receives.json").write_text(json.dumps(receives,indent=2)+"\n");time.sleep(.2)
finally:
    if gateway.poll() is None:gateway.terminate();gateway.wait(timeout=30)
    provider.shutdown();provider.server_close();log.close()
    (out/"provider-receives.json").write_text(json.dumps(receives,indent=2)+"\n")
    if (state/"control.sqlite3").exists():
        with sqlite3.connect(state/"control.sqlite3") as db:
            facts={"versions":db.execute("SELECT id,status,revision FROM config_versions").fetchall(),"accounts":db.execute("SELECT config_version_id,id,upstream_id,kind,status,revision FROM upstream_credentials").fetchall(),"bindings":db.execute("SELECT config_version_id,endpoint_id,credential_id,enabled FROM endpoint_credential_bindings").fetchall()}
        (out/"stored-facts.json").write_text(json.dumps(facts,indent=2)+"\n")
