#!/usr/bin/env python3
"""Schema39 -> 40/42, reviewed prices, controls and stock-process large recovery.

Extends the unchanged #75 frozen distribution; helpers only use synthetic sources.
All raw state, payloads and credential materials remain in the private state root.
"""
import argparse
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import time

spec = importlib.util.spec_from_file_location("capacity", Path(__file__).with_name("cpar75-history-capacity.py"))
capacity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capacity)
base = capacity.base


class Provider(base.Provider):
    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        self.server.wave2_bodies.append(json.loads(raw))
        self.server.wave3_headers.append(self.headers.get("x-request-tag") == self.server.wave3_header)
        self.rfile = io.BytesIO(raw)
        super().do_POST()


class Matrix(capacity.Matrix):
    def __init__(self, args):
        super().__init__(args)
        self.wave2_binary = self.root / "wave2-support"
        shutil.copy2(args.wave2_support, self.wave2_binary)
        self.report["wave2_support_sha256"] = capacity.digest_file(self.wave2_binary)
        self.peer.RequestHandlerClass = Provider
        self.peer.wave2_bodies = []
        self.peer.wave3_headers = []
        self.peer.wave3_header = "synthetic-" + os.urandom(16).hex()
        self.report["scope"] = f"integrated_schema{args.expected_schema}_frozen_capacity"
        self.report["historical_binary_label"] = "schema35 label reused for actual schema39 binary"

    def helper(self, action, value):
        port = base.unused_port()
        value = {**value, "port": port}
        source, output, job = [self.private / (action + suffix) for suffix in ("-input.json", "-output.json", "-job.json")]
        base.dump(source, value)
        base.dump(job, dict(database=str(self.db), keys=str(self.keys), input=str(source), output=str(output), action=action))
        profile = self.private / (action + ".sb")
        profile.write_text(self.fence.read_text() + f'\n(allow network-outbound (remote ip "localhost:{port}"))\n')
        command = ["/usr/bin/sandbox-exec", "-f", str(profile), "/usr/bin/env", "CPAR_WAVE2_JOB=" + str(job),
                   str(self.wave2_binary), "--ignored", "--exact", "wave2_operator_job", "--nocapture"]
        run = subprocess.run(command, env=self.env, cwd=self.root, capture_output=True, timeout=60)
        (self.out / (action + "-helper.log")).write_bytes(run.stdout + run.stderr)
        self.check(action + " helper completed without private stdout", run.returncode == 0)
        return json.loads(output.read_text())

    def prepare(self):
        self.isolation()
        self.start("schema35", "actual-schema39-seed")
        self.check("historical stock actually creates schema39", self.schema() == 39)
        self.seed()
        templates = [self.capture(kind) for kind in ("ordinary", "usage", "summary")]
        capacity.meta.Matrix.capture_metadata(self, 2)
        self.stop()
        self.seed_scale(templates)
        self.start("schema35", "schema39-materialize")
        self.settle_billing("schema39")
        self.stop()
        before = self.snapshot("schema39-before-upgrade")
        self.start("current", f"schema{self.args.expected_schema}-upgrade")
        self.settle_billing("integrated")
        self.check_schema()
        after = self.snapshot("integrated-after-upgrade")
        retained = self.snapshot("original-column-projection", projection=before)
        self.check("upgrade retains every original business cell and ciphertext", all(
            retained[table] == value for table, value in before.items() if table != "schema_migrations"))
        self.check("migration40 initially has an empty review table", after["billing_catalog_reviews"]["rows"] == 0)
        fork = self.api("POST", f"/admin/config-versions/{self.scope}/fork", {"id":"wave2-names", "description":"synthetic integration"}, 201)
        self.scope, self.revision = fork["id"], '"' + fork["revision"] + '"'
        self.api("POST", "/admin/public-models", {"id":"wave2-model", "model_name":"public-wave2", "display_name":"Wave2",
            "status":"active", "capabilities":{"reasoning":True,"stored_responses":True}}, 201)
        self.api("POST", "/admin/public-models/wave2-model/aliases", {"prefix":"team", "alias":"public-wave2"}, 201)
        self.api("POST", "/admin/public-models/wave2-model/routes", {"id":"wave2-route", "policy":"fill_first" if self.args.expected_schema == 42 else "smooth_weighted_round_robin",
            "max_attempts":1,"bootstrap_timeout_ms":10000}, 201)
        self.api("POST", "/admin/routes/wave2-route/candidates", {"id":"wave2-candidate","endpoint_id":"ordinary-endpoint",
            "upstream_model":"gpt-5.5","credential_scope":"all_active","transform_mode":"canonical_bridge","enabled":True,
            "priority":0,"weight":1,"capability_override":{"allow_unlisted_model":True,"stored_responses":True}}, 201)
        self.api("POST", "/admin/access-groups/owner-group/routes", {"route_id":"wave2-route","enabled":True}, 201)
        if self.args.expected_schema == 42:
            self.api("POST", "/admin/upstreams/ordinary/credentials", {"id":"wave3-header", "kind":"upstream_header", "secret":self.peer.wave3_header, "status":"active"}, 201)
            endpoint = self.api("GET", "/admin/endpoints/ordinary-endpoint")
            body = {k:endpoint[k] for k in ("id","adapter_id","api_format","base_url","inference_path","models_path","transport","enabled")}
            body["request_policy"] = {"rules":[{"models":["public-wave2"],"default":{"temperature":0.2},"override":{"max_output_tokens":128},"filter":["top_p"]}], "headers":{"x-request-tag":"wave3-header"}}
            self.api("PATCH", "/admin/endpoints/ordinary-endpoint", body)
        self.publish()
        price = self.api("POST", "/admin/config-versions", {"id":"wave2-prices","description":"synthetic price review"}, 201)
        price_revision = '"' + price["revision"] + '"'
        self.stop()
        self.review = self.helper("review", {"management_key":self.management,"config_version":"wave2-prices","revision":price_revision})
        self.check("review has both nonempty 64-character digests", all(len(self.review["review"][key]) == 64 for key in ("source_digest","content_digest")))
        # The source is controlled and injected into production management handlers over a real socket.
        # The stock gateway subsequently reads the durable result through its normal API.
        while time.time() * 1000 <= self.review["effective_at_ms"]:
            time.sleep(.05)
        self.start("current", "integrated-reviewed-source")
        self.verify_review("source")
        self.capture_thinking()
        self.verify_thinking("source")
        self.stop()
        if self.args.expected_schema == 42:
            # Synthetic durable continuity state; live selection is verified by #70 socket tests.
            with sqlite3.connect(self.db) as db:
                now = int(time.time() * 1000)
                db.execute("INSERT INTO grok_build_cache_affinities(client_key_id,provider_id,upstream_model,cache_identity,credential_id,expires_at_ms,reason,updated_at_ms,binding_fingerprint,credential_revision) VALUES ('wave3-fixture','grok.build','grok-4.5','opaque-fixture','synthetic-account',?,'prompt_cache',?,?,7)", [now+86400000,now,"7"*64])
                db.execute("INSERT INTO grok_build_affinity_breaks(client_key_id,upstream_model,cache_identity,prior_credential_id,next_credential_id,reason,estimated_cache_loss_tokens,occurred_at_ms) VALUES ('wave3-fixture','grok-4.5','opaque-fixture','old','synthetic-account','policy_reselection',NULL,?)", [now])
                db.commit()
        base.dump(self.private / "context.json", {"scope":self.scope,"revision":self.revision,"owner":self.owner,
            "sibling":self.sibling,"restricted":self.restricted,"disabled_native":self.disabled_native,
            "records":self.records,"unknowns":self.unknowns,"materials":self.peer.materials})

    def check_schema(self):
        with sqlite3.connect(self.db) as db:
            self.check(f"schema{self.args.expected_schema} complete unique migration prefix", db.execute("SELECT version FROM schema_migrations ORDER BY version").fetchall() == [(i,) for i in range(1, self.args.expected_schema + 1)])
            self.check("39 and 40 registered exactly once", db.execute("SELECT count(*) FROM schema_migrations WHERE version IN (39,40)").fetchone() == (2,))

    def verify_review(self, label):
        before = len(self.peer.receives)
        review = self.api("GET", "/admin/billing/catalogs/wave2-reviewed/review", extra={"X-Config-Version":"wave2-prices"})
        self.check(label + " stock HTTP reads exact applied review", review == self.review["review"])
        with sqlite3.connect(self.db) as db:
            self.check(label + " review catalog and ledger all nonempty", all(db.execute(f"SELECT count(*) FROM {table}").fetchone()[0] > 0
                for table in ("billing_catalog_reviews", "billing_price_catalog_versions", "billing_price_catalog_entries", "billing_ledger_entries")))
        self.check(label + " review read is offline", len(self.peer.receives) == before)

    def capture_thinking(self):
        self.peer.case = "ordinary"
        name = "team/public-wave2(high)"
        status, raw, headers = self.data("/v1/responses", {"model":name,"input":"wave2 question","store":True,"top_p":0.9,"max_output_tokens":1000,"reasoning":{"effort":"low"}})
        self.check("named stored thinking succeeds", status == 200)
        self.thinking_response = json.loads(raw)["id"]
        request = next(v for k,v in headers.items() if k.lower() == "x-request-id")
        self.wait_lineage(request, 1)
        with sqlite3.connect(self.db) as db:
            event = json.loads(db.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='request' AND request_id=?", [request]).fetchone()[0])["request"]
            self.report["named_request_identity"] = {k:v for k,v in event.items() if "model" in k}
            self.check("suffix keeps public model billing identity", event["requested_model"] == name and event["public_model"] == "public-wave2")
            ledger = db.execute("SELECT model,catalog_version_id,cost_microunits FROM billing_ledger_entries WHERE request_id=?", [request]).fetchone()
            self.check("exact public identity selects reviewed price rather than alias or upstream decoys", ledger == ("public-wave2","wave2-reviewed",81))
            self.check("nonempty encrypted stored response exists", db.execute("SELECT count(*) FROM stored_responses WHERE client_key_id='owner' AND response_id=? AND length(ciphertext)>0", [self.thinking_response]).fetchone() == (1,))
        wire = self.peer.wave2_bodies[-1]
        self.check("independent receiver sees actual model and suffix priority", wire["model"] == "gpt-5.5" and wire["reasoning"]["effort"] == "high")
        self.verify_rule_wire("source")
        self.report["thinking_storage"] = self.helper("thinking", {"response_id":self.thinking_response})

    def verify_rule_wire(self, label):
        if self.args.expected_schema != 42:
            return
        wire = self.peer.wave2_bodies[-1]
        self.check(label + " stock sends default override filter and decrypted header", wire.get("temperature") == 0.2 and wire.get("max_output_tokens") == 128 and "top_p" not in wire and self.peer.wave3_headers[-1])

    def verify_integrated_state(self, label):
        if self.args.expected_schema != 42:
            return
        endpoint = self.api("GET", "/admin/endpoints/ordinary-endpoint")
        self.check(label + " stock reads nonempty rules and header references", endpoint["request_policy"]["headers"] == {"x-request-tag":"wave3-header"} and len(endpoint["request_policy"]["rules"]) == 1)
        self.check(label + " stock reads explicit selection strategy", self.api("GET", "/admin/routes/wave2-route")["policy"] == "fill_first")
        with sqlite3.connect(self.db) as db:
            self.check(label + " encrypted header remains ciphertext", db.execute("SELECT count(*) FROM upstream_credentials WHERE config_version_id=? AND id='wave3-header' AND kind='upstream_header' AND length(ciphertext)>0", [self.scope]).fetchone() == (1,))
            self.check(label + " qualified affinity and unknown loss survive", db.execute("SELECT binding_fingerprint,credential_revision FROM grok_build_cache_affinities WHERE client_key_id='wave3-fixture'").fetchone() == ("7"*64,7) and db.execute("SELECT reason,estimated_cache_loss_tokens FROM grok_build_affinity_breaks WHERE client_key_id='wave3-fixture'").fetchone() == ("policy_reselection",None))

    def verify_thinking(self, label):
        before = len(self.peer.receives)
        path = "/v1/responses/" + self.thinking_response
        status, body, _ = self.data(path, method="GET")
        self.check(label + " stored response authorized HTTP read", status == 200 and json.loads(body)["id"] == self.thinking_response)
        for key in (self.sibling, self.restricted):
            status, _, _ = self.data(path, method="GET", key=key)
            self.check(label + " foreign stored response concealed", status == 404)
            status, _, _ = self.data("/v1/responses", {"model":"team/public-wave2(low)","input":"denied",
                "previous_response_id":self.thinking_response}, key=key)
            self.check(label + " continuation cannot enlarge key access", status == 404)
        self.check(label + " denied requests and reads cause no upstream send", len(self.peer.receives) == before)
        self.check(label + " exact encrypted model_thinking readback", self.helper("thinking", {"response_id":self.thinking_response}) == self.report["thinking_storage"])

    def verify_public(self, label):
        before = len(self.peer.receives)
        self.check(label + " ordinary disabled intent", self.api("GET", "/admin/credentials/ordinary-disabled")["status"] == "disabled")
        native = next(r for r in self.api("GET", "/admin/native-accounts?limit=100")["items"] if r["id"] == self.disabled_native)
        self.check(label + " native identity and disabled intent", not native["enabled"] and native["identity"]["email"] == "recovery65@example.invalid")
        status, raw, _ = self.data("/v1/models", method="GET")
        names = {r["id"] for r in json.loads(raw)["data"]}
        # Existing discovery lists exact upstream names plus explicit prefixed aliases.
        # The resolved public billing identity is checked separately on the captured Request.
        self.report.setdefault("model_discovery", {})[label] = sorted(names)
        self.check(label + " restored model and prefix graph", status == 200 and names == {"grok-4.5","legacy-model","gpt-5.5","team/public-wave2"})
        status, raw, _ = self.data("/v1/models", method="GET", key=self.restricted)
        self.check(label + " restricted discovery remains empty", status == 200 and json.loads(raw)["data"] == [])
        status, _, _ = self.data("/v1/responses", {"model":"team/public-wave2(high)","input":"denied"}, key=self.restricted)
        self.check(label + " permissions deny before sending", status == 404 and len(self.peer.receives) == before)
        self.check_schema()
        self.verify_integrated_state(label)
        if hasattr(self, "review"):
            self.verify_review(label)

    def run(self):
        super().run()
        with (self.root / "coherent.encrypted").open("rb") as artifact:
            header = artifact.read(45)
        self.check(f"v2 authentic format identifies schema{self.args.expected_schema}", header[:9] == b"CPABKP02\x02" and int.from_bytes(header[9:13],"big") == self.args.expected_schema)
        self.start("current", "restored-thinking-continuation")
        self.verify_thinking("restored")
        self.verify_review("restored")
        before = len(self.peer.receives)
        self.peer.case = "ordinary"
        status, raw, _ = self.data("/v1/responses", {"model":"team/public-wave2(low)","input":"wave2 followup","top_p":0.9,"max_output_tokens":1000,
            "previous_response_id":self.thinking_response})
        self.check("restored named continuation succeeds with exactly one send", status == 200 and len(self.peer.receives) == before + 1)
        wire = self.peer.wave2_bodies[-1]
        history = json.dumps(wire["input"])
        self.check("restored continuation carries exact supported history and current controls", "wave2 question" in history and "legacy known reply" in history
            and "wave2 followup" in history and wire["model"] == "gpt-5.5" and wire["reasoning"]["effort"] == "low")
        self.verify_rule_wire("restored")
        self.stop()
        self.report["status"] = "PASS_LOCAL_WAVE3_INTEGRATION" if self.args.expected_schema == 42 else "PASS_LOCAL_WAVE2_INTEGRATION"
        self.save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("current-binary", "historical-binary", "support-binary", "wave2-support", "current-source", "historical-source", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--expected-schema", type=int, choices=(40,42), default=40)
    args = parser.parse_args()
    args.mode, args.fixture = "fixed", None
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
    print(json.dumps({"status":matrix.report["status"],"receipt":str(matrix.out / "matrix.json"),"private_state_root":str(matrix.root)}))


if __name__ == "__main__":
    main()
