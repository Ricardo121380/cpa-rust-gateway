#!/usr/bin/env python3
"""Value-free, isolated regression tests for the formal C acceptance oracle."""
import importlib.util
import copy
import contextlib
import io
import json
import hashlib
import datetime
import os
from pathlib import Path
import tempfile
import sqlite3
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import unittest


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).parent / "acceptance" / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


legacy = load("legacy", "cpar-batch-c-live.py")
oracle = legacy if os.environ.get("CPAR_TEST_LEGACY") else load("oracle", "cpar_new_build_acceptance.py")
SOURCE_SHA = "2e43ea4bccd7c1253df7d5ceddfbc3f4e4bc61fb"


def response(protocol="responses", tools=False):
    if protocol == "chat":
        message = {"role": "assistant", "content": None if tools else "OK"}
        if tools:
            message["tool_calls"] = [{"id": "call_1", "type": "function", "function": {"name": "lookup", "arguments": '{"value":1}'}}]
        return {"id": "resp_1", "model": "local-model", "choices": [{"index": 0, "message": message, "finish_reason": "tool_calls" if tools else "stop"}], "usage": {"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5}}
    if protocol == "messages":
        content = [{"type": "tool_use", "id": "call_1", "name": "lookup", "input": {"value": 1}}] if tools else [{"type": "text", "text": "OK"}]
        return {"id": "resp_1", "type": "message", "role": "assistant", "model": "local-model", "content": content, "stop_reason": "tool_use" if tools else "end_turn", "usage": {"input_tokens": 3, "output_tokens": 2}}
    output = [{"id": "fc_1", "type": "function_call", "status": "completed", "call_id": "call_1", "name": "lookup", "arguments": '{"value":1}'}] if tools else [{"id": "msg_1", "type": "message", "status": "completed", "role": "assistant", "content": [{"type": "output_text", "text": "OK", "annotations": []}]}]
    return {"id": "resp_1", "model": "local-model", "status": "completed", "output": output, "usage": {"input_tokens": 3, "output_tokens": 2, "total_tokens": 5}}


def frames(events):
    return b"".join(("data: " + json.dumps(event) + "\n\n").encode() for event in events)


def wire_events(protocol, value):
    if protocol == "chat":
        head = {"id": value["id"], "model": value["model"]}
        message = value["choices"][0]["message"]
        events = [{**head, "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": None}]}]
        if message.get("tool_calls"):
            for index, call in enumerate(message["tool_calls"]):
                events.append({**head, "choices": [{"index": 0, "delta": {"tool_calls": [{"index": index, **call}]}, "finish_reason": None}]})
        else:
            events.append({**head, "choices": [{"index": 0, "delta": {"content": message["content"]}, "finish_reason": None}]})
        events.extend([{**head, "choices": [{"index": 0, "delta": {}, "finish_reason": value["choices"][0]["finish_reason"]}]}, {**head, "choices": [], "usage": value["usage"]}])
        return events
    if protocol == "messages":
        events = [{"type": "message_start", "message": {**value, "content": [], "stop_reason": None, "usage": {**value["usage"], "output_tokens": 0}}}]
        for index, block in enumerate(value["content"]):
            start = {**block, "input": {}} if block["type"] == "tool_use" else {**block, "text": ""}
            delta = {"type": "input_json_delta", "partial_json": json.dumps(block["input"])} if block["type"] == "tool_use" else {"type": "text_delta", "text": block["text"]}
            events.extend([{"type": "content_block_start", "index": index, "content_block": start}, {"type": "content_block_delta", "index": index, "delta": delta}, {"type": "content_block_stop", "index": index}])
        events.extend([{"type": "message_delta", "delta": {"stop_reason": value["stop_reason"]}, "usage": value["usage"]}, {"type": "message_stop"}])
        return events
    events = [{"type": "response.created", "response": {**value, "status": "in_progress", "output": []}}]
    for index, item in enumerate(value["output"]):
        start = {**item, "status": "in_progress"}
        if item["type"] == "function_call":
            start["arguments"] = ""
            events.append({"type": "response.output_item.added", "output_index": index, "item": start})
            for part in (item["arguments"][:5], item["arguments"][5:]):
                events.append({"type": "response.function_call_arguments.delta", "output_index": index, "item_id": item["id"], "delta": part})
            events.append({"type": "response.function_call_arguments.done", "output_index": index, "item_id": item["id"], "arguments": item["arguments"]})
        else:
            start["content"] = []
            events.append({"type": "response.output_item.added", "output_index": index, "item": start})
            for part_index, part in enumerate(item["content"]):
                empty = {**part, "text": ""}
                events.extend([{"type": "response.content_part.added", "output_index": index, "content_index": part_index, "part": empty}, {"type": "response.output_text.delta", "output_index": index, "content_index": part_index, "delta": part["text"]}, {"type": "response.output_text.done", "output_index": index, "content_index": part_index, "text": part["text"]}, {"type": "response.content_part.done", "output_index": index, "content_index": part_index, "part": part}])
        events.append({"type": "response.output_item.done", "output_index": index, "item": item})
    events.append({"type": "response.completed", "response": value})
    return events


def wire(protocol, value, mode):
    if mode == "json":
        return json.dumps(value)
    return (frames(wire_events(protocol, value)) + (b"data: [DONE]\n\n" if protocol == "chat" else b"")).decode()


def runtime():
    return {"method": "proc_executable", "instance": "isolated-fixture", "pid": 1, "process_start": "1", "observed_at_utc": oracle.now(), "stable_process": True, "artifact_sha": SOURCE_SHA, "source_sha": SOURCE_SHA, "artifact_sha256": "1" * 64}


def target(protocol="responses", mode="json"):
    return {"channel": "openai-compatible", "endpoint_id": "local-endpoint", "route_id": "local-route", "route_candidate_id": "local-candidate", "upstream_model": "local-model", "public_model": "local-model", "response_model": "local-model", "credential_evidence_id": oracle.opaque("synthetic-account"), "credential_revision": 3, "config_version_id": "local", "config_revision": 4, "egress_evidence_id": oracle.opaque("direct"), "egress_revision": 1, "protocol": protocol, "mode": mode, "parameters": {"max_output_tokens" if protocol == "responses" else "max_tokens": 128}, "capabilities": {key: False for key in oracle.EXTENSIONS}, "declaration_observed_at_utc": oracle.now()}


def evidence(current_target, value, number=1):
    source = {"input_tokens": 3, "output_tokens": 2, "cache_read_tokens": None, "cache_creation_tokens": None, "cached_tokens": None, "reasoning_tokens": None, "provenance": "measured", "input_accounting": "inclusive"}
    value["usage"]["cpar_usage"] = source
    request_id, attempt_id = "request_" + str(number), "attempt_" + str(number)
    attempt = {field: current_target[field] for field in oracle.TARGET_FIELDS}
    attempt.update(request_id=request_id, attempt_id=attempt_id, outcome="success")
    usage = {"request_id": request_id, "attempt_id": attempt_id, "response_id": value["id"], "usage": source}
    return {"correlation": "response_id_usage", "request_id": request_id, "attempts": [attempt], "usages": [usage], "ledger": [copy.deepcopy(usage)]}


def bundle(protocol="responses", mode="json"):
    current_target = target(protocol, mode)
    result = {"layer": "LOCAL_SIMULATED", "source_sha": SOURCE_SHA, "runtime": runtime(), "target": current_target, "scenarios": {}}
    for scenario, count in (("text_multi_turn", 2), ("single_tool", 2), ("two_tool_cycles", 3)):
        request = oracle.make_request(current_target, scenario, "ACCEPT:synthetic")
        samples = []
        for number in range(count):
            tools = scenario != "text_multi_turn" and number < count - 1
            value = response(protocol, tools)
            value["id"] += "_" + scenario + str(number)
            if tools:
                if protocol == "responses":
                    value["output"][0]["call_id"] += str(number)
                elif protocol == "chat":
                    value["choices"][0]["message"]["tool_calls"][0]["id"] += str(number)
                else:
                    value["content"][0]["id"] += str(number)
            else:
                text = "READY" if number < count - 1 else "ACCEPT:synthetic"
                if protocol == "responses":value["output"][0]["content"][0]["text"] = text
                elif protocol == "chat":value["choices"][0]["message"]["content"] = text
                else:value["content"][0]["text"] = text
            lineage = evidence(current_target, value, number)
            sample = {"http_status": 200, "request": copy.deepcopy(request), "wire": wire(protocol, value, mode), "evidence": lineage}
            samples.append(sample)
            if number == count - 1:
                sample["expected_text"] = "ACCEPT:synthetic"
            else:
                sample["next_token"] = "Return exactly ACCEPT:synthetic"
                _, calls = oracle.observe(protocol, value)
                request = oracle.conversation_step(protocol, request, value, calls, sample["next_token"])
        result["scenarios"][scenario] = samples
    return result


class LegacyDefects(unittest.TestCase):
    def test_duplicate_success_terminal_is_rejected(self):
        event = {"type": "response.completed", "response": response()}
        with self.assertRaises(ValueError):
            oracle.decode_stream("responses", frames([*wire_events("responses", response()), event]))

    def test_semantic_output_after_success_is_rejected(self):
        with self.assertRaises(ValueError):
            oracle.decode_stream("responses", frames([*wire_events("responses", response()), {"type": "response.output_text.delta", "output_index": 0, "content_index": 0, "delta": "late"}]))

    def test_stop_with_tool_calls_is_not_success(self):
        value = response("chat", True)
        value["choices"][0]["finish_reason"] = "stop"
        with self.assertRaises(ValueError):
            oracle.observe("chat", value)


@unittest.skipIf(bool(os.environ.get("CPAR_TEST_LEGACY")), "formal mode only")
class FormalOracle(unittest.TestCase):
    def test_repository_source_usage_fields_are_retained(self):
        value = response()
        source = {"input_tokens": 3, "output_tokens": 2, "reasoning_tokens": 1, "cache_read_tokens": None, "cache_creation_tokens": None, "cached_tokens": 1, "provenance": "measured", "input_accounting": "inclusive"}
        value["usage"]["cpar_usage"] = source
        self.assertEqual(oracle.native_usage("responses", value)["source"], source)

    def test_all_six_basic_combinations(self):
        for protocol in oracle.PROTOCOLS:
            for mode in ("json", "sse"):
                with self.subTest(protocol=protocol, mode=mode):
                    receipt = oracle.audit_bundle(bundle(protocol, mode))
                    self.assertEqual(receipt["basic_status"], "PASS", receipt["checks"][:3])
                    self.assertEqual(receipt["status"], "NOT_RUN")

    def test_target_and_revision_fail_closed(self):
        for field in oracle.TARGET_FIELDS:
            value = bundle()
            attempt = value["scenarios"]["text_multi_turn"][0]["evidence"]["attempts"][0]
            attempt[field] = "wrong" if isinstance(attempt[field], str) else 99
            self.assertEqual(oracle.audit_bundle(value)["basic_status"], "FAIL", field)
            attempt[field] = None
            self.assertEqual(oracle.audit_bundle(value)["basic_status"], "BLOCKED", field)

    def test_operator_commit_cannot_attest_runtime(self):
        value = bundle()
        value["runtime"] = {"runtime_commit": SOURCE_SHA}
        self.assertEqual(oracle.audit_bundle(value)["status"], "BLOCKED")
        value["runtime"] = runtime()
        value["runtime"]["artifact_sha"] = "2" * 40
        self.assertEqual(oracle.audit_bundle(value)["status"], "FAIL")

    def test_live_requires_matching_artifact_manifest(self):
        value = bundle()
        value["layer"] = "LIVE_NEW_BUILD"
        self.assertEqual(oracle.audit_bundle(value)["status"], "BLOCKED")
        manifest = {"schema_version": "cpa-rust-gateway-artifact-manifest-v1", "revision": SOURCE_SHA, "target": "aarch64-unknown-linux-gnu", "files": [{"name": "gateway-aarch64-unknown-linux-gnu", "sha256": "1" * 64}]}
        oracle.bind_manifest(value["runtime"], manifest)
        self.assertEqual(oracle.audit_bundle(value)["basic_status"], "PASS")
        manifest["files"][0]["sha256"] = "3" * 64
        oracle.bind_manifest(value["runtime"], manifest)
        self.assertEqual(oracle.audit_bundle(value)["status"], "BLOCKED")

    def test_second_cycle_and_results_are_required(self):
        for protocol in oracle.PROTOCOLS:
            value = bundle(protocol)
            value["scenarios"]["two_tool_cycles"].pop(1)
            self.assertEqual(oracle.audit_bundle(value)["basic_status"], "FAIL")
            value = bundle(protocol)
            last = value["scenarios"]["two_tool_cycles"][-1]["request"]
            field = "input" if protocol == "responses" else "messages"
            last[field].pop()
            self.assertEqual(oracle.audit_bundle(value)["basic_status"], "FAIL")

    def test_tool_argument_id_and_terminal_mutations(self):
        for protocol in oracle.PROTOCOLS:
            value = response(protocol, True)
            if protocol == "responses":value["output"][0]["arguments"] = '{"value":'
            elif protocol == "chat":value["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"] = '{"value":'
            else:value["content"][0]["input"] = []
            with self.assertRaises(ValueError):oracle.observe(protocol, value)
            events = wire_events(protocol, response(protocol, True))
            with self.assertRaises(ValueError):oracle.decode_stream(protocol, frames(events[:-1]))

    def test_usage_missing_unknown_and_misattributed(self):
        value = bundle()
        sample = value["scenarios"]["single_tool"][0]
        response_value = json.loads(sample["wire"])
        response_value.pop("usage")
        sample["wire"] = json.dumps(response_value)
        self.assertEqual(oracle.audit_bundle(value)["basic_status"], "BLOCKED")
        for mutation in ("attempt", "native_count", "ledger_count", "missing_ledger"):
            value = bundle()
            sample = value["scenarios"]["single_tool"][0]
            if mutation == "attempt":sample["evidence"]["usages"][0]["attempt_id"] = "other"
            if mutation == "native_count":
                response_value = json.loads(sample["wire"]);response_value["usage"]["total_tokens"] = 999;sample["wire"] = json.dumps(response_value)
            if mutation == "ledger_count":sample["evidence"]["ledger"][0]["usage"]["input_tokens"] = 999
            if mutation == "missing_ledger":sample["evidence"].pop("ledger")
            self.assertEqual(oracle.audit_bundle(value)["basic_status"], "BLOCKED" if mutation == "missing_ledger" else "FAIL")

    def test_declared_extensions_never_disappear(self):
        value = bundle()
        value["target"]["capabilities"]["stored"] = True
        receipt = oracle.audit_bundle(value)
        rows = [row for row in receipt["checks"] if row.get("extension") == "stored"]
        self.assertEqual(len(rows), 6)
        self.assertTrue(all(row["status"] == "NOT_RUN" and row["applicable"] for row in rows))
        value["target"]["capabilities"]["stored"] = None
        self.assertEqual(oracle.audit_bundle(value)["status"], "BLOCKED")
        value["target"]["capabilities"]["stored"] = False
        value["target"]["capabilities"]["unexpected"] = True
        self.assertEqual(oracle.audit_bundle(value)["status"], "BLOCKED")

    def test_boundaries_count_actual_sends(self):
        current_target = target()
        value = response()
        observation = evidence(current_target, value)
        observation.update(upstream_send_count=1, resource_released=True, observation_closed_at_ms=30, post_terminal_send_count=0)
        sample = {"case": "error", "target": current_target, "events": [{"type": "response.created"}, {"type": "response.failed"}], "evidence": observation}
        oracle.boundary_check(sample, current_target)
        for mutate in ("duplicate_terminal", "fake_success", "resend", "unreleased"):
            changed = copy.deepcopy(sample)
            if mutate == "duplicate_terminal":changed["events"].append({"type": "response.failed"})
            if mutate == "fake_success":changed["events"].append({"type": "response.completed"})
            if mutate == "resend":changed["evidence"]["post_terminal_send_count"] = 1
            if mutate == "unreleased":changed["evidence"]["resource_released"] = False
            with self.assertRaises(ValueError):oracle.boundary_check(changed, current_target)
        sample["case"] = "retry_before_output"
        sample["events"] = [{"type": "response.completed"}]
        sample["max_attempts"] = 2
        sample["evidence"]["first_semantic_at_ms"] = 10
        sample["evidence"]["attempts"][0]["started_at_ms"] = 1
        sample["evidence"]["attempts"].append({**sample["evidence"]["attempts"][0], "attempt_id": "second", "started_at_ms": 4})
        sample["evidence"]["upstream_send_count"] = 2
        oracle.boundary_check(sample, current_target)
        sample["evidence"]["attempts"][1]["started_at_ms"] = 11
        with self.assertRaises(ValueError):oracle.boundary_check(sample, current_target)

    def test_cancel_timeout_and_truncation_boundaries(self):
        current_target = target()
        for case in sorted(oracle.BOUNDARIES - {"error", "retry_before_output"}):
            with self.subTest(case=case):
                lineage = evidence(current_target, response())
                lineage.update(upstream_send_count=1, resource_released=True, observation_closed_at_ms=30, post_terminal_send_count=0, cancel_observed=True, timeout_observed=True, semantic_output_observed=True)
                events = [{"type": "response.created"}]
                if case == "cancel_before_send":
                    lineage.update(attempts=[], upstream_send_count=0)
                    events = []
                sample = {"case": case, "target": current_target, "events": events, "evidence": lineage}
                oracle.boundary_check(sample, current_target)
                if case == "cancel_before_send":sample["events"] = [{"type": "response.created"}]
                elif case.startswith("cancel"):lineage["cancel_observed"] = False
                elif case.startswith("timeout"):lineage["timeout_observed"] = False
                elif case == "no_retry_after_output":lineage["semantic_output_observed"] = False
                else:sample["events"].append({"type": "response.completed"})
                with self.assertRaises(ValueError):oracle.boundary_check(sample, current_target)

    def test_extension_rejections_need_authenticated_exact_operation(self):
        current_target = target()
        sample = {"target": current_target, "http_status": 404, "authenticated": True, "operation": "stored_read", "owner_client_evidence_id": oracle.opaque("owner"), "actual_client_evidence_id": oracle.opaque("other"), "evidence": {"upstream_send_count": 0}}
        oracle.extension_check("stored", "foreign_owner", sample, current_target)
        for field, value in (("http_status", 401), ("authenticated", False), ("actual_client_evidence_id", sample["owner_client_evidence_id"])):
            changed = copy.deepcopy(sample);changed[field] = value
            with self.assertRaises(ValueError):oracle.extension_check("stored", "foreign_owner", changed, current_target)
        changed = copy.deepcopy(sample);changed["evidence"]["upstream_send_count"] = 1
        with self.assertRaises(ValueError):oracle.extension_check("stored", "foreign_owner", changed, current_target)

    def test_no_secrets_or_wire_bodies_in_receipt(self):
        value = bundle()
        receipt = oracle.audit_bundle(value)
        rendered = json.dumps(receipt)
        for token in ("ACCEPT:synthetic", "Return exactly", '"wire"', '"request"', "synthetic-account"):
            self.assertNotIn(token, rendered)

    def test_cli_preflight_retains_layer_and_never_sends(self):
        with tempfile.TemporaryDirectory() as directory:
            plan = {"layer": "LOCAL_SIMULATED", "source_sha": SOURCE_SHA, "target": target()}
            plan_path, runtime_path, out = (Path(directory, name) for name in ("plan.json", "runtime.json", "receipt.json"))
            plan_path.write_text(json.dumps(plan));runtime_path.write_text(json.dumps({"runtime": runtime()}))
            with contextlib.redirect_stdout(io.StringIO()):
                code = oracle.main(["run", "--plan", str(plan_path), "--runtime", str(runtime_path), "--base-url", "http://127.0.0.1:1", "--out", str(out)])
            receipt = json.loads(out.read_text())
            self.assertEqual(code, 2)
            self.assertEqual(receipt["layer"], "LOCAL_SIMULATED")
            self.assertEqual(receipt["sent_requests"], 0)
            self.assertEqual(receipt["category"], "execution_not_requested")

    def test_remote_get_projection_and_process_script(self):
        compile(oracle.REMOTE_READ.replace("SERVICE", repr("gateway.service")).replace("INVENTORY", "True"), "remote", "exec")
        self.assertNotIn("identity'", oracle.REMOTE_READ)

    def test_collector_requires_exact_ledger_source_event(self):
        with tempfile.TemporaryDirectory() as directory:
            database = sqlite3.connect(str(Path(directory, "control.sqlite3")))
            database.executescript("CREATE TABLE gateway_event_log(event_id TEXT,event_type TEXT,request_id TEXT,event_ordinal INTEGER,payload_json TEXT); CREATE TABLE billing_ledger_entries(source_event_id TEXT,request_id TEXT,response_id TEXT,input_tokens INTEGER,output_tokens INTEGER,reasoning_tokens INTEGER,cache_read_tokens INTEGER,cache_creation_tokens INTEGER,cached_tokens INTEGER,usage_evidence_json TEXT);")
            value = response()
            lineage = evidence(target(), value)
            usage, attempt = lineage["usages"][0], lineage["attempts"][0]
            database.execute("INSERT INTO gateway_event_log VALUES(?,?,?,?,?)", ("source-a", "usage", usage["request_id"], 1, json.dumps({"usage": usage})))
            database.execute("INSERT INTO gateway_event_log VALUES(?,?,?,?,?)", ("attempt-a", "attempt", usage["request_id"], 2, json.dumps({"attempt": attempt})))
            database.execute("INSERT INTO billing_ledger_entries VALUES(?,?,?,?,?,?,?,?,?,?)", ("wrong-source", usage["request_id"], value["id"], 3, 2, None, None, None, None, json.dumps({"provenance": "measured", "input_accounting": "inclusive"})))
            database.commit()
            def collect():
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    exec(oracle.REMOTE_COLLECT.replace("RESPONSE_ID", repr(value["id"])), {"opts": {"--state-dir": directory}, "pathlib": __import__("pathlib"), "json": json, "datetime": datetime, "hashlib": hashlib, "opaque": oracle.opaque, "runtime": runtime()})
                return json.loads(output.getvalue())["evidence"]
            self.assertIsNone(collect()["ledger"][0]["attempt_id"])
            database.execute("UPDATE billing_ledger_entries SET source_event_id='source-a'")
            database.commit()
            self.assertEqual(collect()["ledger"][0]["attempt_id"], attempt["attempt_id"])
            database.execute("ALTER TABLE billing_ledger_entries DROP COLUMN usage_evidence_json")
            database.commit()
            self.assertIsNone(collect()["ledger"][0]["usage"]["provenance"])
            database.close()

    def test_real_http_runner_and_current_receipts(self):
        for protocol in oracle.PROTOCOLS:
            for mode in ("json", "sse"):
                with self.subTest(protocol=protocol, mode=mode), tempfile.TemporaryDirectory() as directory:
                    current_target = target(protocol, mode)
                    observed = []
                    fault = [None]
                    class Handler(BaseHTTPRequestHandler):
                        def log_message(self, *_):pass
                        def do_POST(self):
                            request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                            observed.append(request)
                            choice = request.get("tool_choice")
                            tools = choice == "required" or choice == {"type": "any"}
                            value = response(protocol, tools)
                            value["id"] = "loopback-response-" + str(len(observed))
                            if tools:
                                if protocol == "responses":value["output"][0]["call_id"] = "call-" + str(len(observed))
                                elif protocol == "chat":value["choices"][0]["message"]["tool_calls"][0]["id"] = "call-" + str(len(observed))
                                else:value["content"][0]["id"] = "call-" + str(len(observed))
                            else:
                                content = json.dumps(request)
                                import re
                                tokens = re.findall("ACCEPT:[0-9a-f]+", content)
                                text = tokens[-1] if len(observed) not in (1,) else "READY"
                                if protocol == "responses":value["output"][0]["content"][0]["text"] = text
                                elif protocol == "chat":value["choices"][0]["message"]["content"] = text
                                else:value["content"][0]["text"] = text
                            lineage = evidence(current_target, value, len(observed))
                            if fault[0] == "wrong_target":lineage["attempts"][0]["endpoint_id"] = "other-endpoint"
                            if fault[0] == "missing_revision":lineage["attempts"][0]["credential_revision"] = None
                            Path(directory, oracle.opaque(value["id"]) + ".json").write_text(json.dumps(lineage))
                            raw = wire(protocol, value, mode).encode()
                            self.send_response(503 if fault[0] == "http_error" else 200)
                            self.send_header("Content-Type", "text/event-stream" if mode == "sse" else "application/json")
                            self.send_header("Content-Length", str(len(raw)))
                            self.end_headers()
                            for start in range(0, len(raw), 7):self.wfile.write(raw[start:start+7])
                    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
                    thread = threading.Thread(target=server.serve_forever, daemon=True);thread.start()
                    try:
                        plan = {"layer": "LOCAL_SIMULATED", "source_sha": SOURCE_SHA, "target": current_target, "observation_dir": directory}
                        result = oracle.run_basic(plan, runtime(), "synthetic-key", "http://127.0.0.1:" + str(server.server_port))
                        self.assertEqual(len(observed), 7)
                        self.assertEqual(oracle.audit_bundle(result)["basic_status"], "PASS")
                        if os.environ.get("CPAR_TEST_RECEIPT_DIR"):
                            oracle.write_receipt(Path(os.environ["CPAR_TEST_RECEIPT_DIR"]) / (protocol + "-" + mode + ".json"), oracle.audit_bundle(result))
                        for kind, status in (("wrong_target", "FAIL"), ("missing_revision", "BLOCKED"), ("http_error", "FAIL")):
                            fault[0] = kind
                            before = len(observed)
                            rejected = oracle.run_basic(plan, runtime(), "synthetic-key", "http://127.0.0.1:" + str(server.server_port))
                            self.assertEqual(len(observed) - before, 1, kind)
                            self.assertEqual(oracle.audit_bundle(rejected)["status"], status, rejected.get("execution_failure"))
                    finally:
                        server.shutdown();server.server_close();thread.join(2)

    def test_http_absolute_deadline_and_redirect_no_retry(self):
        observed = []
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):pass
            def do_POST(self):
                observed.append(self.path)
                self.rfile.read(int(self.headers["Content-Length"]))
                self.send_response(302 if len(observed) == 1 else 200)
                self.send_header("Content-Type", "application/json")
                if len(observed) == 1:
                    self.send_header("Location", "/other")
                    self.send_header("Content-Length", "2")
                    self.end_headers();self.wfile.write(b"{}")
                else:
                    self.send_header("Content-Length", "1000")
                    self.end_headers()
                    try:
                        for _ in range(50):
                            self.wfile.write(b" ");self.wfile.flush();time.sleep(.02)
                    except (BrokenPipeError, ConnectionResetError):pass
        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True);thread.start()
        try:
            base = "http://127.0.0.1:" + str(server.server_port)
            request = oracle.make_request(target(), "text_multi_turn", "synthetic")
            self.assertEqual(oracle.http_call(base, "synthetic-key", "responses", request)["http_status"], 302)
            self.assertEqual(len(observed), 1)
            started = time.monotonic()
            with self.assertRaises(TimeoutError):oracle.http_call(base, "synthetic-key", "responses", request, timeout=.12)
            self.assertLess(time.monotonic() - started, .7)
            self.assertEqual(len(observed), 2)
        finally:
            server.shutdown();server.server_close();thread.join(2)


if __name__ == "__main__":
    unittest.main(verbosity=2)
