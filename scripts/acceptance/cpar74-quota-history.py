#!/usr/bin/env python3
"""Stock serve -> management HTTP -> isolated quota peer -> SQLite -> new process.

macOS envelope only. Synthetic credentials, child-only TLS/DNS shim, OS outbound
fence, dynamic ports, no inference, and background provider tasks disabled.
"""
import argparse
import base64
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
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def stamp(seconds):
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(seconds))


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_GET(self):
        self.server.paths.append(self.path)
        if self.server.delay:
            self.server.entered.set()
            assert self.server.release.wait(10), "controlled response release"
        code = self.server.code
        if self.path == "/v1/billing?format=credits":
            body = {"config": {"credit_usage_percent": self.server.percent,
                    "current_period": {"end": self.server.reset},
                    "product_usage": [{"product": "chat", "usage_percent": 15}, {}]}}
        else:
            code, body = 404, {}
        raw = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)


class Run:
    def __init__(self, args):
        self.root = args.output.resolve()
        self.root.mkdir(parents=True, mode=0o700, exist_ok=False)
        self.repo = args.source.resolve()
        self.process = None
        self.report = {"status": "RUNNING", "checks": [], "processes": [],
                       "real_provider": "NOT_RUN", "inference": "NOT_RUN",
                       "background_provider_tasks": "disabled", "cost": "UNKNOWN"}
        self.private = self.root / "private"
        self.private.mkdir(mode=0o700)
        for name in ("state", "credentials", "home", "tmp"):
            (self.private / name).mkdir(mode=0o700)
        credentials = self.private / "credentials"
        for name in ("master-key", "backup-key", "client-key-pepper", "grok-build-cache-key"):
            (credentials / name).write_bytes(secrets.token_bytes(32))
        self.key = "mgmt_" + secrets.token_hex(24)
        (credentials / "management-key").write_text(self.key)
        (credentials / "management-csrf").write_text("csrf_" + secrets.token_hex(24))
        self.binary = self.private / "gateway"
        shutil.copy2(args.binary, self.binary)
        self.report["binary_sha256"] = hashlib.sha256(self.binary.read_bytes()).hexdigest()
        self.report["source_head"] = subprocess.check_output(["git", "-C", str(self.repo), "rev-parse", "HEAD"]).decode().strip()
        self.report["source_diff_sha256"] = hashlib.sha256(subprocess.check_output(["git", "-C", str(self.repo), "diff", "HEAD"])).hexdigest()
        self.tls()
        self.peer = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.peer.paths, self.peer.code, self.peer.percent = [], 200, 100
        self.peer.reset = stamp(time.time() - 1)
        self.peer.delay = False
        self.peer.entered, self.peer.release = threading.Event(), threading.Event()
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(self.private / "tls.pem", self.private / "tls.key")
        self.peer.socket = context.wrap_socket(self.peer.socket, server_side=True)
        threading.Thread(target=self.peer.serve_forever, daemon=True).start()
        self.shim = self.private / "quota.dylib"
        subprocess.run(["/usr/bin/cc", "-dynamiclib", "-Wall", "-Wextra", "-Werror", "-o", str(self.shim),
                        str(self.repo / "scripts/acceptance/cpar74-quota-loopback.c")], check=True)
        self.fence = self.private / "network.sb"
        self.fence.write_text('(version 1) (allow default) (deny network-outbound)\n'
            + '(allow network-outbound (remote ip "localhost:' + str(self.peer.server_port) + '"))\n'
            + '(deny file-read* (subpath "/Users") (subpath "/Volumes"))\n'
            + '(allow file-read* (subpath ' + json.dumps(str(self.root)) + '))\n'
            + '(allow file-read-metadata ' + ' '.join('(literal ' + json.dumps(str(p)) + ')' for p in self.root.parents) + ')\n'
            + '(deny file-write*) (allow file-write* (subpath ' + json.dumps(str(self.root)) + ') (literal "/dev/null"))\n')
        self.admin, self.data = port(), port()
        while self.admin == self.data:
            self.data = port()
        self.client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        self.env = {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": str(self.private / "home"),
                    "TMPDIR": str(self.private / "tmp"), "SSL_CERT_FILE": str(self.private / "ca.pem")}
        self.scope, self.revision = None, None

    def tls(self):
        p = self.private
        (p / "tls.ext").write_text("subjectAltName=DNS:cli-chat-proxy.grok.com\nbasicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\n")
        for cmd in (["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-keyout", str(p / "ca.key"), "-out", str(p / "ca.pem"), "-subj", "/CN=CPAR74 synthetic CA", "-addext", "basicConstraints=critical,CA:TRUE"],
                    ["req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", str(p / "tls.key"), "-out", str(p / "tls.csr"), "-subj", "/CN=cli-chat-proxy.grok.com"],
                    ["x509", "-req", "-in", str(p / "tls.csr"), "-CA", str(p / "ca.pem"), "-CAkey", str(p / "ca.key"), "-CAcreateserial", "-out", str(p / "tls.pem"), "-days", "2", "-extfile", str(p / "tls.ext")]):
            subprocess.run(["openssl", *cmd], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def save(self):
        self.report["receiver_paths"] = self.peer.paths
        (self.root / "receipt.json").write_text(json.dumps(self.report, indent=2) + "\n")

    def check(self, name, condition):
        self.report["checks"].append({"name": name, "pass": bool(condition)})
        self.save()
        assert condition, name

    def api(self, method, path, body=None, expected=200, auth=True):
        headers = {"Content-Type": "application/json"}
        if auth:
            headers["X-Management-Key"] = self.key
        if self.scope:
            headers["X-Config-Version"] = self.scope
        if self.revision and method != "GET":
            headers["If-Match"] = self.revision
        request = urllib.request.Request(f"http://127.0.0.1:{self.admin}" + path, method=method,
                    headers=headers, data=None if body is None else json.dumps(body).encode())
        try:
            response = self.client.open(request, timeout=25)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read()
            value = json.loads(raw) if raw else None
            assert response.status == expected, (path, response.status, value)
            if method != "GET" and response.headers.get("ETag"):
                self.revision = response.headers["ETag"]
            return value

    def start(self):
        command = ["/usr/bin/sandbox-exec", "-f", str(self.fence), "/usr/bin/env",
                   "DYLD_INSERT_LIBRARIES=" + str(self.shim), "CPAR_QUOTA_LOOPBACK_PORT=" + str(self.peer.server_port),
                   str(self.binary), "serve", "--state-dir", str(self.private / "state"),
                   "--credential-dir", str(self.private / "credentials"), "--data-listen", f"127.0.0.1:{self.data}",
                   "--management-listen", f"127.0.0.1:{self.admin}", "--background-provider-tasks", "disabled"]
        with (self.root / f"serve-{len(self.report['processes'])}.log").open("wb") as log:
            self.process = subprocess.Popen(command, env=self.env, cwd=self.private, stdout=log, stderr=log)
        self.report["processes"].append({"pid": self.process.pid})
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            assert self.process.poll() is None, "serve exited"
            try:
                self.api("GET", "/admin/config-versions")
                return
            except OSError:
                time.sleep(.05)
        raise AssertionError("readiness")

    def stop(self):
        self.process.terminate()
        self.process.wait(timeout=45)
        self.report["processes"][-1]["exit_code"] = self.process.returncode
        self.check("actual process exited cleanly", self.process.returncode == 0)
        self.process = None

    def history(self, suffix=""):
        return self.api("GET", f"/admin/quota-history?from_ms={self.begin - 10000}&to_ms={self.begin + 600000}&limit=200" + suffix)["entries"]

    def run(self):
        # Prove the OS fence denies external sockets independently of DNS interposition.
        blocked = subprocess.run(["/usr/bin/sandbox-exec", "-f", str(self.fence), "/usr/bin/nc", "-z", "-G", "1", "1.1.1.1", "443"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.check("OS denies external connection", blocked.returncode != 0)
        self.start()
        self.begin = int(time.time()*1000)
        self.api("POST", "/admin/config-versions", {"id": "quota-74", "description": "synthetic quota history"}, 201)
        self.scope = "quota-74"
        ids = []
        for label in ("a", "b"):
            claims = base64.urlsafe_b64encode(json.dumps({"sub": "quota74-" + label, "email": "quota@example.invalid", "name": "Synthetic quota", "client_id": "synthetic-client", "scope": "chat"}).encode()).decode().rstrip("=")
            credential = {"access_token": "synthetic-access-" + label, "refresh_token": "synthetic-refresh-" + label,
                          "id_token": "eyJhbGciOiJub25lIn0." + claims + ".synthetic", "expires_at": stamp(time.time()+86400)}
            self.api("POST", "/admin/native-accounts/import", {"id": "quota-"+label, "channel": "grok.build", "secret": json.dumps(credential)}, 201)
            ids.append(self.api("GET", "/admin/native-account-imports/quota-"+label)["account_id"])
        a, b = ids
        self.api("GET", f"/admin/native-accounts/{a}/usage?revision=0")
        time.sleep(.01)
        self.peer.percent, self.peer.reset = 10, stamp(time.time()+3600)
        self.api("GET", f"/admin/native-accounts/{a}/usage?revision=0")
        self.api("GET", f"/admin/native-accounts/{b}/usage?revision=0")
        self.peer.code = 403
        self.api("GET", f"/admin/native-accounts/{a}/usage?revision=0")
        self.peer.code = 200
        history = self.history()
        self.check("cross-account samples", {r["account_id"] for r in history} == set(ids))
        credit = self.history("&account_id=" + a + "&window=build.credits")
        self.check("exhaustion and expiration independent", credit[0]["status"] == "exhausted" and credit[0]["stale"])
        self.check("reset is reported evidence with sampling gap", credit[1]["reported_reset_changed"] and credit[1]["gap_before"] is not None)
        self.check("partial projection retained", credit[1]["completeness"] == "partial")
        self.check("failure is unknown quantity", credit[-1]["status"] == "failed" and credit[-1]["used_percent"] is None)
        self.check("history excludes synthetic credentials", all(x not in json.dumps(history) for x in ("synthetic-access", "synthetic-refresh", "quota@example.invalid")))
        self.api("GET", f"/admin/quota-history?from_ms={self.begin}&to_ms={self.begin+10000}&limit=1", expected=404, auth=False)
        # Late reads cannot override a disable or recreate a deleted account.
        for account, mutation in ((a, "disable"), (b, "delete")):
            count = len(self.history())
            self.peer.delay = True
            self.peer.entered.clear()
            self.peer.release.clear()
            result = []
            def pending():
                try:
                    self.api("GET", f"/admin/native-accounts/{account}/usage?revision=0", expected=409)
                    result.append(True)
                except Exception:
                    result.append(False)
            thread = threading.Thread(target=pending)
            thread.start()
            assert self.peer.entered.wait(10)
            if mutation == "disable":
                self.api("PATCH", f"/admin/native-accounts/{account}", {"revision": 0, "enabled": False})
            else:
                self.api("DELETE", f"/admin/native-accounts/{account}?revision=0")
            self.peer.release.set()
            thread.join(25)
            self.peer.delay = False
            self.check("late response rejected after " + mutation, result == [True] and len(self.history()) == count)
        rows = self.api("GET", "/admin/native-accounts")["items"]
        self.check("operator disable and deletion preserved", len(rows) == 1 and not rows[0]["enabled"])
        before = self.history()
        self.stop()
        receives = len(self.peer.paths)
        self.start()
        after = self.history()
        self.check("new process reads same durable evidence", before == after)
        self.check("restart and history query send no upstream request", len(self.peer.paths) == receives)
        with sqlite3.connect(self.private / "state/control.sqlite3") as db:
            self.check("schema 39 loaded", db.execute("SELECT max(version) FROM schema_migrations").fetchone()[0] == 39)
            self.check("no Usage or billing fabrication", db.execute("SELECT count(*) FROM gateway_event_log").fetchone()[0] == 0 and db.execute("SELECT count(*) FROM billing_ledger_entries").fetchone()[0] == 0)
        self.check("receiver saw quota reads only", set(self.peer.paths) == {"/v1/billing?format=credits"})
        self.stop()
        self.report["status"] = "PASS"
        self.save()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=pathlib.Path, default=pathlib.Path.cwd())
    parser.add_argument("--binary", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    run = Run(parser.parse_args())
    try:
        run.run()
    except Exception:
        run.report["status"] = "FAIL"
        run.save()
        raise
    finally:
        run.peer.release.set()
        if run.process and run.process.poll() is None:
            run.process.terminate()
            run.process.wait(timeout=45)
        run.peer.shutdown()
    print("quota history process acceptance: PASS")


if __name__ == "__main__":
    main()
