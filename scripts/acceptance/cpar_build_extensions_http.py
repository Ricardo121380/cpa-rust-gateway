"""Directed public-boundary verification using the existing Batch B fixture."""
import copy
import base64
import datetime
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import socketserver
import struct
import subprocess
import sys
import time
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import cpar_build_extensions as extensions
import cpar_new_build_acceptance as oracle


def local_drills(context, observe, private, positive_samples):
    """Owned synthetic operations, separate from the seven absent CLI executors.

    These captures exercise current product boundaries. They do not qualify a
    real 30-day expiry, live fault operation, or unimplemented execution entry.
    """
    api, check = context["api"], context["check"]
    base_url = "http://127.0.0.1:" + str(context["data_port"]) + "/v1"
    key, state = context["issued_key"], context["state"]
    records, raw_records = [], []
    store_sample = next(sample for _, sample in positive_samples if sample["case"] == "store_read" and sample["target"]["mode"] == "json")
    stream_target = next(sample["target"] for _, sample in positive_samples if sample["case"] == "store_read" and sample["target"]["mode"] == "sse")
    compact_sample = next(sample for _, sample in positive_samples if sample["case"] == "compact_roundtrip" and sample["target"]["mode"] == "json")
    root = json.loads(store_sample["wire"])
    compact = json.loads(compact_sample["compact_wire"])

    def exchange(path, body=None, method="POST"):
        extensions.observe_local_process({"binary": str(context["binary"]), "local_state_dir": str(state)}, observe())
        result = oracle.http_exchange(base_url, key, path, body, method=method)
        raw_records.append({"method": method, "path": path, "request": body, **result})
        return result

    def counts():
        return len(context["calls"]), extensions.attempt_count(state)

    def record(case, before, **facts):
        after = counts()
        row = {"case": case, "status": "PASS", "scope": "LOCAL_DRILL_ONLY", "http_receive_count": after[0] - before[0], "attempt_delta": after[1] - before[1], **facts}
        records.append(row)
        return row

    def collect_failure(identifier, expected_request):
        deadline = time.monotonic() + 5
        while True:
            collected = extensions.collect_local(state, identifier, observe())
            if collected["evidence"].get("request_outcome") is not None and collected["evidence"].get("usages") and collected["evidence"].get("ledger"):
                break
            assert time.monotonic() < deadline, "local failed Request collector deadline"
            time.sleep(.05)
        raw_records.append({"failed_collection": collected})
        evidence = collected["evidence"]
        check("extension_drill_exact_failed_attempt", len(evidence["attempts"]) == 1 and evidence["request_outcome"] == expected_request)
        # Transport Attempt success does not imply Request success: persistence,
        # cancellation or stream validation can fail after the upstream attempt.
        oracle.verify_attempts(stream_target, evidence, identifier, request_expected=expected_request)
        oracle.verify_usage_lineage({"source": evidence["usages"][0]["usage"]}, evidence)
        check("extension_drill_failed_usage_source_event_durable", bool(evidence["usages"]) and bool(evidence["ledger"]))
        return evidence

    before = counts()
    process_before = observe()
    context["restart"]()
    process_after = observe()
    read = exchange("/responses/" + root["id"], method="GET")
    check("extension_drill_restart_owner_read_exact", read["http_status"] == 200 and json.loads(read["wire"]) == root and process_before["process_start"] != process_after["process_start"] and process_before["artifact_sha256"] == process_after["artifact_sha256"] and counts() == before)
    record("restart_read", before, binary_unchanged=True, process_changed=True)

    expiry_sample = next(sample for _, sample in positive_samples if sample["case"] == "store_read" and sample["target"]["mode"] == "sse")
    expiry_root = oracle.decode_stream("responses", expiry_sample["wire"].encode())
    before = counts()
    with sqlite3.connect(state / "control.sqlite3") as database:
        created, expires = database.execute("SELECT created_at_ms, expires_at_ms FROM stored_responses WHERE response_id=?", (expiry_root["id"],)).fetchone()
        check("extension_drill_fixed_30_day_policy", expires - created == 30 * 24 * 60 * 60 * 1000)
        database.execute("UPDATE stored_responses SET expires_at_ms=created_at_ms+1 WHERE response_id=?", (expiry_root["id"],))
    context["restart"]()
    denied = exchange("/responses/" + expiry_root["id"], method="GET")
    check("extension_drill_synthetic_expiry_zero_send", denied["http_status"] == 404 and counts() == before)
    record("expired_read", before, fixed_ttl_days=30, synthetic_expiry_only=True, real_30_day_expiry="BLOCKED; authentic aged root required")

    before = counts()
    with sqlite3.connect(state / "control.sqlite3") as database:
        database.execute("CREATE TRIGGER extension_local_store_failure BEFORE INSERT ON stored_responses BEGIN SELECT RAISE(ABORT, 'owned local persistence drill'); END")
    try:
        failure = exchange("/responses", {"model": "model-responses", "input": "text-round", "max_output_tokens": 1024, "stream": True, "store": True})
    finally:
        with sqlite3.connect(state / "control.sqlite3") as database:
            database.execute("DROP TRIGGER extension_local_store_failure")
    frames = oracle.sse_events(failure["wire"].encode())
    created_frame = next(frame for frame in frames if frame.get("type") == "response.created")
    identifier = created_frame["response"]["id"]
    check("extension_drill_persistence_failure_terminal", failure["http_status"] == 200 and sum(frame.get("type") in ("response.failed", "error") for frame in frames) == 1 and not any(frame.get("type") == "response.completed" for frame in frames))
    denied = exchange("/responses/" + identifier, method="GET")
    evidence = collect_failure(identifier, "failed")
    check("extension_drill_persistence_failure_not_stored", denied["http_status"] == 404 and counts()[0] == before[0] + 1 and counts()[1] == before[1] + 1)
    record("persistence_failure", before, failed_terminal=True, stored_read_status=404, usage_records=len(evidence["usages"]), ledger_rows=len(evidence["ledger"]))

    before = counts()
    context["held_started"].clear()
    context["held_closed"].clear()
    ws = context["WebSocket"](key)
    cancellation_frames = []
    try:
        ws.send({"type": "response.create", "model": "model-responses", "input": "held-round", "max_output_tokens": 1024, "stream": True, "store": True})
        check("extension_drill_cancel_upstream_started", context["held_started"].wait(5))
        for _ in range(100):
            opcode, payload = ws.receive()
            assert opcode == 1
            frame = json.loads(payload)
            cancellation_frames.append(frame)
            if frame["type"] == "response.output_text.delta":
                break
        identifier = next(frame["response"]["id"] for frame in cancellation_frames if frame["type"] == "response.created")
        check("extension_drill_cancel_after_semantic_output", cancellation_frames[-1]["type"] == "response.output_text.delta")
    finally:
        ws.close()
    check("extension_drill_cancel_upstream_eof", context["held_closed"].wait(5))
    denied = exchange("/responses/" + identifier, method="GET")
    evidence = collect_failure(identifier, "cancelled")
    raw_records.append({"ws_cancel_frames": cancellation_frames})
    check("extension_drill_cancel_not_stored_no_retry", denied["http_status"] == 404 and counts()[0] == before[0] + 1 and counts()[1] == before[1] + 1)
    record("ws_cancel", before, semantic_output=True, upstream_socket_eof=True, stored_read_status=404, ledger_rows=len(evidence.get("ledger", [])))

    before = counts()
    ws = context["WebSocket"](key)
    try:
        frames = ws.turn({"model": "model-responses", "input": "truncated-round", "max_output_tokens": 1024, "store": True})
        identifier = next(frame["response"]["id"] for frame in frames if frame["type"] == "response.created")
        check("extension_drill_failed_ws_terminal", frames[-1]["type"] in ("error", "response.failed") and not any(frame["type"] == "response.completed" for frame in frames))
        failure_count = counts()
        rejected = ws.turn({"model": "model-responses", "input": "next", "previous_response_id": identifier, "max_output_tokens": 1024})
        check("extension_drill_failed_ws_root_cannot_continue", rejected[-1]["type"] == "error" and counts() == failure_count)
    finally:
        ws.close()
    raw_records.append({"ws_failed_frames": frames, "ws_failed_root_rejection": rejected})
    denied = exchange("/responses/" + identifier, method="GET")
    collect_failure(identifier, "failed")
    check("extension_drill_failed_ws_not_stored_no_retry", denied["http_status"] == 404 and counts()[0] == before[0] + 1 and counts()[1] == before[1] + 1)
    record("ws_failed_root", before, failed_terminal=True, continuation_frames=1, stored_read_status=404)

    # With exactly one credential at concurrency one, a selected candidate after
    # each terminal proves no capacity remains held. Explain is read-only.
    for protocol in ("chat", "responses", "messages"):
        explanation = api("GET", "/admin/routes/route-" + protocol + "/explain?requested_model=model-" + protocol + "&protocol=" + {"chat": "openai_chat_completions", "responses": "openai_responses", "messages": "anthropic_messages"}[protocol])
        check("extension_drill_capacity_released_" + protocol, any(row["candidate_id"] == "candidate-" + protocol and row["decision"] == "selected" for row in explanation["candidates"]))

    before = counts()
    draft = api("POST", "/admin/config-versions/" + context["scope"] + "/fork", {"id": "extension-lineage-local", "description": "Owned synthetic lineage rejection drill"}, expected=201)
    context["scope"], context["revision"] = draft["id"], draft["revision"]
    account = api("GET", "/admin/credentials/account-responses-a")
    api("PATCH", "/admin/credentials/account-responses-a/status", {"status": "disabled", "credential_revision": account["revision"]})
    context["publish"]()
    context["restart"]()
    read = exchange("/responses/" + root["id"], method="GET")
    check("extension_drill_lineage_root_still_owned", read["http_status"] == 200 and json.loads(read["wire"]) == root)
    denied = exchange("/responses", {"model": "model-responses", "input": "next", "previous_response_id": root["id"], "max_output_tokens": 1024, "stream": False})
    check("extension_drill_continuation_lineage_zero_send", denied["http_status"] in (400, 404, 409) and counts() == before)
    record("lineage_rejection", before, disabled_original_account=True)
    denied = exchange("/responses/compact", {"model": "model-responses", "previous_response_id": root["id"], "stream": False})
    rejected = exchange("/responses", {"model": "model-responses", "input": [compact["output"][0], {"role": "user", "content": "next"}], "max_output_tokens": 1024, "stream": False})
    check("extension_drill_compact_lineage_zero_send", denied["http_status"] in (400, 404, 409) and rejected["http_status"] in (400, 404, 409) and counts() == before)
    record("compact_lineage_rejection", before, disabled_original_account=True, summary_and_consume_rejected=True)
    path = private / "local-drills.json"
    path.write_text(json.dumps(raw_records, indent=2) + "\n")
    return {"status": "PASS", "executor_qualification": "NOT_IMPLEMENTED; separate synthetic product drills only", "captures_sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "cases": records}


def observe_runtime(context, source, instance):
    """Read the owned process and current serving declaration for local executors."""
    api, binary = context["api"], context["binary"]
    before = [row for row in api("GET", "/admin/config-versions") if row["status"] == "active"]
    projection = api("GET", "/admin/models/effective?client_key_id=local-key&limit=200")
    declarations = []
    for model in projection["items"]:
        for candidate in model["sources"]:
            evidence = candidate["capability_evidence"]
            declarations.append({**evidence, "client_key_evidence_id": oracle.opaque("local-key"), "projection_id": projection["projection_id"],
                                 "observed_at_utc": datetime.datetime.fromtimestamp(projection["observed_at_ms"] / 1000, datetime.timezone.utc).isoformat(),
                                 "route_id": model["route_id"], "route_candidate_id": candidate["candidate_id"], "endpoint_id": candidate["endpoint_id"], "upstream_model": model["id"]})
    pools = context["all_items"]("/admin/operations/account-pools?limit=100")
    for row in pools:
        row["account_id"] = oracle.opaque(row["account_id"])
    inventory = {
        "routes": api("GET", "/admin/routes"),
        "candidates": context["all_items"]("/admin/route-candidates?limit=200"),
        "account_pools": pools,
        "native_pools": context["all_items"]("/admin/operations/provider-account-pools?limit=100"),
    }
    for row in inventory["native_pools"]:
        row["account_id"] = oracle.opaque(row["account_id"])
    inventory = {key: {"items": value.get("items", []) if isinstance(value, dict) else value, "complete": True, "error": None} for key, value in inventory.items()}
    after = [row for row in api("GET", "/admin/config-versions") if row["status"] == "active"]
    process_start = subprocess.check_output(["ps", "-p", str(context["gateway"].pid), "-o", "lstart="], text=True).strip() + ":" + str(context["gateway"].pid)
    runtime = {"method": "local_subprocess", "instance": instance, "pid": context["gateway"].pid,
               "process_start": process_start, "observed_at_utc": oracle.now(), "stable_process": context["gateway"].poll() is None,
               "artifact_sha": source, "source_sha": source, "artifact_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
               "capability_observations": declarations,
               "acceptance_inventory": {"configuration_stable": before == after, "active_configuration": {key: before[0][key] for key in ("id", "revision")}, "inventory_observed_at_utc": oracle.now(), "inventory": inventory}}
    return runtime


def verify(context):
    api, check = context["api"], context["check"]
    private = context["root"] / "extensions"
    private.mkdir(mode=0o700)
    public = context["out"] / "extensions"
    public.mkdir(mode=0o700)
    binary = context["binary"]
    source = "1b6023f52f4c00ea6d8296524d306c141e4edae7"
    plans, receipts, actual_receives = {}, [], []
    probe = 'import socket\ntry: socket.create_connection(("192.0.2.1",443),timeout=.5)\nexcept PermissionError as error: assert error.errno == 1\nelse: raise AssertionError("non-loopback was not denied")\n'
    subprocess.run(context["cpar_t12_upgrade_http"].sandbox_command([sys.executable, "-c", probe]), check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    check("extension_os_denies_non_loopback_network", True)

    def observe():
        return observe_runtime(context, source, "owned-local-extension-fixture")

    def frozen(protocol, mode):
        key = protocol + "-" + mode
        if key not in plans:
            pin = api("POST", "/admin/operations/channel-pin", {"provider_id": "upstream-" + protocol, "channel_id": "endpoint-" + protocol,
                "route_id": "route-" + protocol, "credential_id": "account-" + protocol + "-a", "requested_model": "model-" + protocol,
                "protocol": {"chat": "openai_chat_completions", "responses": "openai_responses", "messages": "anthropic_messages"}[protocol], "mode": mode,
                "client_key_id": "local-key", "credential_revision": 0})
            runtime = observe()
            plan = {"layer": "LOCAL_SIMULATED", "source_sha": source, "target": {"protocol": protocol, "mode": mode, "public_model": "model-" + protocol,
                    "response_model": "model-" + protocol, "parameters": {"max_output_tokens" if protocol == "responses" else "max_tokens": 1024}},
                    "collector": {"local_state_dir": str(context["state"]), "binary": str(binary)}}
            plans[key] = oracle.freeze_from_pin(plan, runtime, pin)
            (private / (key + "-pin.json")).write_text(json.dumps(pin, indent=2) + "\n")
            check("extension_pin_" + key, pin["outcome"] == "succeeded" and pin["attempt_count"] == 1)
        return copy.deepcopy(plans[key])

    def run(case, protocol="responses", mode="json", *, changes=None, expected_status="PASS", expected_sends=None, actual_key=None, transport_fault=None):
        plan = frozen(protocol, mode)
        plan["extension"] = {"case": case, "prompt": "thinking-round" if case in ("visible_reasoning", "reasoning_history") else "text-round",
                             "next_instruction": "Repeat the exact first answer.", "expected_text": "synthetic answer", "foreign_client_evidence_id": oracle.opaque("sibling-key")}
        if case in ("visible_reasoning", "reasoning_history"):
            plan["target"]["parameters"]["reasoning_effort" if protocol == "chat" else "reasoning"] = "high" if protocol == "chat" else {"effort": "high"}
        if protocol == "chat" and mode == "sse":
            plan["target"]["parameters"]["stream_options"] = {"include_usage": True}
        if changes:
            changes(plan)
        runtime = observe()
        label = str(len(receipts)) + "-" + protocol + "-" + mode + "-" + case
        plan_file, runtime_file = private / (label + "-plan.json"), private / (label + "-runtime.json")
        plan_file.write_text(json.dumps(plan, indent=2) + "\n")
        runtime_file.write_text(json.dumps({"runtime": runtime}, indent=2) + "\n")
        before = len(context["calls"])
        base_url = "http://127.0.0.1:" + str(context["data_port"]) + "/v1"
        proxy, thread, forwarded = None, None, []
        if transport_fault:
            upstream_url = base_url
            class Handler(BaseHTTPRequestHandler):
                def log_message(self, *_):pass
                def do_POST(self):
                    body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                    saved = json.loads((private / (label + "-bundle.json")).read_text())
                    check("extension_cli_prewrite_request_" + label, saved["samples"][-1]["request"] == body)
                    response = oracle.http_exchange(upstream_url, context["issued_key"], self.path[3:], body)
                    if transport_fault.startswith("compact_") and self.path.endswith("/compact"):
                        value = json.loads(response["wire"])
                        if transport_fault == "compact_extra_output":value["output"].append({"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "SYNTHETIC_PLAINTEXT_SUMMARY"}]})
                        elif transport_fault == "compact_item_plaintext":value["output"][0]["summary"] = "SYNTHETIC_PLAINTEXT_SUMMARY"
                        elif transport_fault == "compact_response_plaintext":value["summary"] = "SYNTHETIC_PLAINTEXT_SUMMARY"
                        else:value["incomplete_details"] = {"reason": "max_output_tokens"}
                        response["wire"] = json.dumps(value)
                    forwarded.append(response["wire"])
                    if transport_fault == "post_process":context["restart"]()
                    raw = response["wire"].encode()
                    self.send_response(response["http_status"]);self.send_header("Content-Type", "text/plain" if transport_fault == "content_type" else "application/json")
                    self.send_header("Content-Length", str(len(raw)));self.end_headers();self.wfile.write(raw)
            if transport_fault == "ws_late":
                class SocketHandler(socketserver.BaseRequestHandler):
                    def handle(self):
                        upstream = None
                        def exact(size):
                            raw = bytearray()
                            while len(raw) < size:
                                part = self.request.recv(size - len(raw));assert part
                                raw.extend(part)
                            return bytes(raw)
                        try:
                            self.request.settimeout(10)
                            headers = bytearray()
                            while not headers.endswith(b"\r\n\r\n"):headers.extend(exact(1))
                            nonce = next(line.split(": ", 1)[1] for line in headers.decode().split("\r\n") if line.startswith("Sec-WebSocket-Key:"))
                            accept = base64.b64encode(hashlib.sha1((nonce + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
                            self.request.sendall(("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: " + accept + "\r\n\r\n").encode())
                            head = exact(2);size = head[1] & 0x7f
                            if size == 126:size = struct.unpack("!H", exact(2))[0]
                            elif size == 127:size = struct.unpack("!Q", exact(8))[0]
                            mask, payload = exact(4), exact(size)
                            body = json.loads(bytes(value ^ mask[index % 4] for index, value in enumerate(payload)))
                            assert body.pop("type") == "response.create"
                            upstream = extensions.WebSocket(upstream_url, context["issued_key"])
                            wire = upstream.turn(body)
                            frames = [*oracle.sse_events(wire.encode()), {"type": "response.output_text.delta", "output_index": 0, "content_index": 0, "delta": "LATE"}]
                            forwarded.append("".join("data: " + json.dumps(frame, ensure_ascii=False) + "\n\n" for frame in frames))
                            raw = bytearray()
                            for frame in frames:
                                part = json.dumps(frame, ensure_ascii=False).encode()
                                raw.extend(b"\x81" + (bytes([len(part)]) if len(part) < 126 else b"\x7e" + struct.pack("!H", len(part))) + part)
                            self.request.sendall(raw)
                            close = exact(2)
                            check("extension_ws_late_output_no_next_create", close[0] & 0xf == 8)
                        finally:
                            if upstream is not None:upstream.close()
                proxy = socketserver.ThreadingTCPServer(("127.0.0.1", 0), SocketHandler)
            else:
                proxy = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
            thread = threading.Thread(target=proxy.serve_forever, daemon=True);thread.start()
            base_url = "http://127.0.0.1:" + str(proxy.server_address[1]) + "/v1"
        try:
            result = subprocess.run([sys.executable, str(context["repo"] / "scripts/acceptance/cpar-batch-c-live.py"), "--new-build", "run-extension",
                                "--plan", str(plan_file), "--runtime", str(runtime_file), "--base-url", base_url,
                                "--private-out", str(private / (label + "-bundle.json")), "--out", str(public / (label + ".json")), "--execute"],
                                env={**os.environ, "CPAR_ACCEPTANCE_CLIENT_KEY": actual_key or context["issued_key"], "CPAR_ACCEPTANCE_FOREIGN_KEY": context["sibling_key"]}, capture_output=True, text=True, timeout=90)
        finally:
            if proxy is not None:proxy.shutdown();proxy.server_close();thread.join(2)
        (private / (label + "-cli.log")).write_text(result.stdout + result.stderr)
        receipt = json.loads((public / (label + ".json")).read_text())
        receives = context["calls"][before:]
        actual_receives.append({"case": case, "protocol": protocol, "mode": mode, "http_receive_count": len(receives), "intent_count": receipt["inference_intents"]})
        check("extension_cli_" + label + "_" + expected_status, receipt["status"] == expected_status and receipt["stopped"] is True)
        if expected_sends is not None:
            check("extension_cli_exact_sends_" + label, len(receives) == expected_sends)
        elif expected_status == "PASS":
            check("extension_cli_intents_match_independent_fixture_receives_" + label, len(receives) == receipt["inference_intents"])
        if expected_status == "PASS" and case in ("visible_reasoning", "reasoning_history"):
            for receive in receives:
                body = receive["body"]
                check("extension_effort_preserved_" + label, body.get("reasoning_effort") == "high" if protocol == "chat" else body.get("reasoning") == {"effort": "high"})
            if case == "reasoning_history":
                body = receives[-1]["body"]
                thought = "synthetic visible thought"
                check("extension_reasoning_history_reached_actual_upstream_" + label, thought in json.dumps(body) and "thinking-round" in json.dumps(body))
        if expected_status == "PASS" and case == "compact_roundtrip":
            check("extension_summary_and_consume_separate_" + label, len(receives) == 3 and receives[1]["compact"] is True and receives[1]["body"]["max_output_tokens"] == 2048 and receives[2]["compact"] is False)
        receipts.append(receipt)
        bundle_path = private / (label + "-bundle.json")
        if transport_fault:
            captured = json.loads(bundle_path.read_text())
            compact_failure = transport_fault.startswith("compact_")
            sample = captured["samples"][-1]
            check("extension_cli_failed_transport_raw_retained_" + label, len(captured["samples"]) == (2 if compact_failure else 1) and sample["wire"] == forwarded[-1] and
                  (bool(base64.b64decode(sample["wire_bytes_b64"])) if transport_fault == "ws_late" else base64.b64decode(sample["wire_bytes_b64"]).decode() == forwarded[-1]))
            if compact_failure:
                check("extension_cli_bad_summary_stops_before_consume_" + label, len(forwarded) == receipt["inference_intents"] == 2 and
                      [row["operation"] for row in captured["operations"]] == ["response_create", "compact_summary"])
        if expected_status == "PASS":
            audit_path = public / (label + "-audit.json")
            audit = subprocess.run([sys.executable, str(context["repo"] / "scripts/acceptance/cpar-batch-c-live.py"), "--new-build", "audit", "--bundle", str(bundle_path), "--out", str(audit_path)], capture_output=True, text=True, timeout=30)
            (private / (label + "-audit.log")).write_text(audit.stdout + audit.stderr)
            check("extension_official_cli_audit_" + label, audit.returncode == 0 and json.loads(audit_path.read_text())["status"] == "PASS")
            actual_bundle = json.loads(bundle_path.read_text())
            if case in ("store_read", "foreign_owner", "deleted_read", "compact_foreign_owner", "ws_multiturn"):
                mutations = ("successful_origin",) if case == "ws_multiturn" else ("missing_resource", "resource_status", "resource_wire", "resource_path", "resource_request")
                for mutation in mutations:
                    changed = copy.deepcopy(actual_bundle)
                    if mutation == "successful_origin":
                        changed["handshake"]["ws_origin"] = "https://example.invalid"
                        changed["handshake"]["ws_request"]["origin"] = "https://example.invalid"
                    elif mutation == "missing_resource":
                        changed.pop("resource_operations")
                    elif mutation == "resource_status":
                        changed["resource_operations"][-1]["http_status"] = 200 if case != "store_read" else 404
                    elif mutation == "resource_wire":
                        changed["resource_operations"][-1]["wire_bytes_b64"] = base64.b64encode(b"{}").decode()
                    elif mutation == "resource_path":
                        changed["resource_operations"][-1]["path"] = "/responses/unrelated-root"
                    else:
                        changed["resource_operations"][-1]["request"] = {"unfrozen": True}
                    mutation_path, audit_path = private / (label + "-" + mutation + ".json"), public / (label + "-" + mutation + "-audit.json")
                    mutation_path.write_text(json.dumps(changed) + "\n")
                    audit = subprocess.run([sys.executable, str(context["repo"] / "scripts/acceptance/cpar-batch-c-live.py"), "--new-build", "audit", "--bundle", str(mutation_path), "--out", str(audit_path)], capture_output=True, text=True, timeout=30)
                    check("extension_official_cli_rejects_" + label + "_" + mutation, audit.returncode != 0 and json.loads(audit_path.read_text())["status"] in ("FAIL", "BLOCKED"))
            if case in ("deleted_read", "compact_roundtrip"):
                mutations = ("delete_empty_204", "delete_numeric_true") if case == "deleted_read" else ("compact_extra_output", "compact_item_plaintext", "compact_response_plaintext", "compact_incomplete_terminal")
                for mutation in mutations:
                    changed = copy.deepcopy(actual_bundle)
                    if mutation == "delete_empty_204":
                        changed["resource_operations"][1].update(http_status=204, wire="", wire_bytes_b64="")
                        changed["verified_sample"]["delete_http_status"] = 204
                    elif mutation == "delete_numeric_true":
                        deletion = changed["resource_operations"][1]
                        value = json.loads(deletion["wire"]);value["deleted"] = 1
                        deletion["wire"] = json.dumps(value);deletion["wire_bytes_b64"] = base64.b64encode(deletion["wire"].encode()).decode()
                    else:
                        summary = changed["samples"][1]
                        value = json.loads(summary["wire"])
                        if mutation == "compact_extra_output":value["output"].append({"type": "message", "role": "assistant", "status": "completed", "content": [{"type": "output_text", "text": "SYNTHETIC_PLAINTEXT_SUMMARY", "annotations": []}]})
                        elif mutation == "compact_item_plaintext":value["output"][0]["summary"] = "SYNTHETIC_PLAINTEXT_SUMMARY"
                        elif mutation == "compact_response_plaintext":value["summary"] = "SYNTHETIC_PLAINTEXT_SUMMARY"
                        else:value["incomplete_details"] = {"reason": "max_output_tokens"}
                        summary["wire"] = json.dumps(value)
                        summary["wire_bytes_b64"] = base64.b64encode(summary["wire"].encode()).decode()
                        final = changed["verified_sample"]
                        final["compact_wire"] = summary["wire"]
                        final["request"]["input"] = copy.deepcopy(value["output"]) + [final["request"]["input"][-1]]
                        changed["samples"][-1]["request"] = copy.deepcopy(final["request"])
                        for row in (final, changed["samples"][-1]):
                            row["request_wire_b64"] = base64.b64encode(json.dumps(row["request"]).encode()).decode()
                            row["request_binding_sha256"] = extensions.request_binding(row)
                    mutation_path, audit_path = private / (label + "-" + mutation + ".json"), public / (label + "-" + mutation + "-audit.json")
                    mutation_path.write_text(json.dumps(changed) + "\n")
                    audit = subprocess.run([sys.executable, str(context["repo"] / "scripts/acceptance/cpar-batch-c-live.py"), "--new-build", "audit", "--bundle", str(mutation_path), "--out", str(audit_path)], capture_output=True, text=True, timeout=30)
                    check("extension_official_cli_rejects_" + label + "_" + mutation, audit.returncode != 0 and json.loads(audit_path.read_text())["status"] in ("FAIL", "BLOCKED"))
            if case == "ws_handshake_rejection":
                for mutation in ("missing_handshake", "wrong_handshake_status", "partial_handshake_body", "wrong_handshake_origin", "wrong_handshake_path"):
                    changed = copy.deepcopy(actual_bundle)
                    for row in (changed["samples"][0], changed["verified_sample"]):
                        if mutation == "missing_handshake":row.pop("wire_bytes_b64")
                        elif mutation == "wrong_handshake_origin":row["ws_request"]["origin"] = "https://different.invalid"
                        elif mutation == "wrong_handshake_path":row["ws_path"] = row["ws_request"]["path"] = "/unrelated/responses"
                        else:
                            raw = base64.b64decode(row["wire_bytes_b64"])
                            row["wire_bytes_b64"] = base64.b64encode(raw[:-1] if mutation == "partial_handshake_body" else raw.replace(b"403 Forbidden", b"101 Switching Protocols")).decode()
                    mutation_path, audit_path = private / (label + "-" + mutation + ".json"), public / (label + "-" + mutation + "-audit.json")
                    mutation_path.write_text(json.dumps(changed) + "\n")
                    audit = subprocess.run([sys.executable, str(context["repo"] / "scripts/acceptance/cpar-batch-c-live.py"), "--new-build", "audit", "--bundle", str(mutation_path), "--out", str(audit_path)], capture_output=True, text=True, timeout=30)
                    check("extension_official_cli_rejects_" + mutation, audit.returncode != 0 and json.loads(audit_path.read_text())["status"] in ("FAIL", "BLOCKED"))
            else:
                for mutation in ("missing_capture", "missing_capture_adjusted_count", "unfrozen_control", "unrelated_final", "wrong_root_owner", "wrong_root_terminal"):
                    changed = copy.deepcopy(actual_bundle)
                    if mutation.startswith("missing_capture"):
                        changed["samples"].pop(0)
                        if mutation == "missing_capture_adjusted_count":changed["counts"]["inference_intents"] -= 1
                    elif mutation == "unfrozen_control":
                        changed["samples"][0]["request"]["temperature"] = .9
                        changed["samples"][0]["request_binding_sha256"] = extensions.request_binding(changed["samples"][0])
                    elif mutation == "unrelated_final":
                        changed["verified_sample"]["request"]["input" if protocol == "responses" else "messages"] = "uncaptured input"
                        changed["verified_sample"]["request_binding_sha256"] = extensions.request_binding(changed["verified_sample"])
                    elif mutation == "wrong_root_owner":
                        changed["samples"][0]["evidence"]["client_key_evidence_id"] = oracle.opaque("wrong-owner")
                    elif mode == "json":
                        value = json.loads(changed["samples"][0]["wire"])
                        if protocol == "chat":
                            value["choices"][0]["finish_reason"] = "length"
                        else:
                            value["status"] = "failed"
                        changed["samples"][0]["wire"] = json.dumps(value)
                    else:
                        changed["samples"][0]["wire"] += 'data: {"type":"error"}\n\n'
                    check("extension_bundle_audit_" + label + "_" + mutation, extensions.audit_extension_bundle(changed)["status"] in ("FAIL", "BLOCKED"))
                    if case == "exact_continuation" and mode == "json":
                        mutation_path, audit_path = private / (label + "-" + mutation + ".json"), public / (label + "-" + mutation + "-audit.json")
                        mutation_path.write_text(json.dumps(changed) + "\n")
                        audit = subprocess.run([sys.executable, str(context["repo"] / "scripts/acceptance/cpar-batch-c-live.py"), "--new-build", "audit", "--bundle", str(mutation_path), "--out", str(audit_path)], capture_output=True, text=True, timeout=30)
                        check("extension_official_cli_rejects_" + mutation, audit.returncode != 0 and json.loads(audit_path.read_text())["status"] in ("FAIL", "BLOCKED"))
                if case in ("exact_continuation", "ws_multiturn", "compact_roundtrip"):
                    changed = copy.deepcopy(actual_bundle)
                    changed["verified_sample"]["parent_response_id"] = "uncaptured_root"
                    if case == "compact_roundtrip":
                        changed["samples"][1]["request"]["previous_response_id"] = "uncaptured_root"
                        changed["verified_sample"]["compact_request"]["previous_response_id"] = "uncaptured_root"
                    else:
                        for row in (changed["samples"][-1], changed["verified_sample"]):
                            row["request"]["previous_response_id"] = "uncaptured_root"
                    for row in changed["samples"]:
                        sent = {**row["request"], "type": "response.create"} if "ws_frame" in row else row["request"]
                        row["request_wire_b64"] = base64.b64encode(json.dumps(sent).encode()).decode()
                        row["request_binding_sha256"] = extensions.request_binding(row)
                    changed["verified_sample"]["request_binding_sha256"] = extensions.request_binding(changed["verified_sample"])
                    check("extension_bundle_uncaptured_parent_" + label, extensions.audit_extension_bundle(changed)["status"] in ("FAIL", "BLOCKED"))
        return plan, json.loads(bundle_path.read_text()) if bundle_path.exists() else None

    for protocol in ("chat", "responses"):
        for mode in ("json", "sse"):
            for case in ("visible_reasoning", "reasoning_history"):
                run(case, protocol, mode)
    positive_samples = []
    for mode in ("json", "sse"):
        for case in ("store_read", "exact_continuation", "compact_roundtrip"):
            plan, bundle = run(case, mode=mode)
            positive_samples.append((plan, bundle["verified_sample"]))
    for case in ("foreign_owner", "deleted_read", "compact_foreign_owner"):
        run(case)
    run("ws_multiturn", mode="sse")
    run("ws_handshake_rejection", mode="sse")
    for case in sorted(set(extensions.CASE_CAPABILITY) - extensions.IMPLEMENTED):
        run(case, expected_status="BLOCKED", expected_sends=0)
    run("reasoning_history", "messages", expected_status="BLOCKED", expected_sends=0)
    run("reasoning_history", changes=lambda plan: plan.update(layer="LIVE_NEW_BUILD"), expected_status="BLOCKED", expected_sends=0)
    run("reasoning_history", changes=lambda plan: plan.pop("collector"), expected_status="BLOCKED", expected_sends=0)
    run("reasoning_history", changes=lambda plan: plan["extension"].update(expected_text="WRONG"), expected_status="FAIL", expected_sends=1)
    run("reasoning_history", actual_key=context["sibling_key"], expected_status="FAIL", expected_sends=1)
    for fault in ("content_type", "post_process"):
        run("exact_continuation", expected_status="FAIL", expected_sends=1, transport_fault=fault)
    for fault in ("compact_extra_output", "compact_item_plaintext", "compact_response_plaintext", "compact_incomplete_terminal"):
        run("compact_roundtrip", expected_status="FAIL", expected_sends=2, transport_fault=fault)
    run("ws_multiturn", mode="sse", expected_status="FAIL", expected_sends=1, transport_fault="ws_late")
    run("ws_multiturn", mode="sse", changes=lambda plan: plan["extension"].update(prompt="truncated-round"), expected_status="FAIL", expected_sends=1)
    context["restart"]()
    run("reasoning_history", changes=lambda plan: plan["extension"].update(prompt="always-fail"), expected_status="FAIL", expected_sends=1)
    # Deliberate upstream failure marks runtime health. Reopen only this owned
    # synthetic fixture before independent fault drills; never replay a case.
    context["restart"]()
    # Mutate captured actual samples. These guard the audit boundary in addition
    # to all public CLI executions above, not instead of public execution.
    for plan, sample in positive_samples:
        for kind in ("wrong_owner", "missing_evidence", "wrong_terminal", "changed_control"):
            changed = copy.deepcopy(sample)
            if kind == "wrong_owner":
                changed["evidence"]["client_key_evidence_id"] = oracle.opaque("wrong-owner")
            elif kind == "missing_evidence":
                changed["evidence"]["attempts"] = []
            elif kind == "changed_control":
                changed["request"]["max_output_tokens"] += 1
                changed["request_binding_sha256"] = extensions.request_binding(changed)
            else:
                if plan["target"]["mode"] == "json":
                    value = json.loads(changed["wire"])
                    value["status"] = "failed"
                    changed["wire"] = json.dumps(value)
                else:
                    changed["wire"] += 'data: {"type":"response.failed"}\n\n'
            result = oracle.verdict(lambda: oracle.extension_check(extensions.CASE_CAPABILITY[changed["case"]], changed["case"], changed, plan["target"]))
            check("extension_captured_negative_" + changed["case"] + "_" + plan["target"]["mode"] + "_" + kind, result["status"] in ("FAIL", "BLOCKED"))
    drills = local_drills(context, observe, private, positive_samples)
    report = {"status": "PASS", "layer": "LOCAL_SIMULATED", "real_provider_requests": 0, "source_sha": source,
              "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "formal_tool_sha256": hashlib.sha256(Path(oracle.__file__).read_bytes()).hexdigest(),
              "extension_tool_sha256": hashlib.sha256(Path(extensions.__file__).read_bytes()).hexdigest(), "cli_runs": len(receipts),
              "implemented_cases": sorted(extensions.IMPLEMENTED), "not_implemented_cases": sorted(set(extensions.CASE_CAPABILITY) - extensions.IMPLEMENTED),
              "results": [{key: row.get(key) for key in ("case", "status", "category", "inference_intents", "http_intents", "ws_frame_intents", "private_bundle_sha256")} for row in receipts],
              "independent_fixture_receives": actual_receives, "private_state_evidence_id": oracle.opaque(str(context["root"])),
              "local_drills": drills,
              "candidate_proof": "source-matched local debug build, not a release artifact", "observed_at_utc": oracle.now()}
    report["parallel_false_build_qualification"] = "BLOCKED; generic fixture declares parallel true; native Build remains false and unchanged"
    (context["out"] / "extension-receipt.json").write_text(json.dumps(report, indent=2) + "\n")
