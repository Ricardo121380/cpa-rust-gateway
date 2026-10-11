#!/usr/bin/env python3
"""Value-free, isolated regression tests for the formal C acceptance oracle."""
import importlib.util
import base64
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
import socketserver
import struct
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import unittest
from unittest.mock import patch


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).parent / "acceptance" / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


legacy = load("legacy", "cpar-batch-c-live.py")
oracle = legacy if os.environ.get("CPAR_TEST_LEGACY") else load("oracle", "cpar_new_build_acceptance.py")
SOURCE_SHA = "2e43ea4bccd7c1253df7d5ceddfbc3f4e4bc61fb"
sys.path.insert(0, str(Path(__file__).parent / "acceptance"))
import cpar_build_extensions as extensions


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
    account = oracle.opaque("synthetic-account")
    inventory = {name: {"complete": True, "error": None, "items": items} for name, items in {
        "routes": [{"id": "local-route", "max_attempts": 1}],
        "candidates": [{"id": "local-candidate", "route_id": "local-route", "endpoint_id": "local-endpoint", "upstream_model": "local-model", "enabled": True, "credential_scope": "all_active"}],
        "account_pools": [{"channel_id": "local-endpoint", "account_id": account, "configured_enabled": True, "binding_enabled": True, "account_status": "active", "account_revision": 3}],
        "native_pools": [],
    }.items()}
    return {"method": "proc_executable", "instance": "isolated-fixture", "pid": 1, "process_start": "1", "observed_at_utc": oracle.now(), "stable_process": True, "artifact_sha": SOURCE_SHA, "source_sha": SOURCE_SHA, "artifact_sha256": "1" * 64, "capability_observations": [target()["declaration_evidence"]], "acceptance_inventory": {"configuration_stable": True, "active_configuration": {"id": "local", "revision": 4}, "inventory_observed_at_utc": oracle.now(), "inventory": inventory}}


def target(protocol="responses", mode="json"):
    result = {"channel": "openai-compatible", "endpoint_id": "local-endpoint", "route_id": "local-route", "route_candidate_id": "local-candidate", "upstream_model": "local-model", "public_model": "local-model", "response_model": "local-model", "credential_evidence_id": oracle.opaque("synthetic-account"), "credential_revision": 3, "config_version_id": "local", "config_revision": 4, "egress_evidence_id": oracle.opaque("direct"), "egress_revision": {"source": "not_applicable"}, "protocol": protocol, "mode": mode, "parameters": {"max_output_tokens" if protocol == "responses" else "max_tokens": 128}, "capabilities": {key: False for key in oracle.EXTENSIONS}, "declaration_observed_at_utc": oracle.now()}
    result["declaration_evidence"] = {"source": "serving_snapshot", "projection_id": "1" * 64, "observed_at_utc": oracle.now(), "client_key_evidence_id": oracle.opaque("synthetic-client"), **{field: result[field] for field in ("config_version_id", "config_revision", "route_id", "route_candidate_id", "endpoint_id", "upstream_model", "capabilities")}}
    return result


def evidence(current_target, value, number=1):
    source = {"input_tokens": 3, "output_tokens": 2, "cache_read_tokens": None, "cache_creation_tokens": None, "cached_tokens": None, "reasoning_tokens": None, "provenance": "measured", "input_accounting": "exclusive" if current_target["protocol"] == "messages" else "inclusive"}
    value["usage"]["cpar_usage"] = source
    request_id, attempt_id = "request_" + str(number), "attempt_" + str(number)
    attempt = {field: current_target[field] for field in oracle.TARGET_FIELDS}
    attempt.update(request_id=request_id, attempt_id=attempt_id, outcome="success", capabilities=current_target["capabilities"])
    usage = {"request_id": request_id, "attempt_id": attempt_id, "response_id": value["id"], "usage": source}
    return {"correlation": "response_id_usage", "request_id": request_id, "client_key_evidence_id": current_target["declaration_evidence"]["client_key_evidence_id"], "protocol": {"responses": "openai_responses", "chat": "openai_chat_completions", "messages": "anthropic_messages"}[current_target["protocol"]], "stream": current_target["mode"] == "sse", "request_outcome": "succeeded", "attempts": [attempt], "usages": [usage], "ledger": [copy.deepcopy(usage)]}


def bundle(protocol="responses", mode="json", version=3):
    current_target = target(protocol, mode)
    result = {"basic_scenario_version": version, "layer": "LOCAL_SIMULATED", "source_sha": SOURCE_SHA, "runtime": runtime(), "target": current_target, "scenarios": {}}
    for scenario, count in oracle.BASIC_SCENARIOS if version == 3 else oracle.HISTORICAL_BASIC_SCENARIOS:
        request = oracle.make_request(current_target, scenario, "ACCEPT:synthetic")
        expected_text = "ACCEPT:synthetic" + (":1" if version == 3 and scenario != "text_multi_turn" else "")
        samples = []
        for number in range(count):
            tools = scenario != "text_multi_turn" and number < count - 1 and not (version == 3 and scenario == "two_tool_cycles" and number == 0)
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
                text = "READY" if number < count - 1 else expected_text
                if protocol == "responses":value["output"][0]["content"][0]["text"] = text
                elif protocol == "chat":value["choices"][0]["message"]["content"] = text
                else:value["content"][0]["text"] = text
            lineage = evidence(current_target, value, number)
            sample = {"http_status": 200, "request": copy.deepcopy(request), "wire": wire(protocol, value, mode), "evidence": lineage, "runtime": runtime()}
            samples.append(sample)
            if number == count - 1:
                sample["expected_text"] = expected_text
            else:
                sample["next_token"] = "Return exactly ACCEPT:synthetic" if version != 3 else "Repeat the remembered token exactly." if scenario == "text_multi_turn" else "Call lookup with value 1 again." if number < count - 2 else "Return exactly ACCEPT:synthetic: followed by the lookup result, with no other text."
                _, calls = oracle.observe(protocol, value)
                if version == 2 and calls:
                    # Original v2 wire shape, independent of the current history builder.
                    request = legacy.followup(protocol, request, value, calls)
                    if protocol == "responses":
                        for item in request["input"][-len(calls):]:item["output"] = sample["next_token"]
                    elif protocol == "chat":
                        for item in request["messages"][-len(calls):]:item["content"] = sample["next_token"]
                    else:
                        for item in request["messages"][-1]["content"]:item["content"] = sample["next_token"]
                else:
                    request = oracle.conversation_step(protocol, request, value, calls, sample["next_token"])
        result["scenarios"][scenario] = samples
    return result


def reviewed_authorization(plan, observed, base):
    return {"status": "APPROVED", "basis": "user_explicit", "binding_sha256": oracle.execution_binding(plan, observed, base), "max_inference_requests": 9, "max_attempts": 1, "allow_fallback": False, "currency": "USD", "budget_limit": 1, "cost_bound": {"status": "PROVEN", "evidence_sha256": "4" * 64, "covers": ["input", "visible_output", "reasoning_output", "tool_output", "cache", "request_fees"], "per_request_upper": .01}}


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
    def test_responses_nullable_error_accepts_completed_json_and_sse(self):
        for mode in ("json", "sse"):
            current = bundle("responses", mode)
            for samples in current["scenarios"].values():
                for sample in samples:
                    value = oracle.decode_stream("responses", sample["wire"].encode()) if mode == "sse" else json.loads(sample["wire"])
                    value.update(error=None, incomplete_details=None)
                    sample["wire"] = wire("responses", value, mode)
            with self.subTest(mode=mode):
                self.assertEqual(oracle.audit_bundle(current)["basic_status"], "PASS")

    def test_responses_nonnull_error_and_failed_terminal_never_pass(self):
        for error in ({"code": "provider_error"}, {}, False, 0, ""):
            value = response()
            value["error"] = error
            with self.subTest(error=error), self.assertRaisesRegex(ValueError, "response_error"):
                oracle.observe("responses", value)
        for status in ("failed", "incomplete", "in_progress"):
            value = response()
            value.update(status=status, error=None)
            with self.subTest(status=status), self.assertRaises(ValueError):
                oracle.observe("responses", value)
        for protocol in ("chat", "messages"):
            value = response(protocol)
            value["error"] = None
            with self.subTest(protocol=protocol), self.assertRaisesRegex(ValueError, "response_error"):
                oracle.observe(protocol, value)

    def test_execution_failure_controls_the_basic_batch_verdict(self):
        for status in ("FAIL", "BLOCKED"):
            current = bundle()
            current["scenarios"] = {"text_multi_turn": current["scenarios"]["text_multi_turn"][:1]}
            current["execution_failure"] = {"status": status, "category": "response_error"}
            receipt = oracle.audit_bundle(current)
            with self.subTest(status=status):
                self.assertEqual(receipt["status"], status)
                self.assertEqual(receipt["basic_status"], status)
                self.assertEqual(receipt["sent_requests"], 1)
                self.assertEqual(next(row for row in receipt["checks"] if row["case"] == "text_multi_turn")["status"], "NOT_RUN")

    def test_freeze_uses_actual_lease_and_scoped_effective_declaration(self):
        original = target()
        plan = {"layer": "LOCAL_SIMULATED", "source_sha": SOURCE_SHA, "target": copy.deepcopy(original)}
        for field in (*oracle.TARGET_FIELDS, "declaration_evidence", "declaration_observed_at_utc"):
            plan["target"][field] = None
        plan["target"]["capabilities"] = {name: None for name in oracle.EXTENSIONS}
        pin = {"outcome": "succeeded", "attempt_count": 1, "upstream_sent": True,
               "runtime_build": SOURCE_SHA + ":1.97.1:aarch64-unknown-linux-gnu", "observed_at_ms": int(time.time() * 1000),
               "config_version_id": "local", "config_revision": 4, "requested_model": "local-model",
               "protocol": "openai_responses", "mode": "json", "channel_id": "local-endpoint", "route_id": "local-route",
               "credential_id": "synthetic-account", "credential_revision": 99, "client_key_id": "synthetic-client",
               "execution": {"attempt_id": "request-attempt-1", "route_candidate_id": "local-candidate", "upstream_model": "local-model",
                             "identity": {"channel": "openai-compatible", "credential_revision": 3, "config_version_id": "local", "config_revision": 4,
                                          "egress": {"kind": "direct"}, "capabilities": original["capabilities"]}}}
        frozen = oracle.freeze_from_pin(plan, runtime(), pin)
        self.assertEqual(frozen["target"]["credential_revision"], 3)
        self.assertEqual(frozen["target"]["egress_revision"], {"source": "not_applicable"})
        self.assertEqual(frozen["target"]["declaration_evidence"]["source"], "serving_snapshot")
        self.assertIsNone(plan["target"]["credential_revision"])
        live = {"layer": "LIVE_NEW_BUILD", "source_sha": SOURCE_SHA, "target": copy.deepcopy(original)}
        observed = runtime()
        observed.update(manifest_sha256="8" * 64, manifest_matches=True)
        base = "http://127.0.0.1:1"
        live["authorization"] = reviewed_authorization(live, observed, base)
        fresh_time = (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(seconds=1)).isoformat()
        observed["capability_observations"][0]["observed_at_utc"] = fresh_time
        refreshed = oracle.freeze_from_pin(live, observed, pin)
        with self.assertRaisesRegex(ValueError, "live_authorization_binding_changed"):
            oracle.verify_execution_policy(refreshed, observed, base)
        refreshed["authorization"]["binding_sha256"] = oracle.execution_binding(refreshed, observed, base)
        oracle.verify_execution_policy(refreshed, observed, base)
        for field in ("projection_id", "client_key_evidence_id"):
            changed_runtime, changed_pin = copy.deepcopy(observed), copy.deepcopy(pin)
            changed_runtime["capability_observations"][0][field] = "9" * 64
            if field == "client_key_evidence_id":
                changed_pin["client_key_id"] = "unrelated-client"
                changed_runtime["capability_observations"][0][field] = oracle.opaque("unrelated-client")
            with self.subTest(changed_scope=field), self.assertRaisesRegex(ValueError, "pin_declaration_changed"):
                oracle.freeze_from_pin(live, changed_runtime, changed_pin)
        for mutation in ("legacy", "unknown_revision", "wrong_access", "wrong_configuration", "wrong_capabilities", "wrong_egress"):
            changed = copy.deepcopy(pin)
            actual = changed["execution"]["identity"]
            if mutation == "legacy":changed.pop("execution")
            if mutation == "unknown_revision":actual["credential_revision"] = None
            if mutation == "wrong_access":changed["client_key_id"] = "unrelated-client"
            if mutation == "wrong_configuration":actual["config_revision"] = 98
            if mutation == "wrong_capabilities":actual["capabilities"]["parallel"] = True
            if mutation == "wrong_egress":actual["egress"] = {"kind": "configured_proxy", "target_id": "pool", "node_id": "node", "config_version_id": "local", "config_revision": 98}
            with self.subTest(mutation=mutation), self.assertRaises((oracle.Blocked, ValueError)):
                oracle.freeze_from_pin(plan, runtime(), changed)

    def test_repository_source_usage_fields_are_retained(self):
        value = response()
        source = {"input_tokens": 3, "output_tokens": 2, "reasoning_tokens": 1, "cache_read_tokens": None, "cache_creation_tokens": None, "cached_tokens": 1, "provenance": "measured", "input_accounting": "inclusive"}
        value["usage"]["cpar_usage"] = source
        value["usage"].update(input_tokens_details={"cached_tokens": 1}, output_tokens_details={"reasoning_tokens": 1})
        self.assertEqual(oracle.native_usage("responses", value)["source"], source)

    def test_native_input_and_details_must_match_source_accounting(self):
        for protocol in oracle.PROTOCOLS:
            value = bundle(protocol)
            sample = value["scenarios"]["single_tool"][0]
            decoded = json.loads(sample["wire"])
            decoded["usage"]["prompt_tokens" if protocol == "chat" else "input_tokens"] = 999
            if "total_tokens" in decoded["usage"]:decoded["usage"]["total_tokens"] = 1001
            sample["wire"] = json.dumps(decoded)
            self.assertEqual(oracle.audit_bundle(value)["basic_status"], "FAIL", protocol)

    def test_usage_accounting_conversion_and_detail_negatives(self):
        for protocol in oracle.PROTOCOLS:
            value = response(protocol)
            source = {"input_tokens": 3 if protocol != "messages" else 6, "output_tokens": 2, "reasoning_tokens": 1, "cache_read_tokens": 1, "cache_creation_tokens": 2, "cached_tokens": 3, "provenance": "measured", "input_accounting": "exclusive" if protocol != "messages" else "inclusive"}
            value["usage"]["cpar_usage"] = source
            field = "prompt_tokens" if protocol == "chat" else "input_tokens"
            value["usage"][field] = 6 if protocol != "messages" else 3
            if protocol == "messages":value["usage"].update(cache_read_input_tokens=1, cache_creation_input_tokens=2)
            else:
                value["usage"].update(total_tokens=8)
                value["usage"]["prompt_tokens_details" if protocol == "chat" else "input_tokens_details"] = {"cached_tokens": 3}
                value["usage"]["completion_tokens_details" if protocol == "chat" else "output_tokens_details"] = {"reasoning_tokens": 1}
            oracle.native_usage(protocol, value)
            changed = copy.deepcopy(value)
            if protocol == "messages":changed["usage"]["cache_read_input_tokens"] = 999
            else:changed["usage"]["completion_tokens_details" if protocol == "chat" else "output_tokens_details"]["reasoning_tokens"] = 999
            with self.assertRaises(ValueError):oracle.native_usage(protocol, changed)
            changed = copy.deepcopy(value)
            changed["usage"]["cpar_usage"]["cached_tokens" if protocol == "messages" else "cache_read_tokens"] = None
            with self.assertRaises(oracle.Blocked):oracle.native_usage(protocol, changed)

    def test_public_failure_projection_excludes_private_input(self):
        value = bundle()
        value["target"]["credential_evidence_id"] = "synthetic.private@example.invalid"
        rendered = json.dumps(oracle.audit_bundle(value))
        self.assertNotIn("synthetic.private@example.invalid", rendered)
        value = bundle()
        value["execution_failure"] = {"status": "FAIL", "category": "synthetic_failure", "wire": "PRIVATE_BODY", "secret": "PRIVATE_KEY"}
        rendered = json.dumps(oracle.audit_bundle(value))
        self.assertNotIn("PRIVATE_BODY", rendered)
        self.assertNotIn("PRIVATE_KEY", rendered)

    def test_all_six_basic_combinations(self):
        for protocol in oracle.PROTOCOLS:
            for mode in ("json", "sse"):
                with self.subTest(protocol=protocol, mode=mode):
                    receipt = oracle.audit_bundle(bundle(protocol, mode))
                    self.assertEqual(receipt["basic_status"], "PASS", receipt["checks"][:3])
                    self.assertEqual(receipt["status"], "NOT_RUN")

    def test_historical_seven_request_bundles_keep_original_scope(self):
        for protocol in oracle.PROTOCOLS:
            for mode in ("json", "sse"):
                with self.subTest(protocol=protocol, mode=mode), tempfile.TemporaryDirectory() as directory:
                    value = bundle(protocol, mode, version=2)
                    value.pop("basic_scenario_version")
                    source, out = (Path(directory, name) for name in ("historical.json", "receipt.json"))
                    source.write_text(json.dumps(value))
                    with contextlib.redirect_stdout(io.StringIO()):oracle.main(["audit", "--bundle", str(source), "--out", str(out)])
                    receipt = json.loads(out.read_text())
                    self.assertEqual(receipt["basic_status"], "PASS")
                    self.assertEqual(receipt["basic_scenario_version"], 2)
                    self.assertEqual(receipt["sent_requests"], 7)
                    value["basic_scenario_version"] = 3
                    self.assertEqual(oracle.audit_bundle(value)["basic_status"], "FAIL")

    def test_stale_observation_cannot_erase_sent_requests(self):
        for version, sent in ((2, 7), (3, 9)):
            value = bundle(version=version)
            value["runtime"]["observed_at_utc"] = (datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(minutes=11)).isoformat()
            receipt = oracle.audit_bundle(value)
            self.assertEqual(receipt["status"], "BLOCKED")
            self.assertEqual(receipt["preflight"]["category"], "runtime_observation_stale")
            self.assertEqual(receipt["sent_requests"], sent)

    def test_reviewed_live_policy_and_native_pool_admission(self):
        observed, plan, base = runtime(), {"layer": "LIVE_NEW_BUILD", "source_sha": SOURCE_SHA, "target": target()}, "http://127.0.0.1:1"
        plan["authorization"] = reviewed_authorization(plan, observed, base)
        oracle.verify_execution_policy(plan, observed, base)
        with self.assertRaises(ValueError):oracle.verify_execution_policy(plan, observed, base + "/changed")
        inventory = observed["acceptance_inventory"]["inventory"]
        inventory["account_pools"]["items"] = []
        inventory["native_pools"]["items"] = [{"channel_id": "local-endpoint", "account_id": plan["target"]["credential_evidence_id"], "enabled": True, "auth_status": "active", "runtime_status": "available", "credential_revision": 3}]
        oracle.verify_execution_policy(plan, observed, base)
        inventory["native_pools"]["items"][0]["credential_revision"] = 4
        with self.assertRaises(ValueError):oracle.verify_execution_policy(plan, observed, base)

    def test_unlimited_cost_requires_explicit_reviewed_authorization(self):
        observed, plan, base = runtime(), {"layer": "LIVE_NEW_BUILD", "source_sha": SOURCE_SHA, "target": target()}, "http://127.0.0.1:1"
        approval = reviewed_authorization(plan, observed, base)
        plan["authorization"] = approval
        approval.update(cost_policy="UNLIMITED_USER_EXPLICIT", cost_authorization_evidence_sha256="6" * 64, budget_limit=None, cost_bound={"status": "NOT_PROVEN"})
        oracle.verify_execution_policy(plan, observed, base)
        for mutation in ({"cost_policy": "BOUNDED"}, {"cost_policy": "UNKNOWN"}, {"cost_authorization_evidence_sha256": None}, {"budget_limit": 1}, {"basis": None}, {"binding_sha256": "7" * 64}, {"max_inference_requests": 10}, {"allow_fallback": True}):
            with self.subTest(mutation=mutation):
                invalid = copy.deepcopy(plan)
                invalid["authorization"].update(mutation)
                with self.assertRaises(ValueError):oracle.verify_execution_policy(invalid, observed, base)

    def test_management_revision_tag_matches_only_the_exact_frozen_configuration(self):
        observed, plan, base = runtime(), {"layer": "LIVE_NEW_BUILD", "source_sha": SOURCE_SHA, "target": target()}, "http://127.0.0.1:1"
        plan["authorization"] = reviewed_authorization(plan, observed, base)
        active = observed["acceptance_inventory"]["active_configuration"]
        active["revision"] = "rev-4"
        oracle.verify_execution_policy(plan, observed, base)
        for revision in ("rev-5", '"rev-4"', "rev-04", "4", 4.0, None):
            active["revision"] = revision
            with self.subTest(revision=revision), self.assertRaisesRegex(ValueError, "execution_configuration_changed"):
                oracle.verify_execution_policy(plan, observed, base)
        active.update(id="other", revision="rev-4")
        with self.assertRaisesRegex(ValueError, "execution_configuration_changed"):
            oracle.verify_execution_policy(plan, observed, base)
        active["id"] = "local"
        observed["acceptance_inventory"]["inventory"]["account_pools"]["items"][0]["account_revision"] = "rev-3"
        with self.assertRaisesRegex(ValueError, "execution_managed_account_unavailable"):
            oracle.verify_execution_policy(plan, observed, base)

    def test_boundary_request_terminal_is_separate_from_transport_attempt(self):
        current = bundle()
        evidence = current["scenarios"]["text_multi_turn"][0]["evidence"]
        value = json.loads(current["scenarios"]["text_multi_turn"][0]["wire"])
        for outcome in ("failed", "cancelled"):
            evidence["request_outcome"] = outcome
            with self.assertRaisesRegex(ValueError, "wrong_request_outcome"):
                oracle.verify_attempts(current["target"], evidence, value["id"])
            oracle.verify_attempts(current["target"], evidence, value["id"], request_expected=outcome)
            with self.assertRaisesRegex(ValueError, "unreviewed_request_outcome"):
                oracle.verify_attempts(current["target"], evidence, value["id"], request_expected="unknown")

    def test_explicit_reasoning_and_stream_controls_are_frozen(self):
        current = target("chat", "sse")
        controls = {"max_tokens": 128, "reasoning_effort": "high", "stream_options": {"include_usage": True}}
        current["parameters"] = controls
        oracle.verify_target(current)
        request = oracle.make_request(current, "text_multi_turn", "acceptance-token")
        self.assertTrue(all(request[key] == value for key, value in controls.items()))
        for value in ({"include_usage": False}, {"include_usage": 1}, {"include_usage": True, "unexpected": True}, None):
            with self.assertRaisesRegex(ValueError, "invalid_stream_usage_control"):
                oracle.validate_parameters({"stream_options": value})
        current["mode"] = "json"
        with self.assertRaisesRegex(ValueError, "stream_usage_control_not_applicable"):
            oracle.verify_target(current)
        current = target("responses")
        current["parameters"]["reasoning_effort"] = "high"
        with self.assertRaisesRegex(ValueError, "reasoning_effort_not_applicable"):
            oracle.verify_target(current)
        current["parameters"].pop("reasoning_effort")
        current["parameters"]["reasoning"] = {"effort": "high"}
        self.assertEqual(oracle.make_request(current, "text_multi_turn", "acceptance-token")["reasoning"], {"effort": "high"})

    def test_target_and_revision_fail_closed(self):
        for field in oracle.TARGET_FIELDS:
            value = bundle()
            attempt = value["scenarios"]["text_multi_turn"][0]["evidence"]["attempts"][0]
            attempt[field] = "wrong" if isinstance(attempt[field], str) else 99
            self.assertEqual(oracle.audit_bundle(value)["basic_status"], "FAIL", field)
            attempt[field] = None
            self.assertEqual(oracle.audit_bundle(value)["basic_status"], "BLOCKED", field)

    def test_execution_revision_sources_do_not_substitute_for_lease(self):
        for value in (None, "rev-3", True):
            changed = bundle()
            changed["target"]["credential_revision"] = value
            self.assertEqual(oracle.audit_bundle(changed)["status"], "BLOCKED")
        changed = bundle()
        changed["target"]["egress_revision"] = {"source": "config_version", "config_version_id": "other", "revision": 4}
        self.assertEqual(oracle.audit_bundle(changed)["status"], "FAIL")

    def test_unknown_and_contradictory_capabilities_stop_before_any_send(self):
        for kind, status in (("missing", "BLOCKED"), ("null", "BLOCKED"), ("wrong_revision", "FAIL"), ("wrong_declaration", "FAIL"), ("stale", "BLOCKED")):
            current = runtime()
            if kind == "missing":current.pop("capability_observations")
            if kind == "null":current["capability_observations"][0]["capabilities"] = None
            if kind == "wrong_revision":current["capability_observations"][0]["config_revision"] += 1
            if kind == "wrong_declaration":current["capability_observations"][0]["capabilities"]["reasoning"] = True
            if kind == "stale":current["capability_observations"][0]["observed_at_utc"] = "2020-01-01T00:00:00+00:00"
            current_plan = {"layer": "LOCAL_SIMULATED", "source_sha": SOURCE_SHA, "target": target()}
            sent = []
            saved = oracle.http_call
            oracle.http_call = lambda *_: sent.append(True)
            try:
                result = oracle.run_basic(current_plan, current, "synthetic", "http://127.0.0.1:1")
            finally:
                oracle.http_call = saved
            self.assertEqual(sent, [], kind)
            self.assertEqual(oracle.audit_bundle(result)["status"], status, kind)

    def test_protocol_applicability_keeps_gateway_declaration(self):
        changed = bundle("chat")
        changed["target"]["capabilities"]["stored"] = True
        receipt = oracle.audit_bundle(changed)
        self.assertIs(receipt["capabilities"]["stored"], True)
        row = next(row for row in receipt["checks"] if row.get("extension") == "stored")
        self.assertFalse(row["applicable"])
        self.assertTrue(row["declared"])
        self.assertEqual(row["category"], "protocol_not_applicable")

    def test_request_terminal_access_and_execution_binding_must_match(self):
        for field, replacement, expected in (("request_outcome", "failed", "FAIL"), ("request_outcome", None, "BLOCKED"), ("protocol", "anthropic_messages", "FAIL"), ("stream", True, "FAIL"), ("client_key_evidence_id", "f" * 64, "FAIL")):
            changed = bundle()
            changed["scenarios"]["text_multi_turn"][0]["evidence"][field] = replacement
            self.assertEqual(oracle.audit_bundle(changed)["basic_status"], expected, field)
        changed = bundle()
        changed["scenarios"]["text_multi_turn"][0]["evidence"]["attempts"][0]["execution_evidence_valid"] = False
        self.assertEqual(oracle.audit_bundle(changed)["basic_status"], "FAIL")

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
        sample = {"target": current_target, "http_status": 404, "authenticated": True, "operation": "stored_read", "read_response_id": "owned-response", "owner_read_http_status": 200, "owner_read_wire": json.dumps({"id": "owned-response"}), "owner_client_evidence_id": oracle.opaque("owner"), "actual_client_evidence_id": oracle.opaque("other"), "evidence": {"upstream_send_count": 0}}
        oracle.extension_check("stored", "foreign_owner", sample, current_target)
        for field, value in (("http_status", 401), ("authenticated", False), ("actual_client_evidence_id", sample["owner_client_evidence_id"])):
            changed = copy.deepcopy(sample);changed[field] = value
            with self.assertRaises(ValueError):oracle.extension_check("stored", "foreign_owner", changed, current_target)
        changed = copy.deepcopy(sample);changed["evidence"]["upstream_send_count"] = 1
        with self.assertRaises(ValueError):oracle.extension_check("stored", "foreign_owner", changed, current_target)
        for field, value in (("operation", "unrelated_endpoint"), ("read_response_id", "unrelated-response")):
            changed = copy.deepcopy(sample);changed[field] = value
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

    def test_collector_reads_only_bound_execution_sidecar_and_legacy_stays_unknown(self):
        with tempfile.TemporaryDirectory() as directory:
            database = sqlite3.connect(str(Path(directory, "control.sqlite3")))
            database.executescript("CREATE TABLE gateway_event_log(event_id TEXT,event_type TEXT,request_id TEXT,event_ordinal INTEGER,payload_json TEXT); CREATE TABLE gateway_attempt_execution(attempt_id TEXT,request_id TEXT,attempt_payload_sha256 TEXT,evidence_json TEXT); CREATE TABLE billing_ledger_entries(source_event_id TEXT,request_id TEXT,response_id TEXT);")
            value = response()
            lineage = evidence(target(), value)
            usage, attempt = lineage["usages"][0], lineage["attempts"][0]
            payload = json.dumps({"attempt": attempt})
            for event_id, kind, body, ordinal in (("request_1", "request", {"client_key_id": "synthetic-client", "protocol": "openai_responses", "streaming": False}, 1), (attempt["attempt_id"], "attempt", attempt, 2), ("source-a", "usage", usage, 3), ("request_1", "request_finished", {"outcome": "succeeded"}, 4)):
                database.execute("INSERT INTO gateway_event_log VALUES(?,?,?,?,?)", (event_id, kind, usage["request_id"], ordinal, json.dumps({kind: body})))
            identity = {"channel": "openai-compatible", "credential_revision": 3, "config_version_id": "local", "config_revision": 4, "egress": {"kind": "direct"}, "capabilities": {name: False for name in ("reasoning", "parallel", "stored", "continuation", "compact", "websocket")}}
            def collect():
                database.commit()
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    exec(oracle.REMOTE_COLLECT.replace("RESPONSE_ID", repr(value["id"])), {"opts": {"--state-dir": directory}, "pathlib": __import__("pathlib"), "json": json, "datetime": datetime, "hashlib": hashlib, "opaque": oracle.opaque, "runtime": runtime()})
                return json.loads(output.getvalue())["evidence"]
            legacy_attempt = collect()["attempts"][0]
            self.assertIsNone(legacy_attempt["credential_revision"])
            self.assertIsNone(legacy_attempt["capabilities"])
            canonical = json.dumps(identity, separators=(",", ":"), ensure_ascii=False)
            database.execute("INSERT INTO gateway_attempt_execution VALUES(?,?,?,?)", (attempt["attempt_id"], usage["request_id"], hashlib.sha256(payload.encode()).hexdigest(), canonical))
            actual = collect()
            self.assertIs(actual["attempts"][0]["execution_evidence_valid"], True)
            self.assertEqual(actual["attempts"][0]["credential_revision"], 3)
            self.assertEqual(actual["attempts"][0]["egress_revision"], {"source": "not_applicable"})
            self.assertIs(actual["stream"], False)
            self.assertEqual(actual["request_outcome"], "succeeded")
            duplicated = json.dumps(identity).replace('"credential_revision": 3', '"credential_revision": 99, "credential_revision": 3')
            database.execute("UPDATE gateway_attempt_execution SET evidence_json=?", (duplicated,))
            self.assertIs(collect()["attempts"][0]["execution_evidence_valid"], False)
            for invalid_json in (json.dumps(identity), canonical.replace('"credential_revision":3', '"credential_revision":18446744073709551616')):
                database.execute("UPDATE gateway_attempt_execution SET evidence_json=?", (invalid_json,))
                self.assertIs(collect()["attempts"][0]["execution_evidence_valid"], False)
            database.execute("UPDATE gateway_attempt_execution SET evidence_json=?", (canonical,))
            database.execute("UPDATE gateway_attempt_execution SET attempt_payload_sha256=?", ("0" * 64,))
            self.assertIs(collect()["attempts"][0]["execution_evidence_valid"], False)
            database.execute("UPDATE gateway_attempt_execution SET attempt_payload_sha256=?", (hashlib.sha256(payload.encode()).hexdigest(),))
            invalid = {**identity, "egress": {"kind": "configured_proxy", "target_id": "pool", "node_id": "node", "config_version_id": "local", "config_revision": 99}}
            database.execute("UPDATE gateway_attempt_execution SET evidence_json=?", (json.dumps(invalid),))
            self.assertIs(collect()["attempts"][0]["execution_evidence_valid"], False)
            invalid = {**identity, "raw_body": "PRIVATE_BODY", "secret": "PRIVATE_KEY"}
            database.execute("UPDATE gateway_attempt_execution SET evidence_json=?", (json.dumps(invalid),))
            rendered = json.dumps(collect())
            self.assertNotIn("PRIVATE_BODY", rendered)
            self.assertNotIn("PRIVATE_KEY", rendered)
            self.assertIs(collect()["attempts"][0]["execution_evidence_valid"], False)
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
                            history = request.get("input" if protocol == "responses" else "messages", [])
                            if not isinstance(history, list):history = []
                            if protocol == "responses":
                                tool_outputs = [item["output"] for item in history if item.get("type") == "function_call_output"]
                            elif protocol == "chat":
                                tool_outputs = [item["content"] for item in history if item.get("role") == "tool"]
                            else:
                                tool_outputs = [block["content"] for item in history for block in item.get("content", []) if isinstance(block, dict) and block.get("type") == "tool_result"]
                            user_content = history[-1].get("content", "") if history and history[-1].get("role") == "user" else ""
                            instruction = "".join(block.get("text", "") for block in user_content if isinstance(block, dict) and block.get("type") == "text") if isinstance(user_content, list) else user_content
                            import re
                            replay = re.fullmatch(r"Return exactly (ACCEPT:[0-9a-f]+): followed by the lookup result, with no other text\.", instruction)
                            valid_tool_history = not tool_outputs or (all(output == "1" for output in tool_outputs) and (instruction == "Call lookup with value 1 again." if tools else replay is not None))
                            value = response(protocol, tools)
                            value["id"] = "loopback-response-" + str(len(observed))
                            if tools:
                                if protocol == "responses":value["output"][0]["call_id"] = "call-" + str(len(observed))
                                elif protocol == "chat":value["choices"][0]["message"]["tool_calls"][0]["id"] = "call-" + str(len(observed))
                                else:value["content"][0]["id"] = "call-" + str(len(observed))
                            else:
                                content = json.dumps(request)
                                tokens = re.findall("ACCEPT:[0-9a-f]+", content)
                                text = (replay[1] + ":" + ("2" if fault[0] == "wrong_tool_result" else tool_outputs[-1])) if replay and tool_outputs else (tokens[-1] if tokens and len(observed) != 1 else "READY")
                                if not valid_tool_history:text = "BROKEN_TOOL_RESULT_OR_INSTRUCTION"
                                if protocol == "responses":value["output"][0]["content"][0]["text"] = text
                                elif protocol == "chat":value["choices"][0]["message"]["content"] = text
                                else:value["content"][0]["text"] = text
                            lineage = evidence(current_target, value, len(observed))
                            if fault[0] == "wrong_target":lineage["attempts"][0]["endpoint_id"] = "other-endpoint"
                            if fault[0] == "missing_revision":lineage["attempts"][0]["credential_revision"] = None
                            if fault[0] == "missing_capabilities":lineage["attempts"][0]["capabilities"] = None
                            if fault[0] == "wrong_capabilities":lineage["attempts"][0]["capabilities"] = {**current_target["capabilities"], "parallel": True}
                            current_runtime = runtime()
                            if fault[0] == "wrong_model":value["model"] = "other-model"
                            if fault[0] == "wrong_runtime":current_runtime["artifact_sha"] = "2" * 40
                            if fault[0] == "missing_runtime":current_runtime = None
                            Path(directory, oracle.opaque(value["id"]) + ".json").write_text(json.dumps({**lineage, "runtime": current_runtime}))
                            if fault[0] == "sparse_empty_metadata":
                                for item in value["output"]:
                                    for part in item.get("content", []):
                                        part.update(annotations=[], logprobs=[])
                                events = wire_events(protocol, value)
                                for event in events:
                                    if event["type"] == "response.content_part.added":
                                        event["part"].pop("annotations", None)
                                        event["part"].pop("logprobs", None)
                                raw = ("".join("data: " + json.dumps(event) + "\n\n" for event in events)).encode()
                            else:
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
                        self.assertEqual(len(observed), 9)
                        self.assertEqual(oracle.audit_bundle(result)["basic_status"], "PASS")
                        if protocol == "responses" and mode == "sse":
                            fault[0] = "sparse_empty_metadata"
                            observed.clear()
                            enriched = oracle.run_basic(plan, runtime(), "synthetic-key", "http://127.0.0.1:" + str(server.server_port))
                            self.assertEqual(len(observed), 9)
                            self.assertEqual(oracle.audit_bundle(enriched)["basic_status"], "PASS")
                            fault[0] = None
                        if os.environ.get("CPAR_TEST_RECEIPT_DIR"):
                            oracle.write_receipt(Path(os.environ["CPAR_TEST_RECEIPT_DIR"]) / (protocol + "-" + mode + ".json"), oracle.audit_bundle(result))
                        live = copy.deepcopy(plan)
                        live["layer"] = "LIVE_NEW_BUILD"
                        live_runtime = runtime()
                        live_runtime.update(manifest_sha256="3" * 64, manifest_matches=True)
                        base = "http://127.0.0.1:" + str(server.server_port)
                        live["authorization"] = reviewed_authorization(live, live_runtime, base)
                        live["authorization"].update(cost_policy="UNLIMITED_USER_EXPLICIT", cost_authorization_evidence_sha256="6" * 64, budget_limit=None, cost_bound={"status": "NOT_PROVEN"})
                        observed.clear()
                        unlimited = oracle.run_basic(live, live_runtime, "synthetic-key", base)
                        self.assertEqual(len(observed), 9)
                        self.assertEqual(oracle.audit_bundle(unlimited)["basic_status"], "PASS")
                        fault[0] = "wrong_tool_result"
                        observed.clear()
                        rejected = oracle.run_basic(plan, runtime(), "synthetic-key", base)
                        self.assertEqual(len(observed), 5)
                        self.assertEqual(rejected["execution_failure"], {"status": "FAIL", "category": "history_or_result_not_used"})
                        self.assertEqual(oracle.audit_bundle(rejected)["status"], "FAIL")
                        for kind, status in (("wrong_target", "FAIL"), ("missing_revision", "BLOCKED"), ("http_error", "FAIL"), ("wrong_model", "FAIL"), ("wrong_runtime", "FAIL"), ("missing_runtime", "BLOCKED"), ("missing_capabilities", "BLOCKED"), ("wrong_capabilities", "FAIL")):
                            with self.subTest(fault=kind):
                                fault[0] = kind
                                before = len(observed)
                                rejected = oracle.run_basic(plan, runtime(), "synthetic-key", "http://127.0.0.1:" + str(server.server_port))
                                self.assertEqual(len(observed) - before, 1, kind)
                                self.assertEqual(oracle.audit_bundle(rejected)["status"], status, rejected.get("execution_failure"))
                        fault[0] = None
                        for kind in ("retry", "fallback", "unknown_candidate", "second_account", "missing_inventory", "incomplete", "configuration_changed", "live_approval_missing", "cost_unknown", "cost_tool_output_missing", "budget_insufficient", "approval_binding_changed", "cost_policy_unknown", "cost_unlimited_evidence_missing", "cost_unlimited_budget_conflict"):
                            with self.subTest(preflight=kind):
                                current_runtime, guarded = runtime(), copy.deepcopy(plan)
                                inventory = current_runtime["acceptance_inventory"]["inventory"]
                                if kind == "retry":inventory["routes"]["items"][0]["max_attempts"] = 2
                                if kind == "fallback":inventory["candidates"]["items"].append({**inventory["candidates"]["items"][0], "id": "fallback"})
                                if kind == "unknown_candidate":inventory["candidates"]["items"][0]["enabled"] = None
                                if kind == "second_account":inventory["account_pools"]["items"].append({**inventory["account_pools"]["items"][0], "account_id": oracle.opaque("second")})
                                if kind == "missing_inventory":current_runtime.pop("acceptance_inventory")
                                if kind == "incomplete":inventory["candidates"]["complete"] = False
                                if kind == "configuration_changed":current_runtime["acceptance_inventory"]["active_configuration"]["revision"] = 99
                                if kind.startswith(("live_", "cost_", "budget_", "approval_")):
                                    guarded["layer"] = "LIVE_NEW_BUILD"
                                    current_runtime.update(manifest_sha256="3" * 64, manifest_matches=True)
                                    guarded["authorization"] = reviewed_authorization(guarded, current_runtime, "http://127.0.0.1:" + str(server.server_port))
                                    if kind == "live_approval_missing":guarded.pop("authorization")
                                    if kind == "cost_unknown":guarded["authorization"]["cost_bound"]["status"] = "NOT_PROVEN"
                                    if kind == "cost_tool_output_missing":guarded["authorization"]["cost_bound"]["covers"].remove("tool_output")
                                    if kind == "budget_insufficient":guarded["authorization"]["budget_limit"] = .01
                                    if kind == "approval_binding_changed":guarded["authorization"]["binding_sha256"] = "5" * 64
                                    if kind == "cost_policy_unknown":guarded["authorization"]["cost_policy"] = "UNKNOWN"
                                    if kind.startswith("cost_unlimited_"):
                                        guarded["authorization"].update(cost_policy="UNLIMITED_USER_EXPLICIT", cost_authorization_evidence_sha256="6" * 64, budget_limit=None, cost_bound={"status": "NOT_PROVEN"})
                                        if kind == "cost_unlimited_evidence_missing":guarded["authorization"].pop("cost_authorization_evidence_sha256")
                                        if kind == "cost_unlimited_budget_conflict":guarded["authorization"]["budget_limit"] = 1
                                before = len(observed)
                                rejected = oracle.run_basic(guarded, current_runtime, "synthetic-key", "http://127.0.0.1:" + str(server.server_port))
                                self.assertEqual(len(observed), before, kind)
                                self.assertIn(oracle.audit_bundle(rejected)["status"], ("FAIL", "BLOCKED"))
                    finally:
                        server.shutdown();server.server_close();thread.join(2)

    def test_responses_sparse_empty_metadata_preserves_strict_snapshots(self):
        for logprobs in ([], None):
            value = response("responses")
            value["output"][0]["content"][0].update(logprobs=logprobs)
            events = wire_events("responses", value)
            for event in events:
                if event["type"] == "response.content_part.added":
                    event["part"].pop("annotations")
                    event["part"].pop("logprobs")
            self.assertEqual(oracle.decode_responses(events), value)
            for field, replacement in (("annotations", [{"type": "url_citation"}]), ("logprobs", [{}]), ("text", "CHANGED"), ("extra", [])):
                changed = copy.deepcopy(events)
                next(event for event in changed if event["type"] == "response.content_part.done")["part"][field] = replacement
                with self.subTest(field=field), self.assertRaisesRegex(ValueError, "changed_part_snapshot"):
                    oracle.decode_responses(changed)
            changed = copy.deepcopy(events)
            done = next(event for event in changed if event["type"] == "response.output_item.done")
            done["item"] = copy.deepcopy(done["item"])
            done["item"]["content"][0].pop("logprobs")
            with self.assertRaisesRegex(ValueError, "changed_part_snapshot"):
                oracle.decode_responses(changed)

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

    def test_extension_receipt_rejects_private_values(self):
        for counts in ({"inference_intents": {"wire": "SYNTHETIC_PRIVATE_SENTINEL"}}, [], {"inference_intents": True}):
            receipt = extensions.audit_extension_bundle({"layer": {"wire": "SYNTHETIC_PRIVATE_SENTINEL"}, "case": [], "counts": counts})
            self.assertEqual(receipt["status"], "FAIL")
            self.assertNotIn("SYNTHETIC_PRIVATE_SENTINEL", json.dumps(receipt))
        with tempfile.TemporaryDirectory() as directory:
            receipt = extensions.run_extension({"layer": {"wire": "SYNTHETIC_PRIVATE_SENTINEL"}, "extension": {"case": []}}, {}, None, "http://127.0.0.1:1", Path(directory) / "bundle")
            self.assertEqual(receipt["status"], "FAIL")
            self.assertNotIn("SYNTHETIC_PRIVATE_SENTINEL", json.dumps(receipt))

    def test_extension_ordered_roots_and_full_controls(self):
        current_target, observed = target(), runtime()
        current_target["capabilities"]["continuation"] = True
        captures = []
        for number in (1, 2):
            value = response();value["id"] = "resp_" + str(number)
            lineage = evidence(current_target, value, number)
            request = {"model": "local-model", "stream": False, "max_output_tokens": 128, "input": "OK"}
            request.update({"store": True} if number == 1 else {"previous_response_id": "resp_1"})
            raw = json.dumps(value)
            sample = {"extension_sample_version": 1, "case": "exact_continuation", "layer": "LOCAL_SIMULATED", "source_sha": SOURCE_SHA, "target": current_target, "frozen_runtime": observed, "runtime": observed,
                      "request": request, "request_wire_b64": base64.b64encode(json.dumps(request).encode()).decode(), "wire": raw, "wire_bytes_b64": base64.b64encode(raw.encode()).decode(), "http_status": 200, "operation": "response_create", "expected_text": "OK", "evidence": lineage}
            sample["request_binding_sha256"] = extensions.request_binding(sample);captures.append(sample)
        final = copy.deepcopy(captures[1]);final.update(parent_target=current_target, parent_response_id="resp_1")
        original = {"extension_bundle_version": 1, "layer": "LOCAL_SIMULATED", "case": "exact_continuation", "source_sha": SOURCE_SHA, "runtime": observed, "target": current_target, "samples": captures,
                    "verified_sample": final, "counts": {"inference_intents": 2}, "result": {"status": "PASS", "stopped": True}}
        self.assertEqual(extensions.audit_extension_bundle(original)["status"], "PASS")
        for mutation in ("missing_root", "uncaptured_parent", "extra_control", "unrelated_final", "missing_wire", "wrong_root_case"):
            changed = copy.deepcopy(original)
            if mutation == "missing_root":
                changed["samples"].pop(0);changed["counts"]["inference_intents"] = 1
            elif mutation == "uncaptured_parent":
                for row in (changed["samples"][-1], changed["verified_sample"]):
                    row["request"]["previous_response_id"] = "unknown_root"
                    row["request_binding_sha256"] = extensions.request_binding(row)
                    row["request_wire_b64"] = base64.b64encode(json.dumps(row["request"]).encode()).decode()
                changed["verified_sample"]["parent_response_id"] = "unknown_root"
            elif mutation == "extra_control":
                changed["samples"][0]["request"]["temperature"] = .9
                changed["samples"][0]["request_binding_sha256"] = extensions.request_binding(changed["samples"][0])
                changed["samples"][0]["request_wire_b64"] = base64.b64encode(json.dumps(changed["samples"][0]["request"]).encode()).decode()
            elif mutation == "unrelated_final":
                changed["verified_sample"]["request"]["input"] = "uncaptured_input"
                changed["verified_sample"]["request_binding_sha256"] = extensions.request_binding(changed["verified_sample"])
            elif mutation == "missing_wire":
                changed["samples"][0].pop("wire_bytes_b64")
            else:
                changed["samples"][0]["case"] = "store_read"
            self.assertIn(extensions.audit_extension_bundle(changed)["status"], ("FAIL", "BLOCKED"), mutation)

    def test_extension_http_failures_preserve_sent_request_and_partial_wire(self):
        # Only the process observer is stubbed in this transport regression.
        # The directed fixture separately verifies the actual gateway and CLI.
        current_target, observed = target(), runtime()
        current_target["capabilities"]["continuation"] = True
        observed["capability_observations"] = [current_target["declaration_evidence"]]
        for fault in ("wrong_content_type", "identity_after_response", "body_deadline"):
            with tempfile.TemporaryDirectory() as directory:
                path, received, before_write = Path(directory) / "bundle.json", [], []
                class Handler(BaseHTTPRequestHandler):
                    def log_message(self, *_):pass
                    def do_POST(self):
                        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                        received.append(body)
                        before_write.append(json.loads(path.read_text())["samples"][0]["request"] == body)
                        raw = json.dumps(response()).encode()
                        self.send_response(200);self.send_header("Content-Type", "text/plain" if fault == "wrong_content_type" else "application/json")
                        self.send_header("Content-Length", str(len(raw)));self.end_headers()
                        try:
                            self.wfile.write(raw[:3]);self.wfile.flush()
                            if fault == "body_deadline":time.sleep(.2)
                            self.wfile.write(raw[3:]);self.wfile.flush()
                        except (BrokenPipeError, ConnectionResetError):pass
                server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
                thread = threading.Thread(target=server.serve_forever, daemon=True);thread.start()
                plan = {"layer": "LOCAL_SIMULATED", "source_sha": SOURCE_SHA, "target": current_target, "collector": {"local_state_dir": directory, "binary": "synthetic-observer-only"}, "extension": {"case": "exact_continuation", "prompt": "OK", "expected_text": "OK"}}
                exchange = extensions.oracle.http_exchange
                try:
                    with patch.object(extensions, "observe_local_process", side_effect=[observed, ValueError("local_process_identity_changed")] if fault == "identity_after_response" else None, return_value=observed), \
                         patch.object(extensions.oracle, "http_exchange", side_effect=lambda *args, **kwargs: exchange(*args, timeout=.07, **kwargs)):
                        receipt = extensions.run_extension(plan, observed, "synthetic-key", "http://127.0.0.1:" + str(server.server_port), path, execute=True)
                    retained = json.loads(path.read_text())["samples"]
                    self.assertEqual((receipt["status"], receipt["inference_intents"], len(received)), ("FAIL", 1, 1))
                    self.assertEqual(before_write, [True]);self.assertEqual(len(retained), 1)
                    self.assertEqual(retained[0]["request"], received[0])
                    raw = base64.b64decode(retained[0]["wire_bytes_b64"])
                    self.assertTrue(raw.startswith(b'{"i'))
                    if fault == "body_deadline":self.assertEqual(raw, b'{"i')
                    self.assertEqual(path.stat().st_mode & 0o777, 0o600)
                finally:
                    server.shutdown();server.server_close();thread.join(2)

    def test_extension_websocket_partial_frame_is_retained(self):
        captures = []
        class Handler(socketserver.BaseRequestHandler):
            def handle(self):
                headers = bytearray()
                while not headers.endswith(b"\r\n\r\n"):headers.extend(self.request.recv(1))
                nonce = next(line.split(": ", 1)[1] for line in headers.decode().split("\r\n") if line.startswith("Sec-WebSocket-Key:"))
                accept = base64.b64encode(hashlib.sha1((nonce + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
                self.request.sendall(("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: " + accept + "\r\n\r\n").encode())
                self.request.recv(4096)
                self.request.sendall(b'\x81\x10{"type":')
        server = socketserver.TCPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True);thread.start()
        ws = extensions.WebSocket("http://127.0.0.1:" + str(server.server_address[1]), "synthetic-key")
        try:
            with self.assertRaisesRegex(ValueError, "ws_turn_closed"):
                ws.turn({"model": "local-model", "input": "OK", "stream": True}, on_progress=lambda metadata, raw: captures.append(raw))
            self.assertEqual(captures[-1], b'\x81\x10{"type":')
        finally:
            ws.close();server.shutdown();server.server_close();thread.join(2)

    def test_extension_origin_audit_requires_complete_actual_handshake(self):
        current_target, observed = target("responses", "sse"), runtime()
        current_target["capabilities"]["websocket"] = True
        nonce, body = base64.b64encode(b"n" * 16).decode(), b'{"error":"origin"}'
        raw = b"HTTP/1.1 403 Forbidden\r\nContent-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body
        request = {"model": "local-model", "input": "OK", "stream": True, **current_target["parameters"]}
        sample = {"extension_sample_version": 1, "case": "ws_handshake_rejection", "layer": "LOCAL_SIMULATED", "source_sha": SOURCE_SHA, "target": current_target, "runtime": observed, "frozen_runtime": observed,
                  "request": request, "http_status": 403, "handshake_status": 403, "authenticated": True, "operation": "responses_ws_upgrade", "boundary_before": 0, "boundary_after": 0, "evidence": {"upstream_send_count": 0},
                  "ws_nonce": nonce, "ws_origin": "https://example.invalid", "ws_path": "/v1/responses", "ws_request": {"method": "GET", "path": "/v1/responses", "origin": "https://example.invalid", "nonce": nonce, "version": "13", "upgrade": "websocket", "connection": "Upgrade"}, "wire_bytes_b64": base64.b64encode(raw).decode()}
        sample["request_binding_sha256"] = extensions.request_binding(sample)
        original = {"extension_bundle_version": 1, "layer": "LOCAL_SIMULATED", "case": sample["case"], "source_sha": SOURCE_SHA, "runtime": observed, "target": current_target, "samples": [sample], "verified_sample": copy.deepcopy(sample), "counts": {"inference_intents": 0}, "result": {"status": "PASS", "stopped": True}}
        self.assertEqual(extensions.audit_extension_bundle(original)["status"], "PASS")
        for mutation in ("missing", "wrong_status", "partial_body", "wrong_origin"):
            changed = copy.deepcopy(original)
            for row in (changed["samples"][0], changed["verified_sample"]):
                if mutation == "missing":row.pop("wire_bytes_b64")
                elif mutation == "wrong_status":row["wire_bytes_b64"] = base64.b64encode(raw.replace(b"403 Forbidden", b"101 Switching Protocols")).decode()
                elif mutation == "partial_body":row["wire_bytes_b64"] = base64.b64encode(raw[:-1]).decode()
                else:row["ws_request"]["origin"] = "https://different.invalid"
            self.assertIn(extensions.audit_extension_bundle(changed)["status"], ("FAIL", "BLOCKED"), mutation)

    def test_extension_successful_handshake_requires_absent_origin(self):
        nonce = base64.b64encode(b"n" * 16).decode()
        accept = base64.b64encode(hashlib.sha1((nonce + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        raw = ("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: " + accept + "\r\n\r\n").encode()
        sample = {"ws_nonce": nonce, "ws_accept": accept, "ws_origin": None, "ws_path": "/v1/responses", "http_status": 101, "handshake_status": 101,
                  "ws_request": {"method": "GET", "path": "/v1/responses", "origin": None, "nonce": nonce, "version": "13", "upgrade": "websocket", "connection": "Upgrade"}, "wire_bytes_b64": base64.b64encode(raw).decode()}
        extensions.verify_handshake(sample, 101)
        sample["ws_origin"] = sample["ws_request"]["origin"] = "https://example.invalid"
        with self.assertRaisesRegex(ValueError, "ws_handshake_unconfirmed"):
            extensions.verify_handshake(sample, 101)

    def test_extension_resource_audit_uses_captured_operations(self):
        current_target, observed = target(), runtime()
        current_target["capabilities"]["stored"] = current_target["capabilities"]["compact"] = True
        owner, foreign = current_target["declaration_evidence"]["client_key_evidence_id"], oracle.opaque("foreign-client")
        value = response();lineage = evidence(current_target, value)
        request = {"model": "local-model", "stream": False, "max_output_tokens": 128, "input": "OK", "store": True}
        raw = json.dumps(value)
        def operation(method, path, body, identity, status, wire):
            return {"method": method, "path": path, "request": body, "client_evidence_id": identity, "runtime": observed, "http_status": status, "wire": wire,
                    "request_wire_b64": base64.b64encode(json.dumps(body).encode() if body is not None else b"").decode(),
                    "wire_bytes_b64": base64.b64encode(wire.encode()).decode(), "boundary_before": 1, "boundary_after": 1}
        for case in ("store_read", "foreign_owner", "deleted_read", "compact_foreign_owner"):
            root = {"extension_sample_version": 1, "case": case, "layer": "LOCAL_SIMULATED", "source_sha": SOURCE_SHA, "target": current_target,
                    "runtime": observed, "frozen_runtime": observed, "request": request, "expected_text": "OK", "operation": "response_create", "http_status": 200,
                    "request_wire_b64": base64.b64encode(json.dumps(request).encode()).decode(), "wire": raw, "wire_bytes_b64": base64.b64encode(raw.encode()).decode(), "evidence": lineage}
            root["request_binding_sha256"] = extensions.request_binding(root)
            final = copy.deepcopy(root)
            final.update(root_sample=copy.deepcopy(root), read_wire=raw, read_http_status=200, read_response_id=value["id"])
            path = "/responses/" + value["id"]
            operations = [operation("GET", path, None, owner, 200, raw)]
            if case in ("foreign_owner", "compact_foreign_owner"):
                operations.append(operation("GET", "/models", None, foreign, 200, json.dumps({"data": [{"id": "local-model"}]})))
                final.update(owner_read_http_status=200, owner_read_wire=raw, owner_client_evidence_id=owner, actual_client_evidence_id=foreign,
                             foreign_auth_status=200, foreign_visible_model="local-model")
            elif case == "deleted_read":
                operations.append(operation("DELETE", path, None, owner, 200, json.dumps({"id": value["id"], "object": "response.deleted", "deleted": True})))
                final.update(delete_http_status=200, deleted_response_id=value["id"])
            if case != "store_read":
                compact_request = {"model": "local-model", "previous_response_id": value["id"], "stream": False}
                operations.append(operation("POST" if case == "compact_foreign_owner" else "GET", "/responses/compact" if case == "compact_foreign_owner" else path,
                                            compact_request if case == "compact_foreign_owner" else None, owner if case == "deleted_read" else foreign, 404, '{"error":{"type":"not_found"}}'))
                final.update(http_status=404, operation="responses_compact" if case == "compact_foreign_owner" else "stored_read", authenticated=True,
                             boundary_before=1, boundary_after=1, evidence={"upstream_send_count": 0})
                if case == "compact_foreign_owner":final["compact_request"] = compact_request
            original = {"extension_bundle_version": 1, "layer": "LOCAL_SIMULATED", "case": case, "source_sha": SOURCE_SHA, "runtime": observed, "target": current_target,
                        "samples": [root], "verified_sample": final, "resource_operations": operations, "counts": {"inference_intents": 1}, "result": {"status": "PASS", "stopped": True}}
            self.assertEqual(extensions.audit_extension_bundle(original)["status"], "PASS", case)
            for mutation in ("missing", "status", "wire", "path", "request", "identity", "runtime", "send_count"):
                changed = copy.deepcopy(original)
                if mutation == "missing":changed.pop("resource_operations")
                elif mutation == "status":changed["resource_operations"][-1]["http_status"] = 404 if case == "store_read" else 200
                elif mutation == "wire":changed["resource_operations"][-1]["wire_bytes_b64"] = base64.b64encode(b"{}").decode()
                elif mutation == "path":changed["resource_operations"][-1]["path"] = "/responses/unrelated-root"
                elif mutation == "request":changed["resource_operations"][-1]["request"] = {"unfrozen": True}
                elif mutation == "identity":changed["resource_operations"][-1]["client_evidence_id"] = oracle.opaque("unrelated-client")
                elif mutation == "runtime":changed["resource_operations"][-1]["runtime"]["pid"] += 1
                else:changed["resource_operations"][-1]["boundary_after"] += 1
                self.assertIn(extensions.audit_extension_bundle(changed)["status"], ("FAIL", "BLOCKED"), (case, mutation))

    def test_extension_websocket_completed_then_output_fails_before_next_turn(self):
        captures, creates = [], []
        class Handler(socketserver.BaseRequestHandler):
            def handle(self):
                headers = bytearray()
                while not headers.endswith(b"\r\n\r\n"):headers.extend(self.request.recv(1))
                nonce = next(line.split(": ", 1)[1] for line in headers.decode().split("\r\n") if line.startswith("Sec-WebSocket-Key:"))
                accept = base64.b64encode(hashlib.sha1((nonce + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
                self.request.sendall(("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: " + accept + "\r\n\r\n").encode())
                creates.append(self.request.recv(4096))
                frames = wire_events("responses", response()) + [{"type": "response.output_text.delta", "output_index": 0, "content_index": 0, "delta": "LATE"}]
                raw = bytearray()
                for value in frames:
                    part = json.dumps(value).encode()
                    raw.extend(b"\x81" + (bytes([len(part)]) if len(part) < 126 else b"\x7e" + struct.pack("!H", len(part))) + part)
                self.request.sendall(raw)
                self.request.settimeout(.5)
                self.request.recv(4096)
        server = socketserver.TCPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True);thread.start()
        ws = extensions.WebSocket("http://127.0.0.1:" + str(server.server_address[1]), "synthetic-key")
        try:
            with self.assertRaisesRegex(ValueError, "duplicate_or_late_terminal"):
                ws.turn({"model": "local-model", "input": "OK", "stream": True}, on_progress=lambda metadata, raw: captures.append((metadata, raw)))
            late = next(metadata["wire"] for metadata, _ in reversed(captures) if "wire" in metadata)
            self.assertIn('"LATE"', late)
            with self.assertRaises(ValueError):oracle.decode_stream("responses", late.encode())
            self.assertEqual(len(creates), 1)
        finally:
            ws.close();server.shutdown();server.server_close();thread.join(2)


if __name__ == "__main__":
    unittest.main(verbosity=2)
