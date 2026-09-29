"""Batch A: real gateway, SQLite publication and loopback TLS Provider only.

All credentials are generated synthetic fixtures. No real Provider or production state.
Run after cargo build -p gateway --bin gateway; writes a value-free JSON receipt.
"""
import concurrent.futures
import http.client
import json
import os
import pathlib
import secrets
import socket
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
repo = pathlib.Path.cwd()
out = pathlib.Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=True)
root = pathlib.Path(tempfile.mkdtemp(prefix="cpar-a-http-"))
state, credentials = root / "state", root / "credentials"
state.mkdir()
credentials.mkdir()
for name in ["master-key", "backup-key", "client-key-pepper", "grok-build-cache-key"]:
    (credentials / name).write_bytes(secrets.token_bytes(32))
for name, prefix in [("management-key", "mgmt_"), ("management-csrf", "csrf_")]:
    (credentials / name).write_text(prefix + secrets.token_hex(32))
ca, ca_key = root / "ca.pem", root / "ca.key"
extension = root / "tls.ext"
extension.write_text("subjectAltName=IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\n")
for command in [
    ["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-keyout", str(ca_key), "-out", str(ca), "-subj", "/CN=CPAR A local CA", "-addext", "basicConstraints=critical,CA:TRUE"],
    ["openssl", "req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", str(root / "tls.key"), "-out", str(root / "tls.csr"), "-subj", "/CN=127.0.0.1"],
    ["openssl", "x509", "-req", "-in", str(root / "tls.csr"), "-CA", str(ca), "-CAkey", str(ca_key), "-CAcreateserial", "-out", str(root / "tls.pem"), "-days", "2", "-extfile", str(extension)],
]:
    subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

keys = {name: "synthetic-" + secrets.token_hex(24) for name in ["a", "b"]}
calls = []
catalog_calls = []
catalog_mode = "success"
calls_lock = threading.Lock()
stream_started, release_stream = threading.Event(), threading.Event()
failure_started, release_failure = threading.Event(), threading.Event()
downstream_delta = threading.Event()


class Provider(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def reply(self, status, body, retry_after=None):
        raw = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        if retry_after is not None:
            self.send_header("Retry-After", str(retry_after))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        catalog_calls.append({"scenario": catalog_mode})
        if catalog_mode == "failure":
            return self.reply(503, {"error": {"type": "server_error"}})
        self.reply(200, {"data": [{"id": "catalog-only-model"}] if catalog_mode == "success" else [], "has_more": False})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        owner = next((name for name, key in keys.items() if self.headers.get("Authorization") == "Bearer " + key), None)
        if owner is None:
            return self.reply(401, {"error": {"type": "invalid_api_key"}})
        text = json.dumps(body)
        scenario = next((name for name in ["held-stream", "held-failure", "quota-block", "auth-block"] if name in text), "short")
        with calls_lock:
            calls.append({"account": owner, "scenario": scenario, "stream": bool(body.get("stream"))})
        if scenario == "quota-block":
            return self.reply(429, {"error": {"type": "rate_limit_exceeded"}}, retry_after=60)
        if scenario == "auth-block":
            return self.reply(401, {"error": {"type": "invalid_api_key"}})
        if scenario == "held-failure" and owner == "a":
            failure_started.set()
            if not release_failure.wait(20):
                raise RuntimeError("failure barrier expired")
            return self.reply(503, {"error": {"type": "server_error"}})
        item = {"id": "msg_local", "type": "message", "role": "assistant", "status": "completed", "content": [{"type": "output_text", "text": "local complete", "annotations": []}]}
        response = {"id": "resp_" + secrets.token_hex(8), "object": "response", "status": "completed", "model": body["model"], "output": [item], "usage": {"input_tokens": 10, "output_tokens": 2, "total_tokens": 12}}
        if not body.get("stream"):
            return self.reply(200, response)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True

        def frame(kind, **payload):
            self.wfile.write(("event: " + kind + "\ndata: " + json.dumps({"type": kind, **payload}) + "\n\n").encode())
            self.wfile.flush()

        frame("response.created", response={**response, "status": "in_progress", "output": []})
        frame("response.output_item.added", output_index=0, item={**item, "status": "in_progress", "content": []})
        frame("response.content_part.added", output_index=0, content_index=0, item_id=item["id"], part={"type": "output_text", "text": "", "annotations": []})
        frame("response.output_text.delta", output_index=0, content_index=0, item_id=item["id"], delta="local complete")
        if scenario == "held-stream":
            stream_started.set()
            if not release_stream.wait(20):
                raise RuntimeError("stream barrier expired")
        frame("response.output_text.done", output_index=0, content_index=0, item_id=item["id"], text="local complete")
        frame("response.content_part.done", output_index=0, content_index=0, item_id=item["id"], part=item["content"][0])
        frame("response.output_item.done", output_index=0, item=item)
        frame("response.completed", response=response)


provider = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(root / "tls.pem", root / "tls.key")
provider.socket = tls.wrap_socket(provider.socket, server_side=True)
threading.Thread(target=provider.serve_forever, daemon=True).start()


def port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


admin_port, data_port = port(), port()
base = f"http://127.0.0.1:{admin_port}"
client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
headers = {"X-Management-Key": (credentials / "management-key").read_text(), "Content-Type": "application/json"}
scope, revision = None, None
log = (root / "gateway.log").open("ab")
binary = pathlib.Path(os.environ.get("CARGO_TARGET_DIR", str(repo / "target"))) / "debug/gateway"
subprocess.run([str(binary), "admin-login", "init", "--state-dir", str(state), "--password-file", str(root / "initial-password")], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
gateway = subprocess.Popen([str(binary), "serve", "--state-dir", str(state), "--credential-dir", str(credentials), "--data-listen", f"127.0.0.1:{data_port}", "--management-listen", f"127.0.0.1:{admin_port}"], stdout=log, stderr=log, env={**os.environ, "SSL_CERT_FILE": str(ca)})


def api(method, path, body=None, extra=None, expected=200):
    global revision
    request_headers = {**headers, **({"X-Config-Version": scope} if scope else {}), **({"If-Match": revision} if revision and method != "GET" else {}), **(extra or {})}
    try:
        response = client.open(urllib.request.Request(base + path, method=method, headers=request_headers, data=None if body is None else json.dumps(body).encode()), timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        raw = response.read()
        value = json.loads(raw) if raw else None
        assert response.status == expected, (method, path, response.status, value if path.endswith("/validate") else value.get("error", {}).get("code") if isinstance(value, dict) else None)
        if method != "GET" and response.headers.get("ETag"):
            revision = response.headers["ETag"]
        return value


def publish():
    events = api("GET", "/admin/audit-events")
    event = max([row["id"] for row in events if row["action"] in ["config_published", "config_rolled_back"]] or [0])
    active = next((row["id"] for row in api("GET", "/admin/config-versions") if row["status"] == "active"), None)
    assert api("POST", f"/admin/config-versions/{scope}/validate")["valid"]
    return api("POST", f"/admin/config-versions/{scope}/publish", extra={"X-Expected-Active-Version": json.dumps(active), "X-Expected-Lifecycle-Event": str(event)})


def set_a_enabled(enabled):
    global scope, revision
    original = scope
    target = "local-edit-" + secrets.token_hex(4)
    draft = api("POST", f"/admin/config-versions/{original}/fork", {"id": target, "description": "Explicit local disable intent"}, expected=201)
    scope, revision = draft["id"], draft["revision"]
    current = api("GET", "/admin/credentials/account-a")
    api("PATCH", "/admin/credentials/account-a/status", {"status": "active" if enabled else "disabled", "credential_revision": current["revision"]})
    publish()


def data_request(text, stream=False, client_key=None, model="local-model"):
    connection = http.client.HTTPConnection("127.0.0.1", data_port, timeout=30)
    connection.request("POST", "/v1/responses", json.dumps({"model": model, "input": text, "stream": stream}), {"Authorization": "Bearer " + (client_key or issued_key), "Content-Type": "application/json"})
    response = connection.getresponse()
    chunks = []
    while True:
        line = response.readline() if stream else response.read()
        if not line:
            break
        chunks.append(line)
        if b"response.output_text.delta" in line:
            downstream_delta.set()
    connection.close()
    return response.status, b"".join(chunks)


def visible_models(client_key):
    connection = http.client.HTTPConnection("127.0.0.1", data_port, timeout=30)
    connection.request("GET", "/v1/models", headers={"Authorization": "Bearer " + client_key})
    response = connection.getresponse()
    value = json.loads(response.read())
    assert response.status == 200, ("public models", response.status)
    connection.close()
    return {row["id"] for row in value["data"]}


try:
    for _ in range(200):
        if gateway.poll() is not None:
            raise RuntimeError("owned gateway failed to start; inspect private local log")
        try:
            api("GET", "/admin/config-versions")
            break
        except (OSError, AssertionError):
            time.sleep(0.1)
    draft = api("POST", "/admin/config-versions", {"id": "local-a", "description": "Batch A controlled acceptance"}, expected=201)
    scope, revision = draft["id"], draft["revision"]
    api("POST", "/admin/egress-policies", {"id": "local-policy", "name": "Loopback only", "allowed_schemes": ["https"], "allowed_hosts": ["127.0.0.1"], "allowed_ports": [provider.server_port], "allowed_cidrs": ["127.0.0.1/32"], "redirect_mode": "deny", "max_redirects": 0}, expected=201)
    api("POST", "/admin/upstreams", {"id": "local-provider", "name": "Local synthetic", "kind": "openai-compatible", "enabled": True, "tags": [], "egress_policy_id": "local-policy"}, expected=201)
    api("POST", "/admin/upstreams/local-provider/endpoints", {"id": "local-endpoint", "adapter_id": "openai-compatible.responses", "api_format": "openai/responses", "base_url": f"https://127.0.0.1:{provider.server_port}/v1", "inference_path": "/responses", "models_path": None, "transport": "https", "enabled": True}, expected=201)
    for name, priority in [("a", 0), ("b", 10)]:
        account = api("POST", "/admin/upstreams/local-provider/account-import", {"id": "account-" + name, "channel": "openai-compatible", "secret": keys[name]}, expected=201)
        api("POST", "/admin/endpoints/local-endpoint/credential-bindings", {"credential_id": account["id"], "enabled": True, "priority": priority, "weight": 1, "concurrency": 7}, expected=201)
    api("POST", "/admin/upstreams", {"id": "catalog-provider", "name": "Catalog only synthetic", "kind": "openai-compatible", "enabled": True, "tags": [], "egress_policy_id": "local-policy"}, expected=201)
    api("POST", "/admin/upstreams/catalog-provider/account-import", {"id": "catalog-account", "channel": "openai-compatible", "secret": keys["b"]}, expected=201)
    api("POST", "/admin/upstreams/catalog-provider/endpoints", {"id": "catalog-endpoint", "adapter_id": "openai-compatible.responses", "api_format": "openai/responses", "base_url": f"https://127.0.0.1:{provider.server_port}/catalog-v1", "inference_path": "/responses", "models_path": "/models", "transport": "https", "enabled": True}, expected=201)
    api("POST", "/admin/endpoints/catalog-endpoint/credential-bindings", {"credential_id": "catalog-account", "enabled": True, "priority": 0, "weight": 1, "concurrency": 1}, expected=201)
    for model, model_id, route_id in [("local-model", "public-local", "local-route"), ("local-other", "public-other", "other-route")]:
        api("POST", "/admin/public-models", {"id": model_id, "model_name": model, "display_name": model, "status": "active", "capabilities": {}}, expected=201)
        api("POST", f"/admin/public-models/{model_id}/routes", {"id": route_id, "policy": "smooth_weighted_round_robin", "max_attempts": 2, "bootstrap_timeout_ms": 20000}, expected=201)
        api("POST", f"/admin/routes/{route_id}/candidates", {"id": "candidate-" + route_id, "endpoint_id": "local-endpoint", "upstream_model": model, "credential_scope": "all_active", "transform_mode": "canonical_bridge", "enabled": True, "priority": 0, "weight": 1, "capability_override": {"allow_unlisted_model": True}}, expected=201)
    api("POST", "/admin/access-groups", {"id": "local-group", "name": "Local client", "status": "active", "limits": {}}, expected=201)
    api("POST", "/admin/access-groups/local-group/routes", {"route_id": "other-route", "enabled": True}, expected=201)
    issued = api("POST", "/admin/client-keys", {"id": "local-key", "access_group_id": "local-group", "status": "active", "expires_at_ms": None}, expected=201)
    issued_key = issued["key"]
    sibling = api("POST", "/admin/client-keys", {"id": "sibling-key", "access_group_id": "local-group", "status": "active", "expires_at_ms": None}, expected=201)
    sibling_key = sibling["key"]
    rates = {name: 0 for name in ["reasoning_microunits_per_million", "cache_read_microunits_per_million", "cache_creation_microunits_per_million", "cached_microunits_per_million"]}
    api("POST", "/admin/billing/catalogs", {"catalog_version_id": "local-prices", "effective_at_ms": 0, "source": "operator", "entries": [{"provider_id": "local-provider", "channel_id": "local-endpoint", "model": model, "input_microunits_per_million": 1_000_000, "output_microunits_per_million": 2_000_000, **rates} for model in ["local-model", "local-other"]]}, expected=201)
    publish()
    assert not calls, "save, publish and reads must not invoke inference"
    directory_target = {"endpoint_id": "catalog-endpoint", "credential_id": "catalog-account"}
    success = api("POST", "/admin/catalog/refresh", directory_target)
    assert success["model_count"] == 1
    catalog_mode = "empty"
    empty = api("POST", "/admin/catalog/refresh", directory_target)
    assert empty["model_count"] == 0
    status = next(row for row in api("GET", "/admin/catalog/status") if row["endpoint_id"] == "catalog-endpoint")
    assert status["observation_state"] == "empty" and status["source"] == "upstream_catalog" and status["last_success_at_ms"]
    retained = api("GET", "/admin/catalog/models?endpoint_id=catalog-endpoint&credential_id=catalog-account&limit=100")
    assert retained["current_model_count"] == 0 and retained["total_count"] == 1
    assert retained["items"][0]["model"] == "catalog-only-model" and retained["items"][0]["present_in_last_success"] is False
    catalog_mode = "failure"
    api("POST", "/admin/catalog/refresh", directory_target, expected=502)
    failed_catalog = next(row for row in api("GET", "/admin/catalog/status") if row["endpoint_id"] == "catalog-endpoint")
    assert failed_catalog["observation_state"] == "failed" and failed_catalog["last_success_at_ms"] == status["last_success_at_ms"]
    assert api("GET", "/admin/catalog/models?endpoint_id=catalog-endpoint&credential_id=catalog-account&limit=100")["items"] == retained["items"]
    unsupported = next(row for row in api("GET", "/admin/catalog/status") if row["endpoint_id"] == "local-endpoint" and row["credential_id"] == "account-a")
    assert unsupported["observation_state"] == "unsupported"
    assert not calls, "directory GETs and refreshes never invoke inference"
    assert visible_models(issued_key) == {"local-other"}
    assert visible_models(sibling_key) == {"local-other"}
    denied_status, _ = data_request("closed-before-explicit-grant")
    assert denied_status >= 400 and not calls, "a new model stays closed before explicit authorization"
    draft = api("POST", f"/admin/config-versions/{scope}/fork", {"id": "explicit-permission", "description": "Exact selected key model grant"}, expected=201)
    scope, revision = draft["id"], draft["revision"]
    api("POST", "/admin/access-groups", {"id": "selected-key-group", "name": "Only selected Key", "status": "active", "limits": {}}, expected=201)
    for route_id in ["other-route", "local-route"]:
        api("POST", "/admin/access-groups/selected-key-group/routes", {"route_id": route_id, "enabled": True}, expected=201)
    api("PATCH", "/admin/client-keys/local-key", {"id": "local-key", "access_group_id": "selected-key-group", "status": "active", "expires_at_ms": None})
    publish()
    assert visible_models(issued_key) == {"local-model", "local-other"}
    assert visible_models(sibling_key) == {"local-other"}, "an explicit grant must not broaden the sibling Key"
    sibling_status, _ = data_request("sibling-remains-closed", client_key=sibling_key)
    assert sibling_status >= 400 and not calls
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        stream = executor.submit(data_request, "held-stream", True)
        assert stream_started.wait(10) and downstream_delta.wait(10), "must observe actual downstream semantic output before disabling"
        set_a_enabled(False)
        new_status, new_body = data_request("new-after-disable")
        release_stream.set()
        status, body = stream.result(20)
        if new_status != 200:
            debug={"error":json.loads(new_body).get("error",{}).get("code"),"effective":api("GET","/admin/models/effective?client_key_id=local-key"),"availability":api("GET","/admin/runtime/availability"),"pools":api("GET","/admin/operations/provider-account-pools?limit=100")}
            (out / "batch-a-http-debug.json").write_text(json.dumps(debug,indent=2)+"\n")
        assert new_status == 200, ("new request after disable", new_status)
        assert status == 200 and b"response.completed" in body and b"local complete" in body
        assert calls[0]["account"] == "a" and calls[1]["account"] == "b"
        other_status, _ = data_request("other-model-permission-retained", client_key=sibling_key, model="local-other")
        assert other_status == 200, "the sibling's prior model remains callable"
        before = len(calls)
        pin_body = {"provider_id": "local-provider", "channel_id": "local-endpoint", "route_id": "local-route", "credential_id": "account-b", "credential_revision": 0, "requested_model": "local-model", "protocol": "openai_responses", "mode": "json"}
        api("POST", "/admin/operations/channel-pin", pin_body, expected=403)
        api("POST", "/admin/operations/channel-pin", {**pin_body, "client_key_id": "sibling-key"}, expected=403)
        assert len(calls) == before
        pin = api("POST", "/admin/operations/channel-pin", {**pin_body, "client_key_id": "local-key"})
        (out / "batch-a-pin.json").write_text(json.dumps(pin,indent=2)+"\n")
        assert pin["outcome"] == "succeeded" and pin["attempt_count"] == 1 and len(calls) == before + 1
        assert pin["credential_id"] == "account-b" and pin["client_key_id"] == "local-key" and pin["runtime_build"] and pin["server_instance"]
        set_a_enabled(True)
        failed = executor.submit(data_request, "held-failure")
        assert failure_started.wait(10), "first attempt must have reached the controlled upstream"
        set_a_enabled(False)
        release_failure.set()
        failure_status, _ = failed.result(20)
        assert failure_status >= 400
        assert not any(row["account"] == "b" and row["scenario"] == "held-failure" for row in calls), "a retired execution must not allocate a fallback attempt"
    operator_scope = scope
    operator_target = {"provider_id": "local-provider", "channel_id": "local-endpoint", "account_id": "account-b"}
    action_path = "/admin/operations/provider-account-pools/actions"
    before = len(calls)
    assert api("POST", action_path, {**operator_target, "action": "cool_down", "cooldown_ms": 60000}, expected=202)["state"] == "cooling"
    released = api("POST", action_path, {**operator_target, "action": "request_recovery"}, expected=202)
    assert released["state"] == "released" and released["audit_recorded"]
    assert api("POST", action_path, {**operator_target, "account_id": "account-a", "action": "request_recovery"}, expected=202)["state"] == "rejected"
    api("POST", action_path, {**operator_target, "channel_id": "missing-channel", "action": "request_recovery"}, expected=409)
    assert data_request("permission-after-local-release", client_key=sibling_key)[0] >= 400
    assert len(calls) == before, "release and rejected operations cannot invoke inference or grant permissions"
    # The held 503 also starts the ordinary five-second Endpoint cooldown.
    # An account-local release must not bypass that independent state.
    time.sleep(5.1)
    before = len(calls)
    assert data_request("quota-block")[0] >= 400
    assert len(calls) == before + 1 and calls[-1]["scenario"] == "quota-block"
    before = len(calls)
    quota_receipt = api("POST", action_path, {**operator_target, "action": "request_recovery"}, expected=202)
    assert quota_receipt["state"] == "recovery_required", quota_receipt
    assert data_request("still-quota-blocked")[0] >= 400 and len(calls) == before
    set_a_enabled(True)
    api("POST", action_path, {**operator_target, "account_id": "account-a", "action": "cool_down", "cooldown_ms": 60000}, expected=202)
    api("POST", action_path, {**operator_target, "account_id": "account-a", "action": "request_recovery"}, expected=202)
    assert data_request("auth-block", model="local-other")[0] >= 400
    before = len(calls)
    auth_receipt = api("POST", action_path, {**operator_target, "account_id": "account-a", "action": "request_recovery"}, expected=202)
    assert auth_receipt["state"] == "rejected" and len(calls) == before
    audit = api("GET", "/admin/resource-audit-events?limit=100")["items"] + api("GET", "/admin/resource-audit-events?limit=100", extra={"X-Config-Version": operator_scope})["items"]
    assert any(row["action"] == "provider_account_recovery_requested" for row in audit)
    assert any(row["action"] == "provider_account_action_completed" for row in audit)
    assert any(row["action"] == "provider_account_action_observed" for row in audit)
    report = {"result": "PASS", "environment": "real-gateway-loopback-tls-synthetic", "model_default_closed": True, "catalog_success_empty_failure_retention_and_unsupported": True, "directory_reads_are_not_inference": True, "public_model_view_follows_explicit_grant": True, "sibling_key_and_prior_model_preserved": True, "stream_natural_completion": True, "new_request_uses_enabled_sibling": True, "retired_retry_sends_no_fallback": True, "key_required_before_send": True, "explicit_pin_sends_once": True, "local_release_sends_zero_inference": True, "release_preserves_disabled_auth_quota_and_permission_blocks": True, "release_audit_distinguishes_completion": True, "calls": calls, "catalog_calls": catalog_calls, "root": str(root)}
    (out / "batch-a-http.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: value for key, value in report.items() if key != "calls"}), flush=True)
finally:
    (out / "batch-a-http-calls.json").write_text(json.dumps({"root":str(root),"calls":calls},indent=2)+"\n")
    release_stream.set()
    release_failure.set()
    if gateway.poll() is None:
        gateway.terminate()
        gateway.wait(timeout=40)
    provider.shutdown()
