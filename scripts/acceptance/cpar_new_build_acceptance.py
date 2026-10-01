"""Formal C acceptance through the existing live runner; stdlib only.

Wire bodies and authentication stay in memory or operator-owned private bundles.
Public receipts contain allowlisted observations, checks and opaque identities.
The historical smoke entry point deliberately retains its original evidence layer.
"""
import argparse
import copy
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

MAX_BODY = 4 * 1024 * 1024
PROTOCOLS = {"chat": "/chat/completions", "messages": "/messages", "responses": "/responses"}
EXTENSIONS = {"reasoning", "parallel", "stored", "continuation", "compact", "websocket"}
BOUNDARIES = {"error", "truncation", "cancel_before_send", "cancel_bootstrap", "cancel_after_output", "timeout_bootstrap", "timeout_after_output", "retry_before_output", "no_retry_after_output"}
EXTENSION_CHECKS = {
    "reasoning": {"visible_reasoning", "reasoning_history"},
    "parallel": {"parallel_calls", "parallel_results", "serial_control"},
    "stored": {"store_read", "foreign_owner", "deleted_read", "expired_read", "restart_read", "persistence_failure"},
    "continuation": {"exact_continuation", "lineage_rejection"},
    "compact": {"compact_roundtrip", "compact_foreign_owner", "compact_lineage_rejection"},
    "websocket": {"ws_multiturn", "ws_handshake_rejection", "ws_cancel", "ws_failed_root"},
}
TARGET_FIELDS = ("channel", "endpoint_id", "route_id", "route_candidate_id", "upstream_model", "credential_evidence_id", "credential_revision", "config_version_id", "config_revision", "egress_evidence_id", "egress_revision")
RUNTIME_FIELDS = ("method", "instance", "pid", "process_start", "observed_at_utc", "artifact_sha256", "artifact_sha", "source_sha", "stable_process", "manifest_sha256", "manifest_matches")
USAGE_FIELDS = ("input_tokens", "output_tokens", "reasoning_tokens", "cache_read_tokens", "cache_creation_tokens", "cached_tokens", "provenance", "input_accounting")


class Blocked(ValueError):
    """Necessary evidence is unavailable; this is distinct from an executed failure."""


def require(condition, category):
    if not condition:
        raise ValueError(category)


def available(condition, category):
    if not condition:
        raise Blocked(category)


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def opaque(value):
    return None if value is None else hashlib.sha256(("cpar-evidence:" + str(value)).encode()).hexdigest()


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def integer(value):
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def load_json(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, "duplicate_json_key")
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs, parse_constant=lambda _: (_ for _ in ()).throw(ValueError("invalid_json_number")))


def write_receipt(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    value["tool_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    value["legacy_runner_sha256"] = hashlib.sha256(Path(__file__).with_name("cpar-batch-c-live.py").read_bytes()).hexdigest()
    # Never replay exception strings, wire content, private input paths or authentication.
    with path.open("w") as output:
        json.dump(value, output, ensure_ascii=False, indent=2)
        output.write("\n")


spec = importlib.util.spec_from_file_location("cpar_legacy_live", Path(__file__).with_name("cpar-batch-c-live.py"))
legacy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(legacy)


def sse_events(raw):
    require(len(raw) <= MAX_BODY, "response_size")
    text = raw.decode("utf-8").replace("\r\n", "\n")
    require(not text.strip() or text.endswith("\n\n"), "truncated_sse_frame")
    events = []
    for frame in text.split("\n\n"):
        fields = [line[5:].lstrip(" ") for line in frame.splitlines() if line.startswith("data:")]
        if not fields:
            continue
        data = "\n".join(fields)
        event = "[DONE]" if data == "[DONE]" else load_json(data)
        require(event == "[DONE]" or isinstance(event, dict), "invalid_sse_event")
        names = [line[6:].strip() for line in frame.splitlines() if line.startswith("event:")]
        require(len(names) <= 1 and (not names or isinstance(event, dict) and event.get("type") == names[0]), "event_type_mismatch")
        events.append(event)
    require(bool(events), "empty_sse")
    return events


def observe(protocol, value):
    require(isinstance(value, dict) and "error" not in value, "response_error")
    require(isinstance(value.get("id"), str) and bool(value["id"]), "missing_response_identity")
    require(isinstance(value.get("model"), str) and bool(value["model"]), "missing_response_model")
    if protocol == "chat":
        choices = value.get("choices", [])
        require(len(choices) == 1 and choices[0].get("index", 0) == 0, "invalid_choices")
        message, finish = choices[0]["message"], choices[0].get("finish_reason")
        require(message.get("role") == "assistant", "invalid_role")
        calls = [(item["id"], item["function"]["name"], load_json(item["function"]["arguments"])) for item in message.get("tool_calls", [])]
        text = message.get("content") or ""
        require(finish == ("tool_calls" if calls else "stop"), "contradictory_terminal")
    elif protocol == "messages":
        require(value.get("role") == "assistant" and value.get("type") == "message", "invalid_role")
        calls = [(item["id"], item["name"], item["input"]) for item in value.get("content", []) if item.get("type") == "tool_use"]
        text = "".join(item.get("text", "") for item in value.get("content", []) if item.get("type") == "text")
        require(value.get("stop_reason") == ("tool_use" if calls else "end_turn"), "contradictory_terminal")
    else:
        require(value.get("status") == "completed" and not value.get("incomplete_details") and not value.get("error"), "incomplete_response")
        output = value.get("output", [])
        require(isinstance(output, list), "invalid_output")
        identities = [item.get("id") for item in output]
        require(all(identities) and len(set(identities)) == len(identities), "invalid_item_identity")
        require(all(item.get("status", "completed") == "completed" for item in output), "incomplete_item")
        require(all(item.get("role") == "assistant" for item in output if item.get("type") == "message"), "invalid_role")
        calls = [(item["call_id"], item["name"], load_json(item["arguments"])) for item in output if item.get("type") == "function_call"]
        text = "".join(part.get("text", "") for item in output if item.get("type") == "message" for part in item.get("content", []) if part.get("type") == "output_text")
    require(isinstance(text, str), "invalid_text")
    require(len({identity for identity, _, _ in calls}) == len(calls), "duplicate_tool_id")
    require(all(isinstance(identity, str) and identity and name == "lookup" and isinstance(args, dict) and set(args) == {"value"} and integer(args["value"]) for identity, name, args in calls), "invalid_tool_call")
    return text, calls


def identity_event(event, identity):
    if event.get("id") is not None:
        require(event["id"] == identity[0], "changed_response_identity")
    if event.get("model") is not None:
        require(event["model"] == identity[1], "changed_response_model")


def decode_chat(events):
    require(events[-1] == "[DONE]" and events.count("[DONE]") == 1, "chat_done_boundary")
    message = {"role": "assistant", "content": None}
    calls, usage, finish, identity, role = {}, None, None, None, False
    for event in events[:-1]:
        require(isinstance(event, dict) and not event.get("error"), "response_error")
        if identity is None:
            identity = (event.get("id"), event.get("model"))
        identity_event(event, identity)
        choices = event.get("choices", [])
        if not choices:
            require(finish is not None and isinstance(event.get("usage"), dict), "misordered_usage")
            usage = event["usage"]
            continue
        require(finish is None and len(choices) == 1 and choices[0].get("index") == 0, "duplicate_or_late_terminal")
        choice, delta = choices[0], choices[0].get("delta", {})
        if "role" in delta:
            require(not role and delta["role"] == "assistant", "invalid_role")
            role = True
        require(role, "missing_start_role")
        for field in ("content", "reasoning_content"):
            if delta.get(field) is not None:
                message[field] = (message.get(field) or "") + delta[field]
        for item in delta.get("tool_calls", []):
            index = item["index"]
            require(integer(index) and index <= len(calls), "invalid_tool_index")
            call = calls.setdefault(index, {"id": item.get("id"), "type": "function", "function": {"name": "", "arguments": ""}})
            if item.get("id") is not None:
                require(call["id"] == item["id"], "changed_tool_id")
            for field in ("name", "arguments"):
                call["function"][field] += item.get("function", {}).get(field) or ""
        if choice.get("finish_reason") is not None:
            finish = choice["finish_reason"]
        if event.get("usage") is not None:
            usage = event["usage"]
    require(finish is not None, "missing_success_terminal")
    if calls:
        message["tool_calls"] = [calls[index] for index in sorted(calls)]
    return {"id": identity[0], "model": identity[1], "choices": [{"index": 0, "message": message, "finish_reason": finish}], "usage": usage}


def decode_messages(events):
    blocks, arguments, open_blocks, usage = {}, {}, set(), {}
    message, finish, stopped = None, None, False
    for event in events:
        require(isinstance(event, dict) and not stopped and not event.get("error") and event.get("type") != "error", "late_or_error_event")
        kind = event.get("type")
        if kind == "message_start":
            require(message is None, "duplicate_start")
            message = copy.deepcopy(event["message"])
            require(not message.get("content") and not message.get("stop_reason"), "invalid_start")
            usage.update(message.get("usage") or {})
            continue
        require(message is not None, "missing_start")
        if kind == "ping":
            continue
        if kind == "content_block_start":
            index = event["index"]
            require(finish is None and index == len(blocks), "reopened_or_misordered_block")
            blocks[index] = copy.deepcopy(event["content_block"])
            open_blocks.add(index)
        elif kind == "content_block_delta":
            index, delta = event["index"], event["delta"]
            require(delta.get("type") in ("text_delta", "thinking_delta", "signature_delta", "citations_delta", "input_json_delta"), "unreviewed_stream_event")
            require(finish is None and index in open_blocks, "delta_after_block_done")
            block = blocks[index]
            for field in ("text", "thinking", "signature"):
                if field in delta:
                    block[field] = block.get(field, "") + delta[field]
            if "citation" in delta:
                block.setdefault("citations", []).append(delta["citation"])
            if "partial_json" in delta:
                require(block.get("type") == "tool_use", "argument_on_non_tool")
                arguments[index] = arguments.get(index, "") + delta["partial_json"]
        elif kind == "content_block_stop":
            index = event["index"]
            require(index in open_blocks, "duplicate_block_stop")
            open_blocks.remove(index)
        elif kind == "message_delta":
            require(not open_blocks and finish is None, "premature_or_duplicate_terminal")
            finish = event.get("delta", {}).get("stop_reason")
            require(finish is not None, "missing_stop_reason")
            usage.update(event.get("usage") or {})
        elif kind == "message_stop":
            require(finish is not None and not open_blocks, "premature_terminal")
            stopped = True
        else:
            raise ValueError("unreviewed_stream_event")
    require(stopped, "missing_success_terminal")
    for index, raw in arguments.items():
        blocks[index]["input"] = load_json(raw)
    message.update(content=list(blocks.values()), usage=usage, stop_reason=finish)
    return message


def decode_responses(events):
    identity, items, completed, parts, buffers, closed_parts = None, {}, {}, {}, {}, set()
    terminal = None
    for event in events:
        require(isinstance(event, dict) and terminal is None and not event.get("error"), "duplicate_or_late_terminal")
        kind = event.get("type", "")
        require(kind not in ("error", "response.failed", "response.incomplete"), "response_error")
        if kind == "response.created":
            require(identity is None, "duplicate_start")
            value = event["response"]
            identity = (value.get("id"), value.get("model"))
            require(value.get("status") in ("in_progress", "queued"), "invalid_start")
            continue
        require(identity is not None, "missing_start")
        if "response" in event:
            identity_event(event["response"], identity)
        if kind == "response.in_progress":
            continue
        if kind == "response.completed":
            value = event["response"]
            require(len(value.get("output", [])) == len(items) == len(completed), "unfinished_output_item")
            require([completed[index] for index in sorted(completed)] == value["output"], "changed_terminal_snapshot")
            require(not (set(parts) - closed_parts), "unfinished_content_part")
            terminal = value
            continue
        index = event.get("output_index")
        require(integer(index), "missing_output_index")
        if kind == "response.output_item.added":
            require(index == len(items), "reopened_or_misordered_item")
            items[index] = copy.deepcopy(event["item"])
            continue
        require(index in items and index not in completed, "event_after_item_done")
        item = items[index]
        if event.get("item_id") is not None:
            require(event["item_id"] == item.get("id"), "changed_item_id")
        for field in ("call_id", "name"):
            if event.get(field) is not None:
                require(event[field] == item.get(field), "changed_tool_identity")
        if kind == "response.output_item.done":
            value = event["item"]
            for field in ("id", "type", "call_id", "name", "role", "phase"):
                if field in item:
                    require(value.get(field) == item[field], "changed_item_snapshot")
            if item.get("type") == "function_call":
                require(value.get("arguments") == buffers.get((index, "arguments"), item.get("arguments", "")), "changed_tool_arguments")
            for key, part in parts.items():
                if key[0] == index:
                    field, part_index = key[1], key[2]
                    require(key in closed_parts and part_index < len(value.get(field, [])) and value[field][part_index] == part, "changed_part_snapshot")
            completed[index] = value
        elif kind.startswith("response.function_call_arguments."):
            require(item.get("type") == "function_call", "argument_on_non_tool")
            key = (index, "arguments")
            if kind.endswith(".delta"):
                require((index, "arguments_done") not in buffers, "arguments_after_done")
                buffers[key] = buffers.get(key, item.get("arguments", "")) + event["delta"]
            elif kind.endswith(".done"):
                require((index, "arguments_done") not in buffers and event["arguments"] == buffers.get(key, item.get("arguments", "")), "changed_tool_arguments")
                load_json(event["arguments"])
                buffers[(index, "arguments_done")] = True
            else:
                raise ValueError("unreviewed_stream_event")
        elif any(stem in kind for stem in ("content_part.", "reasoning_summary_part.")):
            field = "summary" if "reasoning_summary" in kind else "content"
            part_index = event.get("summary_index" if field == "summary" else "content_index")
            key = (index, field, part_index)
            require(integer(part_index), "invalid_part_index")
            if kind.endswith(".added"):
                require(key not in parts and part_index == sum(k[0:2] == (index, field) for k in parts), "reopened_or_misordered_part")
                parts[key] = copy.deepcopy(event["part"])
            elif kind.endswith(".done"):
                require(key in parts and key not in closed_parts and event["part"] == parts[key], "changed_part_snapshot")
                closed_parts.add(key)
            else:
                raise ValueError("unreviewed_stream_event")
        elif ".annotation." not in kind and any(stem in kind for stem in ("output_text.", "reasoning_text.", "reasoning_summary_text.")):
            field = "summary" if "reasoning_summary" in kind else "content"
            key = (index, field, event.get("summary_index" if field == "summary" else "content_index"))
            require(key in parts and key not in closed_parts, "text_after_part_done")
            part = parts[key]
            if kind.endswith(".delta"):
                require((key, "text_done") not in buffers, "text_after_done")
                part["text"] = part.get("text", "") + event["delta"]
            elif kind.endswith(".done"):
                require((key, "text_done") not in buffers and event["text"] == part.get("text", ""), "changed_text_snapshot")
                buffers[(key, "text_done")] = True
            else:
                raise ValueError("unreviewed_stream_event")
        elif kind == "response.output_text.annotation.added":
            key = (index, "content", event.get("content_index"))
            require(key in parts and key not in closed_parts, "annotation_after_part_done")
            annotations = parts[key].setdefault("annotations", [])
            require(event.get("annotation_index") == len(annotations), "invalid_annotation_index")
            annotations.append(event["annotation"])
        else:
            raise ValueError("unreviewed_stream_event")
    require(terminal is not None, "missing_success_terminal")
    return terminal


def decode_stream(protocol, raw):
    events = sse_events(raw)
    value = {"chat": decode_chat, "messages": decode_messages, "responses": decode_responses}[protocol](events)
    observe(protocol, value)
    return value


def native_usage(protocol, value):
    usage = value.get("usage")
    available(isinstance(usage, dict), "native_usage_unavailable")
    fields = ("prompt_tokens", "completion_tokens") if protocol == "chat" else ("input_tokens", "output_tokens")
    available(all(usage.get(field) is not None for field in fields), "native_usage_incomplete")
    require(all(integer(usage[field]) for field in fields), "invalid_native_usage")
    if usage.get("total_tokens") is not None:
        require(integer(usage["total_tokens"]) and usage["total_tokens"] == sum(usage[field] for field in fields), "invalid_native_usage_total")
    source = usage.get("cpar_usage")
    if source is not None:
        require(isinstance(source, dict) and source.get("provenance") in ("measured", "estimated", "unknown") and source.get("input_accounting") in ("inclusive", "exclusive", "unknown"), "invalid_usage_evidence")
        require(set(source) == set(USAGE_FIELDS) and all(source[field] is None or integer(source[field]) for field in USAGE_FIELDS[:6]), "invalid_usage_evidence")
        for source_field, native_field in (("output_tokens", fields[1]),):
            if source[source_field] is not None:
                require(source[source_field] == usage[native_field], "native_source_usage_changed")
        expected_input = source["input_tokens"]
        target_accounting = "exclusive" if protocol == "messages" else "inclusive"
        if source["input_accounting"] not in (target_accounting, "unknown") and expected_input is not None:
            if target_accounting == "inclusive":
                available(source["cache_read_tokens"] is not None and source["cache_creation_tokens"] is not None, "native_input_accounting_unconfirmed")
                expected_input += source["cache_read_tokens"] + source["cache_creation_tokens"]
            else:
                available(source["cached_tokens"] is not None, "native_input_accounting_unconfirmed")
                expected_input -= source["cached_tokens"]
        available(expected_input is not None, "source_input_usage_unknown")
        require(integer(expected_input) and expected_input == usage[fields[0]], "native_source_usage_changed")
    native = {field: usage.get(field) for field in (*fields, "total_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")}
    for details_name, field in (("prompt_tokens_details" if protocol == "chat" else "input_tokens_details", "cached_tokens"), ("completion_tokens_details" if protocol == "chat" else "output_tokens_details", "reasoning_tokens")):
        details = usage.get(details_name) or {}
        require(isinstance(details, dict) and (details.get(field) is None or integer(details[field])), "invalid_native_usage_detail")
        native[field] = details.get(field)
        if source is not None and protocol != "messages" and source[field] is not None:
            available(native[field] is not None, "native_usage_detail_missing")
            require(native[field] == source[field], "native_source_usage_changed")
    for source_field, native_field in (("cache_read_tokens", "cache_read_input_tokens"), ("cache_creation_tokens", "cache_creation_input_tokens")):
        require(usage.get(native_field) is None or integer(usage[native_field]), "invalid_native_usage_detail")
        if source is not None and protocol == "messages" and source[source_field] is not None:
            available(usage.get(native_field) is not None, "native_usage_detail_missing")
            require(usage[native_field] == source[source_field], "native_source_usage_changed")
    return {"native": native, "source": None if source is None else {field: source[field] for field in USAGE_FIELDS}}


def verify_runtime(runtime, source_sha, layer):
    require(layer in ("LOCAL_SIMULATED", "LIVE_NEW_BUILD", "PRODUCTION"), "invalid_evidence_layer")
    available(runtime.get("method") == "proc_executable" and runtime.get("stable_process") is True, "runtime_unconfirmed")
    available(all(runtime.get(field) is not None for field in ("instance", "pid", "process_start", "observed_at_utc", "artifact_sha256", "artifact_sha", "source_sha")), "runtime_identity_incomplete")
    available(source_sha is not None, "source_version_unknown")
    require(re.fullmatch("[0-9a-f]{40}", source_sha or "") is not None, "invalid_source_sha")
    require(runtime["source_sha"] == runtime["artifact_sha"] == source_sha, "wrong_runtime_version")
    require(re.fullmatch("[0-9a-f]{64}", runtime["artifact_sha256"]) is not None, "invalid_artifact_checksum")
    if layer != "LOCAL_SIMULATED":
        available(runtime.get("manifest_sha256") is not None and runtime.get("manifest_matches") is True, "artifact_manifest_unconfirmed")
    observed = datetime.datetime.fromisoformat(runtime["observed_at_utc"])
    available(observed.tzinfo is not None and -30 <= (datetime.datetime.now(datetime.timezone.utc) - observed).total_seconds() <= 600, "runtime_observation_stale")


def verify_turn_runtime(observed, baseline, source_sha, layer):
    available(isinstance(observed, dict), "per_turn_runtime_unavailable")
    verify_runtime({**observed, "manifest_sha256": baseline.get("manifest_sha256"), "manifest_matches": baseline.get("manifest_matches")}, source_sha, layer)
    require(all(observed.get(key) == baseline.get(key) for key in ("instance", "pid", "process_start", "artifact_sha256")), "runtime_changed_during_acceptance")


def verify_target(target):
    available(all(target.get(field) is not None for field in (*TARGET_FIELDS, "public_model", "response_model", "capabilities", "declaration_observed_at_utc")), "target_identity_incomplete")
    require(target.get("protocol") in PROTOCOLS and target.get("mode") in ("json", "sse"), "invalid_combination")
    available(isinstance(target["capabilities"], dict) and all(isinstance(target["capabilities"].get(key), bool) for key in EXTENSIONS), "capabilities_unknown")
    available(not (set(target["capabilities"]) - EXTENSIONS), "unreviewed_declared_capability")
    for field in ("credential_evidence_id", "egress_evidence_id"):
        require(re.fullmatch("[0-9a-f]{64}", target[field]) is not None, "invalid_opaque_identity")
    validate_parameters(target.get("parameters", {}))
    if target["channel"] == "grok.web":
        raise Blocked("software_web_native_multiturn")
    if target["channel"] == "kiro" and (target["protocol"] == "messages" or any(key in target.get("parameters", {}) for key in ("max_tokens", "max_completion_tokens", "max_output_tokens"))):
        raise Blocked("software_kiro_native_output_cap")
    require(target["protocol"] != "messages" or integer(target.get("parameters", {}).get("max_tokens")) and target["parameters"]["max_tokens"] > 0, "messages_output_limit_required")


def validate_parameters(parameters):
    require(isinstance(parameters, dict) and set(parameters).issubset({"max_tokens", "max_completion_tokens", "max_output_tokens", "temperature", "reasoning", "thinking", "parallel_tool_calls"}), "unreviewed_parameter")
    for key, value in parameters.items():
        if key.startswith("max_"):
            require(integer(value) and value > 0, "invalid_output_limit")
        elif key == "temperature":
            require(type(value) in (int, float) and 0 <= value <= 2, "invalid_temperature")
        elif key == "parallel_tool_calls":
            require(isinstance(value, bool), "invalid_parallel_control")
        elif key == "reasoning":
            require(isinstance(value, dict) and set(value).issubset({"effort", "summary"}) and value.get("effort") in ("none", "minimal", "low", "medium", "high", "xhigh") and value.get("summary") in (None, "auto", "concise", "detailed"), "invalid_reasoning_control")
        else:
            require(isinstance(value, dict) and set(value).issubset({"type", "budget_tokens"}) and value.get("type") in ("enabled", "disabled", "adaptive") and ("budget_tokens" not in value or integer(value["budget_tokens"]) and value["budget_tokens"] > 0), "invalid_thinking_control")


def verify_attempts(target, evidence, response_id, expected="success"):
    available(evidence.get("correlation") in ("request_id", "response_id_usage"), "attempt_correlation_ambiguous")
    request_id = evidence.get("request_id")
    attempts = evidence.get("attempts", [])
    available(request_id is not None and bool(attempts), "attempt_unavailable")
    require(len({attempt.get("attempt_id") for attempt in attempts}) == len(attempts), "duplicate_attempt")
    require(len(attempts) == 1, "unexpected_retry")
    for attempt in attempts:
        require(attempt.get("request_id") == request_id, "wrong_request_attribution")
        for field in TARGET_FIELDS:
            available(attempt.get(field) is not None, "attempt_identity_incomplete")
            require(attempt[field] == target[field], "wrong_attempt_target")
        require(attempt.get("outcome") == expected, "wrong_attempt_outcome")
    usages = evidence.get("usages", [])
    available(bool(usages), "durable_usage_unavailable")
    require(all(row.get("request_id") == request_id and row.get("response_id") == response_id and row.get("attempt_id") == attempts[0]["attempt_id"] for row in usages), "usage_attribution_mismatch")
    ledger = evidence.get("ledger")
    available(bool(ledger), "ledger_observation_unavailable")
    require(all(row.get("request_id") == request_id and row.get("attempt_id") == attempts[0]["attempt_id"] for row in ledger), "ledger_attribution_mismatch")
    return {"request_evidence_id": opaque(request_id), "attempt_evidence_ids": [opaque(row["attempt_id"]) for row in attempts], "attempt_count": len(attempts), "ledger_rows": len(ledger), "usage_rows": len(usages)}


def verify_usage_lineage(native, evidence):
    source = native["source"]
    available(source is not None, "source_usage_evidence_unavailable")
    for row in evidence["usages"]:
        require(row.get("usage") == source, "durable_usage_changed")
    for row in evidence["ledger"]:
        require(row.get("usage") == source, "ledger_usage_changed")


def conversation_step(protocol, request, answer, calls, result_token):
    if calls:
        result = legacy.followup(protocol, request, answer, calls)
        if protocol == "responses":
            for item in result["input"][-len(calls):]:
                item["output"] = result_token
        elif protocol == "chat":
            for item in result["messages"][-len(calls):]:
                item["content"] = result_token
        else:
            for item in result["messages"][-1]["content"]:
                item["content"] = result_token
        return result
    result = copy.deepcopy(request)
    if protocol == "responses":
        history = [{"role": "user", "content": request["input"]}] if isinstance(request["input"], str) else request["input"]
        result["input"] = [*history, *answer["output"], {"role": "user", "content": result_token}]
    else:
        assistant = answer["choices"][0]["message"] if protocol == "chat" else {"role": "assistant", "content": answer["content"]}
        result["messages"].extend([assistant, {"role": "user", "content": result_token}])
    return result


def tool_choice(protocol, required):
    return {"type": "any" if required else "none"} if protocol == "messages" else "required" if required else "none"


def make_request(target, scenario, token):
    protocol = target["protocol"]
    tools = scenario != "text_multi_turn"
    result = legacy.payload(protocol, target["public_model"], target["mode"] == "sse", tools)
    # The plan supplies exact controls. No inherited smoke limit may silently replace them.
    for key in ("max_tokens", "max_output_tokens"):
        result.pop(key, None)
    parameters = target.get("parameters", {})
    require(set(parameters).issubset({"max_tokens", "max_completion_tokens", "max_output_tokens", "temperature", "reasoning", "thinking", "parallel_tool_calls"}), "unreviewed_parameter")
    result.update(copy.deepcopy(parameters))
    if not tools:
        prompt = "Remember " + token + ". Reply READY."
        if protocol == "responses":
            result["input"] = prompt
        else:
            result["messages"][0]["content"] = prompt
    else:
        result["tool_choice"] = tool_choice(protocol, True)
    return result


def check_history(protocol, previous, request, answer, calls, token):
    expected = conversation_step(protocol, previous, answer, calls, token)
    field = "input" if protocol == "responses" else "messages"
    require(request.get(field) == expected[field], "incomplete_or_reordered_history")
    for key in previous:
        if key not in (field, "tool_choice"):
            require(request.get(key) == previous[key], "changed_request_control")


def boundary_check(sample, target):
    case = sample["case"]
    require(case in BOUNDARIES, "unreviewed_boundary")
    require(sample.get("target") == target, "wrong_boundary_target")
    evidence = sample.get("evidence", {})
    available(evidence.get("upstream_send_count") is not None and evidence.get("resource_released") is not None, "boundary_observation_unavailable")
    require(evidence["resource_released"] is True, "resource_not_released")
    attempts = evidence.get("attempts", [])
    require(evidence["upstream_send_count"] == len(attempts), "send_attempt_count_mismatch")
    for attempt in attempts:
        require(all(attempt.get(field) == target.get(field) for field in TARGET_FIELDS), "wrong_attempt_target")
    events = sample.get("events", [])
    successes = [event for event in events if event.get("type") in ("response.completed", "message_stop", "chat.done")]
    errors = [event for event in events if event.get("type") in ("response.failed", "error", "response.incomplete")]
    if case == "cancel_before_send":
        require(not attempts and not events, "send_after_cancel")
    elif case == "retry_before_output":
        require(1 < len(attempts) <= sample.get("max_attempts", 0) and len(successes) == 1 and not errors, "invalid_retry_boundary")
        semantic_at = evidence.get("first_semantic_at_ms")
        available(semantic_at is not None, "semantic_time_unknown")
        require(all(row.get("started_at_ms", semantic_at) < semantic_at for row in attempts), "retry_after_semantic_output")
    else:
        require(len(attempts) == 1 and not successes, "success_or_retry_after_failure")
        if case == "error":
            require(len(errors) == 1 and events[-1] == errors[0], "invalid_error_terminal")
        else:
            require(len(errors) <= 1 and (not errors or events[-1] == errors[0]), "duplicate_or_late_error")
        if "after_output" in case:
            require(evidence.get("semantic_output_observed") is True, "semantic_output_not_observed")
        if case.startswith("timeout"):
            require(evidence.get("timeout_observed") is True, "timeout_not_observed")
        if case.startswith("cancel"):
            require(evidence.get("cancel_observed") is True, "cancel_not_observed")
    available(evidence.get("observation_closed_at_ms") is not None, "retry_observation_window_open")
    require(evidence.get("post_terminal_send_count") == 0, "send_after_terminal")


def extension_check(name, case, sample, target):
    require(case in EXTENSION_CHECKS[name] and sample.get("target") == target, "wrong_extension_target")
    evidence = sample.get("evidence", {})
    # These are raw observations, never a supplied PASS or a coverage boolean.
    if case in {"foreign_owner", "deleted_read", "expired_read", "lineage_rejection", "compact_foreign_owner", "compact_lineage_rejection", "ws_handshake_rejection"}:
        statuses = {404} if case in {"foreign_owner", "deleted_read", "expired_read", "compact_foreign_owner"} else {403} if case == "ws_handshake_rejection" else {400, 404, 409}
        require(sample.get("http_status") in statuses, "extension_rejection_missing")
        require(evidence.get("upstream_send_count") == 0, "rejected_extension_sent_upstream")
        available(sample.get("operation") is not None, "extension_operation_unknown")
        operation = {"foreign_owner": "stored_read", "deleted_read": "stored_read", "expired_read": "stored_read", "lineage_rejection": "responses_continue", "compact_foreign_owner": "responses_compact", "compact_lineage_rejection": "responses_compact", "ws_handshake_rejection": "responses_ws_upgrade"}[case]
        require(sample["operation"] == operation, "wrong_extension_operation")
        require(sample.get("authenticated") is True, "extension_authentication_unconfirmed")
        if case in {"foreign_owner", "compact_foreign_owner"}:
            available(sample.get("owner_client_evidence_id") is not None and sample.get("actual_client_evidence_id") is not None, "extension_owner_unknown")
            require(sample["owner_client_evidence_id"] != sample["actual_client_evidence_id"], "foreign_owner_not_tested")
            owned = load_json(sample.get("owner_read_wire", "null"))
            require(sample.get("owner_read_http_status") == 200 and isinstance(owned, dict) and owned.get("id") == sample.get("read_response_id") and sample.get("read_response_id") is not None, "extension_resource_unconfirmed")
            if case == "compact_foreign_owner":
                require(sample.get("compact_request", {}).get("previous_response_id") == sample["read_response_id"], "extension_resource_unconfirmed")
        if case == "deleted_read":
            require(sample.get("delete_http_status") in (200, 204) and sample.get("deleted_response_id") == sample.get("read_response_id") and sample.get("read_response_id") is not None, "deletion_not_observed")
        if case == "expired_read":
            require(sample.get("observed_at_ms", 0) >= sample.get("expires_at_ms", float("inf")), "expiry_not_observed")
        return
    if case in {"persistence_failure", "ws_cancel", "ws_failed_root"}:
        boundary_check({**sample, "case": "cancel_after_output" if case == "ws_cancel" else "error"}, target)
        require(sample.get("stored_read_status") == 404, "failed_response_stored")
        return
    value = decode_stream(target["protocol"], sample["wire"].encode()) if sample.get("mode", target["mode"]) == "sse" else load_json(sample["wire"])
    text, calls = observe(target["protocol"], value)
    require(value["model"] == target["response_model"], "wrong_response_model")
    usage = native_usage(target["protocol"], value)
    verify_attempts(target, evidence, value["id"])
    verify_usage_lineage(usage, evidence)
    if case in {"visible_reasoning", "reasoning_history"}:
        reasoning = value.get("choices", [{}])[0].get("message", {}).get("reasoning_content") or "".join(block.get("thinking", "") for block in value.get("content", [])) or "".join(part.get("text", "") for item in value.get("output", []) if item.get("type") == "reasoning" for field in ("summary", "content") for part in item.get(field, []))
        require(bool(reasoning), "reasoning_missing")
        if case == "reasoning_history":
            previous = load_json(sample.get("previous_wire", "null"))
            _, previous_calls = observe(target["protocol"], previous)
            check_history(target["protocol"], sample["previous_request"], sample["request"], previous, previous_calls, sample["next_token"])
    elif case == "parallel_calls":
        require(len(calls) == 2 and [args for _, _, args in calls] == [{"value": 1}, {"value": 2}], "parallel_calls_incomplete")
        if target["mode"] == "sse":
            order = []
            for event in sse_events(sample["wire"].encode()):
                if not isinstance(event, dict):continue
                if event.get("type") == "response.function_call_arguments.delta":order.append(event["output_index"])
                elif event.get("type") == "content_block_delta" and "partial_json" in event.get("delta", {}):order.append(event["index"])
                for choice in event.get("choices", []):
                    order.extend(call["index"] for call in choice.get("delta", {}).get("tool_calls", []) if call.get("function", {}).get("arguments"))
            require(len(set(order)) == 2 and sum(a != b for a, b in zip(order, order[1:])) >= 2, "parallel_not_interleaved")
    elif case in {"parallel_results", "serial_control"}:
        require(len(calls) <= 1 if case == "serial_control" else not calls and bool(text), "parallel_control_or_result")
        if case == "serial_control":
            require(sample.get("request", {}).get("parallel_tool_calls") is False, "parallel_pairing_missing")
        else:
            previous = load_json(sample.get("previous_wire", "null"))
            _, previous_calls = observe(target["protocol"], previous)
            require(len(previous_calls) == 2, "parallel_pairing_missing")
            check_history(target["protocol"], sample["previous_request"], sample["request"], previous, previous_calls, sample["next_token"])
    elif case in {"store_read", "restart_read"}:
        require(sample.get("request", {}).get("store") is True, "store_not_requested")
        require(load_json(sample.get("read_wire", "null")) == value and sample.get("read_http_status") == 200, "stored_read_mismatch")
        if case == "restart_read":
            before, after = sample.get("process_before", {}), sample.get("process_after", {})
            verify_runtime(before, before.get("source_sha"), sample.get("layer", "LIVE_NEW_BUILD"))
            verify_runtime(after, after.get("source_sha"), sample.get("layer", "LIVE_NEW_BUILD"))
            available(before.get("process_start") is not None and after.get("process_start") is not None, "restart_unconfirmed")
            require(before["process_start"] != after["process_start"] and before.get("source_sha") == after.get("source_sha"), "restart_unconfirmed")
    elif case in {"exact_continuation", "compact_roundtrip", "ws_multiturn"}:
        require(sample.get("parent_response_id") is not None, "continuation_parent_mismatch")
        require(sample.get("parent_target") == target and bool(text), "continuation_lineage_mismatch")
        if case == "compact_roundtrip":
            compact = load_json(sample.get("compact_wire", "null"))
            require(sample.get("compact_http_status") == 200 and isinstance(compact, dict) and bool(compact.get("output")), "compact_roundtrip_incomplete")
            item = compact["output"][0]
            require(item.get("type") == "compaction" and item.get("created_by") == "cpar" and isinstance(item.get("encrypted_content"), str) and item["encrypted_content"].startswith("cpar_compact_v1."), "compact_domain_unconfirmed")
            require(sample.get("compact_request", {}).get("previous_response_id") == sample["parent_response_id"] and item in sample.get("request", {}).get("input", []), "compact_roundtrip_incomplete")
        else:
            require(sample.get("request", {}).get("previous_response_id") == sample["parent_response_id"], "continuation_parent_mismatch")
        if case == "ws_multiturn":
            transcript = sample.get("ws_turns", [])
            require(sample.get("handshake_status") == 101 and len(transcript) >= 2, "ws_multiturn_incomplete")
            import base64
            expected_accept = base64.b64encode(hashlib.sha1((sample.get("ws_nonce", "") + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
            require(bool(sample.get("ws_nonce")) and sample.get("ws_accept") == expected_accept, "ws_handshake_unconfirmed")
            for turn in transcript:
                decode_stream("responses", turn["wire"].encode())


def verdict(action):
    try:
        action()
        return {"status": "PASS", "category": "verified"}
    except Blocked as error:
        return {"status": "BLOCKED", "category": str(error)}
    except (ValueError, KeyError, TypeError, IndexError, UnicodeError) as error:
        category = str(error) if type(error) is ValueError and re.fullmatch("[a-z_]{1,80}", str(error)) else "malformed_observation"
        return {"status": "FAIL", "category": category}


def public_identity(key, value):
    if value is None:
        return None
    if key in ("credential_evidence_id", "egress_evidence_id", "source_sha", "artifact_sha", "artifact_sha256", "manifest_sha256"):
        width = 40 if key in ("source_sha", "artifact_sha") else 64
        return value if isinstance(value, str) and re.fullmatch("[0-9a-f]{" + str(width) + "}", value) else None
    if type(value) in (int, bool):
        return value
    return value if isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9_./:+-]{1,256}", value) else None


def public_failure(value):
    return {"status": value.get("status") if value.get("status") in ("FAIL", "BLOCKED") else "FAIL", "category": value.get("category") if isinstance(value.get("category"), str) and re.fullmatch("[a-z_]{1,80}", value["category"]) else "malformed_observation"}


def basic_answer(target, sample, scenario, index, turns_needed, seen_ids):
    request = sample["request"]
    require(request.get("model") == target["public_model"] and request.get("stream") is (target["mode"] == "sse"), "wrong_request_target")
    require(all(request.get(key) == expected for key, expected in target.get("parameters", {}).items()), "changed_request_control")
    require(sample.get("http_status") == 200, "http_error")
    answer = decode_stream(target["protocol"], sample["wire"].encode()) if target["mode"] == "sse" else load_json(sample["wire"])
    text, calls = observe(target["protocol"], answer)
    require(answer["model"] == target["response_model"], "wrong_response_model")
    if scenario != "text_multi_turn" and index < turns_needed - 1:
        require(len(calls) == 1 and calls[0][2] == {"value": 1}, "missing_tool_cycle")
        require(calls[0][0] not in seen_ids, "reused_tool_cycle_id")
        seen_ids.add(calls[0][0])
    else:
        require(bool(text) and not calls, "missing_final_answer")
        if index == turns_needed - 1:
            require(sample.get("expected_text") is not None and text.strip() == sample["expected_text"], "history_or_result_not_used")
    return answer, calls, native_usage(target["protocol"], answer)


def audit_bundle(bundle):
    layer = bundle.get("layer")
    require(layer in ("LOCAL_SIMULATED", "LIVE_NEW_BUILD", "PRODUCTION"), "invalid_evidence_layer")
    target = bundle.get("target", {})
    receipt = {"schema_version": 2, "layer": layer, "source_sha": public_identity("source_sha", bundle.get("source_sha")), "observed_at_utc": now(), "runtime": {key: public_identity(key, bundle.get("runtime", {}).get(key)) for key in RUNTIME_FIELDS}, "target": {key: public_identity(key, target.get(key)) for key in (*TARGET_FIELDS, "protocol", "mode", "public_model", "response_model")}, "parameters_sha256": digest(target.get("parameters")), "capabilities": {key: target["capabilities"][key] if isinstance(target["capabilities"].get(key), bool) else None for key in EXTENSIONS} if isinstance(target.get("capabilities"), dict) else None, "declaration_observed_at_utc": public_identity("observed_at_utc", target.get("declaration_observed_at_utc")), "value_free": True, "checks": [], "evidence_paths": [path for path in bundle.get("evidence_paths", []) if isinstance(path, str) and re.fullmatch(r"[A-Za-z0-9_./-]{1,256}", path) and not path.startswith("/") and ".." not in path]}
    preflight = verdict(lambda: (verify_runtime(bundle.get("runtime", {}), bundle.get("source_sha"), layer), verify_target(target)))
    receipt["preflight"] = preflight
    if preflight["status"] != "PASS":
        receipt["status"] = preflight["status"]
        return receipt
    receipt["parameters"] = target.get("parameters", {})
    receipt["sent_requests"] = sum(len(rows) for rows in bundle.get("scenarios", {}).values())
    if bundle.get("execution_failure"):
        receipt["checks"].append({"case": "execution", **public_failure(bundle["execution_failure"])})
    for scenario, turns_needed in (("text_multi_turn", 2), ("single_tool", 2), ("two_tool_cycles", 3)):
        samples = bundle.get("scenarios", {}).get(scenario)
        row = {"case": scenario, "status": "NOT_RUN", "category": "samples_unavailable"}
        if samples is not None and len(samples) < turns_needed and bundle.get("execution_failure"):
            row.update(status="NOT_RUN", category="execution_stopped", executed_turn_count=len(samples))
        elif samples is not None:
            def check(samples=samples, scenario=scenario, turns_needed=turns_needed):
                require(len(samples) == turns_needed, "incomplete_tool_cycles" if scenario == "two_tool_cycles" else "incomplete_turns")
                seen_ids, previous, answer, calls, token = set(), None, None, [], None
                for index, sample in enumerate(samples):
                    request = sample["request"]
                    if previous is not None:
                        check_history(target["protocol"], previous, request, answer, calls, token)
                    answer, calls, native = basic_answer(target, sample, scenario, index, turns_needed, seen_ids)
                    verify_turn_runtime(sample.get("runtime"), bundle["runtime"], bundle["source_sha"], layer)
                    attribution = verify_attempts(target, sample.get("evidence", {}), answer["id"])
                    verify_usage_lineage(native, sample["evidence"])
                    row.setdefault("turns", []).append({"round": index + 1, "response_evidence_id": opaque(answer["id"]), "native_usage": native, "tool_call_count": len(calls), "runtime": {key: public_identity(key, sample["runtime"].get(key)) for key in RUNTIME_FIELDS}, **attribution})
                    previous, token = request, sample.get("next_token")
                    if index < turns_needed - 1:
                        require(isinstance(token, str) and bool(token), "missing_history_token")
            row.update(verdict(check))
        receipt["checks"].append(row)
    for case in sorted(BOUNDARIES):
        sample = bundle.get("boundaries", {}).get(case)
        receipt["checks"].append({"case": case, **(verdict(lambda s=sample: boundary_check(s, target)) if sample is not None else {"status": "NOT_RUN", "category": "controlled_boundary_sample_required"})})
    for extension in sorted(EXTENSIONS):
        declared = target["capabilities"][extension]
        for case in sorted(EXTENSION_CHECKS[extension]) if declared else ["declaration"]:
            sample = bundle.get("extensions", {}).get(extension, {}).get(case)
            result = verdict(lambda n=extension, c=case, s=sample: extension_check(n, c, s, target)) if sample is not None and declared else {"status": "NOT_RUN", "category": "declared_extension_sample_required" if declared else "explicitly_not_declared"}
            receipt["checks"].append({"extension": extension, "case": case, "applicable": declared, **result})
    applicable = [row["status"] for row in receipt["checks"] if row.get("applicable", True)]
    receipt["status"] = next((status for status in ("FAIL", "BLOCKED", "NOT_RUN") if status in applicable), "PASS")
    receipt["basic_status"] = next((status for status in ("FAIL", "BLOCKED", "NOT_RUN") if any(row["case"] in ("text_multi_turn", "single_tool", "two_tool_cycles") and row["status"] == status for row in receipt["checks"])), "PASS")
    return receipt


# Linux process observation and GET-only management projection execute on the already
# authorized host. No token, account display identity or unfiltered response crosses SSH.
REMOTE_READ = r'''
import datetime, hashlib, json, pathlib, re, subprocess, urllib.request, urllib.parse, urllib.error, socket
def opaque(v): return None if v is None else hashlib.sha256(('cpar-evidence:'+str(v)).encode()).hexdigest()
service=SERVICE
pid=int(subprocess.check_output(['systemctl','show',service,'--value','--property=MainPID']))
proc=pathlib.Path('/proc')/str(pid)
start=(proc/'stat').read_text().rsplit(')',1)[1].split()[19]
args=(proc/'cmdline').read_bytes().decode().split('\0')
opts={k:args[args.index(k)+1] for k in ['--state-dir','--credential-dir','--management-listen'] if k in args}
with (proc/'exe').open('rb') as binary:
    content=binary.read(512*1024*1024+1)
if len(content)>512*1024*1024: raise ValueError('artifact_size')
revisions=sorted(set(x.decode() for x in re.findall(rb'gateway-release-revision=([0-9a-f]{40})\n',content)))
runtime={'method':'proc_executable','instance':socket.gethostname(),'pid':pid,'process_start':start,'observed_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'artifact_sha256':hashlib.sha256(content).hexdigest(),'artifact_sha':revisions[0] if len(revisions)==1 else None,'source_sha':revisions[0] if len(revisions)==1 else None,'executable':str((proc/'exe').resolve())}
runtime['stable_process']=pid==int(subprocess.check_output(['systemctl','show',service,'--value','--property=MainPID'])) and start==(proc/'stat').read_text().rsplit(')',1)[1].split()[19]
result={'runtime':runtime}
if INVENTORY:
    key=(pathlib.Path(opts['--credential-dir'])/'management-key').read_text().strip()
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    host=opts['--management-listen']
    if not host.startswith(('127.0.0.1:', '[::1]:')): raise ValueError('management_not_loopback')
    headers={'X-Management-Key':key}
    def get(path):
        try:
            with opener.open(urllib.request.Request('http://'+host+path,headers=headers),timeout=15) as response:
                raw=response.read(4*1024*1024+1)
                if len(raw)>4*1024*1024:raise ValueError('inventory_size')
                return json.loads(raw),response.headers.get('ETag'),None
        except urllib.error.HTTPError as e:return None,None,'http_'+str(e.code)
    versions,_,error=get('/admin/config-versions')
    active=[v for v in versions or [] if v.get('status')=='active']
    result['active_configuration']={k:active[0].get(k) for k in ['id','status','revision']} if len(active)==1 else None
    if len(active)==1:headers['X-Config-Version']=active[0]['id']
    projections={
      '/admin/upstreams':('upstreams',['id','kind','enabled','egress_policy_id']),
      '/admin/endpoints':('endpoints',['id','upstream_id','adapter_id','api_format','base_url','inference_path','enabled']),
      '/admin/accounts/inventory?limit=100':('credentials',['id','provider','category','status','managed','native']),
      '/admin/routes':('routes',['id','public_model_id','max_attempts','bootstrap_timeout_ms']),
      '/admin/route-candidates?limit=200':('candidates',['id','route_id','endpoint_id','upstream_model','enabled','capability_override','credential_scope','transform_mode']),
      '/admin/public-models':('public_models',['id','model_name','status','capabilities']),
      '/admin/client-keys':('client_keys',['id','access_group_id','status','expires_at_ms']),
      '/admin/access-groups':('access_groups',['id','status']),
      '/admin/native-accounts?limit=100':('native_accounts',['id','provider','auth_status','enabled','revision']),
      '/admin/runtime/availability':('availability',[]),
      '/admin/operations/account-pools?limit=100':('account_pools',['provider_id','provider_kind','channel_id','adapter_id','api_format','account_id','account_kind','account_revision','account_status','binding_enabled','configured_enabled','route_ids','egress_policy_id']),
      '/admin/operations/provider-account-pools?limit=100':('native_pools',['provider_id','channel_id','account_id','account_kind','active_leases','auth_status','enabled','expires_at_ms','max_concurrency','runtime_status','credential_revision']),
      '/admin/operations/provider-egress-status?limit=100':('provider_egress',['provider_id','channel_id','domain','target_kind','target_id','credential_id','credential_revision','session_revision','state','deadline_ms','expires_at_ms']),
      '/admin/egress-policies':('egress_policies',['id','allowed_hosts','allowed_ports','allowed_schemes','max_redirects','redirect_mode']),
    }
    result['inventory']={}
    for path,(name,fields) in projections.items():
        rows,etags,field_names=[],[],set()
        next_path=path
        for page_number in range(20):
            value,etag,error=get(next_path)
            etags.append(etag)
            if error:break
            if name=='availability':
                page=value if isinstance(value,list) else [value]
                rows=[{k:row.get(k) for k in ['endpoint_id','credential_id','upstream_model','availability','reason','snapshot_version']} for row in page]
                for row in rows:row['credential_id']=opaque(row.get('credential_id'))
                break
            page=value.get('items',[]) if isinstance(value,dict) else value
            for row in page or []:
                field_names.update(row)
                projected={k:row.get(k) for k in fields}
                if name=='credentials':
                    managed=row.get('managed') or {}
                    credential=managed.get('credential') or managed
                    projected={k:credential.get(k) for k in ['id','upstream_id','kind','status','revision','secret_present']}
                    projected['provider']=row.get('provider')
                    projected['native']=row.get('native')
                    projected['id']=row.get('id')
                if name=='endpoints' and projected.get('base_url'):
                    url=urllib.parse.urlsplit(projected['base_url']);projected['base_url']=url.scheme+'://'+(url.hostname or '')+url.path
                if name in ('credentials','native_accounts','client_keys'):projected['id']=opaque(projected['id'])
                for k in ['credential_id','access_group_id','account_id']:
                    if k in projected:projected[k]=opaque(projected[k])
                if name=='access_groups':projected['id']=opaque(projected['id'])
                rows.append(projected)
            cursor=value.get('next_cursor') if isinstance(value,dict) else None
            if not cursor:break
            next_path=path+('&' if '?' in path else '?')+'cursor='+urllib.parse.quote(cursor,safe='')
        else:error='pagination_limit'
        result['inventory'][name]={'items':rows,'etags':etags,'error':error,'complete':error is None and len(set(etags))<=1,'item_field_names':sorted(field_names)}
    versions_after,_,_=get('/admin/config-versions')
    result['configuration_stable']=active==[v for v in versions_after or [] if v.get('status')=='active']
    result['route_grants']=[]
    groups,_,_=get('/admin/access-groups')
    groups=groups.get('items',[]) if isinstance(groups,dict) else groups
    for group in groups or []:
        grants,_,error=get('/admin/access-groups/'+urllib.parse.quote(group['id'],safe='')+'/routes')
        grants=grants.get('items',[]) if isinstance(grants,dict) else grants
        result['route_grants'].append({'access_group_evidence_id':opaque(group['id']),'error':error,'routes':[{k:row.get(k) for k in ['route_id','enabled']} for row in grants or []]})
    result['bindings']=[]
    endpoints,_,_=get('/admin/endpoints')
    endpoints=endpoints.get('items',[]) if isinstance(endpoints,dict) else endpoints
    for endpoint in endpoints or []:
        bindings,_,error=get('/admin/endpoints/'+urllib.parse.quote(endpoint['id'],safe='')+'/credential-bindings')
        bindings=bindings.get('items',[]) if isinstance(bindings,dict) else bindings
        result['bindings'].append({'endpoint_id':endpoint['id'],'error':error,'items':[{'credential_evidence_id':opaque(row.get('credential_id')),'enabled':row.get('enabled')} for row in bindings or []]})
runtime['stable_process']=runtime['stable_process'] and pid==int(subprocess.check_output(['systemctl','show',service,'--value','--property=MainPID'])) and start==(proc/'stat').read_text().rsplit(')',1)[1].split()[19]
result['inventory_observed_at_utc']=datetime.datetime.now(datetime.timezone.utc).isoformat()
print(json.dumps(result))
'''


def remote_read(host, service, inventory=False, sudo=False):
    require(re.fullmatch(r"[A-Za-z0-9_.@-]+", host or "") is not None and not host.startswith("-"), "invalid_host")
    require(re.fullmatch(r"[A-Za-z0-9_.@-]+\.service", service or "") is not None, "invalid_service")
    script = REMOTE_READ.replace("SERVICE", repr(service)).replace("INVENTORY", repr(inventory))
    command = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", host, "sudo -n python3 -" if sudo else "python3 -"]
    completed = subprocess.run(command, input=script, text=True, capture_output=True, timeout=180)
    available(completed.returncode == 0, "remote_read_unavailable")
    return load_json(completed.stdout)


REMOTE_COLLECT = r'''
import sqlite3
database=sqlite3.connect('file:'+str(pathlib.Path(opts['--state-dir'])/'control.sqlite3')+'?mode=ro',uri=True,timeout=5)
database.row_factory=sqlite3.Row
response_id=RESPONSE_ID
usage_sources={row[0]:json.loads(row[1])['usage'] for row in database.execute("SELECT event_id,payload_json FROM gateway_event_log WHERE event_type='usage' AND json_extract(payload_json,'$.usage.response_id')=? LIMIT 3",(response_id,))}
observations=[{k:row.get(k) for k in ['request_id','response_id','attempt_id','usage']} for row in usage_sources.values()]
request_ids=sorted(set(row['request_id'] for row in observations))
evidence={'correlation':'response_id_usage' if len(request_ids)==1 else None,'request_id':request_ids[0] if len(request_ids)==1 else None,'attempts':[],'usages':observations,'ledger':None,'observed_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
if len(request_ids)==1:
    request_id=request_ids[0]
    attempts=[json.loads(row[0])['attempt'] for row in database.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='attempt' AND request_id=? ORDER BY event_ordinal LIMIT 17",(request_id,))]
    fields=['request_id','attempt_id','channel','endpoint_id','route_id','route_candidate_id','upstream_model','credential_revision','config_version_id','config_revision','egress_revision','started_at_ms','ended_at_ms','retry_decision']
    for row in attempts:
        item={k:row.get(k) for k in fields}
        item['credential_evidence_id']=opaque(row.get('credential_id'))
        item['egress_evidence_id']=opaque(row.get('egress_id'))
        item['outcome']='success' if row.get('outcome')=='succeeded' else 'failed'
        item['source_record_sha256']=hashlib.sha256(json.dumps(row,sort_keys=True,separators=(',',':')).encode()).hexdigest()
        evidence['attempts'].append(item)
    evidence['ledger']=[]
    columns={row[1] for row in database.execute('PRAGMA table_info(billing_ledger_entries)')}
    fields=['source_event_id','request_id','response_id','input_tokens','output_tokens','reasoning_tokens','cache_read_tokens','cache_creation_tokens','cached_tokens','usage_evidence_json']
    select=','.join(field if field in columns else 'NULL AS '+field for field in fields)
    for row in database.execute('SELECT '+select+' FROM billing_ledger_entries WHERE request_id=? AND response_id=? LIMIT 3',(request_id,response_id)):
        source=usage_sources.get(row['source_event_id'])
        source_matches=source is not None and source.get('request_id')==row['request_id'] and source.get('response_id')==row['response_id']
        metadata=json.loads(row['usage_evidence_json']) if row['usage_evidence_json'] else {}
        usage={k:row[k] for k in ['input_tokens','output_tokens','reasoning_tokens','cache_read_tokens','cache_creation_tokens','cached_tokens']}
        usage.update(provenance=metadata.get('provenance'),input_accounting=metadata.get('input_accounting'))
        evidence['ledger'].append({'request_id':row['request_id'],'response_id':row['response_id'],'attempt_id':source.get('attempt_id') if source_matches else None,'usage':usage,'attribution_method':'exact_usage_source_event' if source_matches else None,'source_event_evidence_id':opaque(row['source_event_id'])})
database.close()
print(json.dumps({'runtime':runtime,'evidence':evidence}))
'''


def collect_attempt(host, service, response_id, sudo=False):
    require(isinstance(response_id, str) and len(response_id) <= 256, "invalid_response_identity")
    # Reuse the process observation; only the final print is replaced. No credential read.
    script = REMOTE_READ.replace("SERVICE", repr(service)).replace("INVENTORY", "False").rsplit("print(json.dumps(result))", 1)[0] + REMOTE_COLLECT.replace("RESPONSE_ID", repr(response_id))
    require(re.fullmatch(r"[A-Za-z0-9_.@-]+", host or "") is not None and not host.startswith("-"), "invalid_host")
    require(re.fullmatch(r"[A-Za-z0-9_.@-]+\.service", service or "") is not None, "invalid_service")
    completed = subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", host, "sudo -n python3 -" if sudo else "python3 -"], input=script, text=True, capture_output=True, timeout=30)
    available(completed.returncode == 0, "exact_attempt_collection_unavailable")
    return load_json(completed.stdout)


def bind_manifest(runtime, manifest):
    require(manifest.get("schema_version") == "cpa-rust-gateway-artifact-manifest-v1", "invalid_artifact_manifest")
    matches = [row for row in manifest.get("files", []) if row.get("name") == "gateway-" + manifest.get("target", "")]
    runtime["manifest_sha256"] = digest(manifest)
    runtime["manifest_matches"] = manifest.get("revision") == runtime.get("artifact_sha") and any(row.get("sha256") == runtime.get("artifact_sha256") for row in matches)
    return runtime


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_):
        return None


def http_call(base_url, key, protocol, body, timeout=90):
    parsed = urllib.parse.urlsplit(base_url)
    require(parsed.scheme == "https" or parsed.scheme == "http" and parsed.hostname in ("127.0.0.1", "::1", "localhost"), "unsafe_data_url")
    require(not parsed.username and not parsed.password and not parsed.query and not parsed.fragment, "unsafe_data_url")
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    request = urllib.request.Request(base_url.rstrip("/") + PROTOCOLS[protocol], data=json.dumps(body, separators=(",", ":")).encode(), headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"}, method="POST")
    started = time.monotonic()
    try:
        response = opener.open(request, timeout=timeout)
    except socket.timeout:
        raise TimeoutError("acceptance_deadline") from None
    except urllib.error.HTTPError as error:
        response = error
    with response:
        wire_response = response.fp if isinstance(response, urllib.error.HTTPError) else response
        raw = bytearray()
        while len(raw) <= MAX_BODY:
            if wire_response.fp is None:
                break
            remaining = timeout - (time.monotonic() - started)
            if remaining <= 0:
                raise TimeoutError("acceptance_deadline")
            # urllib owns an HTTPResponse with a SocketIO raw stream on supported Python.
            # read1 makes one bounded network read, allowing an absolute deadline even when
            # a peer dribbles bytes indefinitely. This is tested with an actual slow socket.
            wire_response.fp.raw._sock.settimeout(min(remaining, 15))
            try:
                chunk = wire_response.read1(min(65536, MAX_BODY + 1 - len(raw)))
            except socket.timeout:
                raise TimeoutError("acceptance_deadline") from None
            if not chunk:
                break
            raw.extend(chunk)
        require(len(raw) <= MAX_BODY, "response_size")
        content_type = response.headers.get_content_type()
        require(content_type == ("text/event-stream" if body["stream"] and response.status == 200 else "application/json"), "wrong_content_type")
        return {"http_status": response.status, "wire": raw.decode("utf-8"), "request_id": response.headers.get("x-request-id"), "duration_ms": round((time.monotonic() - started) * 1000)}


def run_basic(plan, runtime, key, base_url, progress=None):
    # Captured attribution is joined later by request-ID / response-ID, never by time window.
    # Without it every affected row is BLOCKED and no subsequent inference is sent.
    bundle = {"schema_version": 2, "layer": plan["layer"], "source_sha": plan["source_sha"], "runtime": runtime, "target": plan["target"], "scenarios": {}, "boundaries": plan.get("boundaries", {}), "extensions": plan.get("extensions", {})}
    try:
        execute_basic(plan, runtime, key, base_url, bundle, progress)
    except (ValueError, KeyError, TypeError, IndexError) as error:
        bundle["execution_failure"] = verdict(lambda: (_ for _ in ()).throw(error))
    except (OSError, subprocess.SubprocessError):
        bundle["execution_failure"] = {"status": "FAIL", "category": "transport_or_collector_failure"}
    return bundle


def execute_basic(plan, runtime, key, base_url, bundle, progress):
    verify_runtime(runtime, plan["source_sha"], plan["layer"])
    verify_target(plan["target"])
    target, protocol = plan["target"], plan["target"]["protocol"]
    for scenario, count in (("text_multi_turn", 2), ("single_tool", 2), ("two_tool_cycles", 3)):
        token = "ACCEPT:" + os.urandom(12).hex()
        request, samples = make_request(target, scenario, token), []
        bundle["scenarios"][scenario] = samples
        seen = set()
        for index in range(count):
            sample = http_call(base_url, key, protocol, request)
            sample["request"] = copy.deepcopy(request)
            sample["sent_at_utc"] = now()
            samples.append(sample)
            if index == count - 1:
                sample["expected_text"] = token
            if progress:
                progress(bundle)
            answer, calls, usage = basic_answer(target, sample, scenario, index, count, seen)
            # The private observation directory must be supplied by the existing protected
            # event/ledger collector. It is independent of public success; no inferred account.
            if plan.get("collector"):
                config = plan["collector"]
                observed = collect_attempt(config["host"], config.get("service", "cpa-rust-gateway.service"), answer["id"], config.get("sudo", False))
                sample["evidence"] = observed["evidence"]
            else:
                path = Path(plan["observation_dir"]) / (opaque(answer["id"]) + ".json")
                available(path.is_file(), "exact_attempt_observation_required")
                observed = load_json(path.read_text())
                sample["evidence"] = observed.get("evidence", observed)
            sample["runtime"] = observed.get("runtime")
            verify_turn_runtime(sample["runtime"], runtime, plan["source_sha"], plan["layer"])
            verify_attempts(target, sample["evidence"], answer["id"])
            verify_usage_lineage(usage, sample["evidence"])
            if index < count - 1:
                next_token = "Repeat the remembered token exactly." if scenario == "text_multi_turn" else "Call lookup with value 1 again." if index < count - 2 else "Return exactly " + token
                sample["next_token"] = next_token
                request = conversation_step(protocol, request, answer, calls, next_token)
                if calls:
                    request["tool_choice"] = tool_choice(protocol, index < count - 2)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    inspect = commands.add_parser("inspect", help="GET-only current process/config/account/route inventory")
    inspect.add_argument("--host", required=True)
    inspect.add_argument("--service", default="cpa-rust-gateway.service")
    inspect.add_argument("--sudo", action="store_true", help="Use existing noninteractive read permission")
    inspect.add_argument("--manifest", type=Path)
    inspect.add_argument("--out", type=Path, required=True)
    collect = commands.add_parser("collect", help="Read exact response-ID Usage, Attempt and ledger records; never infer missing revisions")
    collect.add_argument("--host", required=True)
    collect.add_argument("--service", default="cpa-rust-gateway.service")
    collect.add_argument("--sudo", action="store_true")
    collect.add_argument("--response-id", required=True)
    collect.add_argument("--out", type=Path, required=True)
    audit = commands.add_parser("audit", help="Review a private wire/attempt/ledger bundle without inference")
    audit.add_argument("--bundle", type=Path, required=True)
    audit.add_argument("--out", type=Path, required=True)
    run = commands.add_parser("run", help="Execute one frozen combination; seven basic requests, no retries")
    run.add_argument("--plan", type=Path, required=True)
    run.add_argument("--runtime", type=Path, required=True)
    run.add_argument("--base-url", required=True)
    run.add_argument("--client-key-env", default="CPAR_ACCEPTANCE_CLIENT_KEY")
    run.add_argument("--execute", action="store_true")
    run.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    receipt = {"schema_version": 2, "status": "BLOCKED", "observed_at_utc": now(), "value_free": True}
    try:
        if args.command == "inspect":
            receipt = remote_read(args.host, args.service, True, args.sudo)
            receipt.update(schema_version=2, layer="READ_ONLY_ENVIRONMENT", value_free=True, status="PASS")
            if args.manifest:
                bind_manifest(receipt["runtime"], load_json(args.manifest.read_text()))
        elif args.command == "audit":
            receipt = audit_bundle(load_json(args.bundle.read_text()))
        elif args.command == "collect":
            receipt = collect_attempt(args.host, args.service, args.response_id, args.sudo)
            receipt.update(schema_version=2, layer="READ_ONLY_ATTRIBUTION", status="PASS", value_free=True)
        else:
            plan = load_json(args.plan.read_text())
            runtime = load_json(args.runtime.read_text()).get("runtime", {})
            receipt = audit_bundle({"layer": plan.get("layer"), "source_sha": plan.get("source_sha"), "runtime": runtime, "target": plan.get("target", {})})
            verify_runtime(runtime, plan.get("source_sha"), plan.get("layer"))
            verify_target(plan.get("target", {}))
            available(args.execute, "execution_not_requested")
            available(plan.get("collector") is not None or plan.get("observation_dir") is not None and Path(plan["observation_dir"]).is_dir(), "exact_attempt_collector_required")
            key = os.environ.get(args.client_key_env)
            available(bool(key), "client_authentication_unavailable")
            def progress(bundle):
                pending = audit_bundle(bundle)
                pending.update(status="NOT_RUN", category="execution_in_progress")
                write_receipt(args.out, pending)
            receipt = audit_bundle(run_basic(plan, runtime, key, args.base_url, progress))
    except (ValueError, KeyError, TypeError, IndexError, OSError, subprocess.SubprocessError) as error:
        result = verdict(lambda: (_ for _ in ()).throw(error)) if isinstance(error, (ValueError, KeyError, TypeError, IndexError)) else {"status": "BLOCKED", "category": "environment_or_transport_unavailable"}
        receipt.update(result)
    receipt["command"] = "cpar-batch-c-live.py --new-build " + args.command
    receipt["environment"] = {"platform": sys.platform, "python": sys.version.split()[0]}
    write_receipt(args.out, receipt)
    print(json.dumps({key: receipt.get(key) for key in ("status", "basic_status", "layer", "category")}), flush=True)
    return 0 if receipt["status"] == "PASS" else 2 if receipt["status"] in ("BLOCKED", "NOT_RUN") else 1


if __name__ == "__main__":
    raise SystemExit(main())
