"""Execution evidence checks using the existing isolated gateway/TLS/SQLite harness."""
import hashlib
import json
import sqlite3
import concurrent.futures
import select
import socket
import socketserver
import threading
import contextlib
import datetime
import io
import pathlib
import cpar_new_build_acceptance as oracle


class LoopbackSocks:
    """Actual SOCKS5 CONNECT relay restricted to this harness's single TLS upstream."""
    def __init__(self, upstream_port):
        self.sends = 0
        owner = self
        class Relay(socketserver.BaseRequestHandler):
            def read(self, size):
                result = b""
                while len(result) < size:
                    part = self.request.recv(size - len(result))
                    if not part:
                        raise ConnectionError("fixture socks EOF")
                    result += part
                return result
            def handle(self):
                self.request.settimeout(10)
                version, methods = self.read(2)
                assert version == 5 and 0 in self.read(methods)
                self.request.sendall(b"\x05\x00")
                assert self.read(4) == b"\x05\x01\x00\x01"
                assert self.read(4) == socket.inet_aton("127.0.0.1")
                assert int.from_bytes(self.read(2), "big") == upstream_port
                with socket.create_connection(("127.0.0.1", upstream_port), timeout=10) as upstream:
                    owner.sends += 1
                    self.request.sendall(b"\x05\x00\x00\x01\x7f\x00\x00\x01\x00\x00")
                    while True:
                        readable, _, _ = select.select([self.request, upstream], [], [], 10)
                        if not readable:
                            return
                        for source in readable:
                            value = source.recv(65536)
                            if not value:
                                return
                            (upstream if source is self.request else self.request).sendall(value)
        class Server(socketserver.ThreadingTCPServer):
            daemon_threads = True
        self.server = Server(("127.0.0.1", 0), Relay)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
    @property
    def url(self):
        return "socks5://127.0.0.1:" + str(self.server.server_address[1])
    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(2)


def initial(context):
    data, body_for, check = (context[name] for name in ("data", "body_for", "check"))
    response_ids = []
    for source, (path, _, _) in context["PROTOCOLS"].items():
        for stream in (False, True):
            status, raw, headers = data(path, body_for(source, source, "text-round", stream))
            check(f"execution_wire_{source}_{stream}", status == 200 and b"synthetic answer" in raw)
            response_ids.append(context["calls"][-1]["response_id"])
    database_path = context["state"] / "control.sqlite3"
    with sqlite3.connect(database_path) as database:
        check("execution_sidecar_present", database.execute(
            "SELECT count(*) FROM sqlite_master WHERE name='gateway_attempt_execution'"
        ).fetchone()[0] == 1)
        request_ids = []
        for response_id in response_ids:
            usages = database.execute("SELECT request_id FROM gateway_event_log WHERE event_type='usage' "
                "AND json_extract(payload_json,'$.usage.response_id')=?", (response_id,)).fetchall()
            check("execution_response_exact_usage_correlation", len(usages) == 1)
            request_ids.append(usages[0][0])
        for request_id in request_ids:
            rows = database.execute(
                "SELECT a.payload_json,e.evidence_json,e.attempt_payload_sha256 "
                "FROM gateway_event_log a JOIN gateway_attempt_execution e ON e.attempt_id=a.event_id "
                "WHERE a.event_type='attempt' AND a.request_id=?", (request_id,)
            ).fetchall()
            check("execution_exact_request_attempt", len(rows) == 1)
            payload, evidence_json, checksum = rows[0]
            attempt, evidence = json.loads(payload)["attempt"], json.loads(evidence_json)
            check("execution_payload_bound", checksum == hashlib.sha256(payload.encode()).hexdigest())
            check("execution_lease_and_configuration", evidence["credential_revision"] == 0
                  and evidence["config_version_id"] == "local-b"
                  and evidence["config_revision"] > 0)
            check("execution_actual_channel", evidence["channel"] == (
                "anthropic-compatible" if attempt["endpoint_id"] == "endpoint-messages" else "openai-compatible"))
            check("execution_direct_revision_not_applicable", evidence["egress"] == {"kind": "direct"})
            check("execution_declaration_inherited", evidence["capabilities"]["reasoning"] is True
                  and evidence["capabilities"]["parallel"] is True)
            projection = context["api"]("GET", "/admin/models/effective?client_key_id=local-key&limit=200")
            models = [row for row in projection["items"] if row["id"] == attempt["upstream_model"]]
            sources = [row for model in models for row in model["sources"] if row["candidate_id"] == attempt["route_candidate_id"]]
            check("execution_protected_capabilities_match_actual_candidate", len(sources) == 1
                  and sources[0]["capability_evidence"] == {"source": "serving_snapshot", "config_version_id": evidence["config_version_id"], "config_revision": evidence["config_revision"], "capabilities": evidence["capabilities"]})
            check("execution_value_free", all(value not in evidence_json for value in (
                "synthetic answer", "text-round", context["issued_key"], *context["keys"].values())))
    return request_ids


def final(context, initial_ids):
    check = context["check"]
    with sqlite3.connect(context["state"] / "control.sqlite3") as database:
        missing = database.execute(
            "SELECT count(*) FROM gateway_event_log a LEFT JOIN gateway_attempt_execution e "
            "ON e.attempt_id=a.event_id WHERE a.event_type='attempt' AND e.attempt_id IS NULL"
        ).fetchone()[0]
        check("execution_all_started_attempts_including_errors_cancellation_timeout", missing == 0)
        rows = database.execute(
            "SELECT a.payload_json,e.evidence_json FROM gateway_event_log a "
            "JOIN gateway_attempt_execution e ON e.attempt_id=a.event_id WHERE a.event_type='attempt'"
        ).fetchall()
        by_request = {}
        for payload, evidence_json in rows:
            attempt, evidence = json.loads(payload)["attempt"], json.loads(evidence_json)
            by_request.setdefault(attempt["request_id"], []).append((attempt, evidence))
        check("execution_old_requests_keep_original_configuration", all(
            by_request[request_id][0][1]["config_version_id"] == "local-b" for request_id in initial_ids))
        retries = [items for items in by_request.values() if len(items) == 2]
        check("execution_retry_changes_candidate_and_keeps_exact_evidence", bool(retries) and all(
            items[0][0]["route_candidate_id"] != items[1][0]["route_candidate_id"]
            and items[0][1]["config_version_id"] == items[1][1]["config_version_id"]
            for items in retries))
        check("execution_configuration_switches_are_observed", {e["config_version_id"] for items in
            by_request.values() for _, e in items}.issuperset({"local-b", "local-retries", "local-budget"}))
        for request_id in initial_ids:
            row = database.execute("SELECT json_extract(payload_json,'$.usage.response_id') FROM gateway_event_log WHERE event_type='usage' AND request_id=?", (request_id,)).fetchall()
            check("execution_formal_collector_unique_response", len(row) == 1)
            response_id = row[0][0]
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                exec(oracle.REMOTE_COLLECT.replace("RESPONSE_ID", repr(response_id)), {
                    "opts": {"--state-dir": str(context["state"])}, "pathlib": pathlib,
                    "json": json, "datetime": datetime, "hashlib": hashlib,
                    "opaque": oracle.opaque, "runtime": {},
                })
            lineage = json.loads(output.getvalue())["evidence"]
            attempt = lineage["attempts"][0]
            target = {field: attempt[field] for field in oracle.TARGET_FIELDS}
            target.update(capabilities=attempt["capabilities"], declaration_evidence={"client_key_evidence_id": oracle.opaque("local-key")},
                          protocol={"openai_responses": "responses", "openai_chat_completions": "chat", "anthropic_messages": "messages"}[lineage["protocol"]], mode="sse" if lineage["stream"] else "json")
            oracle.verify_attempts(target, lineage, response_id)
            oracle.verify_usage_lineage({"source": lineage["usages"][0]["usage"]}, lineage)
            check("execution_formal_collector_actual_request_attempt_usage_ledger", attempt["execution_evidence_valid"] is True)


def rotations(context):
    api, check, data = (context[name] for name in ("api", "check", "data"))
    relays = [LoopbackSocks(context["provider"].server_port) for _ in range(2)]
    def fork(name):
        draft = api("POST", f"/admin/config-versions/{context['scope']}/fork", {"id": name, "description": "Isolated execution identity rotation"}, expected=201)
        context["scope"], context["revision"] = draft["id"], draft["revision"]
    def node(name, relay):
        return {"id": name, "upstream_id": "upstream-chat", "pool_id": "execution-pool", "name": name, "proxy_endpoint": relay.url, "enabled": True, "weight": 1, "maximum_concurrency": 4}
    try:
        fork("execution-before")
        api("PATCH", "/admin/routes/route-chat", {"id": "route-chat", "policy": "smooth_weighted_round_robin", "max_attempts": 1, "bootstrap_timeout_ms": 20000})
        api("POST", "/admin/compatible-proxy-pools", {"id": "execution-pool", "upstream_id": "upstream-chat", "name": "Isolated relay", "enabled": True}, expected=201)
        api("POST", "/admin/compatible-proxy-nodes", node("execution-node-before", relays[0]), expected=201)
        for name in ("a", "b"):
            api("POST", "/admin/compatible-egress-bindings", {"endpoint_id": "endpoint-chat", "credential_id": "account-chat-" + name, "target_kind": "proxy_pool", "target_id": "execution-pool", "failure_scope": "egress_node", "stickiness": "credential_and_egress", "pre_submit_max_attempts": 1}, expected=201)
        context["publish"]()
        context["restart"]()
        context["held_started"].clear()
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            old = pool.submit(data, "/v1/chat/completions", context["body_for"]("chat", "chat", "identity-switch"))
            check("execution_old_request_reached_actual_proxy_and_upstream", context["held_started"].wait(5) and relays[0].sends == 1)
            fork("execution-after")
            api("DELETE", "/admin/compatible-proxy-nodes/execution-node-before", expected=204)
            api("POST", "/admin/compatible-proxy-nodes", node("execution-node-after", relays[1]), expected=201)
            for name in ("a", "b"):
                context["keys"][name] = "synthetic-rotated-" + context["secrets"].token_hex(24)
                api("PATCH", "/admin/credentials/account-chat-" + name, {"id": "account-chat-" + name, "kind": "bearer", "secret": context["keys"][name], "status": "active"})
            context["publish"]()
            status, raw, _ = data("/v1/chat/completions", context["body_for"]("chat", "chat", "text-round"))
            check("execution_new_request_uses_new_secret_and_real_proxy", status == 200 and b"synthetic answer" in raw and relays[1].sends == 1)
            new_response = json.loads(raw)["id"]
            context["identity_release"].set()
            status, raw, _ = old.result(timeout=5)
            check("execution_old_request_survives_configuration_publication", status == 200 and b"synthetic answer" in raw)
            old_response = json.loads(raw)["id"]
        def observation(database, response):
            row = database.execute("SELECT a.payload_json,e.evidence_json FROM gateway_event_log u JOIN gateway_event_log a ON a.request_id=u.request_id AND a.event_type='attempt' JOIN gateway_attempt_execution e ON e.attempt_id=a.event_id WHERE u.event_type='usage' AND json_extract(u.payload_json,'$.usage.response_id')=?", (response,)).fetchall()
            check("execution_rotation_exact_correlation", len(row) == 1)
            return json.loads(row[0][0])["attempt"], json.loads(row[0][1])
        with sqlite3.connect(context["state"] / "control.sqlite3") as database:
            old_attempt, old_identity = observation(database, old_response)
            new_attempt, new_identity = observation(database, new_response)
            check("execution_refreshed_credential_does_not_rewrite_old_request", old_attempt["credential_id"] == new_attempt["credential_id"] and old_identity["credential_revision"] + 1 == new_identity["credential_revision"])
            check("execution_inflight_configuration_isolation", old_identity["config_version_id"] == "execution-before" and new_identity["config_version_id"] == "execution-after")
            for actual, version, name in ((old_identity, "execution-before", "execution-node-before"), (new_identity, "execution-after", "execution-node-after")):
                check("execution_actual_selected_node_and_version_revision", actual["egress"] == {"kind": "configured_proxy", "target_id": "execution-pool", "node_id": name, "config_version_id": version, "config_revision": actual["config_revision"]})
        pin = api("POST", "/admin/operations/channel-pin", {"provider_id": "upstream-chat", "channel_id": "endpoint-chat", "route_id": "route-chat", "credential_id": new_attempt["credential_id"], "requested_model": "model-chat", "protocol": "openai_chat_completions", "mode": "json", "client_key_id": "local-key", "credential_revision": new_identity["credential_revision"]})
        check("execution_protected_pin_uses_actual_lease", pin["execution"]["identity"] == new_identity and pin["execution"]["route_candidate_id"] == new_attempt["route_candidate_id"] and pin["execution"]["upstream_model"] == "model-chat")
        check("execution_protected_pin_no_secret_or_body", all(value not in json.dumps(pin) for value in (*context["keys"].values(), context["issued_key"], "synthetic answer", relays[1].url)))
    finally:
        context["identity_release"].set()
        for relay in relays:
            relay.close()
