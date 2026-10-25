#!/usr/bin/env python3
"""Owned version-explicit capacity experiment. Reuses #65 stock serve, fence and recovery.

Synthetic SQL scale rows are explicitly distinct from public-HTTP evidence.
Keys, artifacts and decrypted diagnostics must stay in the private state root.
"""
import argparse
import copy
import csv
import hashlib
import http.client
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import signal
import socket
import secrets
import shutil
import sqlite3
import subprocess
import threading
import time
import urllib.parse
import urllib.request

spec = importlib.util.spec_from_file_location("metadata_recovery", Path(__file__).with_name("cpar64-metadata-recovery.py"))
meta = importlib.util.module_from_spec(spec)
spec.loader.exec_module(meta)
base = meta.base
START, SPAN, REQUESTS, QUOTAS = 1700000000000, 30 * 86400000, 25000, 40000


def digest_file(path):
    with Path(path).open("rb") as source:
        digest = hashlib.sha256()
        for chunk in iter(lambda: source.read(65536), b""):
            digest.update(chunk)
        return digest.hexdigest()


def append_lineage(db, events, ledger, request, ts):
    original = next(json.loads(p)["request"]["request_id"] for kind, _, p in events if kind == "request")
    old_attempt = next(json.loads(p)["attempt"]["attempt_id"] for kind, _, p in events if kind == "attempt")
    old_usage = next(event_id for kind, event_id, _ in events if kind == "usage")
    attempt = f"attempt:{len(request)}:{request}:1"
    usage = "usage-attempt:" + base.sha(attempt.encode())
    mapping = {original: request, old_attempt: attempt, old_usage: usage}
    def rewrite(value):
        if isinstance(value, dict): return {k: rewrite(v) for k, v in value.items()}
        if isinstance(value, list): return [rewrite(v) for v in value]
        if isinstance(value, str): return mapping.get(value, value)
        # These templates have only timestamp values >=1e12.
        if isinstance(value, int) and value >= 1000000000000: return ts
        return value
    ids = dict(request=request, request_finished=request, attempt=attempt, usage=usage)
    for kind, _, payload in events:
        db.execute("INSERT INTO gateway_event_log(event_type,event_id,request_id,occurred_at_ms,payload_json) VALUES(?,?,?,?,?)",
                   [kind, ids[kind], request, None if kind == "usage" else ts, json.dumps(rewrite(json.loads(payload)), separators=(",", ":"))])
    if ledger is None: return  # The real stock BillingWorker materializes the frozen scale seed.
    row = {k: rewrite(v) for k, v in ledger.items() if k not in ("ledger_id", "source_fingerprint")}
    row["retention_expires_at_ms"] = ledger["retention_expires_at_ms"]
    digest = hashlib.sha256()
    for key in ("source_event_id", "request_id", "response_id", "provider_id", "channel_id", "account_id", "model"):
        digest.update(row[key].encode() + b"\0")
    digest.update(row["occurred_at_ms"].to_bytes(8, "big") + b"\0")
    digest.update(b"\0" if row["catalog_version_id"] is None else b"\1" + row["catalog_version_id"].encode())
    digest.update(row["retention_expires_at_ms"].to_bytes(8, "big"))
    for key in ("input_tokens", "output_tokens", "reasoning_tokens", "cache_read_tokens", "cache_creation_tokens", "cached_tokens", "cost_microunits"):
        digest.update(((2**64 - 1) if row[key] is None else row[key]).to_bytes(8, "big"))
    digest.update(row["cost_confidence"].encode())
    usage = json.loads(next(payload for kind, _, payload in events if kind == "usage"))["usage"]["usage"]
    if usage["provenance"] != "unknown" or usage["input_accounting"] != "unknown":
        digest.update(json.dumps([usage["provenance"], usage["input_accounting"]], separators=(",", ":")).encode())
    row["source_fingerprint"] = digest.hexdigest()
    db.execute("INSERT INTO billing_ledger_entries(" + ','.join(row) + ") VALUES(" + ','.join('?' for _ in row) + ")", list(row.values()))


def concurrent_rows(db):
    result = {}
    for (request,) in db.execute("SELECT DISTINCT request_id FROM gateway_event_log WHERE request_id LIKE 'concurrent75-%' ORDER BY request_id"):
        data = list(db.execute("SELECT * FROM gateway_event_log WHERE request_id=? ORDER BY event_ordinal", [request]))
        data += list(db.execute("SELECT * FROM billing_ledger_entries WHERE request_id=? ORDER BY ledger_id", [request]))
        result[request] = base.sha(json.dumps(data, separators=(",", ":")).encode())
    return result


def csv_matches(actual, expected):
    return len(actual) == len(expected) and all(set(cells) == set(row) and all(
        json.loads(cells[field]) == value if isinstance(value, (dict, list)) else
        cells[field] == ("" if value is None else str(value).lower() if isinstance(value, bool) else str(value))
        for field, value in row.items()) for cells, row in zip(actual, expected))


class Matrix(base.Matrix):
    def __init__(self, args):
        super().__init__(args)
        self.report.update(scope=f"issue75_owned_schema{args.expected_schema}_capacity", mode=args.mode,
                           driver_sha256=digest_file(__file__), measurements=[])
        self.metadata = []
        self.rss_stop = threading.Event()
        self.rss = None

    def start(self, binary, label):
        super().start(binary, label)
        if binary == "current":
            with sqlite3.connect(self.db) as db:
                self.check(label + " complete expected schema prefix", db.execute(
                    "SELECT version FROM schema_migrations ORDER BY version").fetchall()
                    == [(n,) for n in range(1, self.args.expected_schema + 1)])
                if self.args.expected_schema == 40:
                    self.check(label + " schema40 review table exists", db.execute(
                        "SELECT count(*) FROM billing_catalog_reviews").fetchone() is not None)
        self.rss_stop.clear()
        self.rss = {"label": label, "sampled_max_rss_bytes": 0, "samples": 0}
        self.report.setdefault("serve_resources", []).append(self.rss)
        pid, row = self.process.pid, self.rss

        def sample():
            while not self.rss_stop.is_set():
                result = subprocess.run(["/bin/ps", "-o", "rss=", "-p", str(pid)], capture_output=True)
                if result.returncode == 0 and result.stdout.strip():
                    row["sampled_max_rss_bytes"] = max(row["sampled_max_rss_bytes"], int(result.stdout) * 1024)
                    row["samples"] += 1
                self.rss_stop.wait(.05)
        self.rss_thread = threading.Thread(target=sample, daemon=True)
        self.rss_thread.start()

    def stop(self):
        process = self.process
        process.terminate()
        deadline = time.monotonic() + 45
        while True:
            pid, status, usage = os.wait4(process.pid, os.WNOHANG)
            if pid:
                break
            assert time.monotonic() < deadline, "gateway stop deadline"
            time.sleep(.02)
        process.returncode = os.waitstatus_to_exitcode(status)
        self.report["processes"][-1].update(exit_code=process.returncode, reaped=True,
                                            os_high_water_rss_bytes=usage.ru_maxrss)
        self.process = None
        self.rss_stop.set()
        self.rss_thread.join()
        self.check("old serve exited and reaped", process.returncode == 0)
        with sqlite3.connect(self.db) as db:
            self.check("graceful writer drain", db.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()[0] == 0)
        self.save()

    def read(self, item, *args, **kwargs):
        if item["kind"] == "metadata":
            return meta.Matrix.read(self, item, *args, **kwargs)
        return base.Matrix.read(self, item, *args, **kwargs)

    def snapshot(self, label):
        result = {}
        with sqlite3.connect(self.db) as db:
            self.check(label + " SQLite integrity", db.execute("PRAGMA quick_check").fetchone() == ("ok",)
                       and not db.execute("PRAGMA foreign_key_check").fetchall())
            tables = [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")]
            for table in tables:
                if table in ("sqlite_sequence", "billing_materializer_checkpoints", "gateway_request_recording"):
                    continue  # Worker/checkpoint and recovery state have their own ownership.
                columns = list(db.execute('PRAGMA table_info("' + table + '")'))
                order = [r[1] for r in sorted(columns, key=lambda c: c[5]) if r[5]] or [r[1] for r in columns]
                where = " WHERE request_id NOT LIKE 'concurrent75-%'" if table in ('gateway_event_log', 'billing_ledger_entries') else ''
                query = 'SELECT * FROM "' + table + '"' + where + ' ORDER BY ' + ','.join('"' + c + '"' for c in order)
                digest, count = hashlib.sha256(), 0
                for row in db.execute(query):
                    digest.update(json.dumps([{"blob_sha256": base.sha(v)} if isinstance(v, bytes) else v for v in row],
                                             ensure_ascii=False, separators=(",", ":")).encode() + b"\n")
                    count += 1
                result[table] = {"columns": [r[1] for r in columns], "rows": count, "rows_sha256": digest.hexdigest()}
        self.report["snapshots"][label] = result
        self.save()
        return result

    def seed_scale(self, templates):
        started = time.monotonic()
        with sqlite3.connect(self.db) as db:
            originals = []
            for item in templates:
                events = list(db.execute("SELECT event_type,event_id,payload_json FROM gateway_event_log WHERE request_id=? ORDER BY event_ordinal", [item["request"]]))
                db.row_factory = sqlite3.Row
                ledger = dict(db.execute("SELECT * FROM billing_ledger_entries WHERE request_id=?", [item["request"]]).fetchone())
                db.row_factory = None
                originals.append((item, events, ledger))
            for i in range(REQUESTS):
                item, events, ledger = originals[0 if i % 10 < 8 else i % 10 - 7]
                append_lineage(db, events, None, f"capacity75-{i:06}", START + i * SPAN // REQUESTS)
            for i in range(QUOTAS):
                ts = START + i * SPAN // QUOTAS
                status = ["observed", "exhausted", "unknown", "failed"][i % 4]
                sample = dict(provider="grok_build", source="billing", window="window-" + str(i % 2), observed_at_ms=ts,
                              confidence="unknown" if i % 4 >= 2 else "observed", completeness="partial" if i % 7 == 0 else "projected",
                              status=status, used_percent=None if i % 4 >= 2 else (100.0 if i % 4 == 1 else 25.0),
                              used=None, limit=None, unit=None, reset_at_ms=ts + 3600000, duration_seconds=3600)
                db.execute("INSERT INTO quota_history(account_kind,config_version_id,account_id,revision,provider,source,window,observed_at_ms,evidence_json) VALUES(?,?,?,?,?,?,?,?,?)",
                           ["native" if i % 2 else "credential", "" if i % 2 else self.scope,
                            "retained-deleted-synthetic" if i % 2 else "ordinary-disabled", 1, sample["provider"], sample["source"], sample["window"], ts, json.dumps(sample)])
            db.commit()
            db.execute("ANALYZE")
            db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
        self.report["seed"] = dict(requests=REQUESTS, events=4 * REQUESTS, quotas=QUOTAS,
                                   elapsed_seconds=time.monotonic() - started, database_bytes=self.db.stat().st_size)
        self.save()

    def query(self, label):
        before = len(self.peer.receives)
        query = {"from_ms": START + (REQUESTS - 300) * SPAN // REQUESTS,
                 "to_ms": START + SPAN, "bucket_ms": 3600000}
        started = time.monotonic()
        result = self.api("POST", "/admin/operations/usage-snapshots", query, 201)
        self.report["measurements"].append({"operation": label + "-selective-capture", "seconds": time.monotonic() - started})
        snapshot = result["snapshot_id"]
        detail = self.api("GET", f"/admin/operations/usage-snapshots/{snapshot}")
        rows, cursor = [], None
        while True:
            suffix = "&cursor=" + urllib.parse.quote(cursor) if cursor else ""
            page = self.api("GET", f"/admin/operations/usage-snapshots/{snapshot}/rows?limit=100" + suffix)
            rows.extend(page["items"])
            cursor = page.get("next_cursor")
            if not cursor:
                break
        self.check(label + " selective capture contains every requested Attempt", len(rows) == 300)
        for fmt in ("csv", "jsonl"):
            exported, cursor = [], None
            while True:
                suffix = "&cursor=" + urllib.parse.quote(cursor) if cursor else ""
                req = urllib.request.Request(self.base + f"/admin/operations/usage-snapshots/{snapshot}/export?format={fmt}&limit=100" + suffix,
                                             headers={"X-Management-Key": self.management})
                with self.client.open(req, timeout=120) as response:
                    text = response.read().decode()
                    exported.extend(list(csv.DictReader(io.StringIO(text))) if fmt == "csv" else [json.loads(s) for s in text.splitlines()])
                    cursor = response.headers.get("X-Next-Cursor")
                if not cursor:
                    break
            self.check(label + " complete " + fmt + " export", len(exported) == len(rows) and (exported == rows if fmt == "jsonl" else csv_matches(exported, rows)))
        self.api("DELETE", f"/admin/operations/usage-snapshots/{snapshot}", expected=204)
        started = time.monotonic()
        self.api("POST", "/admin/operations/usage-snapshots", {"from_ms": START, "to_ms": START + SPAN, "bucket_ms": 86400000}, 413)
        self.report["measurements"].append({"operation": label + "-full-explicit-limit", "seconds": time.monotonic() - started})
        started = time.monotonic()
        quota = self.api("GET", f"/admin/quota-history?from_ms={START}&to_ms={START + SPAN}&limit=200")
        self.report["measurements"].append({"operation": label + "-quota-page", "seconds": time.monotonic() - started})
        self.check(label + " quota bounded and offline", len(quota["entries"]) == 200 and len(self.peer.receives) == before)
        # Handles/watermarks are process local; compare the actual immutable data product.
        return {"rows_sha256": base.sha(json.dumps(rows, sort_keys=True).encode()),
                "quota_sha256": base.sha(json.dumps({k: v for k, v in quota.items() if k != "as_of_ms"}, sort_keys=True).encode()), "summary": detail["summary"]}

    def operator_measure(self, artifact, expect_success):
        path = self.private / ("job-" + secrets.token_hex(8) + ".json")
        base.dump(path, dict(scope="isolated_synthetic_cpar65", action="encrypted_backup", database=str(self.db),
                            keys=str(self.keys), active=self.active, input=str(self.credentials / "backup-key"),
                            output=str(artifact), expected_persisted=False))
        command = self.sandbox(["/usr/bin/time", "-l", "/usr/bin/env", "CPAR65_OPERATOR_JOB=" + str(path),
                               self.binary["support"], "--ignored", "--exact", "cpar65_operator_job", "--nocapture"])
        started = time.monotonic()
        run = subprocess.run(command, env=self.env, cwd=self.root, capture_output=True, timeout=180)
        raw = run.stdout + run.stderr
        (self.out / "backup-measure.log").write_bytes(raw)
        rss = re.search(rb"(\d+)\s+maximum resident set size", raw)
        self.report["backup"] = {"seconds": time.monotonic() - started, "exit": run.returncode,
                                 "os_high_water_rss_bytes": int(rss[1]) if rss else None,
                                 "artifact_bytes": artifact.stat().st_size if artifact.exists() else None,
                                 "artifact_sha256": digest_file(artifact) if artifact.exists() else None}
        self.check("backup outcome matches frozen mode", (run.returncode == 0) == expect_success)
        if not expect_success:
            self.check("baseline preserves precise capacity failure", b"MaterialTooLarge" in raw and not artifact.exists())

    def binary_post(self, path, artifact, expected):
        # File body is streamed by http.client; driver never retains a large artifact buffer.
        started = time.monotonic()
        conn = http.client.HTTPConnection("127.0.0.1", self.admin_port, timeout=180)
        with artifact.open("rb") as body:
            conn.request("POST", path, body, {"X-Management-Key": self.management,
                         "Content-Type": "application/octet-stream", "Content-Length": str(artifact.stat().st_size)})
            response = conn.getresponse()
            raw = response.read()
            self.check(path + " completed through protected HTTP", response.status == expected)
        conn.close()
        self.report["measurements"].append({"operation": path, "seconds": time.monotonic() - started})
        self.save()
        return json.loads(raw)

    def prepare(self):
        self.isolation()
        self.start("current", "synthetic-seed")
        self.check(f"actual stock schema{self.args.expected_schema}", self.schema() == self.args.expected_schema)
        self.seed()
        templates = [self.capture(kind) for kind in ("ordinary", "usage", "summary")]
        meta.Matrix.capture_metadata(self, 2)
        self.stop()
        self.seed_scale(templates)
        base.dump(self.private / "context.json", {"scope": self.scope, "revision": self.revision,
            "owner": self.owner, "sibling": self.sibling, "restricted": self.restricted,
            "disabled_native": self.disabled_native, "records": self.records, "unknowns": self.unknowns})

    def reuse(self):
        original = self.args.fixture.resolve()
        shutil.copy2(original / "state/control.sqlite3", self.db)
        for name in ("credentials", "keys"):
            for path in (original / name).iterdir():
                shutil.copy2(path, getattr(self, name) / path.name)
        context = json.loads((original / "private/context.json").read_text())
        for key, value in context.items():
            setattr(self, key, value)
        for item in self.records:
            dest = self.private / Path(item["private"]).name
            shutil.copy2(item["private"], dest)
            item["private"] = str(dest)
        self.management = (self.credentials / "management-key").read_text()
        self.check("source input clone byte identical", digest_file(self.db) == digest_file(original / "state/control.sqlite3"))

    def settle_billing(self, label):
        def current():
            value = self.api("GET", "/admin/operations/billing-processing")
            assert value["state"] not in ("failed", "needs_repair"), value
            return value if value["state"] == "current" and value["unresolved_failures"] == 0 and value["checkpoint_ordinal"] == value["source_ordinal"] else None
        value = base.wait_for(current, label + " real billing materialization/replay", timeout=180)
        self.report.setdefault("billing_processing", []).append({"stage": label, **value})
        self.save()

    def copy_phase(self):
        try: files = [(p, p.stat().st_size) for p in self.root.glob(".cpa-backup-*.sqlite3")]
        except FileNotFoundError: return False
        growing = [size for _, size in files if 1048576 < size < self.copy_database_bytes]
        return bool(growing and any(size == 0 for _, size in files))

    def concurrent_writer(self):
        self.writer_ready = threading.Event()
        self.writer_receipt = []
        self.copy_database_bytes = self.db.stat().st_size
        def append():
            try:
                with sqlite3.connect(self.db, timeout=20) as db:
                    template = list(db.execute("SELECT event_type,event_id,payload_json FROM gateway_event_log WHERE request_id='capacity75-000000' ORDER BY event_ordinal"))
                    db.row_factory = sqlite3.Row
                    ledger = dict(db.execute("SELECT * FROM billing_ledger_entries WHERE request_id='capacity75-000000'").fetchone())
                    db.row_factory = None
                    for batch in range(10):
                        if batch == 1:
                            deadline = time.monotonic() + 30
                            while not self.copy_phase():
                                assert time.monotonic() < deadline, "actual SQLite copy not observed"
                                time.sleep(.002)
                        copy_before = self.copy_phase()
                        committed_ids = [f"concurrent75-{i:04}" for i in range(batch * 10, batch * 10 + 10)]
                        for i, request in enumerate(committed_ids):
                            append_lineage(db, template, ledger, request, START + SPAN + 100000 + batch * 10 + i)
                        db.commit()
                        self.writer_receipt.append({"batch": batch, "request_ids": committed_ids, "committed_at_monotonic": time.monotonic(), "copy_before_commit": copy_before, "copy_after_commit": self.copy_phase()})
                        self.writer_ready.set()
                        time.sleep(.05)
            except BaseException as error:
                self.writer_receipt.append({"failure": repr(error)})
                self.writer_ready.set()
        self.writer_thread = threading.Thread(target=append)
        self.writer_thread.start()
        assert self.writer_ready.wait(20)

    def verify_concurrent_snapshot(self):
        with sqlite3.connect(self.db) as db:
            events = list(db.execute("SELECT request_id,count(*),count(DISTINCT event_type) FROM gateway_event_log WHERE request_id LIKE 'concurrent75-%' GROUP BY request_id ORDER BY request_id"))
            ledger = db.execute("SELECT count(*) FROM billing_ledger_entries WHERE request_id LIKE 'concurrent75-%'").fetchone()[0]
            actual = concurrent_rows(db)
        self.check("backup includes only complete concurrently committed lineages and ledger", 0 < len(events) <= 100
                   and len(events) % 10 == 0 and all(row[1:] == (4, 4) for row in events) and ledger == len(events))
        self.check("all restored concurrent cells equal an exact committed source prefix", list(actual) == list(self.committed_rows)[:len(actual)]
                   and all(value == self.committed_rows[key] for key, value in actual.items()))
        self.report["concurrent_snapshot"] = {"included_requests": len(events), "source_committed_requests": len(self.committed_rows),
                                             "batch_receipts": self.writer_receipt}

    def write_limit_failure(self, artifact):
        for action in ("encrypted_backup", "restore_encrypted_backup"):
            target = self.root / (action + "-file-limit")
            job = self.private / (action + "-limit.json")
            base.dump(job, dict(scope="isolated_synthetic_cpar65", action=action,
                database=str(self.db if action == "encrypted_backup" else artifact), keys=str(self.keys), active=self.active,
                input=str(self.credentials / "backup-key"), output=str(target), expected_persisted=False))
            script = "import os,resource,signal,sys;resource.setrlimit(resource.RLIMIT_FSIZE,(8388608,8388608));signal.signal(signal.SIGXFSZ,signal.SIG_IGN);os.execv(sys.argv[1],sys.argv[1:])"
            command = self.sandbox(["/usr/bin/env", "CPAR65_OPERATOR_JOB=" + str(job), "/usr/bin/python3", "-c", script,
                                   self.binary["support"], "--ignored", "--exact", "cpar65_operator_job", "--nocapture"])
            result = subprocess.run(command, env=self.env, cwd=self.root, capture_output=True, timeout=180)
            (self.out / (action + "-file-limit.log")).write_bytes(result.stdout + result.stderr)
            self.check(action + " file-size write failure publishes no target", result.returncode != 0 and not target.exists()
                       and (b"FilesystemUnavailable" in result.stdout + result.stderr or b"SnapshotUnavailable" in result.stdout + result.stderr))
        self.report["write_failure"] = {"mechanism": "owned child RLIMIT_FSIZE=8MiB, SIGXFSZ ignored", "physical_disk_exhaustion": "NOT_RUN"}

    def interrupt_restore(self, artifact):
        self.start("current", "interrupted-restore-host")
        # Keep a large upload open. Another authenticated upload must be rejected before buffering.
        partial = socket.create_connection(("127.0.0.1", self.admin_port))
        partial.sendall(("POST /admin/restores HTTP/1.1\r\nHost: localhost\r\nX-Management-Key: " + self.management
                        + "\r\nContent-Type: application/octet-stream\r\nContent-Length: " + str(artifact.stat().st_size) + "\r\n\r\n").encode())
        with artifact.open("rb") as source: partial.sendall(source.read(65536))
        time.sleep(.1)
        self.api("POST", "/admin/restores/preflight", b"incomplete", 503, {"Content-Type": "application/octet-stream"})
        partial.close()
        def released():
            try:
                self.api("POST", "/admin/restores/preflight", b"incomplete", 400, {"Content-Type": "application/octet-stream"})
                return True
            except AssertionError: return False
        base.wait_for(released, "aborted upload releases bounded admission")
        self.check("cancelled upload leaves target absent", not (self.state / "restore-target.sqlite3").exists())
        failed = []
        def submit():
            try: self.binary_post("/admin/restores", artifact, 202)
            except (OSError, AssertionError) as error: failed.append(type(error).__name__)
        sender = threading.Thread(target=submit)
        sender.start()
        def decrypting():
            return any(p.stat().st_size > 1024 * 1024 for p in self.state.rglob(".cpa-backup-*.sqlite3"))
        base.wait_for(decrypting, "observe large restore staging before interruption", timeout=180)
        process = self.process
        process.kill()
        process.wait(timeout=45)
        self.report["processes"][-1].update(exit_code=process.returncode, reaped=True, intentional_sigkill=True)
        self.process = None
        self.rss_stop.set()
        self.rss_thread.join()
        sender.join(timeout=20)
        self.check("interrupted restore never publishes a partial target", not (self.state / "restore-target.sqlite3").exists()
                   and process.returncode == -signal.SIGKILL and not sender.is_alive())
        self.report["interruption"] = {"sender_errors": failed, "partial_private_staging_retained": True}
        # Restart the same restore host and perform the explicit operator retry into the still absent target.

    def run(self):
        if self.args.fixture:
            self.reuse()
        else:
            self.prepare()
        self.start("current", "large-source-query")
        self.settle_billing("source")
        self.verify_public("large-source")
        before_query = self.query("source")
        self.stop()
        before = self.snapshot("source-before-backup")
        artifact = self.root / "coherent.encrypted"
        if self.args.mode == "fixed": self.concurrent_writer()
        self.operator_measure(artifact, self.args.mode == "fixed")
        if self.args.mode == "fixed":
            self.writer_thread.join(timeout=30)
            self.check("bounded concurrent writer completed", not self.writer_thread.is_alive()
                       and [row.get("batch") for row in self.writer_receipt] == list(range(10)))
            self.check("committed write overlaps the actual SQLite copy", any(row["copy_before_commit"] and row["copy_after_commit"] for row in self.writer_receipt))
            self.start("current", "concurrent-ledger-replay")
            self.settle_billing("concurrent-ledger-replay")
            self.stop()
            with sqlite3.connect(self.db) as db: self.committed_rows = concurrent_rows(db)
            ids = [request for row in self.writer_receipt for request in row["request_ids"]]
            self.check("committed source IDs match every writer receipt", len(ids) == 100 and ids == list(self.committed_rows))
        self.check("backup preserves original source data", self.snapshot("source-after-backup") == before)
        if self.args.mode == "baseline":
            self.report["status"] = "MEASURED_BASELINE_LIMIT"
            return
        self.write_limit_failure(artifact)
        self.check("write failures preserve source", self.snapshot("source-after-write-failures") == before)
        source_state = self.state
        self.state = self.root / "restore-host"
        self.state.mkdir(mode=0o700)
        self.interrupt_restore(artifact)
        self.start("current", "restore-host")
        self.binary_post("/admin/restores/preflight", artifact, 200)
        self.binary_post("/admin/restores", artifact, 202)
        self.stop()
        target = self.state / "restore-target.sqlite3"
        self.state = self.root / "restored-serving-state"
        self.state.mkdir(mode=0o700)
        shutil.copy2(target, self.db)
        self.start("current", "restored-serving")
        self.settle_billing("restored")
        self.check("new gateway adopts exact restored business tables", self.snapshot("restored-serving") == before)
        self.verify_concurrent_snapshot()
        self.verify_public("restored")
        self.verify_reads("restored")
        self.check(f"schema{self.args.expected_schema} query/export/quota remain identical", self.query("restored") == before_query)
        self.stop()
        self.report["restoration"] = {"source_state": str(source_state), "management_target": str(target),
                                      "adopted_control_database": str(self.db), "replacement": False}
        self.report["status"] = "PASS_LOCAL_CAPACITY"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("current-binary", "support-binary", "current-source", "output"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--mode", choices=("baseline", "fixed"), required=True)
    parser.add_argument("--fixture", type=Path)
    parser.add_argument("--expected-schema", type=int, choices=(39, 40), default=39)
    args = parser.parse_args()
    args.historical_binary, args.historical_source = args.current_binary, args.current_source
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
        matrix.rss_stop.set()
        matrix.save()
        matrix.peer.shutdown()
    print(json.dumps({"status": matrix.report["status"], "receipt": str(matrix.out / "matrix.json"),
                      "private_state_root": str(matrix.root)}))


if __name__ == "__main__":
    main()
