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
import io
import json
import os
from pathlib import Path
import re
import secrets
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


def verify_capture(sample, target):
    """Validate every captured successful inference, including an owned root."""
    oracle.available(sample.get("extension_sample_version") == 1, "extension_sample_version_required")
    oracle.require(sample.get("target") == target and sample.get("layer") == "LOCAL_SIMULATED", "wrong_extension_target")
    oracle.verify_turn_runtime(sample["runtime"], sample["frozen_runtime"], sample["source_sha"], sample["layer"])
    oracle.require(sample.get("request_binding_sha256") == request_binding(sample), "extension_request_binding_changed")
    request = sample["request"]
    summary = sample.get("operation") == "compact_summary"
    controls = {"max_output_tokens": 2048} if summary else target["parameters"]
    expected_stream = False if summary else target["mode"] == "sse"
    oracle.require(request.get("model") == target["public_model"] and request.get("stream") is expected_stream and sample.get("http_status") == 200, "extension_response_control_changed")
    if summary:
        oracle.require(set(request) == {"model", "previous_response_id", "stream"}, "compact_summary_control_changed")
    else:
        oracle.require(all(request.get(key) == value for key, value in controls.items()), "changed_request_control")
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
    oracle.require(all(request.get(key) == value for key, value in target.get("parameters", {}).items()), "changed_request_control")
    case = sample["case"]
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
            oracle.require(turn["request"].get("model") == target["public_model"] and turn["request"].get("stream") is True and
                           all(turn["request"].get(key) == value for key, value in target.get("parameters", {}).items()), "changed_request_control")
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


def audit_extension_bundle(bundle):
    receipt = {"schema_version": 3, "layer": bundle.get("layer"), "value_free": True,
               "case": bundle.get("case") if bundle.get("case") in CASE_CAPABILITY else None,
               "inference_intents": bundle.get("counts", {}).get("inference_intents", 0),
               "extension_tool_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    def audit():
        oracle.require(bundle.get("extension_bundle_version") == 1, "unknown_extension_bundle_version")
        oracle.available(bundle.get("layer") == "LOCAL_SIMULATED", "extension_layer_not_authorized")
        result = bundle.get("result", {})
        oracle.require(result.get("status") == "PASS" and result.get("stopped") is True, "extension_execution_not_passed")
        oracle.verify_runtime(bundle["runtime"], bundle["source_sha"], bundle["layer"])
        oracle.verify_target(bundle["target"])
        sample = bundle.get("verified_sample", {})
        oracle.available(sample.get("extension_sample_version") == 1, "extension_sample_version_required")
        oracle.require(sample.get("frozen_runtime") == bundle["runtime"] and sample.get("source_sha") == bundle["source_sha"] and sample.get("case") == bundle["case"], "wrong_extension_target")
        captures = bundle.get("samples", [])
        if bundle["case"] == "ws_handshake_rejection":
            oracle.require(len(captures) == 1 and bundle["counts"]["inference_intents"] == 0, "extension_capture_count_mismatch")
        else:
            oracle.require(len(captures) == bundle["counts"]["inference_intents"] and len(captures) <= REQUEST_LIMITS[bundle["case"]], "extension_capture_count_mismatch")
        for capture in captures:
            if bundle["case"] == "ws_handshake_rejection":
                continue
            oracle.require(capture.get("frozen_runtime") == bundle["runtime"] and capture.get("source_sha") == bundle["source_sha"], "wrong_extension_target")
            verify_capture(capture, bundle["target"])
        oracle.extension_check(CASE_CAPABILITY[bundle["case"]], bundle["case"], sample, bundle["target"])
    receipt.update(oracle.verdict(audit))
    return receipt


class WebSocket:
    """Bounded RFC6455 public client; no redirect, reconnect or inference retry."""
    def __init__(self, base_url, key, origin=None, timeout=30):
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
            self.socket.sendall(("\r\n".join(headers) + "\r\n\r\n").encode())
            raw = bytearray()
            deadline = time.monotonic() + timeout
            while not raw.endswith(b"\r\n\r\n"):
                self.socket.settimeout(max(.001, deadline - time.monotonic()))
                oracle.require(time.monotonic() < deadline and len(raw) < 16384, "ws_handshake_bounds")
                part = self.socket.recv(1)
                oracle.require(bool(part), "ws_handshake_closed")
                raw.extend(part)
            lines = raw.decode("ascii").split("\r\n")
            self.status = int(lines[0].split()[1])
            fields = {}
            for line in lines[1:]:
                if not line:
                    continue
                name, value = line.split(":", 1)
                oracle.require(name.lower() not in fields, "duplicate_ws_header")
                fields[name.lower()] = value.strip()
            self.accept = fields.get("sec-websocket-accept")
            if self.status == 101:
                expected = base64.b64encode(hashlib.sha1((self.nonce + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
                oracle.require(self.accept == expected and fields.get("upgrade", "").lower() == "websocket" and "upgrade" in fields.get("connection", "").lower().split(", "), "ws_handshake_unconfirmed")
        except Exception:
            self.socket.close()
            raise

    def send(self, payload, opcode=1):
        raw = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode() if isinstance(payload, dict) else payload
        oracle.require(len(raw) <= oracle.MAX_BODY, "ws_frame_size")
        mask = secrets.token_bytes(4)
        length = bytes([0x80 | len(raw)]) if len(raw) < 126 else b"\xfe" + struct.pack("!H", len(raw)) if len(raw) <= 65535 else b"\xff" + struct.pack("!Q", len(raw))
        self.socket.sendall(bytes([0x80 | opcode]) + length + mask + bytes(value ^ mask[index % 4] for index, value in enumerate(raw)))

    def turn(self, request):
        self.send({**request, "type": "response.create"})
        frames, total = [], 0
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
            return bytes(result)
        for _ in range(4096):
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
            if value.get("type") in ("response.completed", "response.failed", "response.incomplete", "error"):
                return "".join("data: " + json.dumps(frame, ensure_ascii=False) + "\n\n" for frame in frames)
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
    receipt = {"schema_version": 3, "layer": plan.get("layer"), "status": "BLOCKED", "case": case if case in CASE_CAPABILITY else None,
               "inference_intents": 0, "http_intents": 0, "ws_frame_intents": 0, "stopped": True, "value_free": True,
               "extension_tool_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    bundle = {"extension_bundle_version": 1, "layer": plan.get("layer"), "source_sha": plan.get("source_sha"),
              "target": target, "runtime": runtime, "case": case, "samples": [], "operations": []}
    saved, ws = False, None
    try:
        oracle.require(case in CASE_CAPABILITY, "unreviewed_extension_case")
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
            observe_local_process(collector, runtime)
            if inference:
                oracle.require(receipt["inference_intents"] < REQUEST_LIMITS[case], "extension_request_limit")
                receipt["inference_intents"] += 1
            receipt["ws_frame_intents" if socket_frame else "http_intents"] += 1
            bundle["operations"].append(operation)
            save()
        def sample_request(request, *, compact=False):
            intent("compact_summary" if compact else "response_create", inference=True, socket_frame=ws is not None)
            if ws is not None:
                sample = {"http_status": 200, "wire": ws.turn(request), "mode": "sse", "ws_frame": {**request, "type": "response.create"}}
            else:
                sample = oracle.http_exchange(base_url, key, "/responses/compact" if compact else oracle.PROTOCOLS[protocol], request)
            sample.update(extension_sample_version=1, case=case, layer=plan["layer"], source_sha=plan["source_sha"], target=copy.deepcopy(target),
                          frozen_runtime=runtime, request=copy.deepcopy(request), runtime=observe_local_process(collector, runtime), expected_text=specification.get("expected_text"), operation="compact_summary" if compact else "response_create")
            bundle["samples"].append(sample)
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
            intent(method.lower() + "_" + ("compact" if path.endswith("/compact") else "resource"))
            result = oracle.http_exchange(base_url, actual_key, path, body, method=method)
            bundle.setdefault("resource_operations", []).append({"method": method, "path": path, **result})
            save()
            return result
        request = {"model": target["public_model"], "stream": target["mode"] == "sse", **copy.deepcopy(target["parameters"])}
        request["input" if protocol == "responses" else "messages"] = ([{"role": "user", "content": specification.get("prompt", "Remember the acceptance fact.")}] if protocol != "responses" else specification.get("prompt", "Remember the acceptance fact."))
        if case == "ws_handshake_rejection":
            intent("websocket_origin_upgrade")
            before = attempt_count(collector["local_state_dir"])
            ws = WebSocket(base_url, key, origin="https://example.invalid")
            sample = {"extension_sample_version": 1, "case": case, "layer": plan["layer"], "source_sha": plan["source_sha"], "target": target,
                      "frozen_runtime": runtime, "runtime": observe_local_process(collector, runtime), "request": request,
                      "http_status": ws.status, "operation": "responses_ws_upgrade", "authenticated": True,
                      "ws_nonce": ws.nonce, "ws_accept": ws.accept, "ws_origin": "https://example.invalid", "handshake_status": ws.status,
                      "boundary_before": before, "boundary_after": attempt_count(collector["local_state_dir"]), "evidence": {"upstream_send_count": 0}}
            sample["evidence"]["upstream_send_count"] = sample["boundary_after"] - before
            sample["request_binding_sha256"] = request_binding(sample)
            bundle["samples"].append(sample)
        else:
            if case == "ws_multiturn":
                intent("websocket_upgrade")
                ws = WebSocket(base_url, key)
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
    except (OSError, subprocess.SubprocessError):
        receipt.update(status="FAIL" if receipt["inference_intents"] or receipt["http_intents"] else "BLOCKED", category="extension_transport_or_collector_failure")
    finally:
        if ws is not None:
            ws.close()
        bundle["result"] = {key: receipt.get(key) for key in ("status", "category", "inference_intents", "http_intents", "ws_frame_intents", "stopped")}
        if saved:
            private_out.write_text(json.dumps(bundle, indent=2) + "\n")
            receipt["private_bundle_sha256"] = hashlib.sha256(private_out.read_bytes()).hexdigest()
    return receipt
