#!/usr/bin/env python3
"""Bounded #66 qualification of the delivered stock binary, inside the Linux envelope."""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import sqlite3
import subprocess
import threading
import urllib.error
import urllib.request

spec = importlib.util.spec_from_file_location("cpar65", Path(__file__).with_name("cpar65-process-recovery.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Receiver(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        self.server.receives.append({"method": "GET", "path": self.path})
        self.send_response(200)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_POST(self):
        self.server.receives.append({"method": "POST", "path": self.path})
        self.send_error(503)


class Matrix(base.Matrix):
    def __init__(self, args):
        self.args, self.root, self.out = args, args.output, args.output
        self.report = {"scope": "stock_linux_arm64_reader_serve_only", "status": "RUNNING",
            "real_provider": "NOT_RUN", "signing": "NOT_RUN", "cost_in_usd_ticks": "NOT_ADMITTED",
            "checks": [], "processes": [], "reads": [], "network": [], "snapshots": {}}
        self.artifact = json.loads((args.artifact / "artifact-manifest.json").read_text())
        self.binary = args.artifact / "gateway-aarch64-unknown-linux-gnu"
        self.report["binary_sha256"] = sha(self.binary)
        self.report["manifest_sha256"] = sha(args.artifact / "artifact-manifest.json")
        self.report["revision"] = self.artifact["revision"]
        self.process = None
        self.state, self.credentials, self.keys, self.private = [self.root / name
            for name in ("state", "credentials", "keys", "private")]
        for path in (self.state, self.credentials, self.keys, self.private, self.root / "home", self.root / "tmp"):
            path.mkdir(mode=0o700)
        for name in ("master-key", "backup-key", "client-key-pepper", "grok-build-cache-key"):
            (self.credentials / name).write_bytes(secrets.token_bytes(32))
        for name, prefix in (("management-key", "mgmt_"), ("management-csrf", "csrf_")):
            (self.credentials / name).write_text(prefix + secrets.token_hex(32))
        self.management = (self.credentials / "management-key").read_text()
        (self.keys / "1.key").write_bytes((self.credentials / "master-key").read_bytes())
        self.active, self.scope, self.revision = 1, None, None
        self.admin_port, self.data_port = 19181, 19180
        self.base = "http://127.0.0.1:19181"
        self.client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        self.env = {"PATH": "/usr/local/bin:/usr/bin:/bin", "HOME": str(self.root / "home"),
                    "TMPDIR": str(self.root / "tmp"), "LANG": "C.UTF-8"}
        self.peer = ThreadingHTTPServer(("127.0.0.1", 19443), Receiver)
        self.peer.receives = []
        self.peer_thread = threading.Thread(target=self.peer.serve_forever)
        self.peer_thread.start()

    def save(self):
        self.report["receiver"] = list(self.peer.receives) if hasattr(self, "peer") else []
        base.dump(self.out / "qualification.json", self.report)

    def http(self, port, path, key=None, method="GET"):
        headers = {} if key is None else {"X-Management-Key": key}
        try:
            result = self.client.open(urllib.request.Request(f"http://127.0.0.1:{port}" + path,
                headers=headers, method=method), timeout=3)
        except urllib.error.HTTPError as error:
            result = error
        with result:
            return result.status, result.read()

    def isolation(self):
        status = Path("/proc/self/status").read_text()
        self.check("unprivileged capability-free process tree", os.getuid() == 65532
            and "CapEff:\t0000000000000000" in status and "CapBnd:\t0000000000000000" in status
            and "NoNewPrivs:\t1" in status)
        self.check("no host credential or engine mount", not Path("/var/run/docker.sock").exists()
            and not Path("/Users").exists() and not Path("/Volumes").exists())
        self.check("kernel firewall rules retained", "--dport 19443" in (self.root / "ipv4.rules").read_text()
            and "-A OUTPUT -j REJECT" in (self.root / "ipv6.rules").read_text())
        unrelated = socket.socket()
        unrelated.bind(("127.0.0.1", 19999))
        unrelated.listen(2)
        unrelated.settimeout(.2)
        probe = '''import json,socket
rows=[]
for family,host,port in [(socket.AF_INET,"192.0.2.1",443),(socket.AF_INET6,"2001:db8::1",443),(socket.AF_INET,"127.0.0.1",19999)]:
 s=socket.socket(family);s.settimeout(1)
 try:s.connect((host,port));raise AssertionError("uncontrolled network allowed")
 except OSError as e:rows.append({"host":host,"errno":e.errno})
 finally:s.close()
print(json.dumps(rows))
'''
        try:
            result = subprocess.run(["python3", "-c", probe], env=self.env, capture_output=True, timeout=10)
            self.check("child IPv4 IPv6 and live unrelated loopback denied", result.returncode == 0)
            self.report["network"] = json.loads(result.stdout)
            try:
                unrelated.accept()
                raise AssertionError("unrelated receiver accepted")
            except socket.timeout:
                pass
        finally:
            unrelated.close()
        self.check("owned local receiver reachable", self.http(19443, "/cpar66-envelope-probe")[0] == 200)
        for path in (Path("/etc/cpar66-forbidden-write"), Path("/artifact/cpar66-forbidden-write")):
            try:
                path.write_text("denied")
                raise AssertionError("write outside owned state allowed")
            except OSError:
                pass
        self.check("read-only root and artifact writes refused", True)

    def start(self, label):
        assert self.process is None
        command = [str(self.binary), "serve", "--state-dir", str(self.state),
            "--credential-dir", str(self.credentials), "--master-key-dir", str(self.keys),
            "--active-master-key-version", "1", "--data-listen", "127.0.0.1:19180",
            "--management-listen", "127.0.0.1:19181", "--background-provider-tasks", "disabled"]
        with (self.out / (label + ".log")).open("wb") as log:
            self.process = subprocess.Popen(command, env=self.env, cwd=self.root, stdout=log, stderr=log)
        process = self.process
        self.report["processes"].append({"label": label, "pid": process.pid,
            "binary_sha256": sha(self.binary), "argv": command[1:]})
        def ready():
            if process.poll() is not None:
                raise AssertionError("stock serve exited before readiness: " + label)
            try:
                return self.http(19180, "/healthz") == (200, b'{"status":"ok"}')
            except OSError:
                return False
        base.wait_for(ready, "stock readiness")
        exe = Path(f"/proc/{process.pid}/exe")
        self.check(label + " actual process executable equals delivered binary", sha(exe) == self.report["binary_sha256"])
        process_status = Path(f"/proc/{process.pid}/status").read_text()
        self.check(label + " gateway inherits OS isolation", "Uid:\t65532\t65532\t65532\t65532" in process_status
            and "CapEff:\t0000000000000000" in process_status and "NoNewPrivs:\t1" in process_status)
        self.report["processes"][-1].update(executable=os.readlink(exe), start_time=Path(f"/proc/{process.pid}/stat").read_text().split()[21])
        self.check(label + " schema38", self.schema() == 38)
        self.check(label + " separate health management auth", self.http(19180, "/", method="HEAD")[0] == 200
            and self.http(19180, "/admin/config-versions")[0] == 404
            and self.http(19181, "/admin/config-versions")[0] == 404
            and self.http(19181, "/admin/config-versions", "mgmt_wrong_synthetic")[0] == 404
            and isinstance(self.api("GET", "/admin/config-versions"), list))
        self.check(label + " data auth", self.http(19180, "/v1/models")[0] == 401)

    def stop(self):
        super().stop()
        for port in (19180, 19181):
            with socket.socket() as sock:
                self.check("listener released " + str(port), sock.connect_ex(("127.0.0.1", port)) != 0)

    def read(self, item, label, owner=None, request=None, attempt=None, wrong_key=False):
        output = self.private / (label + "-" + secrets.token_hex(6) + ".json")
        keys = self.keys
        if wrong_key:
            keys = self.root / "wrong-keys"
            keys.mkdir(mode=0o700, exist_ok=True)
            (keys / "1.key").write_bytes(bytes([9]) * 32)
        command = [str(self.binary), item["command"], "--db", str(self.db), "--master-key-dir", str(keys),
            "--active-master-key-version", "1", "--client-key-id", owner or item["owner"],
            "--request-id", request or item["request"], "--attempt-id", attempt or item["attempt"], "--output", str(output)]
        result = subprocess.run(command, env=self.env, cwd=self.root, capture_output=True, timeout=15)
        self.check(label + " no private stdout or error", not result.stdout and b"PRIVATE_" not in result.stderr)
        self.report["reads"].append({"label": label, "command": item["command"], "request": item["request"],
            "binary_sha256": sha(self.binary), "exit": result.returncode})
        if wrong_key:
            self.check(label + " key refusal without output", result.returncode != 0 and not output.exists())
            return
        self.check(label + " exclusive private output", result.returncode == 0 and output.stat().st_mode & 0o777 == 0o600)
        value = json.loads(output.read_text())
        if owner or request or attempt:
            self.check(label + " exact unknown", value == {"status": "unknown"})
            return
        self.check(label + " retained", value["status"] == "retained")
        record = value["record"]
        if item["command"] == "request-metadata-evidence":
            diagnostic, record = record, record["record"]
            self.check(label + " v1/v2 exact source owner raw and actual Attempt", record["version"] == item["version"]
                and record["request"]["client_key_id"] == item["owner"]
                and record["request"]["request_id"] == item["request"]
                and record["evidence"]["raw_json"] == item["raw"]
                and record["evidence"].get("source") == ("anthropic_messages_metadata" if item["version"] == 2 else None)
                and [a["attempt_id"] for a in diagnostic["attempts"]] == [item["attempt"]])
        elif item["command"] == "usage-evidence":
            self.check(label + " exact native Usage bytes and identity", record["evidence"]["raw_json"] == item["raw"]
                and record["client_key_id"] == item["owner"] and record["usage"]["request_id"] == item["request"]
                and record["usage"]["attempt_id"] == item["attempt"])
        else:
            self.check(label + " complete ordered summary and execution identity", record["evidence"] == item["evidence"]
                and record["client_key_id"] == item["owner"] and record["attempt"]["request_id"] == item["request"]
                and record["attempt"]["attempt_id"] == item["attempt"] and record["execution"]["channel"] == "grok.build")
        again = subprocess.run(command, env=self.env, cwd=self.root, capture_output=True, timeout=15)
        self.check(label + " overwrite refusal", again.returncode != 0 and json.loads(output.read_text()) == value)

    def evidence_snapshot(self):
        with sqlite3.connect(self.db) as db:
            result = {}
            for table in ("request_metadata_evidence", "native_usage_evidence", "messages_summary_evidence"):
                rows = db.execute("SELECT * FROM " + table).fetchall()
                result[table] = hashlib.sha256(repr(rows).encode()).hexdigest()
            return result

    def run(self):
        record = next(r for r in self.artifact["files"] if r["name"] == self.binary.name)
        self.check("manifest binds actual binary", record["sha256"] == sha(self.binary)
            and record["bytes"] == self.binary.stat().st_size)
        raw = self.binary.read_bytes()
        for name, value in (("revision", self.artifact["revision"]), ("rust-version", "1.97.1"),
                            ("target", "aarch64-unknown-linux-gnu")):
            self.check("embedded " + name, raw.count(("gateway-release-" + name + "=" + value + "\n").encode()) == 1)
        self.isolation()
        self.start("empty-state")
        self.check("new state empty control plane", self.api("GET", "/admin/config-versions") == [])
        self.stop()
        for kind in ("metadata", "usage", "summary"):
            fixture = self.args.fixtures / kind
            for filename in ("control.sqlite3", "control.sqlite3-wal", "control.sqlite3-shm"):
                (self.state / filename).unlink(missing_ok=True)
            shutil.copyfile(fixture / "control.sqlite3", self.db)
            (self.keys / "1.key").write_bytes((fixture / "1.key").read_bytes())
            items = json.loads((fixture / "expected.json").read_text())
            self.scope = self.revision = None
            self.start(kind + "-adopt")
            marker = "cpar66-" + kind
            self.api("POST", "/admin/config-versions", {"id": marker, "description": "Owned synthetic adoption marker"}, 201)
            for item in items:
                self.read(item, kind + "-authorized")
                self.read(item, kind + "-foreign-owner", owner="sibling")
                self.read(item, kind + "-wrong-request", request="other-request")
                self.read(item, kind + "-wrong-attempt", attempt="other-attempt")
                self.read(item, kind + "-wrong-key", wrong_key=True)
            before = self.evidence_snapshot()
            self.stop()
            if kind == "metadata":
                self.start("metadata-new-process-restart")
                self.check("new gateway process adopts persisted control state", marker in
                    [row["id"] for row in self.api("GET", "/admin/config-versions")])
                self.check("encrypted evidence unchanged across actual process restart", before == self.evidence_snapshot())
                for item in items:
                    self.read(item, "restart-authorized")
                self.stop()
        self.check("no inference or background Provider receives", self.peer.receives ==
            [{"method": "GET", "path": "/cpar66-envelope-probe"}])
        self.check("private identifiers absent from ordinary logs", all(b"PRIVATE_" not in p.read_bytes()
            for p in self.out.glob("*.log")))
        self.check("delivered binary unchanged after all runs", sha(self.binary) == self.report["binary_sha256"])
        remaining = []
        for path in Path("/proc").iterdir():
            if path.name.isdigit():
                try:
                    if os.readlink(path / "exe") == str(self.binary):
                        remaining.append(path.name)
                except (FileNotFoundError, PermissionError):
                    pass
        self.check("no remaining gateway executable process", not remaining)
        self.report["status"] = "PASS_BOUNDED"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("artifact", "fixtures", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
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
            matrix.stop()
        matrix.peer.shutdown()
        matrix.peer.server_close()
        matrix.peer_thread.join(timeout=5)
        with socket.socket() as sock:
            matrix.check("controlled receiver listener and thread released", not matrix.peer_thread.is_alive()
                and sock.connect_ex(("127.0.0.1", 19443)) != 0)
        matrix.check("all stock processes exited and were reaped", all(r.get("reaped") for r in matrix.report["processes"]))
        matrix.save()
    print(json.dumps({"status": matrix.report["status"], "checks": len(matrix.report["checks"])}))


if __name__ == "__main__":
    main()
