#!/usr/bin/env python3
"""macOS opt-in recovery matrix for immutable stock gateway serve binaries.

Only owned synthetic state is used. DNS/connect interposition and a synthetic CA
preserve the production Build encoder, pinned pool and TLS hostname validation.
The test-only Rust companion creates encrypted backups and injects replay faults;
it is never the acceptance server. Private diagnostics and keys stay outside Git.
"""
import argparse
import base64
import copy
import errno
import hashlib
import json
import os
import pathlib
import secrets
import shutil
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


HOST = "cli-chat-proxy.grok.com"
RAW_USAGE = ('{ "input_tokens":20,"output_tokens":7,"total_tokens":27,'
             '"input_tokens_details":{"cached_tokens":0,"unknown":null},'
             '"output_tokens_details":{"reasoning_tokens":3},'
             '"cost_in_usd_ticks":18446744073709551616123,'
             '"fraction":0.123456789012345678901234567890,'
             '"opaque":{"null":null,"zero":0,"array":[],"marker":"PRIVATE_USAGE_65"} }')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def dump(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def wait_for(predicate, label, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(.05)
    raise AssertionError("boundary timeout: " + label)


def unused_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def source_identity(root):
    def git(*args):
        return subprocess.check_output(["git", "-C", str(root), *args]).decode().strip()
    paths = subprocess.check_output(["git", "-C", str(root), "ls-files", "-z",
                                     "--cached", "--others", "--exclude-standard"]).split(b"\0")
    inputs = {}
    for raw in sorted(set(paths) - {b""}):
        name = os.fsdecode(raw)
        if (name.endswith((".rs", ".toml", ".lock")) or name.startswith("web/prism/src/")
                or name in {"web/prism/package-lock.json", "web/prism/package.json",
                            "scripts/build-management-spa.sh", "scripts/acceptance/cpar-device-loopback.c",
                            "scripts/acceptance/cpar65-process-recovery.py"}):
            path = root / name
            inputs[name] = {"sha256": sha(path.read_bytes()), "mode": path.stat().st_mode & 0o777}
    return {"root": str(root), "head": git("rev-parse", "HEAD"),
            "tree": git("rev-parse", "HEAD^{tree}"), "inputs": inputs,
            "status": git("status", "--porcelain=v2", "--untracked-files=all")}


class Peer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, certificate, key):
        super().__init__(("127.0.0.1", 0), Provider)
        self.receives, self.accepts, self.snis = [], 0, []
        self.case = "ordinary"
        self.materials = {}
        self.lock = threading.Lock()
        self.context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        self.context.load_cert_chain(certificate, key)
        self.context.set_servername_callback(lambda _s, name, _c: self.snis.append(name))
        threading.Thread(target=self.serve_forever, daemon=True).start()

    def get_request(self):
        raw, address = self.socket.accept()
        with self.lock:
            self.accepts += 1
        raw.settimeout(10)
        try:
            return self.context.wrap_socket(raw, server_side=True), address
        except Exception:
            raw.close()
            raise


class Provider(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        # A background catalog/identity request is an observable acceptance failure.
        with self.server.lock:
            self.server.receives.append({"method": "GET", "path": self.path})
        self.send_error(503)

    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        body = json.loads(raw)
        case = self.server.case
        native = self.headers.get("Host", "").split(":")[0] == HOST
        expected = self.server.materials["native" if native else "ordinary"]
        auth = self.headers.get("Authorization") == "Bearer " + expected
        with self.server.lock:
            ordinal = len(self.server.receives) + 1
            self.server.receives.append({"method": "POST", "path": self.path,
                "native": native, "case": case, "request_sha256": sha(raw),
                "auth_match": auth, "host": self.headers.get("Host"),
                "model": body.get("model"), "override": self.headers.get("x-grok-model-override"),
                "stream": body.get("stream", False),
                "native_auth_header_match": self.headers.get("x-xai-token-auth") == "xai-grok-cli",
                "header_names": sorted(name.lower() for name in self.headers)})
        assert auth and self.path == "/v1/responses"
        if native:
            assert body["model"] == self.headers.get("x-grok-model-override") == "grok-4.5"
        response = {"id": "native-65-" + str(ordinal), "object": "response", "status": "completed",
                    "model": body["model"], "output": [],
                    "usage": {"input_tokens": 20, "output_tokens": 7, "total_tokens": 27,
                              "input_tokens_details": {"cached_tokens": 0}}}
        if case == "summary":
            response["output"] = [
                {"id": "reason-a", "type": "reasoning", "status": "completed", "summary": [
                    {"type": "summary_text", "text": "PRIVATE_SUMMARY_65 计划"},
                    {"type": "summary_text", "text": "辅助 🐦"}]},
                {"id": "answer", "type": "message", "status": "completed", "role": "assistant",
                 "content": [{"type": "output_text", "text": "受控回答", "annotations": []}]},
                {"id": "reason-b", "type": "reasoning", "status": "completed", "content": [],
                 "summary": [{"type": "summary_text", "text": "PRIVATE_SUMMARY_65 核对"}]}]
        else:
            response["output"] = [{"id": "legacy-answer", "type": "message", "status": "completed",
                "role": "assistant", "content": [{"type": "output_text", "text": "legacy known reply", "annotations": []}]}]
        raw_response = json.dumps(response, ensure_ascii=False, separators=(",", ":"))
        if case == "usage":
            raw_response = raw_response[:raw_response.index('"usage":')] + '"usage":' + RAW_USAGE + '}'
        if body.get("stream"):
            created = {"id": response["id"], "model": response["model"], "status": "in_progress", "output": []}
            frames = [{"type": "response.created", "response": created}]
            for index, item in enumerate(response["output"]):
                added = {**item, "status": "in_progress"}
                added["summary" if item["type"] == "reasoning" else "content"] = []
                frames.append({"type": "response.output_item.added", "output_index": index, "item": added})
                for part_index, part in enumerate(item.get("summary", item.get("content", []))):
                    summary = item["type"] == "reasoning"
                    fields = {"item_id": item["id"], "output_index": index,
                              "summary_index" if summary else "content_index": part_index}
                    prefix = "response.reasoning_summary" if summary else "response.content"
                    frames.append({"type": prefix + "_part.added", **fields, "part": {**part, "text": ""}})
                    text_kind = "response.reasoning_summary_text" if summary else "response.output_text"
                    frames.append({"type": text_kind + ".delta", **fields, "delta": part["text"]})
                    frames.append({"type": text_kind + ".done", **fields, "text": part["text"]})
                    frames.append({"type": prefix + "_part.done", **fields, "part": part})
                frames.append({"type": "response.output_item.done", "output_index": index, "item": item})
            chunks = []
            for index, frame in enumerate(frames):
                frame["sequence_number"] = index
                chunks.append("event: " + frame["type"] + "\ndata: " + json.dumps(frame, ensure_ascii=False) + "\n\n")
            if case != "incomplete":
                chunks.append('event: response.completed\ndata: {"type":"response.completed","sequence_number":'
                              + str(len(frames)) + ',"response":' + raw_response + '}\n\n')
            payload = "".join(chunks).encode()
            content_type = "text/event-stream"
        else:
            payload, content_type = raw_response.encode(), "application/json"
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        # Deliberately split the TLS body across small writes for the SSE path.
        for offset in range(0, len(payload), 7 if body.get("stream") else len(payload)):
            self.wfile.write(payload[offset:offset + (7 if body.get("stream") else len(payload))])
        self.wfile.flush()


class Matrix:
    def __init__(self, args):
        self.args = args
        self.out = args.output.resolve()
        self.out.mkdir(mode=0o700, parents=True, exist_ok=False)
        self.root = pathlib.Path(tempfile.mkdtemp(prefix="cpar65-")).resolve()
        self.private = self.root / "private"
        self.private.mkdir(mode=0o700)
        self.state, self.credentials, self.keys = [self.root / name for name in ("state", "credentials", "keys")]
        for path in (self.state, self.credentials, self.keys, self.root / "home", self.root / "tmp"):
            path.mkdir(mode=0o700)
        for name in ("master-key", "backup-key", "client-key-pepper", "grok-build-cache-key"):
            (self.credentials / name).write_bytes(secrets.token_bytes(32))
        for name, prefix in (("management-key", "mgmt_"), ("management-csrf", "csrf_")):
            (self.credentials / name).write_text(prefix + secrets.token_hex(32))
        shutil.copy2(self.credentials / "master-key", self.keys / "1.key")
        (self.keys / "2.key").write_bytes(secrets.token_bytes(32))
        self.active = 1
        self.binary = {}
        for label, path in (("current", args.current_binary), ("schema35", args.historical_binary),
                            ("support", args.support_binary)):
            self.binary[label] = self.root / ("gateway-" + label)
            shutil.copy2(path, self.binary[label])
        self.management = (self.credentials / "management-key").read_text()
        self.scope = self.revision = None
        self.process = None
        self.records, self.unknowns = [], []
        self.client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        self.report = {"status": "RUNNING", "scope": "isolated_synthetic_only",
            "private_state_root": str(self.root), "real_provider": "NOT_RUN", "cost": "UNKNOWN",
            "cost_in_usd_ticks": "NOT_ADMITTED", "processes": [], "reads": [], "snapshots": {},
            "checks": [], "compatibility": [], "fault_fixtures": [],
            "sources": {"current": source_identity(args.current_source),
                        "schema35": source_identity(args.historical_source)},
            "binaries": {label: {"input": str(path), "sha256": sha(path.read_bytes())}
                         for label, path in (("current", args.current_binary),
                                             ("schema35", args.historical_binary), ("support", args.support_binary))}}
        self.tls()
        self.peer = Peer(self.root / "tls.pem", self.root / "tls.key")
        self.peer.materials = {"native": "synthetic-build-" + secrets.token_hex(24),
                               "ordinary": "synthetic-api-" + secrets.token_hex(24)}
        self.admin_port, self.data_port = unused_port(), unused_port()
        self.base = "http://127.0.0.1:" + str(self.admin_port)
        self.shim = self.root / "loopback.dylib"
        shim_source = args.current_source / "scripts/acceptance/cpar-device-loopback.c"
        subprocess.run(["/usr/bin/cc", "-dynamiclib", "-Wall", "-Wextra", "-Werror", "-o", str(self.shim),
                        str(shim_source)], check=True)
        self.env = {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": str(self.root / "home"),
                    "TMPDIR": str(self.root / "tmp"), "SSL_CERT_FILE": str(self.root / "ca.pem"),
                    "LANG": "en_US.UTF-8"}
        self.fence = self.root / "network.sb"
        self.fence.write_text('(version 1) (allow default) (deny network-outbound)\n'
            + '(allow network-outbound (remote ip "localhost:' + str(self.peer.server_port) + '"))\n'
            + '(deny file-read* (subpath "/Users") (subpath "/Volumes"))\n'
            + '(deny file-write*)\n(allow file-read* (subpath ' + json.dumps(str(self.root)) + '))\n'
            + '(allow file-write* (subpath ' + json.dumps(str(self.root)) + ') (literal "/dev/null"))\n')
        self.report["acceptance_conditions"] = {"mechanism": "existing macOS DYLD DNS/connect interposition",
            "shim_source_sha256": sha(shim_source.read_bytes()), "dylib_sha256": sha(self.shim.read_bytes()),
            "OS_profile": self.fence.read_text(), "OS_profile_sha256": sha(self.fence.read_bytes()),
            "CA_certificate_sha256": sha((self.root / "ca.pem").read_bytes()), "hostname": HOST,
            "configured_fixed_endpoint": "https://" + HOST + "/v1/responses",
            "fixture_egress_exception": "127.0.0.1/32", "receiver_port": self.peer.server_port,
            "environment": self.env, "background_provider_tasks": "disabled",
            "public_DNS_and_real_Provider_TLS_qualified": False}
        self.save()

    @property
    def db(self):
        return self.state / "control.sqlite3"

    def save(self):
        self.report["receiver"] = {"accepts": self.peer.accepts, "snis": self.peer.snis,
                                    "receives": list(self.peer.receives)} if hasattr(self, "peer") else {}
        dump(self.out / "matrix.json", self.report)

    def check(self, label, condition):
        self.report["checks"].append({"name": label, "pass": bool(condition)})
        self.save()
        assert condition, label

    def tls(self):
        (self.root / "tls.ext").write_text("subjectAltName=DNS:" + HOST + ",IP:127.0.0.1\n"
            "basicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\n")
        commands = [
            ["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-keyout",
             str(self.root / "ca.key"), "-out", str(self.root / "ca.pem"), "-subj", "/CN=CPAR65 synthetic CA",
             "-addext", "basicConstraints=critical,CA:TRUE"],
            ["openssl", "req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", str(self.root / "tls.key"),
             "-out", str(self.root / "tls.csr"), "-subj", "/CN=" + HOST],
            ["openssl", "x509", "-req", "-in", str(self.root / "tls.csr"), "-CA", str(self.root / "ca.pem"),
             "-CAkey", str(self.root / "ca.key"), "-CAcreateserial", "-out", str(self.root / "tls.pem"),
             "-days", "2", "-extfile", str(self.root / "tls.ext")]]
        for command in commands:
            subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def sandbox(self, command):
        return ["/usr/bin/sandbox-exec", "-f", str(self.fence), *map(str, command)]

    def isolation(self):
        listener = socket.socket()
        listener.bind(("127.0.0.1", 0))
        listener.listen(1)
        listener.settimeout(.2)
        probe = '''import errno,json,socket,sys
result=[]
for family,host,port in [(socket.AF_INET,'192.0.2.1',443),(socket.AF_INET6,'2001:db8::1',443),(socket.AF_INET,'127.0.0.1',int(sys.argv[1]))]:
 s=socket.socket(family);s.settimeout(1)
 try:s.connect((host,port));raise AssertionError('network permitted')
 except OSError as error:assert error.errno==errno.EPERM;result.append({'host':host,'errno':error.errno})
 finally:s.close()
with socket.create_connection(('127.0.0.1',int(sys.argv[2])),timeout=1):pass
print(json.dumps(result))
'''
        result = subprocess.run(self.sandbox(["/usr/bin/python3", "-c", probe, listener.getsockname()[1],
                                              self.peer.server_port]), env=self.env, cwd=self.root, capture_output=True, timeout=10)
        (self.out / "isolation-probe.log").write_bytes(result.stdout + result.stderr)
        self.check("OS outbound IPv4 IPv6 unrelated loopback denied", result.returncode == 0)
        self.report["network_denial"] = json.loads(result.stdout)
        try:
            listener.accept()
            raise AssertionError("unrelated receiver accepted")
        except socket.timeout:
            pass
        finally:
            listener.close()

    def start(self, binary, label):
        assert self.process is None
        command = [str(self.binary[binary]), "serve", "--state-dir", str(self.state),
            "--credential-dir", str(self.credentials), "--master-key-dir", str(self.keys),
            "--active-master-key-version", str(self.active), "--data-listen", "127.0.0.1:" + str(self.data_port),
            "--management-listen", "127.0.0.1:" + str(self.admin_port), "--background-provider-tasks", "disabled"]
        launched = self.sandbox(["/usr/bin/env", "DYLD_INSERT_LIBRARIES=" + str(self.shim),
                                "CPAR_DEVICE_LOOPBACK_PORT=" + str(self.peer.server_port), *command])
        log_path = self.out / (label + "-serve.log")
        with log_path.open("wb") as log:
            self.process = subprocess.Popen(launched, env=self.env, cwd=self.root, stdout=log, stderr=log)
        row = {"label": label, "binary": binary, "binary_sha256": sha(self.binary[binary].read_bytes()),
               "pid": self.process.pid, "argv": command[1:], "state": str(self.state),
               "active_key": self.active, "ready": False}
        self.report["processes"].append(row)
        def ready():
            if self.process.poll() is not None:
                raise AssertionError("serve exited before readiness: " + label)
            try:
                return self.api("GET", "/admin/config-versions") is not None
            except (OSError, AssertionError):
                return False
        wait_for(ready, "authenticated management readiness")
        status, _, _ = self.data("/v1/models", method="GET", key="invalid-synthetic-client")
        self.check(label + " data listener enforces authentication", status == 401)
        row.update(ready=True, schema=self.schema())
        self.save()

    def stop(self):
        process = self.process
        assert process is not None
        process.terminate()
        process.wait(timeout=45)
        self.report["processes"][-1].update(exit_code=process.returncode, reaped=True)
        self.process = None
        try:
            os.kill(process.pid, 0)
            dead = False
        except ProcessLookupError:
            dead = True
        self.check("old serve exited and PID absent", process.returncode == 0 and dead)
        with sqlite3.connect(self.db) as db:
            self.check("graceful writer drain", db.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()[0] == 0)

    def api(self, method, path, body=None, expected=200, extra=None):
        headers = {"X-Management-Key": self.management, "Content-Type": "application/json",
                   **({"X-Config-Version": self.scope} if self.scope else {}),
                   **({"If-Match": self.revision} if self.revision and method != "GET" else {}), **(extra or {})}
        raw = body if isinstance(body, bytes) else None if body is None else json.dumps(body).encode()
        try:
            response = self.client.open(urllib.request.Request(self.base + path, method=method, headers=headers,
                                                                data=raw), timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            value = json.loads(response.read() or "null")
            code = value.get("error", {}).get("code") if isinstance(value, dict) else None
            assert response.status == expected, (path, response.status, code)
            if method != "GET" and response.headers.get("ETag"):
                self.revision = response.headers["ETag"]
            return value

    def data(self, path, body=None, method="POST", key=None):
        headers = {"Authorization": "Bearer " + (key or self.owner), "Content-Type": "application/json",
                   "anthropic-version": "2023-06-01"}
        try:
            response = self.client.open(urllib.request.Request("http://127.0.0.1:" + str(self.data_port) + path,
                method=method, headers=headers, data=None if body is None else json.dumps(body).encode()), timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            return response.status, response.read(), dict(response.headers)

    def schema(self):
        with sqlite3.connect(self.db) as db:
            return db.execute("SELECT max(version) FROM schema_migrations").fetchone()[0]

    def publish(self):
        assert self.api("POST", "/admin/config-versions/" + self.scope + "/validate")["valid"]
        active = next((r["id"] for r in self.api("GET", "/admin/config-versions") if r["status"] == "active"), None)
        event = max([r["id"] for r in self.api("GET", "/admin/audit-events")
                     if r["action"] in ("config_published", "config_rolled_back")] or [0])
        self.api("POST", "/admin/config-versions/" + self.scope + "/publish", extra={
            "X-Expected-Active-Version": json.dumps(active), "X-Expected-Lifecycle-Event": str(event)})

    def seed(self):
        draft = self.api("POST", "/admin/config-versions", {"id": "cpar65-existing", "description": "Synthetic recovery"}, 201)
        self.scope, self.revision = draft["id"], draft["revision"]
        for kind in ("native", "ordinary"):
            native = kind == "native"
            self.api("POST", "/admin/egress-policies", {"id": kind + "-policy", "name": "Owned exact TLS peer " + kind,
                "allowed_schemes": ["https"], "allowed_hosts": [HOST if native else "127.0.0.1"],
                "allowed_ports": [443 if native else self.peer.server_port], "allowed_cidrs": ["127.0.0.1/32"],
                "redirect_mode": "deny", "max_redirects": 0}, 201)
            self.api("POST", "/admin/upstreams", {"id": kind, "name": "Synthetic " + kind,
                "kind": "grok-build-native" if native else "openai-compatible", "enabled": True,
                "tags": [], "egress_policy_id": kind + "-policy"}, 201)
            self.api("POST", "/admin/upstreams/" + kind + "/endpoints", {"id": kind + "-endpoint",
                "adapter_id": "grok.build.responses" if native else "openai-compatible.responses",
                "api_format": "openai/responses", "base_url": "https://" + HOST + "/v1" if native
                    else "https://127.0.0.1:" + str(self.peer.server_port) + "/v1",
                "inference_path": "/responses", "models_path": None, "transport": "https", "enabled": True}, 201)
            if not native:
                for label in ("active", "disabled"):
                    self.api("POST", "/admin/upstreams/ordinary/account-import", {"id": "ordinary-" + label,
                        "channel": "openai-compatible", "secret": self.peer.materials["ordinary"]
                            if label == "active" else "synthetic-disabled-" + secrets.token_hex(24)}, 201)
                    self.api("POST", "/admin/endpoints/ordinary-endpoint/credential-bindings", {
                        "credential_id": "ordinary-" + label, "enabled": True, "priority": 0,
                        "weight": 1, "concurrency": 1}, 201)
                account = self.api("GET", "/admin/credentials/ordinary-disabled")
                self.api("PATCH", "/admin/credentials/ordinary-disabled/status", {
                    "status": "disabled", "credential_revision": account["revision"]})
            model = "grok-4.5" if native else "legacy-model"
            self.api("POST", "/admin/public-models", {"id": kind + "-model", "model_name": model,
                "display_name": "Recovery " + kind, "status": "active", "capabilities": {}}, 201)
            self.api("POST", "/admin/public-models/" + kind + "-model/routes", {"id": kind + "-route",
                "policy": "smooth_weighted_round_robin", "max_attempts": 1, "bootstrap_timeout_ms": 10000}, 201)
            self.api("POST", "/admin/routes/" + kind + "-route/candidates", {"id": kind + "-candidate",
                "endpoint_id": kind + "-endpoint", "upstream_model": model, "credential_scope": "all_active",
                "transform_mode": "canonical_bridge", "enabled": True, "priority": 0, "weight": 1,
                "capability_override": {"allow_unlisted_model": True}}, 201)
        for label in ("active", "disabled"):
            claims = base64.urlsafe_b64encode(json.dumps({"sub": "subject65-" + label, "email": "recovery65@example.invalid",
                "name": "Synthetic recovery65", "client_id": "synthetic-client", "scope": "chat"}).encode()).decode().rstrip("=")
            self.api("POST", "/admin/native-accounts/import", {"id": "native-" + label, "channel": "grok.build",
                "secret": json.dumps({"access_token": self.peer.materials["native"]
                    if label == "active" else "synthetic-disabled-build-" + secrets.token_hex(24),
                    "id_token": "eyJhbGciOiJub25lIn0." + claims + ".synthetic",
                    "refresh_token": "synthetic-refresh-" + label,
                    "expires_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() + 86400))})}, 201)
            account_id = self.api("GET", "/admin/native-account-imports/native-" + label)["account_id"]
            account = next(r for r in self.api("GET", "/admin/native-accounts?limit=100")["items"] if r["id"] == account_id)
            self.api("POST", "/admin/native-accounts/" + account_id + "/identity", {"revision": account["revision"]})
            if label == "disabled":
                self.api("PATCH", "/admin/native-accounts/" + account_id, {"revision": account["revision"], "enabled": False})
                self.disabled_native = account_id
        for group in ("owner-group", "restricted-group"):
            self.api("POST", "/admin/access-groups", {"id": group, "name": group, "status": "active", "limits": {}}, 201)
        for kind in ("native", "ordinary"):
            self.api("POST", "/admin/access-groups/owner-group/routes", {"route_id": kind + "-route", "enabled": True}, 201)
        for key, group in (("owner", "owner-group"), ("sibling", "owner-group"), ("restricted", "restricted-group")):
            setattr(self, key, self.api("POST", "/admin/client-keys", {"id": key, "access_group_id": group,
                "status": "active", "expires_at_ms": None}, 201)["key"])
        self.publish()
        self.check("configuration publication makes no Provider request", not self.peer.receives)

    def capture(self, kind, stream=False):
        before = len(self.peer.receives)
        self.peer.case = kind
        if kind == "summary":
            path, body = "/v1/messages", {"model": "grok-4.5", "max_tokens": 100, "stream": stream,
                "messages": [{"role": "user", "content": "controlled recovery request"}]}
        else:
            path, body = "/v1/responses", {"model": "legacy-model" if kind == "ordinary" else "grok-4.5",
                "input": "controlled recovery request", "stream": stream}
        status, raw, headers = self.data(path, body)
        self.report.setdefault("public_calls", []).append({"kind": kind, "stream": stream,
            "status": status, "body_sha256": sha(raw), "receiver_before": before,
            "receiver_after": len(self.peer.receives)})
        self.check(kind + " single complete independent receive", len(self.peer.receives) == before + 1)
        if kind in ("usage", "incomplete"):
            self.check("strict unadmitted Usage still fails", (status == 502 if not stream else
                status == 200 and raw.count(b"event: response.failed\n") == 1) and b"response.completed" not in raw)
        else:
            self.check(kind + " valid public result", status == 200 and
                (not stream or raw.count(b"event: message_stop") == 1))
        self.check("private evidence absent from public bytes", b"PRIVATE_" not in raw)
        request = next(v for k, v in headers.items() if k.lower() == "x-request-id")
        attempt = self.wait_lineage(request, 0 if kind == "incomplete" else 1)
        item = {"kind": kind, "request": request, "attempt": attempt, "stream": stream,
                "owner": "owner", "public_status": status, "public_sha256": sha(raw), "receiver_ordinal": before + 1}
        with sqlite3.connect(self.db) as db:
            ledger = db.execute("SELECT source_event_id,input_tokens,output_tokens,reasoning_tokens,"
                "cached_tokens,cache_read_tokens,cache_creation_tokens,cost_microunits,cost_confidence "
                "FROM billing_ledger_entries WHERE request_id=?", [request]).fetchall()
            usages = db.execute("SELECT event_id FROM gateway_event_log WHERE request_id=? AND event_type='usage'", [request]).fetchall()
        ledger_names = ("source_event_id", "input_tokens", "output_tokens", "reasoning_tokens", "cached_tokens",
                        "cache_read_tokens", "cache_creation_tokens", "cost_microunits", "cost_confidence")
        self.check("known Usage and unpriced ledger preserve exact source", not ledger if kind == "incomplete" else
            len(ledger) == len(usages) == 1 and ledger[0][:3] == (usages[0][0], 20, 7) and ledger[0][7] is None)
        self.report.setdefault("metering_receipts", []).append({**item, "Request": 1, "Attempt": 1,
            "Usage": len(usages), "ledger": len(ledger), "source_ids": [r[0] for r in usages],
            "ledger_rows": [dict(zip(ledger_names, row)) for row in ledger], "cost": "UNKNOWN",
            "generated_through": "public_HTTP_production_serve"})
        if kind in ("ordinary", "incomplete"):
            self.unknowns.append(item)
        else:
            item["private"] = self.read(item, "capture", binary="schema35" if self.schema() == 35 else "current")
            self.records.append(item)
        return item

    def wait_lineage(self, request, expected_usage):
        def settled():
            with sqlite3.connect(self.db) as db:
                rows = db.execute("SELECT payload_json FROM gateway_event_log WHERE request_id=? AND event_type='attempt'", [request]).fetchall()
                finished = db.execute("SELECT count(*) FROM gateway_event_log WHERE request_id=? AND event_type='request_finished'", [request]).fetchone()[0]
                usage = db.execute("SELECT count(*) FROM gateway_event_log WHERE request_id=? AND event_type='usage'", [request]).fetchone()[0]
                ledger = db.execute("SELECT count(*) FROM billing_ledger_entries WHERE request_id=?", [request]).fetchone()[0]
                return rows if len(rows) == finished == 1 and usage == ledger == expected_usage else None
        return json.loads(wait_for(settled, "exact Request Attempt Usage ledger")[0][0])["attempt"]["attempt_id"]

    def read(self, item, stage, binary="current", keys=None, owner=None, db=None, expected="retained"):
        output = self.private / (stage + "-" + secrets.token_hex(8) + ".json")
        command = [self.binary[binary], "messages-summary-evidence" if item["kind"] == "summary" else "usage-evidence",
            "--db", db or self.db, "--master-key-dir", keys or self.keys, "--active-master-key-version", self.active,
            "--client-key-id", owner or item["owner"], "--request-id", item["request"], "--attempt-id", item["attempt"],
            "--output", output]
        result = subprocess.run(self.sandbox(command), env=self.env, cwd=self.root, capture_output=True, timeout=15)
        row = {"stage": stage, "reader": binary, "reader_sha256": sha(self.binary[binary].read_bytes()),
               "request": item["request"], "attempt": item["attempt"], "kind": item["kind"],
               "owner": owner or item["owner"], "exit_code": result.returncode, "expected": expected,
               "active_key": self.active, "key_files": {p.name: sha(p.read_bytes())
                    for p in (keys or self.keys).glob("*.key")}}
        self.report["reads"].append(row)
        self.check("reader never emits private plaintext", b"PRIVATE_" not in result.stdout + result.stderr)
        if expected == "error":
            self.check(stage + " reader rejects without plaintext file", result.returncode != 0 and not output.exists())
            return None
        self.check(stage + " private authorized output", result.returncode == 0 and output.stat().st_mode & 0o777 == 0o600)
        value = json.loads(output.read_text())
        self.check(stage + " " + expected, value["status"] == expected)
        row["output_sha256"] = sha(output.read_bytes())
        if expected == "retained":
            record = value["record"]
            if item["kind"] == "usage":
                self.check("native Usage preserves exact received object bytes", record["evidence"]["raw_json"] == RAW_USAGE)
                self.check("native Usage completeness", record["completeness"] == "complete_usage_object")
                self.check("native Usage exact identity source and missing counters", record["client_key_id"] == item["owner"]
                    and record["usage"]["request_id"] == item["request"] and record["usage"]["attempt_id"] == item["attempt"]
                    and record["usage"]["response_id"] == "native-65-" + str(item["receiver_ordinal"])
                    and record["evidence"]["source"] == ("build_sse_terminal" if item["stream"] else "build_json")
                    and record["usage"]["usage"]["cached_tokens"] == 0
                    and record["usage"]["usage"]["cache_read_tokens"] is None
                    and record["usage"]["usage"]["cache_creation_tokens"] is None)
            else:
                items = record["evidence"]["items"]
                # Independent received-content oracle, before trusting the first read as a baseline.
                # Compare every part/text/status and body presence, including the order within an item.
                expected_items = [
                    {"id": "reason-a", "type": "reasoning", "status": "completed", "summary": [
                        {"type": "summary_text", "text": "PRIVATE_SUMMARY_65 计划"},
                        {"type": "summary_text", "text": "辅助 🐦"}]},
                    {"id": "reason-b", "type": "reasoning", "status": "completed", "content": [],
                     "summary": [{"type": "summary_text", "text": "PRIVATE_SUMMARY_65 核对"}]}]
                self.check("summary full received content identity hierarchy and order", [r["output_index"] for r in items] == [0, 2]
                    and [r["item"] for r in items] == expected_items
                    and record["evidence"]["completeness"] == "completed_response"
                    and record["evidence"]["response_id"] == "native-65-" + str(item["receiver_ordinal"]))
                self.check("summary source exact owner and execution", record["client_key_id"] == item["owner"]
                    and record["attempt"]["request_id"] == item["request"]
                    and record["attempt"]["attempt_id"] == item["attempt"]
                    and record["execution"]["channel"] == "grok.build"
                    and record["evidence"]["source"] == ("build_sse" if item["stream"] else "build_json"))
            if "private" in item:
                self.check(stage + " authenticated content unchanged", value == json.loads(pathlib.Path(item["private"]).read_text()))
        self.save()
        return str(output)

    def snapshot(self, label):
        tables = ("config_versions", "egress_policies", "upstreams", "upstream_endpoints", "upstream_credentials",
                  "endpoint_credential_bindings", "public_models", "model_routes", "route_candidates",
                  "access_groups", "access_group_routes", "client_keys", "grok_accounts",
                  "native_account_identity_observations", "gateway_event_log", "gateway_attempt_execution",
                  "gateway_request_recording", "native_usage_evidence", "messages_summary_evidence", "billing_ledger_entries")
        result = {}
        with sqlite3.connect(self.db) as db:
            self.check(label + " quick and foreign key checks", db.execute("PRAGMA quick_check").fetchone() == ("ok",)
                       and not db.execute("PRAGMA foreign_key_check").fetchall())
            for table in tables:
                if not db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?", [table]).fetchone():
                    continue
                columns = [row[1] for row in db.execute("PRAGMA table_info(" + table + ")")]
                rows = db.execute("SELECT * FROM " + table).fetchall()
                encoded = sorted(json.dumps([{"blob_sha256": sha(v)} if isinstance(v, bytes) else v for v in row],
                                             ensure_ascii=False, separators=(",", ":")) for row in rows)
                result[table] = {"columns": columns, "rows": len(rows), "rows_sha256": sha(json.dumps(encoded).encode())}
        self.report["snapshots"][label] = result
        self.save()
        return result

    def verify_public(self, label):
        before = len(self.peer.receives)
        self.check(label + " ordinary disabled intent", self.api("GET", "/admin/credentials/ordinary-disabled")["status"] == "disabled")
        native = next(r for r in self.api("GET", "/admin/native-accounts?limit=100")["items"] if r["id"] == self.disabled_native)
        self.check(label + " native identity and disabled intent", native["enabled"] is False
                   and native["identity"]["email"] == "recovery65@example.invalid")
        status, body, _ = self.data("/v1/models", method="GET")
        self.check(label + " models retain exact access scope", status == 200 and
                   {r["id"] for r in json.loads(body)["data"]} == {"grok-4.5", "legacy-model"})
        status, body, _ = self.data("/v1/models", method="GET", key=self.restricted)
        self.check(label + " restricted model scope", status == 200 and json.loads(body)["data"] == [])
        status, _, _ = self.data("/v1/responses", {"model": "grok-4.5", "input": "denied"}, key=self.restricted)
        self.check(label + " permissions deny before sending", status == 404 and len(self.peer.receives) == before)

    def verify_reads(self, label):
        before = len(self.peer.receives)
        for item in self.records:
            self.read(item, label)
            self.read(item, label + "-foreign", owner="sibling", expected="unknown")
        for item in self.unknowns:
            self.read(item, label + "-legacy", expected="unknown")
            summary = {**item, "kind": "summary"}
            self.read(summary, label + "-legacy-summary", expected="unknown")
        self.check(label + " diagnostics have no Provider sends", len(self.peer.receives) == before)

    def operator(self, action, item=None, expected=False, output=None):
        path = self.private / ("job-" + secrets.token_hex(8) + ".json")
        job = {"scope": "isolated_synthetic_cpar65", "action": action, "database": str(self.db),
            "keys": str(self.keys), "active": self.active, "input": str(self.credentials / "backup-key")
                if item is None else item["private"], "output": str(output or self.private / "unused"),
            "expected_persisted": expected}
        dump(path, job)
        result = subprocess.run(self.sandbox(["/usr/bin/env", "CPAR65_OPERATOR_JOB=" + str(path),
            self.binary["support"], "--ignored", "--exact", "cpar65_operator_job", "--nocapture"]),
            env=self.env, cwd=self.root, capture_output=True, timeout=20)
        (self.out / ("operator-" + path.stem + ".log")).write_bytes(result.stdout + result.stderr)
        self.check(action + " value-free operator receipt", result.returncode == 0)

    def run(self):
        self.isolation()
        self.start("schema35", "B-old-nonempty")
        self.check("historical binary actually creates schema35", self.schema() == 35)
        self.seed()
        self.tls_failures()
        self.capture("ordinary")
        self.capture("usage")
        self.capture("usage", True)
        self.capture("incomplete", True)
        self.verify_public("old")
        self.verify_reads("schema35-current-reader")
        self.read({**self.records[0], "kind": "summary"}, "schema35-summary-unknown", expected="unknown")
        self.check("new read-only readers never migrate schema35", self.schema() == 35)
        self.stop()
        old = self.snapshot("B-before-upgrade")
        self.start("current", "B-upgraded")
        self.check("production bootstrap upgrades to36", self.schema() == 36)
        upgraded = self.snapshot("B-after-upgrade")
        self.check("upgrade preserves every old business row and column", all(upgraded[k] == v for k, v in old.items()))
        self.check("upgrade summary namespace is empty", upgraded["messages_summary_evidence"]["rows"] == 0)
        self.verify_public("upgraded")
        self.verify_reads("upgraded")
        self.read(self.records[0], "old-reader-new-schema", binary="schema35", expected="error")
        self.capture("summary")
        self.capture("summary", True)
        self.stop()
        before = self.snapshot("A-before-restart")
        self.start("current", "A-restarted")
        after = self.snapshot("A-after-restart")
        self.check("normal process restart preserves full semantic state", before == after)
        self.verify_public("restarted")
        self.verify_reads("restarted")
        self.stop()
        self.active = 2
        before = self.snapshot("C-before-rotation")
        self.start("current", "C-key2")
        self.check("key switch leaves existing ciphertext and state unchanged", self.snapshot("C-key2-start") == before)
        self.verify_reads("rotated-old")
        for kind in ("usage", "summary"):
            self.capture(kind)
        with sqlite3.connect(self.db) as db:
            for item in self.records:
                table = "messages_summary_evidence" if item["kind"] == "summary" else "native_usage_evidence"
                version = db.execute("SELECT key_version FROM " + table + " WHERE attempt_id=?", [item["attempt"]]).fetchone()[0]
                item["key_version"] = version
            self.check("new evidence uses key2 and original evidence keeps key1", [r["key_version"] for r in self.records] == [1, 1, 1, 1, 2, 2])
        self.verify_public("rotated")
        self.verify_reads("rotated")
        self.stop()
        self.faults()
        self.write_failures()
        self.expiry()
        self.restore()
        self.report["status"] = "PASS_BOUNDED_PROCESS_RECOVERY"
        self.report["compatibility"].extend([
            {"schema": 35, "binary": "schema35", "native_usage": "PASS", "summary": "UNSUPPORTED_COMMAND"},
            {"schema": 36, "reader": "schema35", "result": "REJECTED"},
            {"schema": 36, "reader": "current", "native_usage": "PASS", "summary": "PASS"},
            {"schema": 34, "reader": "current", "result": "CONTRACT_ONLY_NOT_RUN"}])
        self.check("frozen source inputs unchanged", source_identity(self.args.current_source) == self.report["sources"]["current"]
                   and source_identity(self.args.historical_source) == self.report["sources"]["schema35"])
        self.check("all receiver requests authorized and expected", all(r.get("auth_match") and r.get("method") == "POST" for r in self.peer.receives))
        self.check("stock Build TLS SNI and native headers retained", all(name in (None, HOST) for name in self.peer.snis)
                   and self.peer.snis.count(HOST) == 2 + sum(r.get("native", False) for r in self.peer.receives)
                   and all(r["native_auth_header_match"] for r in self.peer.receives if r.get("native")))
        self.privacy()
        self.save()

    def tls_failures(self):
        """Negative handshakes use the same serve/pool, with no HTTP upstream receive."""
        self.stop()
        wrong_ca = self.root / "wrong-ca.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2",
            "-keyout", str(self.root / "wrong-ca.key"), "-out", str(wrong_ca), "-subj", "/CN=Untrusted CPAR65",
            "-addext", "basicConstraints=critical,CA:TRUE"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        (self.root / "wrong-host.ext").write_text("subjectAltName=DNS:other.example.invalid\n"
            "basicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\n")
        subprocess.run(["openssl", "x509", "-req", "-in", str(self.root / "tls.csr"),
            "-CA", str(self.root / "ca.pem"), "-CAkey", str(self.root / "ca.key"), "-CAcreateserial",
            "-out", str(self.root / "wrong-host.pem"), "-days", "2", "-extfile", str(self.root / "wrong-host.ext")],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for label in ("untrusted-ca", "wrong-hostname"):
            self.env["SSL_CERT_FILE"] = str(wrong_ca if label == "untrusted-ca" else self.root / "ca.pem")
            self.peer.context.load_cert_chain(self.root / ("wrong-host.pem" if label == "wrong-hostname" else "tls.pem"),
                                              self.root / "tls.key")
            self.start("schema35", "TLS-" + label)
            before, accepts = len(self.peer.receives), self.peer.accepts
            status, raw, headers = self.data("/v1/responses", {"model": "grok-4.5", "input": "TLS rejection"})
            self.report.setdefault("tls_observations", []).append({"condition": label, "status": status,
                "error_code": json.loads(raw).get("error", {}).get("code"),
                "accepts_delta": self.peer.accepts - accepts, "receives_delta": len(self.peer.receives) - before})
            self.check(label + " production TLS rejects before upstream HTTP", status == 503
                       and json.loads(raw)["error"]["code"] == "EgressUnavailable"
                       and len(self.peer.receives) == before and self.peer.accepts == accepts + 1)
            request = next(v for k, v in headers.items() if k.lower() == "x-request-id")
            attempt = self.wait_lineage(request, 0)
            with sqlite3.connect(self.db) as db:
                self.check(label + " unreceived Usage remains absent", db.execute("SELECT count(*) FROM gateway_event_log WHERE event_type='usage' AND request_id=?", [request]).fetchone()[0] == 0
                           and db.execute("SELECT count(*) FROM billing_ledger_entries WHERE request_id=?", [request]).fetchone()[0] == 0)
            self.unknowns.append({"kind": "usage", "request": request, "attempt": attempt, "owner": "owner", "stream": False})
            self.report.setdefault("tls_negative_receipts", []).append({"condition": label, "request": request,
                "attempt": attempt, "accepts_delta": self.peer.accepts - accepts, "receives_delta": 0,
                "status": status, "public_sha256": sha(raw), "Usage": 0, "ledger": 0, "cost": "UNKNOWN"})
            self.stop()
        self.env["SSL_CERT_FILE"] = str(self.root / "ca.pem")
        self.peer.context.load_cert_chain(self.root / "tls.pem", self.root / "tls.key")
        self.start("schema35", "B-old-after-TLS-negative")

    def write_failures(self):
        """Transient SQL faults resolve the original pending transaction after client failure."""
        self.start("current", "C-live-writer-faults")
        for kind in ("usage", "summary"):
            table = "native_usage_evidence" if kind == "usage" else "messages_summary_evidence"
            with sqlite3.connect(self.db) as db:
                db.execute("CREATE TRIGGER cpar65_write_failure BEFORE INSERT ON " + table
                    + " BEGIN SELECT RAISE(ABORT,'controlled private write failure'); END")
            self.report["fault_fixtures"].append({"kind": "direct_SQL_transient_insert_trigger", "table": table})
            self.peer.case = kind
            before = len(self.peer.receives)
            path, body = ("/v1/responses", {"model": "grok-4.5", "input": "controlled writer fault"}) if kind == "usage" else (
                "/v1/messages", {"model": "grok-4.5", "max_tokens": 100,
                                 "messages": [{"role": "user", "content": "controlled writer fault"}]})
            status, raw, headers = self.data(path, body)
            self.check(kind + " live private write failure never succeeds or resends", status == 503
                       and len(self.peer.receives) == before + 1 and b"PRIVATE_" not in raw)
            request = next(v for k, v in headers.items() if k.lower() == "x-request-id")
            # Remove only this synthetic fault, after the caller has already received failure.
            with sqlite3.connect(self.db) as db:
                db.execute("DROP TRIGGER cpar65_write_failure")
            attempt = self.wait_lineage(request, 1)
            with sqlite3.connect(self.db) as db:
                terminal = json.loads(db.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='request_finished' AND request_id=?", [request]).fetchone()[0])["request_finished"]
                self.check(kind + " failed delivery remains failed after late commit", terminal["outcome"] == "failed"
                           and terminal["error_code"] == "RecordingUnavailable")
            item = {"kind": kind, "request": request, "attempt": attempt, "owner": "owner", "stream": False,
                    "public_status": status, "key_version": self.active, "receiver_ordinal": before + 1}
            item["private"] = self.read(item, "late-original-commit")
            self.records.append(item)
            self.report.setdefault("write_failure_receipts", []).append({k: v for k, v in item.items() if k != "private"})
            self.check(kind + " late confirmation has one Provider receive", len(self.peer.receives) == before + 1)
        self.stop()

    def privacy(self):
        forbidden = [b"PRIVATE_USAGE_65", b"PRIVATE_SUMMARY_65", *[v.encode() for v in self.peer.materials.values()],
                     *[(self.credentials / name).read_bytes() for name in ("management-key", "management-csrf",
                         "master-key", "backup-key", "client-key-pepper", "grok-build-cache-key")],
                     (self.keys / "1.key").read_bytes(), (self.keys / "2.key").read_bytes()]
        with sqlite3.connect(self.db) as db:
            for table in ("gateway_event_log", "gateway_event_quarantine"):
                for row in db.execute("SELECT payload_json FROM " + table):
                    self.check(table + " has no private plaintext or credential", not any(v in row[0].encode() for v in forbidden))
        for path in self.out.glob("*.log"):
            self.check("ordinary log privacy " + path.name, not any(v in path.read_bytes() for v in forbidden))

    def faults(self):
        before = self.snapshot("C-before-faults")
        count = len(self.peer.receives)
        for label in ("missing", "wrong"):
            keys = self.root / (label + "-keys")
            keys.mkdir(mode=0o700)
            shutil.copy2(self.keys / "2.key", keys / "2.key")
            if label == "wrong":
                (keys / "1.key").write_bytes(secrets.token_bytes(32))
            for item in self.records[:4]:
                self.read(item, label + "-key", keys=keys, expected="error")
            if label == "missing":
                for item in self.records[4:]:
                    self.read(item, "key2-only-new-evidence", keys=keys)
        for kind in ("usage", "summary"):
            items = [r for r in self.records if r["kind"] == kind]
            copy_db = self.root / ("swapped-" + kind + ".sqlite3")
            with sqlite3.connect(self.db) as source, sqlite3.connect(copy_db) as target:
                source.backup(target)
            table = "messages_summary_evidence" if kind == "summary" else "native_usage_evidence"
            with sqlite3.connect(copy_db) as db:
                a, b = [db.execute("SELECT ciphertext FROM " + table + " WHERE attempt_id=?", [r["attempt"]]).fetchone()[0] for r in items[:2]]
                for item, cipher in zip(items[:2], (b, a)):
                    db.execute("UPDATE " + table + " SET ciphertext=? WHERE attempt_id=?", [cipher, item["attempt"]])
            self.report["fault_fixtures"].append({"kind": "direct_SQL_ciphertext_swap", "evidence": kind, "database": str(copy_db)})
            for item in items[:2]:
                self.read(item, "swapped", db=copy_db, expected="error")
            self.replay_pair(items[0], retained=True)
        self.check("read and replay failures preserve all original rows and metering", self.snapshot("C-after-faults") == before
                   and len(self.peer.receives) == count)

    def replay_pair(self, item, retained=False):
        self.operator("replay_" + item["kind"], item, expected=retained)
        changed = copy.deepcopy(json.loads(pathlib.Path(item["private"]).read_text()))
        if item["kind"] == "usage":
            changed["record"]["evidence"]["raw_json"] = RAW_USAGE.replace("PRIVATE_USAGE_65", "CHANGED_USAGE_65")
        else:
            changed["record"]["evidence"]["items"][0]["item"]["summary"][0]["text"] += " changed"
        path = self.private / ("changed-" + secrets.token_hex(8) + ".json")
        dump(path, changed)
        self.operator("replay_" + item["kind"], {**item, "private": str(path)})

    def expiry(self):
        self.expired = [next(r for r in self.records if r["kind"] == kind) for kind in ("usage", "summary")]
        with sqlite3.connect(self.db) as db:
            for item in self.expired:
                table = "messages_summary_evidence" if item["kind"] == "summary" else "native_usage_evidence"
                db.execute("UPDATE " + table + " SET created_at_ms=0,expires_at_ms=1 WHERE attempt_id=?", [item["attempt"]])
        self.report["fault_fixtures"].append({"kind": "direct_SQL_retention_clock_fixture", "times": [0, 1],
                                             "attempts": [r["attempt"] for r in self.expired]})
        self.records = [r for r in self.records if r not in self.expired]
        self.start("current", "expiry-GC")
        # A new, explicitly requested public operation triggers the ordinary writer GC.
        self.capture("ordinary")
        self.stop()
        self.receipt_check("expiry-GC")
        for item in self.expired:
            self.read(item, "expired", expected="unknown")
            self.replay_pair(item)
        self.start("current", "expiry-restarted")
        self.receipt_check("expiry-restarted")
        self.verify_reads("expiry-restarted")
        self.stop()
        for item in self.expired:
            self.replay_pair(item)
        self.receipt_check("expiry-restarted-after-replay")

    def receipt_check(self, label):
        usage, summary = self.expired
        with sqlite3.connect(self.db) as db:
            receipt = db.execute("SELECT created_at_ms,expires_at_ms,key_version,ciphertext IS NULL FROM messages_summary_evidence WHERE attempt_id=?", [summary["attempt"]]).fetchone()
            self.check(label + " non-payload summary receipt survives", receipt == (0, 1, 1, 1))
            self.check(label + " native Usage GC retains source event barrier", db.execute("SELECT count(*) FROM native_usage_evidence WHERE attempt_id=?", [usage["attempt"]]).fetchone()[0] == 0
                       and db.execute("SELECT count(*) FROM gateway_event_log WHERE event_type='usage' AND request_id=?", [usage["request"]]).fetchone()[0] == 1)

    def restore(self):
        self.start("current", "D-backup-source")
        self.verify_public("backup-source")
        before = self.snapshot("D-before-backup")
        artifact = self.root / "coherent.encrypted"
        self.operator("encrypted_backup", output=artifact)
        self.report["backup"] = {"sha256": sha(artifact.read_bytes()), "bytes": artifact.stat().st_size,
            "backup_key_sha256": sha((self.credentials / "backup-key").read_bytes()),
            "master_keys": {str(v): sha((self.keys / (str(v) + ".key")).read_bytes()) for v in (1, 2)}}
        self.stop()
        source_state = self.state
        self.state = self.root / "restore-host"
        self.state.mkdir(mode=0o700)
        self.start("current", "D-restore-host")
        for path in ("/admin/restores/preflight", "/admin/restores"):
            value = self.api("POST", path, artifact.read_bytes(), 200 if path.endswith("preflight") else 202,
                             {"Content-Type": "application/octet-stream"})
            if path == "/admin/restores":
                self.check("management restore only creates independent target", value["state"] == "complete"
                           and (self.state / "restore-target.sqlite3").exists())
        self.stop()
        restore_target = self.state / "restore-target.sqlite3"
        self.state = self.root / "restored-serving-state"
        self.state.mkdir(mode=0o700)
        # A fresh control.sqlite3 adopts the validated target; no existing state is overwritten.
        shutil.copy2(restore_target, self.db)
        self.credentials = self.root / "restored-credentials"
        shutil.copytree(self.root / "credentials", self.credentials)
        self.keys = self.root / "restored-keys"
        shutil.copytree(self.root / "keys", self.keys)
        self.start("current", "D-restored-serving")
        self.check("restored real serve uses all original semantic state", self.snapshot("D-restored-serving") == before)
        self.verify_public("restored")
        self.verify_reads("restored")
        self.receipt_check("restored")
        self.stop()
        count = len(self.peer.receives)
        for item in self.expired:
            self.read(item, "restored-expired", expected="unknown")
            self.replay_pair(item)
        self.receipt_check("restored-after-replay")
        self.check("restore replay never invokes Provider", len(self.peer.receives) == count)
        self.report["restoration"] = {"source_state": str(source_state), "management_target": str(restore_target),
                                       "adopted_control_database": str(self.db), "replacement": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("current-binary", "historical-binary", "support-binary", "current-source", "historical-source", "output"):
        parser.add_argument("--" + name, required=True, type=pathlib.Path)
    args = parser.parse_args()
    os.umask(0o077)
    matrix = Matrix(args)
    try:
        matrix.run()
    except BaseException as error:
        matrix.report.update(status="FAIL", failure_type=type(error).__name__, failure=str(error))
        raise
    finally:
        if matrix.process is not None:
            matrix.process.terminate()
            matrix.process.wait(timeout=45)
            matrix.report["processes"][-1].update(cleanup_exit_code=matrix.process.returncode)
        matrix.save()
        matrix.peer.shutdown()
    print(json.dumps({"status": matrix.report["status"], "receipt": str(matrix.out / "matrix.json"),
                      "private_state_root": str(matrix.root)}))


if __name__ == "__main__":
    main()
