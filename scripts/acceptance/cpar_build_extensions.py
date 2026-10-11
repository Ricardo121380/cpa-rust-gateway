"""Extension execution for the formal --new-build entry, with the same collector.

Only an owned local simulation can execute in this version. Live basic approval
does not authorize extension operations. Raw requests, roots and socket frames
are retained in a mode-0600 bundle; the public receipt is an allowlist.
"""
import base64
import contextlib
import copy
import datetime
import hashlib
import http.client
import io
import json
import os
from pathlib import Path
import re
import secrets
import select
import shlex
import socket
import sqlite3
import ssl
import struct
import subprocess
import time
import urllib.parse

import cpar_new_build_acceptance as oracle

IMPLEMENTED = {
    "visible_reasoning", "reasoning_history", "store_read", "foreign_owner",
    "deleted_read", "exact_continuation", "compact_roundtrip",
    "compact_foreign_owner", "ws_multiturn", "ws_handshake_rejection",
}
CASE_CAPABILITY = {case: name for name, cases in oracle.EXTENSION_CHECKS.items()
                   if name != "parallel" for case in cases}
REQUEST_LIMITS = {
    "visible_reasoning": 1, "reasoning_history": 2, "store_read": 1,
    "foreign_owner": 1, "deleted_read": 1, "exact_continuation": 2,
    "compact_roundtrip": 3, "compact_foreign_owner": 1,
    "ws_multiturn": 2, "ws_handshake_rejection": 0,
}
WS_TERMINAL_OBSERVATION_MS = 100


def handshake_headers(raw):
    head, separator, body = raw.partition(b"\r\n\r\n")
    oracle.require(bool(separator) and len(head) <= 16384, "ws_handshake_bounds")
    lines = head.decode("ascii").split("\r\n")
    oracle.require(re.fullmatch(r"HTTP/1\.[01] [1-5][0-9]{2}(?: .*)?", lines[0]) is not None, "ws_handshake_status_invalid")
    fields = {}
    for line in lines[1:]:
        name, value = line.split(":", 1)
        oracle.require(name.lower() not in fields, "duplicate_ws_header")
        fields[name.lower()] = value.strip()
    return int(lines[0].split()[1]), fields, body


def verify_handshake(sample, status):
    oracle.available(isinstance(sample.get("wire_bytes_b64"), str) and isinstance(sample.get("ws_request"), dict), "ws_handshake_capture_missing")
    actual, fields, body = handshake_headers(base64.b64decode(sample["wire_bytes_b64"], validate=True))
    oracle.require(actual == status == sample.get("handshake_status") == sample.get("http_status"), "ws_handshake_status_changed")
    request = sample["ws_request"]
    oracle.require(request == {"method": "GET", "path": sample.get("ws_path"), "origin": sample.get("ws_origin"),
                               "nonce": sample.get("ws_nonce"), "version": "13", "upgrade": "websocket", "connection": "Upgrade"} and
                   isinstance(request["path"], str) and request["path"].endswith("/responses") and len(base64.b64decode(request["nonce"], validate=True)) == 16, "ws_handshake_request_changed")
    if status == 101:
        expected = base64.b64encode(hashlib.sha1((request["nonce"] + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        oracle.require(request["origin"] is None and not body and fields.get("sec-websocket-accept") == sample.get("ws_accept") == expected and
                       fields.get("upgrade", "").lower() == "websocket" and "upgrade" in fields.get("connection", "").lower().split(", "), "ws_handshake_unconfirmed")
    else:
        oracle.require(request["origin"] == "https://example.invalid" and "transfer-encoding" not in fields and
                       fields.get("content-length", "").isdigit() and int(fields["content-length"]) == len(body) <= oracle.MAX_BODY, "ws_handshake_body_incomplete")


def collect_local(state_dir, response_id, runtime):
    """Exact SQLite/source-event join, using the formal collector unchanged."""
    oracle.require(isinstance(response_id, str) and len(response_id) <= 256, "invalid_response_identity")
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        exec(oracle.REMOTE_COLLECT.replace("RESPONSE_ID", repr(response_id)), {
            "opts": {"--state-dir": str(state_dir)}, "pathlib": __import__("pathlib"),
            "json": json, "datetime": datetime, "hashlib": hashlib,
            "opaque": oracle.opaque, "runtime": runtime,
        })
    return oracle.load_json(output.getvalue())


def observe_local_process(collector, baseline):
    binary = Path(collector["binary"]).resolve(strict=True)
    pid = baseline["pid"]
    command = subprocess.check_output(["ps", "-p", str(pid), "-o", "command="], text=True).strip()
    args = shlex.split(command)
    oracle.require(args and Path(args[0]).resolve() == binary and args[1] == "serve", "local_process_identity_changed")
    oracle.require(Path(args[args.index("--state-dir") + 1]).resolve() == Path(collector["local_state_dir"]).resolve(), "local_process_state_changed")
    started = subprocess.check_output(["ps", "-p", str(pid), "-o", "lstart="], text=True).strip() + ":" + str(pid)
    raw = binary.read_bytes()
    revisions = set(match.decode() for match in re.findall(rb"gateway-release-revision=([0-9a-f]{40})", raw))
    oracle.require(revisions == {baseline["source_sha"]}, "local_binary_revision_unconfirmed")
    current = {**baseline, "method": "local_subprocess", "observed_at_utc": oracle.now(),
               "process_start": started, "artifact_sha256": hashlib.sha256(raw).hexdigest(), "stable_process": True}
    oracle.verify_turn_runtime(current, baseline, baseline["source_sha"], "LOCAL_SIMULATED")
    return current


def attempt_count(state_dir):
    with sqlite3.connect("file:" + str(Path(state_dir) / "control.sqlite3") + "?mode=ro", uri=True) as database:
        return database.execute("SELECT count(*) FROM gateway_event_log WHERE event_type='attempt'").fetchone()[0]


def request_binding(sample):
    return oracle.digest({"source_sha": sample["source_sha"], "target": sample["target"],
                          "runtime": {key: sample["runtime"].get(key) for key in ("instance", "pid", "process_start", "artifact_sha256")},
                          "request": sample["request"]})


def verify_controls(request, target, *, summary=False):
    if summary:
        oracle.require(set(request) == {"model", "previous_response_id", "stream"}, "compact_summary_control_changed")
        return
    fields = {"model", "stream", "input" if target["protocol"] == "responses" else "messages", "previous_response_id", "store"}
    oracle.require(set(request) - fields == set(target["parameters"]) and
                   all(request[key] == value for key, value in target["parameters"].items()), "changed_request_control")


def verify_capture(sample, target):
    """Validate every captured successful inference, including an owned root."""
    oracle.available(sample.get("extension_sample_version") == 1, "extension_sample_version_required")
    oracle.require(sample.get("target") == target and sample.get("layer") == "LOCAL_SIMULATED", "wrong_extension_target")
    oracle.verify_turn_runtime(sample["runtime"], sample["frozen_runtime"], sample["source_sha"], sample["layer"])
    oracle.require(sample.get("request_binding_sha256") == request_binding(sample), "extension_request_binding_changed")
    request = sample["request"]
    oracle.available(isinstance(sample.get("request_wire_b64"), str) and isinstance(sample.get("wire_bytes_b64"), str), "extension_raw_capture_missing")
    sent = base64.b64decode(sample["request_wire_b64"], validate=True)
    raw = base64.b64decode(sample["wire_bytes_b64"], validate=True)
    oracle.require(oracle.load_json(sent.decode()) == (sample["ws_frame"] if "ws_frame" in sample else request), "extension_request_wire_changed")
    if "ws_frame" not in sample:
        oracle.require(raw.decode() == sample["wire"], "extension_response_wire_changed")
    else:
        oracle.available(sample.get("terminal_observation_closed") is True and sample.get("terminal_observation_ms") == WS_TERMINAL_OBSERVATION_MS, "ws_terminal_observation_open")
        offset, frames = 0, []
        while offset < len(raw):
            oracle.require(offset + 2 <= len(raw), "ws_capture_incomplete")
            head, size = raw[offset], raw[offset + 1];offset += 2
            oracle.require(head & 0x80 and not head & 0x70 and not size & 0x80, "unsupported_ws_frame")
            size &= 0x7f
            width = 2 if size == 126 else 8 if size == 127 else 0
            oracle.require(offset + width <= len(raw), "ws_capture_incomplete")
            if width:
                size = int.from_bytes(raw[offset:offset + width], "big");offset += width
            oracle.require(size <= oracle.MAX_BODY and offset + size <= len(raw), "ws_capture_incomplete")
            payload = raw[offset:offset + size];offset += size
            opcode = head & 0xf
            if opcode in (9, 10):
                oracle.require(size <= 125, "ws_control_frame_size")
            else:
                oracle.require(opcode == 1, "unsupported_ws_frame")
                frames.append(oracle.load_json(payload.decode()))
        oracle.require(sample["wire"] == "".join("data: " + json.dumps(frame, ensure_ascii=False) + "\n\n" for frame in frames), "extension_response_wire_changed")
    summary = sample.get("operation") == "compact_summary"
    controls = {"max_output_tokens": 2048} if summary else target["parameters"]
    expected_stream = False if summary else target["mode"] == "sse"
    oracle.require(request.get("model") == target["public_model"] and request.get("stream") is expected_stream and sample.get("http_status") == 200, "extension_response_control_changed")
    verify_controls(request, target, summary=summary)
    value = oracle.load_json(sample["wire"]) if summary or target["mode"] == "json" else oracle.decode_stream(target["protocol"], sample["wire"].encode())
    oracle.require(value.get("model") == target["response_model"], "wrong_response_model")
    if summary:
        oracle.require(value.get("status") == "completed" and value.get("error") is None, "compact_summary_terminal_changed")
    else:
        oracle.observe(target["protocol"], value)
    actual_target = {**target, "mode": "json", "parameters": controls} if summary else target
    oracle.verify_attempts(actual_target, sample["evidence"], value["id"])
    oracle.verify_usage_lineage(oracle.native_usage(target["protocol"], value), sample["evidence"])
    return value


def verify_sample(sample, target):
    """Stricter generated-sample schema; historical audit formats remain readable."""
    oracle.require(sample.get("layer") == "LOCAL_SIMULATED", "extension_layer_not_authorized")
    oracle.verify_turn_runtime(sample.get("runtime"), sample["frozen_runtime"], sample["source_sha"], sample["layer"])
    oracle.require(sample.get("request_binding_sha256") == request_binding(sample), "extension_request_binding_changed")
    request = sample["request"]
    oracle.require(request.get("model") == target["public_model"], "extension_request_model_changed")
    verify_controls(request, target)
    case = sample["case"]
    if case == "ws_handshake_rejection":
        verify_handshake(sample, 403)
    if case in {"visible_reasoning", "reasoning_history"}:
        key = "reasoning_effort" if target["protocol"] == "chat" else "reasoning"
        oracle.available(key in target["parameters"], "explicit_reasoning_control_required")
    if case in {"visible_reasoning", "reasoning_history", "store_read", "exact_continuation", "compact_roundtrip", "ws_multiturn"}:
        oracle.require(sample.get("http_status") == 200 and request.get("stream") is (target["mode"] == "sse"), "extension_response_control_changed")
        value = oracle.decode_stream(target["protocol"], sample["wire"].encode()) if target["mode"] == "sse" else oracle.load_json(sample["wire"])
        text, calls = oracle.observe(target["protocol"], value)
        oracle.available(isinstance(sample.get("expected_text"), str) and bool(sample["expected_text"]), "extension_answer_oracle_missing")
        oracle.require(not calls and text == sample["expected_text"], "history_or_result_not_used")
    if case == "compact_roundtrip":
        compact = oracle.load_json(sample["compact_wire"])
        summary_target = {**target, "mode": "json", "parameters": {"max_output_tokens": 2048}}
        oracle.require(sample.get("compact_target") == summary_target, "compact_summary_target_changed")
        oracle.require(sample.get("compact_request") == {"model": target["public_model"], "previous_response_id": sample["parent_response_id"], "stream": False}, "compact_summary_control_changed")
        oracle.require("previous_response_id" not in request, "compact_consume_parent_conflict")
        oracle.verify_attempts(summary_target, sample["compact_evidence"], compact["id"])
        oracle.verify_usage_lineage(oracle.native_usage("responses", compact), sample["compact_evidence"])
    if case == "ws_multiturn":
        for turn in sample["ws_turns"]:
            oracle.require(turn.get("ws_frame") == {**turn["request"], "type": "response.create"}, "ws_request_frame_changed")
            oracle.require(turn["request"].get("model") == target["public_model"] and turn["request"].get("stream") is True, "changed_request_control")
            verify_controls(turn["request"], target)
            oracle.verify_turn_runtime(turn.get("runtime"), sample["frozen_runtime"], sample["source_sha"], sample["layer"])
            oracle.require(turn.get("request_binding_sha256") == request_binding(turn), "extension_request_binding_changed")
            value = oracle.decode_stream("responses", turn["wire"].encode())
            oracle.require(value.get("model") == target["response_model"], "wrong_response_model")
            oracle.verify_attempts(target, turn["evidence"], value["id"])
            oracle.verify_usage_lineage(oracle.native_usage("responses", value), turn["evidence"])
    if case in {"foreign_owner", "compact_foreign_owner"}:
        oracle.require(sample.get("foreign_auth_status") == 200, "extension_authentication_unconfirmed")
        oracle.require(sample.get("foreign_visible_model") == target["public_model"], "extension_foreign_model_access_unconfirmed")
    if case in {"foreign_owner", "deleted_read", "compact_foreign_owner"}:
        root = sample.get("root_sample", {})
        value = verify_capture(root, target)
        oracle.require(root["request"].get("store") is True and sample.get("read_response_id") == value["id"] and sample["request"] == root["request"], "extension_resource_unconfirmed")
    if case in {"foreign_owner", "deleted_read", "compact_foreign_owner", "ws_handshake_rejection"}:
        oracle.available(isinstance(sample.get("boundary_before"), int) and isinstance(sample.get("boundary_after"), int), "boundary_observation_unavailable")
        oracle.require(sample["boundary_before"] == sample["boundary_after"], "rejected_extension_sent_upstream")


def verify_resource_operations(bundle, root):
    """Derive resource conclusions from the ordered captures for this exact root."""
    sample, target, case = bundle["verified_sample"], bundle["target"], bundle["case"]
    rows = bundle.get("resource_operations")
    oracle.available(isinstance(rows, list), "extension_resource_capture_missing")
    oracle.require(len(rows) == (1 if case == "store_read" else 3), "extension_resource_sequence_changed")
    owner = target["declaration_evidence"]["client_key_evidence_id"]
    foreign = sample.get("actual_client_evidence_id")
    path = "/responses/" + urllib.parse.quote(root["id"], safe="")
    expected = [("GET", path, None, owner)]
    if case in ("foreign_owner", "compact_foreign_owner"):
        oracle.require(sample.get("owner_client_evidence_id") == owner and isinstance(foreign, str) and
                       re.fullmatch("[0-9a-f]{64}", foreign) is not None and foreign != owner, "extension_owner_unknown")
        expected.append(("GET", "/models", None, foreign))
    elif case == "deleted_read":
        expected.append(("DELETE", path, None, owner))
    if case == "compact_foreign_owner":
        expected.append(("POST", "/responses/compact", {"model": target["public_model"], "previous_response_id": root["id"], "stream": False}, foreign))
    elif case != "store_read":
        expected.append(("GET", path, None, foreign if case == "foreign_owner" else owner))
    for row, (method, resource, request, identity) in zip(rows, expected):
        oracle.require(isinstance(row, dict) and (row.get("method"), row.get("path"), row.get("request"), row.get("client_evidence_id")) ==
                       (method, resource, request, identity), "extension_resource_sequence_changed")
        oracle.verify_turn_runtime(row.get("runtime"), bundle["runtime"], bundle["source_sha"], bundle["layer"])
        oracle.available(isinstance(row.get("wire_bytes_b64"), str) and isinstance(row.get("request_wire_b64"), str), "extension_resource_capture_missing")
        raw = base64.b64decode(row["wire_bytes_b64"], validate=True)
        sent = base64.b64decode(row["request_wire_b64"], validate=True)
        oracle.require(raw.decode() == row.get("wire") and (not sent if request is None else oracle.load_json(sent.decode()) == request), "extension_resource_wire_changed")
        oracle.available(oracle.integer(row.get("boundary_before")) and oracle.integer(row.get("boundary_after")), "boundary_observation_unavailable")
        oracle.require(row["boundary_before"] == row["boundary_after"], "rejected_extension_sent_upstream")
    read = rows[0]
    oracle.require(read.get("http_status") == sample.get("read_http_status") == 200 and
                   read["wire"] == sample.get("read_wire") and oracle.load_json(read["wire"]) == root, "extension_resource_unconfirmed")
    if case in ("foreign_owner", "compact_foreign_owner"):
        authentication = rows[1]
        oracle.require(read["http_status"] == sample.get("owner_read_http_status") and read["wire"] == sample.get("owner_read_wire") and
                       authentication.get("http_status") == sample.get("foreign_auth_status") == 200 and
                       any(model.get("id") == target["public_model"] for model in oracle.load_json(authentication["wire"]).get("data", [])), "extension_authentication_unconfirmed")
    elif case == "deleted_read":
        deletion = rows[1]
        oracle.require(deletion.get("http_status") == sample.get("delete_http_status") and deletion["http_status"] in (200, 204), "deletion_not_observed")
        oracle.require(not deletion["wire"] if deletion["http_status"] == 204 else
                       oracle.load_json(deletion["wire"]) == {"id": root["id"], "object": "response.deleted", "deleted": True}, "deletion_not_observed")
    if case != "store_read":
        denied = rows[-1]
        oracle.require(denied.get("http_status") == sample.get("http_status") == 404 and
                       denied["boundary_before"] == sample.get("boundary_before") and denied["boundary_after"] == sample.get("boundary_after"), "extension_rejection_missing")
        oracle.require(isinstance(oracle.load_json(denied["wire"]).get("error"), dict), "extension_rejection_missing")


def verify_sequence(bundle, values):
    """A passed execution has exactly the reviewed roots and ordered operations."""
    captures, sample, case, target = bundle["samples"], bundle["verified_sample"], bundle["case"], bundle["target"]
    fields = ("extension_sample_version", "case", "layer", "source_sha", "target", "frozen_runtime", "runtime", "request", "request_binding_sha256", "request_wire_b64", "wire_bytes_b64", "wire", "expected_text")
    def matches(actual, captured, successful=True):
        keys = (*fields, "http_status", "evidence", "operation") if successful else fields
        oracle.require(all(actual.get(key) == captured.get(key) for key in keys), "extension_sample_not_captured")
    if case == "ws_handshake_rejection":
        oracle.require(sample == captures[0], "extension_sample_not_captured")
        verify_handshake(sample, 403)
        return
    ids = [value["id"] for value in values]
    oracle.require(len(set(ids)) == len(ids) and len({row["evidence"]["request_id"] for row in captures}) == len(captures), "extension_capture_identity_reused")
    oracle.require("previous_response_id" not in captures[0]["request"], "extension_root_missing")
    stored = case not in ("visible_reasoning", "reasoning_history", "ws_multiturn")
    for index, capture in enumerate(captures):
        oracle.require(capture.get("operation") == ("compact_summary" if case == "compact_roundtrip" and index == 1 else "response_create"), "extension_operation_sequence_changed")
        oracle.require(capture["request"].get("store") is True if stored and index == 0 else "store" not in capture["request"], "extension_store_control_changed")
    negative = case in ("foreign_owner", "deleted_read", "compact_foreign_owner")
    matches(sample, captures[-1], successful=not negative)
    if negative or case == "store_read":
        oracle.require(sample.get("root_sample") == captures[0], "extension_root_not_captured")
        oracle.require(oracle.load_json(sample["read_wire"]) == values[0], "extension_resource_unconfirmed")
        verify_resource_operations(bundle, values[0])
    if case == "reasoning_history":
        oracle.require(sample.get("previous_request") == captures[0]["request"] and oracle.load_json(sample["previous_wire"]) == values[0], "extension_history_root_not_captured")
        oracle.extension_check("reasoning", "visible_reasoning", {**captures[0], "case": "visible_reasoning"}, target)
    elif case in ("exact_continuation", "compact_roundtrip", "ws_multiturn"):
        oracle.require(sample.get("parent_response_id") == ids[0] and sample.get("parent_target") == target, "continuation_parent_not_captured")
        if case == "compact_roundtrip":
            summary = captures[1]
            oracle.require(summary["request"]["previous_response_id"] == ids[0] and
                           sample.get("compact_request") == summary["request"] and sample.get("compact_wire") == summary["wire"] and
                           sample.get("compact_evidence") == summary["evidence"] and sample.get("compact_http_status") == summary["http_status"], "compact_summary_not_captured")
            inputs, outputs = sample["request"]["input"], values[1]["output"]
            oracle.require(len(inputs) == len(outputs) + 1 and inputs[:-1] == outputs and inputs[-1].get("role") == "user", "compact_history_changed")
        else:
            oracle.require(captures[1]["request"].get("previous_response_id") == ids[0], "continuation_parent_not_captured")
        if case == "ws_multiturn":
            verify_handshake(bundle["handshake"], 101)
            oracle.require(all(sample.get(key) == bundle.get("handshake", {}).get(key) for key in ("handshake_status", "ws_nonce", "ws_accept")), "ws_handshake_not_captured")
            oracle.require(len(sample["ws_turns"]) == len(captures), "ws_capture_sequence_changed")
            for actual, captured in zip(sample["ws_turns"], captures):
                matches(actual, captured)


def audit_extension_bundle(bundle):
    case = bundle.get("case")
    count = bundle.get("counts", {}).get("inference_intents") if isinstance(bundle.get("counts"), dict) else None
    receipt = {"schema_version": 3, "layer": bundle.get("layer") if bundle.get("layer") in ("LOCAL_SIMULATED", "LIVE_NEW_BUILD", "PRODUCTION") else None, "value_free": True,
               "case": case if isinstance(case, str) and case in CASE_CAPABILITY else None,
               "inference_intents": count if oracle.integer(count) and count <= 3 else None,
               "extension_tool_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    def audit():
        oracle.require(bundle.get("extension_bundle_version") == 1, "unknown_extension_bundle_version")
        oracle.require(isinstance(case, str) and case in IMPLEMENTED, "unreviewed_extension_case")
        oracle.available(bundle.get("layer") == "LOCAL_SIMULATED", "extension_layer_not_authorized")
        result = bundle.get("result", {})
        oracle.require(isinstance(result, dict), "extension_execution_not_passed")
        oracle.require(result.get("status") == "PASS" and result.get("stopped") is True, "extension_execution_not_passed")
        oracle.verify_runtime(bundle["runtime"], bundle["source_sha"], bundle["layer"])
        oracle.verify_target(bundle["target"])
        oracle.available(bundle["target"]["capabilities"][CASE_CAPABILITY[case]], "extension_not_declared")
        sample = bundle.get("verified_sample", {})
        oracle.require(isinstance(sample, dict), "extension_sample_version_required")
        oracle.available(sample.get("extension_sample_version") == 1, "extension_sample_version_required")
        oracle.require(sample.get("frozen_runtime") == bundle["runtime"] and sample.get("source_sha") == bundle["source_sha"] and sample.get("case") == bundle["case"], "wrong_extension_target")
        captures = bundle.get("samples", [])
        oracle.require(isinstance(captures, list) and all(isinstance(row, dict) for row in captures), "extension_capture_count_mismatch")
        if bundle["case"] == "ws_handshake_rejection":
            oracle.require(len(captures) == 1 and oracle.integer(count) and count == 0, "extension_capture_count_mismatch")
        else:
            oracle.require(oracle.integer(count) and len(captures) == count == REQUEST_LIMITS[case], "extension_capture_count_mismatch")
        values = []
        for capture in captures:
            if bundle["case"] == "ws_handshake_rejection":
                continue
            oracle.require(capture.get("case") == case and capture.get("frozen_runtime") == bundle["runtime"] and capture.get("source_sha") == bundle["source_sha"], "wrong_extension_target")
            values.append(verify_capture(capture, bundle["target"]))
        verify_sequence(bundle, values)
        oracle.extension_check(CASE_CAPABILITY[bundle["case"]], bundle["case"], sample, bundle["target"])
    receipt.update(oracle.verdict(audit))
    return receipt


class WebSocket:
    """Bounded RFC6455 public client; no redirect, reconnect or inference retry."""
    def __init__(self, base_url, key, origin=None, timeout=30, on_progress=None):
        url = urllib.parse.urlsplit(base_url)
        oracle.require(url.scheme == "https" or url.scheme == "http" and url.hostname in ("127.0.0.1", "::1", "localhost"), "unsafe_data_url")
        oracle.require(not url.username and not url.password and not url.query and not url.fragment, "unsafe_data_url")
        oracle.require(all("\r" not in value and "\n" not in value for value in (key, origin or "")), "invalid_ws_header")
        self.timeout = timeout
        self.socket = socket.create_connection((url.hostname, url.port or (443 if url.scheme == "https" else 80)), timeout=timeout)
        try:
            if url.scheme == "https":
                self.socket = ssl.create_default_context().wrap_socket(self.socket, server_hostname=url.hostname)
            self.nonce = base64.b64encode(secrets.token_bytes(16)).decode()
            path = url.path.rstrip("/") + "/responses"
            headers = ["GET " + path + " HTTP/1.1", "Host: " + url.netloc, "Upgrade: websocket", "Connection: Upgrade", "Sec-WebSocket-Version: 13", "Sec-WebSocket-Key: " + self.nonce, "Authorization: Bearer " + key]
            if origin is not None:
                headers.append("Origin: " + origin)
            if on_progress is not None:
                on_progress({"ws_nonce": self.nonce, "ws_origin": origin, "ws_path": path,
                             "ws_request": {"method": "GET", "path": path, "origin": origin, "nonce": self.nonce, "version": "13", "upgrade": "websocket", "connection": "Upgrade"}}, b"")
            self.socket.sendall(("\r\n".join(headers) + "\r\n\r\n").encode())
            raw = bytearray()
            deadline = time.monotonic() + timeout
            while not raw.endswith(b"\r\n\r\n"):
                self.socket.settimeout(max(.001, deadline - time.monotonic()))
                oracle.require(time.monotonic() < deadline and len(raw) < 16384, "ws_handshake_bounds")
                part = self.socket.recv(1)
                oracle.require(bool(part), "ws_handshake_closed")
                raw.extend(part)
                if on_progress is not None:
                    on_progress({}, bytes(raw))
            self.status, fields, _ = handshake_headers(bytes(raw))
            self.accept = fields.get("sec-websocket-accept")
            if on_progress is not None:
                on_progress({"http_status": self.status, "handshake_status": self.status, "ws_accept": self.accept}, bytes(raw))
            if self.status == 101:
                expected = base64.b64encode(hashlib.sha1((self.nonce + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
                oracle.require(self.accept == expected and fields.get("upgrade", "").lower() == "websocket" and "upgrade" in fields.get("connection", "").lower().split(", "), "ws_handshake_unconfirmed")
            else:
                oracle.require("transfer-encoding" not in fields and fields.get("content-length", "").isdigit(), "ws_handshake_body_unframed")
                size = int(fields["content-length"])
                oracle.require(size <= oracle.MAX_BODY, "ws_handshake_bounds")
                remaining_body = size
                while remaining_body:
                    remaining = deadline - time.monotonic()
                    oracle.require(remaining > 0, "ws_handshake_bounds")
                    self.socket.settimeout(remaining)
                    part = self.socket.recv(min(65536, remaining_body))
                    oracle.require(bool(part), "ws_handshake_closed")
                    raw.extend(part);remaining_body -= len(part)
                    if on_progress is not None:on_progress({}, bytes(raw))
        except Exception:
            self.socket.close()
            raise

    def send(self, payload, opcode=1):
        raw = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode() if isinstance(payload, dict) else payload
        oracle.require(len(raw) <= oracle.MAX_BODY, "ws_frame_size")
        mask = secrets.token_bytes(4)
        length = bytes([0x80 | len(raw)]) if len(raw) < 126 else b"\xfe" + struct.pack("!H", len(raw)) if len(raw) <= 65535 else b"\xff" + struct.pack("!Q", len(raw))
        self.socket.sendall(bytes([0x80 | opcode]) + length + mask + bytes(value ^ mask[index % 4] for index, value in enumerate(raw)))

    def turn(self, request, on_progress=None):
        self.send({**request, "type": "response.create"})
        frames, total, received, terminal_until = [], 0, bytearray(), None
        deadline = time.monotonic() + self.timeout
        def exact(size):
            result = bytearray()
            while len(result) < size:
                remaining = deadline - time.monotonic()
                oracle.require(remaining > 0, "ws_turn_deadline")
                self.socket.settimeout(remaining)
                part = self.socket.recv(size - len(result))
                oracle.require(bool(part), "ws_turn_closed")
                result.extend(part)
                received.extend(part)
                if on_progress is not None:
                    on_progress({}, bytes(received))
            return bytes(result)
        for _ in range(4096):
            if terminal_until is not None:
                pending = isinstance(self.socket, ssl.SSLSocket) and self.socket.pending() > 0
                if not pending and not select.select([self.socket], [], [], max(0, terminal_until - time.monotonic()))[0]:
                    if on_progress is not None:
                        on_progress({"terminal_observation_closed": True, "terminal_observation_ms": WS_TERMINAL_OBSERVATION_MS}, bytes(received))
                    return "".join("data: " + json.dumps(frame, ensure_ascii=False) + "\n\n" for frame in frames)
            header = exact(2)
            oracle.require(header[0] & 0x80 and not header[0] & 0x70 and not header[1] & 0x80, "unsupported_ws_frame")
            size = header[1] & 0x7f
            if size == 126:
                size = struct.unpack("!H", exact(2))[0]
            elif size == 127:
                size = struct.unpack("!Q", exact(8))[0]
            total += size
            oracle.require(total <= oracle.MAX_BODY, "ws_frame_size")
            opcode, raw = header[0] & 0xf, exact(size)
            if opcode == 9:
                oracle.require(size <= 125, "ws_control_frame_size")
                self.send(raw, 10)
                continue
            if opcode == 10:
                continue
            oracle.require(opcode == 1, "ws_turn_closed")
            value = oracle.load_json(raw.decode())
            frames.append(value)
            if on_progress is not None:
                on_progress({"wire": "".join("data: " + json.dumps(frame, ensure_ascii=False) + "\n\n" for frame in frames)}, bytes(received))
            oracle.require(terminal_until is None, "duplicate_or_late_terminal")
            if value.get("type") in ("response.completed", "response.failed", "response.incomplete", "error"):
                terminal_until = time.monotonic() + WS_TERMINAL_OBSERVATION_MS / 1000
        raise ValueError("ws_frame_count")

    def close(self):
        try:
            if self.status == 101:
                self.send(struct.pack("!H", 1000), 8)
        except OSError:
            pass
        finally:
            self.socket.close()


def run_extension(plan, runtime, key, base_url, private_out, *, foreign_key=None, execute=False):
    target, specification = plan.get("target", {}), plan.get("extension", {})
    case = specification.get("case")
    receipt = {"schema_version": 3, "layer": plan.get("layer") if plan.get("layer") in ("LOCAL_SIMULATED", "LIVE_NEW_BUILD", "PRODUCTION") else None, "status": "BLOCKED", "case": case if isinstance(case, str) and case in CASE_CAPABILITY else None,
               "inference_intents": 0, "http_intents": 0, "ws_frame_intents": 0, "stopped": True, "value_free": True,
               "extension_tool_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    bundle = {"extension_bundle_version": 1, "layer": plan.get("layer"), "source_sha": plan.get("source_sha"),
              "target": target, "runtime": runtime, "case": case, "samples": [], "operations": []}
    saved, ws = False, None
    try:
        oracle.require(isinstance(case, str) and case in CASE_CAPABILITY, "unreviewed_extension_case")
        oracle.available(case in IMPLEMENTED, "extension_executor_not_implemented")
        oracle.available(plan.get("layer") == "LOCAL_SIMULATED", "extensions_live_execution_not_authorized")
        oracle.verify_runtime(runtime, plan.get("source_sha"), plan["layer"])
        oracle.verify_target(target)
        oracle.verify_current_declaration(target, runtime)
        oracle.verify_execution_policy(plan, runtime, base_url)
        oracle.available(target["capabilities"][CASE_CAPABILITY[case]], "extension_not_declared")
        protocol = target["protocol"]
        if protocol == "chat" and target["mode"] == "sse":
            oracle.available(target["parameters"].get("stream_options") == {"include_usage": True}, "chat_sse_usage_control_required")
        if case in ("visible_reasoning", "reasoning_history"):
            oracle.available(protocol != "messages", "messages_thinking_build_effort_no_lossless_mapping")
            oracle.available(("reasoning_effort" if protocol == "chat" else "reasoning") in target["parameters"], "explicit_reasoning_control_required")
        else:
            oracle.require(protocol == "responses", "extension_protocol_not_applicable")
        if case.startswith("ws_"):
            oracle.require(target["mode"] == "sse", "websocket_requires_responses_sse_target")
        oracle.available(execute, "execution_not_requested")
        oracle.available(bool(key), "client_authentication_unavailable")
        collector = plan.get("collector", {})
        oracle.available(all(collector.get(field) for field in ("local_state_dir", "binary")), "exact_attempt_collector_required")
        if case in ("foreign_owner", "compact_foreign_owner"):
            foreign_identity = specification.get("foreign_client_evidence_id")
            oracle.available(bool(foreign_key) and foreign_key != key and isinstance(foreign_identity, str) and
                             re.fullmatch("[0-9a-f]{64}", foreign_identity) is not None and foreign_identity != target["declaration_evidence"]["client_key_evidence_id"], "foreign_client_authentication_required")
        private_out = Path(private_out)
        private_out.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        oracle.require(not private_out.exists() and private_out.parent.stat().st_mode & 0o077 == 0, "private_extension_output_not_protected")
        # Every operation intent is persisted before a socket write. A failed/unknown
        # result is archived and no remaining operations execute.
        def save():
            nonlocal saved
            bundle["counts"] = {key: receipt[key] for key in ("inference_intents", "http_intents", "ws_frame_intents")}
            raw = json.dumps(bundle, indent=2) + "\n"
            if not saved:
                with os.fdopen(os.open(private_out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as output:
                    output.write(raw)
                saved = True
            else:
                private_out.write_text(raw)
        def intent(operation, inference=False, socket_frame=False):
            current = observe_local_process(collector, runtime)
            if inference:
                oracle.require(receipt["inference_intents"] < REQUEST_LIMITS[case], "extension_request_limit")
                receipt["inference_intents"] += 1
            receipt["ws_frame_intents" if socket_frame else "http_intents"] += 1
            bundle["operations"].append(operation)
            save()
            return current
        def progress(sample, *, socket_wire=False):
            def capture(metadata, raw):
                sample.update(metadata)
                sample["wire_bytes_b64"] = base64.b64encode(raw).decode()
                if not socket_wire:
                    sample["wire"] = raw.decode("utf-8", errors="replace")
                save()
            return capture
        def sample_request(request, *, compact=False):
            sample = {"extension_sample_version": 1, "case": case, "layer": plan["layer"], "source_sha": plan["source_sha"], "target": copy.deepcopy(target),
                      "frozen_runtime": runtime, "request": copy.deepcopy(request), "expected_text": specification.get("expected_text"),
                      "operation": "compact_summary" if compact else "response_create"}
            if ws is not None:
                sample.update(http_status=200, mode="sse", ws_frame={**request, "type": "response.create"},
                              request_wire_b64=base64.b64encode(json.dumps({**request, "type": "response.create"}, ensure_ascii=False, separators=(",", ":")).encode()).decode())
            else:
                sample["request_wire_b64"] = base64.b64encode(json.dumps(request, separators=(",", ":")).encode()).decode()
            bundle["samples"].append(sample)
            sample["runtime"] = intent({"operation": sample["operation"], "request": copy.deepcopy(request)}, inference=True, socket_frame=ws is not None)
            sample["request_binding_sha256"] = request_binding(sample)
            save()
            if ws is not None:
                sample["wire"] = ws.turn(request, on_progress=progress(sample, socket_wire=True))
            else:
                sample.update(oracle.http_exchange(base_url, key, "/responses/compact" if compact else oracle.PROTOCOLS[protocol], request, on_progress=progress(sample)))
            save()
            sample["runtime"] = observe_local_process(collector, runtime)
            save()
            oracle.require(sample["http_status"] == 200, "extension_http_failure")
            value = oracle.load_json(sample["wire"]) if compact or target["mode"] == "json" else oracle.decode_stream(protocol, sample["wire"].encode())
            # Billing materialization is asynchronous. Poll exact read-only records;
            # never repeat the public operation or infer identity from a time window.
            deadline = time.monotonic() + 5
            while True:
                collected = collect_local(collector["local_state_dir"], value["id"], sample["runtime"])
                evidence = collected["evidence"]
                if evidence.get("ledger") and evidence.get("request_outcome") is not None:
                    break
                oracle.available(time.monotonic() < deadline, "exact_attempt_collection_unavailable")
                time.sleep(.05)
            sample["evidence"] = evidence
            sample["request_binding_sha256"] = request_binding(sample)
            summary_target = {**target, "mode": "json", "parameters": {"max_output_tokens": 2048}} if compact else target
            oracle.verify_attempts(summary_target, evidence, value["id"])
            oracle.verify_usage_lineage(oracle.native_usage(protocol, value), evidence)
            if not compact:
                text, calls = oracle.observe(protocol, value)
                oracle.available(isinstance(sample["expected_text"], str) and bool(sample["expected_text"]), "extension_answer_oracle_missing")
                oracle.require(not calls and text == sample["expected_text"], "history_or_result_not_used")
            save()
            return sample, value
        def exchange(path, body=None, *, method="GET", actual_key=key):
            result = {"method": method, "path": path, "request": copy.deepcopy(body),
                      "client_evidence_id": target["declaration_evidence"]["client_key_evidence_id"] if actual_key == key else specification["foreign_client_evidence_id"],
                      "request_wire_b64": base64.b64encode(json.dumps(body, separators=(",", ":")).encode() if body is not None else b"").decode(),
                      "boundary_before": attempt_count(collector["local_state_dir"])}
            bundle.setdefault("resource_operations", []).append(result)
            result["runtime"] = intent({"operation": method.lower() + "_" + ("compact" if path.endswith("/compact") else "resource"), **result})
            result.update(oracle.http_exchange(base_url, actual_key, path, body, method=method, on_progress=progress(result)))
            result.update(runtime=observe_local_process(collector, runtime), boundary_after=attempt_count(collector["local_state_dir"]))
            save()
            return result
        request = {"model": target["public_model"], "stream": target["mode"] == "sse", **copy.deepcopy(target["parameters"])}
        request["input" if protocol == "responses" else "messages"] = ([{"role": "user", "content": specification.get("prompt", "Remember the acceptance fact.")}] if protocol != "responses" else specification.get("prompt", "Remember the acceptance fact."))
        if case == "ws_handshake_rejection":
            sample = {"extension_sample_version": 1, "case": case, "layer": plan["layer"], "source_sha": plan["source_sha"], "target": target,
                      "frozen_runtime": runtime, "request": request, "operation": "responses_ws_upgrade"}
            bundle["samples"].append(sample)
            sample["runtime"] = intent({"operation": "websocket_origin_upgrade", "method": "GET", "path": "/responses", "origin": "https://example.invalid"})
            save()
            before = attempt_count(collector["local_state_dir"])
            ws = WebSocket(base_url, key, origin="https://example.invalid", on_progress=progress(sample, socket_wire=True))
            sample.update(runtime=observe_local_process(collector, runtime),
                      http_status=ws.status, authenticated=True,
                      ws_nonce=ws.nonce, ws_accept=ws.accept, ws_origin="https://example.invalid", handshake_status=ws.status,
                      boundary_before=before, boundary_after=attempt_count(collector["local_state_dir"]), evidence={"upstream_send_count": 0})
            sample["evidence"]["upstream_send_count"] = sample["boundary_after"] - before
            sample["request_binding_sha256"] = request_binding(sample)
        else:
            if case == "ws_multiturn":
                bundle["handshake"] = {"operation": "websocket_upgrade", "method": "GET", "path": "/responses", "origin": None}
                intent(bundle["handshake"])
                ws = WebSocket(base_url, key, on_progress=progress(bundle["handshake"], socket_wire=True))
                oracle.require(ws.status == 101, "ws_handshake_unconfirmed")
            if case not in ("visible_reasoning", "reasoning_history", "ws_multiturn"):
                request["store"] = True
            root, value = sample_request(request)
            sample = copy.deepcopy(root)
            if case == "reasoning_history":
                oracle.extension_check("reasoning", "visible_reasoning", {**root, "case": "visible_reasoning"}, target)
                next_instruction = specification.get("next_instruction", "Repeat the remembered acceptance fact.")
                next_request = oracle.conversation_step(protocol, request, value, [], next_instruction)
                sample, _ = sample_request(next_request)
                sample.update(previous_wire=json.dumps(value), previous_request=request, next_token=next_instruction)
            elif case in ("exact_continuation", "compact_roundtrip", "ws_multiturn"):
                next_request = {"model": target["public_model"], "stream": target["mode"] == "sse", **copy.deepcopy(target["parameters"]),
                                "previous_response_id": value["id"], "input": specification.get("next_instruction", "Repeat the remembered acceptance fact.")}
                if case == "compact_roundtrip":
                    summary_request = {"model": target["public_model"], "previous_response_id": value["id"], "stream": False}
                    summary, compact = sample_request(summary_request, compact=True)
                    next_request.pop("previous_response_id")
                    next_request["input"] = [*compact["output"], {"role": "user", "content": specification.get("next_instruction", "Repeat the remembered acceptance fact.")}]
                sample, _ = sample_request(next_request)
                sample.update(parent_target=target, parent_response_id=value["id"])
                if case == "compact_roundtrip":
                    sample.update(compact_request=summary_request, compact_wire=summary["wire"], compact_http_status=summary["http_status"], compact_evidence=summary["evidence"],
                                  compact_target={**target, "mode": "json", "parameters": {"max_output_tokens": 2048}})
                elif case == "ws_multiturn":
                    sample.update(handshake_status=ws.status, ws_nonce=ws.nonce, ws_accept=ws.accept, ws_turns=[copy.deepcopy(root), copy.deepcopy(sample)])
            elif case in ("store_read", "foreign_owner", "deleted_read", "compact_foreign_owner"):
                sample["root_sample"] = copy.deepcopy(root)
                path = "/responses/" + urllib.parse.quote(value["id"], safe="")
                read = exchange(path)
                oracle.require(read["http_status"] == 200 and oracle.load_json(read["wire"]) == value, "stored_read_mismatch")
                sample.update(read_http_status=read["http_status"], read_wire=read["wire"])
                if case != "store_read":
                    if case in ("foreign_owner", "compact_foreign_owner"):
                        authentication = exchange("/models", actual_key=foreign_key)
                        oracle.require(authentication["http_status"] == 200, "extension_authentication_unconfirmed")
                        oracle.require(any(model.get("id") == target["public_model"] for model in oracle.load_json(authentication["wire"]).get("data", [])), "extension_foreign_model_access_unconfirmed")
                        sample.update(owner_read_http_status=read["http_status"], owner_read_wire=read["wire"], read_response_id=value["id"],
                                      foreign_auth_status=authentication["http_status"], foreign_visible_model=target["public_model"], owner_client_evidence_id=target["declaration_evidence"]["client_key_evidence_id"], actual_client_evidence_id=specification["foreign_client_evidence_id"])
                    if case == "deleted_read":
                        deletion = exchange(path, method="DELETE")
                        sample.update(delete_http_status=deletion["http_status"], deleted_response_id=value["id"], read_response_id=value["id"])
                    before = attempt_count(collector["local_state_dir"])
                    if case == "compact_foreign_owner":
                        compact_request = {"model": target["public_model"], "previous_response_id": value["id"], "stream": False}
                        denied = exchange("/responses/compact", compact_request, method="POST", actual_key=foreign_key)
                        sample["compact_request"] = compact_request
                    else:
                        denied = exchange(path, actual_key=foreign_key if case == "foreign_owner" else key)
                    sample.update(http_status=denied["http_status"], operation="responses_compact" if case == "compact_foreign_owner" else "stored_read", authenticated=True,
                                  boundary_before=before, boundary_after=attempt_count(collector["local_state_dir"]), evidence={"upstream_send_count": attempt_count(collector["local_state_dir"]) - before})
        oracle.extension_check(CASE_CAPABILITY[case], case, sample, target)
        bundle["verified_sample"] = copy.deepcopy(sample)
        receipt.update(status="PASS", category="verified", sample_request_binding_sha256=sample["request_binding_sha256"])
    except (ValueError, KeyError, TypeError, IndexError) as error:
        receipt.update(oracle.verdict(lambda: (_ for _ in ()).throw(error)))
    except (OSError, subprocess.SubprocessError, http.client.HTTPException):
        receipt.update(status="FAIL" if receipt["inference_intents"] or receipt["http_intents"] else "BLOCKED", category="extension_transport_or_collector_failure")
    finally:
        if ws is not None:
            ws.close()
        bundle["result"] = {key: receipt.get(key) for key in ("status", "category", "inference_intents", "http_intents", "ws_frame_intents", "stopped")}
        if saved:
            private_out.write_text(json.dumps(bundle, indent=2) + "\n")
            receipt["private_bundle_sha256"] = hashlib.sha256(private_out.read_bytes()).hexdigest()
    return receipt
