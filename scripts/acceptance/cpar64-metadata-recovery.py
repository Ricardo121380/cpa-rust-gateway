#!/usr/bin/env python3
"""Bounded M4 stock-process recovery, reusing #65's isolated TLS/OS/restore facilities."""
import argparse
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import secrets
import shutil
import sqlite3
import subprocess
import urllib.error
import urllib.request

spec = importlib.util.spec_from_file_location("cpar65_recovery", Path(__file__).with_name("cpar65-process-recovery.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)


class Provider(base.Provider):
    def do_POST(self):
        original = self.rfile
        raw = original.read(int(self.headers.get("Content-Length", "0")))
        body = json.loads(raw)
        assert not {"metadata", "client_metadata", "context_management"}.intersection(body)
        assert b"M4_PRIVATE" not in raw
        assert all("M4_PRIVATE" not in value for value in self.headers.values())
        self.rfile = io.BytesIO(raw)
        try:
            super().do_POST()
        finally:
            self.rfile = original
        with self.server.lock:
            self.server.receives[-1]["local_metadata_absent_in_body_and_headers"] = True


base.Provider = Provider


class Matrix(base.Matrix):
    def __init__(self, args):
        super().__init__(args)
        self.binary["schema37"] = self.binary.pop("schema35")
        self.report["binaries"]["schema37"] = self.report["binaries"].pop("schema35")
        self.report["sources"]["schema37"] = self.report["sources"].pop("schema35")
        self.report["scope"] = "cpar64_M4_only_isolated_synthetic_stock_serve"
        self.report["original_Claude"] = "SEPARATE_OBSERVATION"
        self.report["driver_sha256"] = base.sha(Path(__file__).read_bytes())
        self.expired = []
        self.save()

    def snapshot(self, label):
        result = super().snapshot(label)
        with sqlite3.connect(self.db) as db:
            rows = db.execute("SELECT request_id,client_key_id,created_at_ms,expires_at_ms,payload_version,key_version,ciphertext "
                              "FROM request_metadata_evidence ORDER BY request_id").fetchall()
        encoded = [[base.sha(v) if isinstance(v, bytes) else v for v in row] for row in rows]
        result["request_metadata_evidence"] = {"rows": len(rows), "rows_sha256": base.sha(json.dumps(encoded).encode())}
        self.save()
        return result

    def capture_metadata(self, version=2, rejected=False, stream=False):
        marker = "M4_PRIVATE_" + secrets.token_hex(8)
        if version == 1:
            field = "client_metadata"
            raw = '{ "session_id" : ' + json.dumps(marker) + ' }'
            body = {"model": "grok-4.5", "input": "bounded local metadata recovery"}
        else:
            field = "metadata"
            user = '{ "device_id":"' + marker + '", "account_uuid":"", "session_id":"opaque" }'
            raw = '{ "user_id" : ' + json.dumps(user + "\u9e1f", ensure_ascii=True) + ' }'
            body = {"model": "grok-4.5", "max_tokens": 100, "stream": stream,
                    "messages": [{"role": "user", "content": "bounded local metadata recovery"}],
                    "context_management": {"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]}}
            if rejected:
                body.update(thinking={"type": "adaptive", "display": "updates"}, output_config={"effort": "high"})
                body["messages"][0]["content"] = [{"type": "text", "text": "bounded rejected controls",
                                                   "cache_control": {"type": "ephemeral"}}]
        encoded = json.dumps(body)[:-1] + ', "' + field + '":' + raw + '}'
        self.peer.case = "summary" if version == 2 else "ordinary"
        before, accepts = len(self.peer.receives), self.peer.accepts
        request = urllib.request.Request("http://127.0.0.1:" + str(self.data_port)
            + ("/v1/responses" if version == 1 else "/v1/messages"), method="POST",
            headers={"Authorization": "Bearer " + self.owner, "Content-Type": "application/json"}, data=encoded.encode())
        try:
            response = self.client.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            status, public, request_id = response.status, response.read(), response.headers["x-request-id"]
        self.check("M4 admission before send, or one exact successful send", status == (400 if rejected else 200)
                   and len(self.peer.receives) - before == int(not rejected)
                   and self.peer.accepts - accepts == int(not rejected) and marker.encode() not in public)
        if version == 2 and not rejected:
            if stream:
                frames = [json.loads(line[6:]) for line in public.decode().splitlines() if line.startswith("data: ")]
                self.check("M3 stock SSE no edits and unique terminal", sum(f["type"] == "message_stop" for f in frames) == 1
                    and next(f for f in frames if f["type"] == "message_delta")["context_management"] == {"applied_edits": []})
            else:
                self.check("M3 stock JSON no applied edits", json.loads(public)["context_management"] == {"applied_edits": []})
        def settled():
            with sqlite3.connect(self.db) as db:
                attempts = db.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='attempt' AND request_id=?", [request_id]).fetchall()
                finished = db.execute("SELECT count(*) FROM gateway_event_log WHERE event_type='request_finished' AND request_id=?", [request_id]).fetchone()[0]
                usage = db.execute("SELECT count(*) FROM gateway_event_log WHERE event_type='usage' AND request_id=?", [request_id]).fetchone()[0]
                ledger = db.execute("SELECT count(*) FROM billing_ledger_entries WHERE request_id=?", [request_id]).fetchone()[0]
            return {"attempts": [json.loads(row[0])["attempt"]["attempt_id"] for row in attempts]} if finished == 1 and len(attempts) == usage == ledger == int(not rejected) else None
        lineage = base.wait_for(settled, "exact metadata Request/Attempt/Usage/ledger")
        item = {"kind": "metadata", "version": version, "request": request_id, "owner": "owner", "raw": raw,
                "attempts": lineage["attempts"], "key_version": self.active, "public_status": status,
                "independent_receive_delta": len(self.peer.receives) - before, "stream": stream}
        item["private"] = self.read(item, "capture")
        self.records.append(item)
        self.report.setdefault("captures", []).append({k: v for k, v in item.items() if k not in ("raw", "private")})
        return item

    def read(self, item, stage, binary="current", keys=None, owner=None, db=None, expected="retained", attempt=None):
        output = self.private / (stage + "-" + secrets.token_hex(8) + ".json")
        command = [self.binary[binary], "request-metadata-evidence", "--db", db or self.db,
                   "--master-key-dir", keys or self.keys, "--active-master-key-version", self.active,
                   "--client-key-id", owner or item["owner"], "--request-id", item["request"], "--output", output]
        if attempt is not None:
            command += ["--attempt-id", attempt]
        result = subprocess.run(self.sandbox(command), env=self.env, cwd=self.root, capture_output=True, timeout=15)
        self.report["reads"].append({"stage": stage, "request": item["request"], "reader": binary,
            "reader_sha256": base.sha(self.binary[binary].read_bytes()), "expected": expected, "exit": result.returncode})
        self.check("private metadata never enters CLI stdout/errors", b"M4_PRIVATE" not in result.stdout + result.stderr)
        if expected == "error":
            self.check(stage + " rejects without plaintext output", result.returncode != 0 and not output.exists())
            return None
        self.check(stage + " private exclusive output", result.returncode == 0 and output.stat().st_mode & 0o777 == 0o600)
        value = json.loads(output.read_text())
        self.check(stage + " " + expected, value["status"] == expected)
        if expected == "retained":
            diagnostic = value["record"]
            record = diagnostic["record"]
            self.check(stage + " exact raw field and authenticated source", record["evidence"]["raw_json"] == item["raw"]
                and record["version"] == item["version"] and record["request"]["request_id"] == item["request"]
                and record["request"]["client_key_id"] == item["owner"]
                and record["evidence"].get("source") == ("anthropic_messages_metadata" if item["version"] == 2 else None))
            self.check(stage + " only real Attempts", [a["attempt_id"] for a in diagnostic["attempts"]] == item["attempts"])
        self.save()
        return str(output)

    def verify_reads(self, stage):
        for item in self.records:
            self.read(item, stage)

    def verify_public(self, stage):
        status, raw, _ = self.data("/v1/models", method="GET")
        self.check(stage + " restored authenticated model view", status == 200
                   and "grok-4.5" in [m["id"] for m in json.loads(raw)["data"]])

    def replay_pair(self, item, retained=False):
        self.operator("replay_metadata", item, expected=retained)
        changed = copy.deepcopy(json.loads(Path(item["private"]).read_text()))
        changed["record"]["record"]["evidence"]["raw_json"] = " " + item["raw"]
        path = self.private / ("changed-" + secrets.token_hex(8) + ".json")
        base.dump(path, changed)
        self.operator("replay_metadata", {**item, "private": str(path)})

    def expire_fixture(self, item):
        # Explicit synthetic clock fixture; this is never a production TTL operation.
        with sqlite3.connect(self.db) as db:
            db.execute("UPDATE request_metadata_evidence SET created_at_ms=0,expires_at_ms=1 WHERE request_id=?", [item["request"]])
        self.report["fault_fixtures"].append({"kind": "direct_SQL_retention_clock", "request": item["request"], "times": [0, 1]})
        self.records.remove(item)
        self.expired.append(item)

    def receipt_check(self, stage):
        with sqlite3.connect(self.db) as db:
            for item in self.expired:
                row = db.execute("SELECT created_at_ms,expires_at_ms,payload_version,key_version,ciphertext IS NULL "
                                 "FROM request_metadata_evidence WHERE request_id=?", [item["request"]]).fetchone()
                self.check(stage + " retains versioned expiry receipt", row == (0, 1, item["version"], item["key_version"], 1))

    def privacy(self):
        with sqlite3.connect(self.db) as db:
            for table in ("gateway_event_log", "gateway_event_quarantine"):
                self.check(table + " no raw metadata", all("M4_PRIVATE" not in row[0] for row in db.execute("SELECT payload_json FROM " + table)))
        self.check("all independent requests exclude local controls", all(r["local_metadata_absent_in_body_and_headers"] for r in self.peer.receives))
        self.check("ordinary logs exclude private identifiers", all(b"M4_PRIVATE" not in p.read_bytes() for p in self.out.glob("*.log")))

    def run(self):
        self.isolation()
        self.start("schema37", "schema37-seed")
        self.check("historical real process uses schema37", self.schema() == 37)
        self.seed()
        old_live = self.capture_metadata(1)
        self.read(old_live, "old-reader-before-migration", binary="schema37")
        old_expired = self.capture_metadata(1)
        self.expire_fixture(old_expired)
        self.capture_metadata(1)  # Existing writer performs bounded GC, retaining the receipt.
        self.receipt_check("before-migration")
        self.stop()
        before = self.snapshot("schema37-before-migration")
        self.start("current", "schema38-adopt")
        self.check("new real process migrates to schema38", self.schema() == 38)
        self.check("migration preserves all prior semantic state and old ciphertext", self.snapshot("schema38-after-migration") == before)
        first = self.capture_metadata()
        self.capture_metadata(rejected=True)
        self.capture_metadata(stream=True)
        self.read(old_live, "old-reader-rejects-new-schema", binary="schema37", expected="error")
        self.read(first, "foreign-owner", owner="sibling", expected="unknown")
        self.read(first, "wrong-existing-attempt", attempt=old_live["attempts"][0], expected="unknown")
        self.read(first, "actual-attempt", attempt=first["attempts"][0])
        self.replay_pair(first, retained=True)
        self.stop()
        before = self.snapshot("before-real-restart")
        self.start("current", "schema38-restart")
        self.check("new PID adopts exact coherent state", self.snapshot("after-real-restart") == before)
        self.verify_reads("after-restart")
        self.stop()
        self.active = 2
        self.start("current", "key2-serving")
        key2 = self.capture_metadata()
        self.verify_reads("rotation")
        wrong = self.root / "wrong-keys"
        wrong.mkdir(mode=0o700)
        (wrong / "2.key").write_bytes(secrets.token_bytes(32))
        self.read(key2, "wrong-key", keys=wrong, expected="error")
        only2 = self.root / "key2-only"
        only2.mkdir(mode=0o700)
        shutil.copy2(self.keys / "2.key", only2 / "2.key")
        self.read(first, "missing-old-key", keys=only2, expected="error")
        self.expire_fixture(first)
        self.capture_metadata()
        self.receipt_check("expired")
        self.stop()
        for item in self.expired:
            self.read(item, "expired", expected="unknown")
            self.replay_pair(item)
        self.start("current", "expired-restart")
        self.receipt_check("expired-restart")
        self.verify_reads("expired-restart")
        self.stop()
        # The existing management restore writes an independent target. A new stock process
        # then adopts it as control.sqlite3; merely reopening a restored reader is insufficient.
        self.restore()
        self.privacy()
        self.report["status"] = "PASS_BOUNDED"
        self.report["preserved_current_inputs"] = base.source_identity(self.args.current_source) == self.report["sources"]["current"]
        self.check("frozen source/manifest/lock unchanged during process matrix", self.report["preserved_current_inputs"])
        self.save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("current-binary", "historical-binary", "support-binary", "current-source", "historical-source", "output"):
        parser.add_argument("--" + name, required=True, type=Path)
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
        matrix.save()
        matrix.peer.shutdown()
    print(json.dumps({"status": matrix.report["status"], "receipt": str(matrix.out / "matrix.json")}))


if __name__ == "__main__":
    main()
