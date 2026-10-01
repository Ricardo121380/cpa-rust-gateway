"""Batch B: real gateway, SQLite publication and loopback TLS Provider only.

All credentials are generated synthetic fixtures. No real Provider or production state.
Run after cargo build -p gateway --bin gateway; writes a value-free JSON receipt.
"""
import concurrent.futures
import base64
import hashlib
import http.client
import json
import os
import pathlib
import secrets
import socket
import ssl
import sqlite3
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import cpar_execution_evidence_http

os.umask(0o077)
repo = pathlib.Path.cwd()
out = pathlib.Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=True)
root = pathlib.Path(tempfile.mkdtemp(prefix="cpar-b-http-"))
state, credentials = root / "state", root / "credentials"
state.mkdir()
credentials.mkdir()
for name in ["master-key", "backup-key", "client-key-pepper", "grok-build-cache-key"]:
    (credentials / name).write_bytes(secrets.token_bytes(32))
for name, prefix in [("management-key", "mgmt_"), ("management-csrf", "csrf_")]:
    (credentials / name).write_text(prefix + secrets.token_hex(32))
ca, ca_key = root / "ca.pem", root / "ca.key"
extension = root / "tls.ext"
extension.write_text("subjectAltName=IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\n")
for command in [
    ["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2", "-keyout", str(ca_key), "-out", str(ca), "-subj", "/CN=CPAR B local CA", "-addext", "basicConstraints=critical,CA:TRUE"],
    ["openssl", "req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", str(root / "tls.key"), "-out", str(root / "tls.csr"), "-subj", "/CN=127.0.0.1"],
    ["openssl", "x509", "-req", "-in", str(root / "tls.csr"), "-CA", str(ca), "-CAkey", str(ca_key), "-CAcreateserial", "-out", str(root / "tls.pem"), "-days", "2", "-extfile", str(extension)],
]:
    subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

keys = {name: "synthetic-" + secrets.token_hex(24) for name in ["a", "b"]}
calls = []
calls_lock = threading.Lock()
held_started, held_closed = threading.Event(), threading.Event()
identity_release = threading.Event()
checks = []
cancelled_response_ids = []
fallback_candidates = {}
passed = False


def check(name, condition):
    assert condition, name
    checks.append({"check": name, "status": "PASS"})


class Provider(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def reply(self, status, body):
        raw = json.dumps(body, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        owner = next((name for name, key in keys.items() if self.headers.get("Authorization") == "Bearer " + key or self.headers.get("x-api-key") == key), None)
        if owner is None:
            return self.reply(401, {"error": {"type": "invalid_api_key"}})
        protocol = "chat" if self.path.endswith("/chat/completions") else "messages" if self.path.endswith("/messages") else "responses"
        text = json.dumps(body, ensure_ascii=False)
        markers = ["identity-switch", "parallel-round", "tool-round", "invalid-args", "serial-violation", "thinking-round", "reasoning-only-round", "usage-error-round", "signed-round", "citation-round", "private-reasoning", "truncated-round", "held-round", "retry-round", "always-fail", "slow-bootstrap", "duplicate-root", "auth-round"]
        scenario = next((marker for marker in markers if marker in text), "text-round")
        has_result = '"tool_call_id"' in text or '"function_call_output"' in text or '"tool_result"' in text
        compact = "Create a concise factual conversation summary" in text
        connection_kind = "primary" if self.server is provider else "fallback"
        call = {"account": owner, "connection": connection_kind, "protocol": protocol, "scenario": scenario, "stream": bool(body.get("stream")), "has_result": has_result, "compact": compact, "body": body}
        with calls_lock:
            calls.append(call)
        if scenario == "identity-switch":
            held_started.set()
            if not identity_release.wait(10):
                return self.reply(503, {"error": {"type": "fixture_deadline"}})
        if scenario == "auth-round" and owner == "a":
            # Credential-local rejection must leave the healthy binding on this endpoint usable.
            # Shared endpoint 5xx behavior is covered by the cross-endpoint Rust loopback tests.
            return self.reply(401, {"error": {"type": "authentication_error" if protocol == "messages" else "invalid_api_key"}})
        if scenario == "always-fail" or scenario == "retry-round" and connection_kind == "primary":
            return self.reply(503, {"error": {"type": "server_error"}})
        if scenario == "slow-bootstrap":
            held_started.set()
            self.close_connection = True
            self.connection.settimeout(3)
            try:
                if self.connection.recv(1) == b"":
                    held_closed.set()
            except (ssl.SSLError, ConnectionError):
                held_closed.set()
            except TimeoutError:
                pass
            return
        call_count = 2 if scenario in ["parallel-round", "serial-violation"] else 1
        tools = scenario in ["tool-round", "parallel-round", "invalid-args", "serial-violation"] and not has_result and not compact
        ids = ["call_local_" + str(index) for index in range(call_count)]
        arguments = [json.dumps({"value": "商界" + str(index)}, ensure_ascii=False, separators=(",", ":")) for index in range(call_count)]
        if scenario == "invalid-args":
            arguments[0] = '{"value":'
        answer = "synthetic summary" if compact else "synthetic answer"
        thought = "synthetic visible thought"
        signature = "synthetic-signature"
        citation = {"type": "page_location", "cited_text": "synthetic source", "document_index": 0, "document_title": "source", "start_page_number": 1, "end_page_number": 2}
        annotation = {"type": "url_citation", "start_index": 0, "end_index": 9, "url": "https://example.invalid/source", "title": "synthetic source"}
        reasoning = scenario in ["thinking-round", "reasoning-only-round", "signed-round"] and not has_result and not compact
        response_id = "resp_ws_duplicate" if scenario == "duplicate-root" and len(body.get("input", [])) == 1 else "resp_" + secrets.token_hex(8)
        call["response_id"] = response_id
        if protocol == "chat":
            usage = {"prompt_tokens": 20, "completion_tokens": 10, "total_tokens": 30, "prompt_tokens_details": {"cached_tokens": 6}, "completion_tokens_details": {"reasoning_tokens": 4}}
            message = {"role": "assistant", "content": None if tools or scenario == "reasoning-only-round" else answer}
            if reasoning:
                message["reasoning_content"] = thought
            if tools:
                message["tool_calls"] = [{"id": identity, "type": "function", "function": {"name": "echo", "arguments": arg}} for identity, arg in zip(ids, arguments)]
            response = {"id": response_id, "object": "chat.completion", "model": body["model"], "created": 1, "choices": [{"index": 0, "message": message, "finish_reason": "tool_calls" if tools else "stop"}], "usage": usage}
        elif protocol == "messages":
            usage = {"input_tokens": 7, "output_tokens": 10, "cache_read_input_tokens": 3, "cache_creation_input_tokens": 2}
            content = []
            if reasoning:
                content.append({"type": "thinking", "thinking": thought, "signature": signature})
            if tools:
                content.extend({"type": "tool_use", "id": identity, "name": "echo", "input": json.loads(arg) if scenario != "invalid-args" else []} for identity, arg in zip(ids, arguments))
            else:
                content.append({"type": "text", "text": answer, **({"citations": [citation]} if scenario == "citation-round" else {})})
            response = {"id": response_id, "type": "message", "role": "assistant", "model": body["model"], "content": content, "stop_reason": "tool_use" if tools else "end_turn", "stop_sequence": None, "usage": usage}
        else:
            usage = {"input_tokens": 20, "output_tokens": 10, "total_tokens": 30, "input_tokens_details": {"cached_tokens": 6}, "output_tokens_details": {"reasoning_tokens": 4}}
            output = []
            if reasoning or scenario == "private-reasoning":
                output.append({"id": "rs_local", "type": "reasoning", "status": "completed", "summary": [{"type": "summary_text", "text": thought}], "content": [], **({"encrypted_content": "unowned-private-ciphertext"} if scenario == "private-reasoning" else {})})
            if tools:
                output.extend({"id": "fc_local_" + str(index), "type": "function_call", "status": "completed", "call_id": identity, "name": "echo", "arguments": arg} for index, (identity, arg) in enumerate(zip(ids, arguments)))
            else:
                output.append({"id": "msg_local", "type": "message", "role": "assistant", "status": "completed", "content": [{"type": "output_text", "text": answer, "annotations": [annotation] if scenario == "citation-round" else []}]})
            response = {"id": response_id, "object": "response", "status": "completed", "model": body["model"], "output": output, "usage": usage}
        if not body.get("stream"):
            return self.reply(200, response)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True

        def frame(kind, **payload):
            raw = ("event: " + kind + "\ndata: " + json.dumps({"type": kind, **payload}, ensure_ascii=False) + "\n\n").encode()
            # Split network writes through UTF-8 boundaries as well as JSON punctuation.
            for index in range(0, len(raw), 7):
                self.wfile.write(raw[index:index + 7])
            self.wfile.flush()

        def chat_frame(delta, finish=None, reported_usage=None):
            value = {"id": response_id, "object": "chat.completion.chunk", "created": 1, "model": body["model"], "choices": [{"index": 0, "delta": delta, "finish_reason": finish}] if delta is not None else [], "usage": reported_usage}
            self.wfile.write(("data: " + json.dumps(value, ensure_ascii=False) + "\n\n").encode())
            self.wfile.flush()

        if protocol == "chat":
            chat_frame({"role": "assistant"})
            if reasoning:
                chat_frame({"reasoning_content": thought})
            if tools:
                for index, identity in enumerate(ids):
                    chat_frame({"tool_calls": [{"index": index, "id": identity, "type": "function", "function": {"name": "echo", "arguments": ""}}]})
                for offset in range(max(map(len, arguments))):
                    for index, arg in enumerate(arguments):
                        if offset < len(arg):
                            chat_frame({"tool_calls": [{"index": index, "function": {"arguments": arg[offset]}}]})
            elif scenario != "reasoning-only-round":
                chat_frame({"content": answer})
            if scenario in ["held-round", "truncated-round"]:
                return self.hold_or_truncate(scenario)
            if scenario == "usage-error-round":
                # One TLS write carries validated Usage followed by a malformed SSE frame.
                # Wait until the earlier semantic frame reached the public stream.
                time.sleep(0.05)
                tail = [
                    {"id": response_id, "object": "chat.completion.chunk", "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]},
                    {"id": response_id, "object": "chat.completion.chunk", "choices": [], "usage": usage},
                ]
                self.wfile.write(("".join("data: " + json.dumps(value) + "\n\n" for value in tail) + "data: malformed\n\n").encode())
                self.wfile.flush()
                return
            chat_frame({}, "tool_calls" if tools else "stop")
            chat_frame(None, reported_usage=usage)
            self.wfile.write(b"data: [DONE]\n\n")
            self.wfile.flush()
        elif protocol == "messages":
            frame("message_start", message={**response, "content": [], "stop_reason": None, "usage": {**usage, "output_tokens": 0}})
            for index, block in enumerate(content):
                empty = {"type": "tool_use", "id": block["id"], "name": "echo", "input": {}} if block["type"] == "tool_use" else {"type": block["type"], "thinking" if block["type"] == "thinking" else "text": ""}
                frame("content_block_start", index=index, content_block=empty)
                if block["type"] == "thinking":
                    frame("content_block_delta", index=index, delta={"type": "thinking_delta", "thinking": thought})
                    for part in [signature[:5], signature[5:]]:
                        frame("content_block_delta", index=index, delta={"type": "signature_delta", "signature": part})
                elif block["type"] == "text":
                    frame("content_block_delta", index=index, delta={"type": "text_delta", "text": answer})
                    if scenario == "citation-round":
                        frame("content_block_delta", index=index, delta={"type": "citations_delta", "citation": citation})
                if block["type"] != "tool_use":
                    frame("content_block_stop", index=index)
            if tools:
                for offset in range(max(map(len, arguments))):
                    for index, arg in enumerate(arguments):
                        if offset < len(arg):
                            frame("content_block_delta", index=index, delta={"type": "input_json_delta", "partial_json": arg[offset]})
            if scenario in ["held-round", "truncated-round"]:
                return self.hold_or_truncate(scenario)
            if tools:
                for index in range(len(content)):
                    frame("content_block_stop", index=index)
            frame("message_delta", delta={"stop_reason": response["stop_reason"], "stop_sequence": None}, usage=usage)
            frame("message_stop")
        else:
            frame("response.created", response={**response, "status": "in_progress", "output": []})
            for index, item in enumerate(output):
                empty = {**item, "status": "in_progress"}
                for field in ["content", "summary"]:
                    if field in empty:
                        empty[field] = []
                if item["type"] == "function_call":
                    empty["arguments"] = ""
                frame("response.output_item.added", output_index=index, item=empty)
                if item["type"] == "reasoning":
                    frame("response.reasoning_summary_text.delta", output_index=index, item_id=item["id"], summary_index=0, delta=thought)
                if item["type"] == "message":
                    frame("response.output_text.delta", output_index=index, item_id=item["id"], content_index=0, delta=answer)
                    if scenario == "citation-round":
                        frame("response.output_text.annotation.added", output_index=index, item_id=item["id"], content_index=0, annotation_index=0, annotation=annotation)
            if tools:
                for offset in range(max(map(len, arguments))):
                    for index, arg in enumerate(arguments):
                        if offset < len(arg):
                            frame("response.function_call_arguments.delta", output_index=index, item_id=output[index]["id"], delta=arg[offset])
            if scenario in ["held-round", "truncated-round"]:
                return self.hold_or_truncate(scenario)
            for index, item in enumerate(output):
                frame("response.output_item.done", output_index=index, item=item)
            frame("response.completed", response=response)

    def hold_or_truncate(self, scenario):
        if scenario == "held-round":
            held_started.set()
            self.connection.settimeout(15)
            try:
                if self.connection.recv(1) == b"":
                    held_closed.set()
            except (ssl.SSLError, ConnectionError):
                held_closed.set()
            except TimeoutError:
                pass


provider = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(root / "tls.pem", root / "tls.key")
provider.socket = tls.wrap_socket(provider.socket, server_side=True)
threading.Thread(target=provider.serve_forever, daemon=True).start()
fallback_provider = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
fallback_provider.socket = tls.wrap_socket(fallback_provider.socket, server_side=True)
threading.Thread(target=fallback_provider.serve_forever, daemon=True).start()


def port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


admin_port, data_port = port(), port()
base = f"http://127.0.0.1:{admin_port}"
client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
headers = {"X-Management-Key": (credentials / "management-key").read_text(), "Content-Type": "application/json"}
scope, revision = None, None
log = (root / "gateway.log").open("ab")
binary = pathlib.Path(os.environ.get("CPAR_GATEWAY_BINARY", str(pathlib.Path(os.environ.get("CARGO_TARGET_DIR", str(repo / "target"))) / "debug/gateway"))).resolve()
subprocess.run([str(binary), "admin-login", "init", "--state-dir", str(state), "--password-file", str(root / "initial-password")], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
gateway = subprocess.Popen([str(binary), "serve", "--state-dir", str(state), "--credential-dir", str(credentials), "--data-listen", f"127.0.0.1:{data_port}", "--management-listen", f"127.0.0.1:{admin_port}"], stdout=log, stderr=log, env={**os.environ, "SSL_CERT_FILE": str(ca)})


def api(method, path, body=None, extra=None, expected=200):
    global revision
    request_headers = {**headers, **({"X-Config-Version": scope} if scope else {}), **({"If-Match": revision} if revision and method != "GET" else {}), **(extra or {})}
    try:
        response = client.open(urllib.request.Request(base + path, method=method, headers=request_headers, data=None if body is None else json.dumps(body).encode()), timeout=30)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        raw = response.read()
        value = json.loads(raw) if raw else None
        assert response.status == expected, (method, path, response.status, value if path.endswith("/validate") else value.get("error", {}).get("code") if isinstance(value, dict) else None)
        if method != "GET" and response.headers.get("ETag"):
            revision = response.headers["ETag"]
        return value


def publish():
    events = api("GET", "/admin/audit-events")
    event = max([row["id"] for row in events if row["action"] in ["config_published", "config_rolled_back"]] or [0])
    active = next((row["id"] for row in api("GET", "/admin/config-versions") if row["status"] == "active"), None)
    assert api("POST", f"/admin/config-versions/{scope}/validate")["valid"]
    return api("POST", f"/admin/config-versions/{scope}/publish", extra={"X-Expected-Active-Version": json.dumps(active), "X-Expected-Lifecycle-Event": str(event)})


def all_items(path):
    items = []
    page_path = path
    for _ in range(10):
        page = api("GET", page_path)
        items.extend(page["items"])
        if not page["next_cursor"]:
            return items
        page_path = path + "&cursor=" + urllib.parse.quote(page["next_cursor"], safe="")
    raise AssertionError("fixture read-model pagination bound")

PROTOCOLS = {
    "chat": ("/v1/chat/completions", "openai-compatible.chat-completions", "openai/chat-completions"),
    "responses": ("/v1/responses", "openai-compatible.responses", "openai/responses"),
    "messages": ("/v1/messages", "anthropic-compatible.messages", "anthropic/messages"),
}
TOOL_SCHEMA = {"type": "object", "properties": {"value": {"type": "string"}}, "required": ["value"]}


def body_for(source, target, text, stream=False, tools=False, parallel=None, choice=None):
    result = {"model": "model-" + target, "stream": stream}
    if source == "responses":
        result["input"] = [{"role": "user", "content": text}]
        result["max_output_tokens"] = 4096
        if tools:
            result["tools"] = [{"type": "function", "name": "echo", "description": "Synthetic echo", "parameters": TOOL_SCHEMA}]
    else:
        result["messages"] = [{"role": "user", "content": text}]
        result["max_tokens"] = 4096
        if tools:
            result["tools"] = [{"name": "echo", "description": "Synthetic echo", "input_schema": TOOL_SCHEMA}] if source == "messages" else [{"type": "function", "function": {"name": "echo", "description": "Synthetic echo", "parameters": TOOL_SCHEMA}}]
    if source == "messages" and (choice is not None or parallel is not None):
        selection = {"type": {"auto": "auto", "required": "any", "none": "none", "named": "tool"}[choice or "auto"]}
        if choice == "named":
            selection["name"] = "echo"
        if parallel is not None:
            selection["disable_parallel_tool_use"] = not parallel
        result["tool_choice"] = selection
    else:
        if parallel is not None:
            result["parallel_tool_calls"] = parallel
        if choice is not None:
            result["tool_choice"] = {"type": "function", "function": {"name": "echo"}} if source == "chat" and choice == "named" else {"type": "function", "name": "echo"} if choice == "named" else choice
    if source == "chat" and stream:
        result["stream_options"] = {"include_usage": True}
    return result


def data(path, body=None, method="POST", key=None, raw=False):
    connection = http.client.HTTPConnection("127.0.0.1", data_port, timeout=30)
    request_headers = {"Authorization": "Bearer " + (key or issued_key), "Content-Type": "application/json"}
    connection.request(method, path, body if raw else json.dumps(body) if body is not None else None, request_headers)
    response = connection.getresponse()
    response_headers = dict(response.getheaders())
    value = response.read()
    connection.close()
    return response.status, value, response_headers


def completed(source, raw, stream):
    if not stream:
        return json.loads(raw)
    frames = [json.loads(line[6:]) for line in raw.decode().splitlines() if line.startswith("data: ") and line != "data: [DONE]"]
    if source == "responses":
        terminal = [frame for frame in frames if frame.get("type") == "response.completed"]
        assert len(terminal) == 1, ("Responses terminal", [frame.get("type") for frame in frames])
        return terminal[0]["response"]
    if source == "messages":
        assert len([frame for frame in frames if frame.get("type") == "message_stop"]) == 1
        blocks = {}
        usage = {}
        for frame in frames:
            if frame.get("type") == "message_start":
                usage.update(frame["message"]["usage"])
            elif frame.get("type") == "content_block_start":
                blocks[frame["index"]] = {**frame["content_block"], "arguments": ""}
            elif frame.get("type") == "content_block_delta":
                block, delta = blocks[frame["index"]], frame["delta"]
                if delta["type"] == "input_json_delta":
                    block["arguments"] += delta["partial_json"]
                elif delta["type"] == "text_delta":
                    block["text"] += delta["text"]
                elif delta["type"] == "thinking_delta":
                    block["thinking"] += delta["thinking"]
                elif delta["type"] == "signature_delta":
                    block["signature"] = block.get("signature", "") + delta["signature"]
                elif delta["type"] == "citations_delta":
                    block.setdefault("citations", []).append(delta["citation"])
            elif frame.get("type") == "message_delta":
                usage.update(frame["usage"])
        for block in blocks.values():
            if block["type"] == "tool_use":
                block["input"] = json.loads(block["arguments"])
            block.pop("arguments", None)
        return {"content": list(blocks.values()), "usage": usage}
    assert raw.count(b"data: [DONE]") == 1
    assert all("error" not in frame for frame in frames), ("Chat stream error", raw.decode())
    message = {"role": "assistant", "content": None}
    tools = {}
    usage = None
    for frame in frames:
        if "usage" in frame and not frame.get("choices"):
            usage = frame["usage"]
            continue
        if not frame.get("choices"):
            continue
        delta = frame["choices"][0]["delta"]
        if isinstance(delta.get("content"), str):
            message["content"] = (message["content"] or "") + delta["content"]
        if delta.get("reasoning_content"):
            message["reasoning_content"] = message.get("reasoning_content", "") + delta["reasoning_content"]
        for call in delta.get("tool_calls", []):
            tool = tools.setdefault(call["index"], {"id": call.get("id"), "type": "function", "function": {"name": "", "arguments": ""}})
            tool["function"]["name"] += call.get("function", {}).get("name", "")
            tool["function"]["arguments"] += call.get("function", {}).get("arguments", "")
    if tools:
        message["tool_calls"] = list(tools.values())
    return {"choices": [{"message": message}], "usage": usage}


def tool_calls(source, response):
    if source == "chat":
        return [(call["id"], call["function"]["name"], json.loads(call["function"]["arguments"])) for call in response["choices"][0]["message"].get("tool_calls", [])]
    if source == "responses":
        return [(item["call_id"], item["name"], json.loads(item["arguments"])) for item in response["output"] if item["type"] == "function_call"]
    return [(block["id"], block["name"], block["input"]) for block in response["content"] if block["type"] == "tool_use"]


def followup(source, request, response):
    result = json.loads(json.dumps(request))
    result.pop("tool_choice", None)
    calls_in_response = tool_calls(source, response)
    if source == "chat":
        result["messages"].append(response["choices"][0]["message"])
        result["messages"].extend({"role": "tool", "tool_call_id": identity, "content": "synthetic tool result"} for identity, _, _ in calls_in_response)
    elif source == "messages":
        result["messages"].append({"role": "assistant", "content": response["content"]})
        result["messages"].append({"role": "user", "content": [{"type": "tool_result", "tool_use_id": identity, "content": "synthetic tool result"} for identity, _, _ in calls_in_response]})
    else:
        # Item identities have no equivalent Chat/Messages request field; omit optional envelopes
        # in the cross-protocol history while retaining every function correlation and argument.
        result["input"].extend({key: value for key, value in item.items() if key not in ["id", "status"]} for item in response["output"])
        result["input"].extend({"type": "function_call_output", "call_id": identity, "output": "synthetic tool result"} for identity, _, _ in calls_in_response)
    return result


class WebSocket:
    """Real RFC6455 client with bounded reads; fixture traffic uses loopback only."""

    def __init__(self, key=None, origin=None):
        self.socket = socket.create_connection(("127.0.0.1", data_port), timeout=10)
        nonce = base64.b64encode(secrets.token_bytes(16)).decode()
        headers = ["GET /v1/responses HTTP/1.1", f"Host: 127.0.0.1:{data_port}", "Upgrade: websocket", "Connection: Upgrade", "Sec-WebSocket-Version: 13", "Sec-WebSocket-Key: " + nonce, "X-Codex-Turn-State: synthetic-turn"]
        if key:
            headers.append("Authorization: Bearer " + key)
        if origin:
            headers.append("Origin: " + origin)
        self.socket.sendall(("\r\n".join(headers) + "\r\n\r\n").encode())
        raw = b""
        while not raw.endswith(b"\r\n\r\n"):
            raw += self.socket.recv(1)
            assert len(raw) < 16384
        self.status = int(raw.split(b" ", 2)[1])
        if self.status == 101:
            accept = base64.b64encode(hashlib.sha1((nonce + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest())
            response_headers = {line.split(b": ", 1)[0].lower(): line.split(b": ", 1)[1] for line in raw.split(b"\r\n")[1:] if b": " in line}
            assert response_headers[b"sec-websocket-accept"] == accept
            assert b"x-codex-turn-state: synthetic-turn" in raw.lower()

    def send(self, payload, opcode=1, final=True):
        if isinstance(payload, dict):
            payload = json.dumps(payload, ensure_ascii=False).encode()
        mask = secrets.token_bytes(4)
        size = len(payload)
        length = bytes([0x80 | size]) if size < 126 else b"\xfe" + struct.pack("!H", size) if size <= 65535 else b"\xff" + struct.pack("!Q", size)
        self.socket.sendall(bytes([(0x80 if final else 0) | opcode]) + length + mask + bytes(value ^ mask[index % 4] for index, value in enumerate(payload)))

    def receive(self):
        def exact(size):
            result = b""
            while len(result) < size:
                chunk = self.socket.recv(size - len(result))
                if not chunk:
                    raise EOFError("fixture WebSocket closed")
                result += chunk
            return result

        # Heartbeats can arrive between any two application frames, including immediately
        # after the handshake. Consume control frames consistently in every test path.
        while True:
            header = exact(2)
            assert header[0] & 0x80 and not header[1] & 0x80
            size = header[1] & 0x7f
            if size == 126:
                size = struct.unpack("!H", exact(2))[0]
            elif size == 127:
                size = struct.unpack("!Q", exact(8))[0]
            assert size <= 4 * 1024 * 1024
            opcode, payload = header[0] & 0xf, exact(size)
            if opcode == 9:
                self.send(payload, opcode=10)
            elif opcode != 10:
                return opcode, payload

    def turn(self, request, fragmented=False):
        payload = json.dumps({**request, "stream": True, "type": "response.create"}, ensure_ascii=False).encode()
        if fragmented:
            split = payload.index("商".encode()) + 1
            self.send(payload[:split], final=False)
            self.send(payload[split:], opcode=0)
        else:
            self.send(payload)
        frames = []
        for _ in range(4096):
            opcode, payload = self.receive()
            assert opcode == 1, ("unexpected WebSocket frame", opcode, payload.decode(errors="replace"))
            frame = json.loads(payload)
            frames.append(frame)
            if frame["type"] in ["response.completed", "response.failed", "error"]:
                return frames
        raise AssertionError("fixture turn exceeded frame bound")

    def close(self):
        try:
            self.send(struct.pack("!H", 1000), opcode=8)
        except OSError:
            pass
        self.socket.close()

    def wait_for_content(self):
        response_id = None
        for _ in range(100):
            opcode, raw = self.receive()
            assert opcode == 1, ("WebSocket content wait", opcode, raw.decode(errors="replace"))
            frame = json.loads(raw)
            if frame["type"] == "response.created":
                response_id = frame["response"]["id"]
            if frame["type"] == "response.output_text.delta":
                assert response_id is not None
                return response_id
            assert frame["type"] not in ["response.failed", "error"], ("WebSocket content failure", frame)
        raise AssertionError("fixture content deadline")


def wait_ready():
    for _ in range(200):
        if gateway.poll() is not None:
            raise RuntimeError("owned gateway failed to start; inspect private local log")
        try:
            api("GET", "/admin/config-versions")
            return
        except (OSError, AssertionError):
            time.sleep(0.1)
    raise RuntimeError("owned gateway readiness deadline")


def restart():
    global gateway
    gateway.terminate()
    gateway.wait(timeout=30)
    gateway = subprocess.Popen([str(binary), "serve", "--state-dir", str(state), "--credential-dir", str(credentials), "--data-listen", f"127.0.0.1:{data_port}", "--management-listen", f"127.0.0.1:{admin_port}"], stdout=log, stderr=log, env={**os.environ, "SSL_CERT_FILE": str(ca)})
    wait_ready()


try:
    wait_ready()
    draft = api("POST", "/admin/config-versions", {"id": "local-b", "description": "Batch B controlled protocol acceptance"}, expected=201)
    scope, revision = draft["id"], draft["revision"]
    api("POST", "/admin/egress-policies", {"id": "local-policy", "name": "Loopback only", "allowed_schemes": ["https"], "allowed_hosts": ["127.0.0.1"], "allowed_ports": [provider.server_port, fallback_provider.server_port], "allowed_cidrs": ["127.0.0.1/32"], "redirect_mode": "deny", "max_redirects": 0}, expected=201)
    for target, (_, adapter, api_format) in PROTOCOLS.items():
        kind = "anthropic-compatible" if target == "messages" else "openai-compatible"
        upstream, endpoint = "upstream-" + target, "endpoint-" + target
        api("POST", "/admin/upstreams", {"id": upstream, "name": "Local " + target, "kind": kind, "enabled": True, "tags": [], "egress_policy_id": "local-policy"}, expected=201)
        api("POST", f"/admin/upstreams/{upstream}/endpoints", {"id": endpoint, "adapter_id": adapter, "api_format": api_format, "base_url": f"https://127.0.0.1:{provider.server_port}/v1", "inference_path": PROTOCOLS[target][0][3:], "models_path": None, "transport": "https", "enabled": True}, expected=201)
        for name, priority in [("a", 0), ("b", 10)]:
            account = api("POST", f"/admin/upstreams/{upstream}/account-import", {"id": "account-" + target + "-" + name, "channel": kind, "secret": keys[name]}, expected=201)
            api("POST", f"/admin/endpoints/{endpoint}/credential-bindings", {"credential_id": account["id"], "enabled": True, "priority": priority, "weight": 1, "concurrency": 2}, expected=201)
        api("POST", "/admin/public-models", {"id": "public-" + target, "model_name": "model-" + target, "display_name": "Local " + target, "status": "active", "capabilities": {}}, expected=201)
        api("POST", f"/admin/public-models/public-{target}/routes", {"id": "route-" + target, "policy": "smooth_weighted_round_robin", "max_attempts": 2, "bootstrap_timeout_ms": 20000}, expected=201)
        override = {"allow_unlisted_model": True, **({"stored_responses": True, "response_compaction": True} if target == "responses" else {})}
        api("POST", f"/admin/routes/route-{target}/candidates", {"id": "candidate-" + target, "endpoint_id": endpoint, "upstream_model": "model-" + target, "credential_scope": "all_active", "transform_mode": "canonical_bridge", "enabled": True, "priority": 0, "weight": 1, "capability_override": override}, expected=201)
        fallback_upstream, fallback_endpoint = "fallback-upstream-" + target, "fallback-endpoint-" + target
        api("POST", "/admin/upstreams", {"id": fallback_upstream, "name": "Local fallback " + target, "kind": kind, "enabled": True, "tags": [], "egress_policy_id": "local-policy"}, expected=201)
        api("POST", f"/admin/upstreams/{fallback_upstream}/endpoints", {"id": fallback_endpoint, "adapter_id": adapter, "api_format": api_format, "base_url": f"https://127.0.0.1:{fallback_provider.server_port}/v1", "inference_path": PROTOCOLS[target][0][3:], "models_path": None, "transport": "https", "enabled": True}, expected=201)
        account = api("POST", f"/admin/upstreams/{fallback_upstream}/account-import", {"id": "fallback-account-" + target, "channel": kind, "secret": keys["b"]}, expected=201)
        api("POST", f"/admin/endpoints/{fallback_endpoint}/credential-bindings", {"credential_id": account["id"], "enabled": True, "priority": 0, "weight": 1, "concurrency": 1}, expected=201)
        fallback_candidates[target] = {"id": "fallback-candidate-" + target, "endpoint_id": fallback_endpoint, "upstream_model": "model-" + target, "credential_scope": "all_active", "transform_mode": "canonical_bridge", "enabled": False, "priority": 10, "weight": 1, "capability_override": override}
        api("POST", f"/admin/routes/route-{target}/candidates", fallback_candidates[target], expected=201)
    api("POST", "/admin/access-groups", {"id": "local-group", "name": "Local client", "status": "active", "limits": {}}, expected=201)
    for target in PROTOCOLS:
        api("POST", "/admin/access-groups/local-group/routes", {"route_id": "route-" + target, "enabled": True}, expected=201)
    issued_key = api("POST", "/admin/client-keys", {"id": "local-key", "access_group_id": "local-group", "status": "active", "expires_at_ms": None}, expected=201)["key"]
    sibling_key = api("POST", "/admin/client-keys", {"id": "sibling-key", "access_group_id": "local-group", "status": "active", "expires_at_ms": None}, expected=201)["key"]
    api("POST", "/admin/access-groups", {"id": "restricted-group", "name": "Synthetic no-model permission", "status": "active", "limits": {}}, expected=201)
    restricted_key = api("POST", "/admin/client-keys", {"id": "restricted-key", "access_group_id": "restricted-group", "status": "active", "expires_at_ms": None}, expected=201)["key"]
    entries = []
    for target in PROTOCOLS:
        entries.append({"provider_id": "upstream-" + target, "channel_id": "endpoint-" + target, "model": "model-" + target, "input_microunits_per_million": 1_000_000, "output_microunits_per_million": 2_000_000, "reasoning_microunits_per_million": 2_000_000, "cache_read_microunits_per_million": 500_000, "cache_creation_microunits_per_million": 3_000_000, "cached_microunits_per_million": 500_000})
    api("POST", "/admin/billing/catalogs", {"catalog_version_id": "local-prices", "effective_at_ms": 0, "source": "operator", "entries": entries}, expected=201)
    publish()
    check("publication_has_no_inference_side_effect", not calls)
    execution_initial_ids = cpar_execution_evidence_http.initial(globals())
    for stream in [False, True]:
        request = body_for("responses", "chat", "text-round", stream)
        request["store"] = True
        before = len(calls)
        status, _, _ = data("/v1/responses", request)
        check(f"stored_requires_selected_candidate_capability_{stream}", status >= 400 and len(calls) == before)
    before = len(calls)
    status, _, _ = data("/v1/responses", body_for("responses", "responses", "text-round"), key=restricted_key)
    check("known_model_permission_before_inference", status == 404 and len(calls) == before)
    status, _, _ = data("/v1/messages/count_tokens", body_for("messages", "messages", "text-round"), key=restricted_key)
    check("known_model_permission_before_count_tokens_capability", status == 404 and len(calls) == before)

    for source in ["chat", "responses"]:
        for target in ["chat", "responses"]:
            # Request-control fidelity is independent of native structured output metadata.
            # The separate thinking cases below test the visible output and bridge rejection.
            request = body_for(source, target, "text-round")
            request["reasoning_effort" if source == "chat" else "reasoning"] = "high" if source == "chat" else {"effort": "high"}
            status, _, _ = data(PROTOCOLS[source][0], request)
            wire = calls[-1]["body"]
            effort = wire.get("reasoning_effort") if target == "chat" else wire.get("reasoning", {}).get("effort")
            check("exact_effort_" + source + "_to_" + target, status == 200 and effort == "high")
    request = body_for("messages", "messages", "thinking-round")
    request["thinking"] = {"type": "enabled", "budget_tokens": 2048}
    status, _, _ = data("/v1/messages", request)
    check("native_exact_thinking_budget", status == 200 and calls[-1]["body"]["thinking"] == request["thinking"])
    for target in PROTOCOLS:
        request = body_for("messages", target, "text-round")
        request["messages"][0]["content"] = [{"type": "text", "text": "text-round", "cache_control": {"type": "ephemeral", "ttl": "1h"}}]
        before = len(calls)
        status, _, _ = data("/v1/messages", request)
        check("native_cache_control_or_exact_rejection_" + target, status == 200 and calls[-1]["body"]["messages"][0]["content"][0] == request["messages"][0]["content"][0] if target == "messages" else status >= 400 and len(calls) == before)

    for stream in [False, True]:
        for source in PROTOCOLS:
            for target in PROTOCOLS:
                path = PROTOCOLS[source][0]
                request = body_for(source, target, "text-round", stream)
                status, raw, _ = data(path, request)
                response = completed(source, raw, stream) if status == 200 else {}
                check(f"text_{source}_to_{target}_{'sse' if stream else 'json'}", status == 200 and b"synthetic answer" in raw)
                evidence = response["usage"]["cpar_usage"]
                check(f"usage_{source}_to_{target}_{stream}", evidence["provenance"] == "measured" and evidence["cache_read_tokens"] == (3 if target == "messages" else None) and evidence["cached_tokens"] == (None if target == "messages" else 6))
                for parallel, selection, marker in [(False, "named", "tool-round"), (True, "required", "parallel-round")]:
                    request = body_for(source, target, marker, stream, tools=True, parallel=parallel, choice=selection)
                    start = len(calls)
                    status, raw, _ = data(path, request)
                    response = completed(source, raw, stream) if status == 200 else {}
                    decoded_calls = tool_calls(source, response) if status == 200 else []
                    check(f"tools_{source}_to_{target}_{stream}_{parallel}", status == 200 and decoded_calls == [("call_local_" + str(index), "echo", {"value": "商界" + str(index)}) for index in range(2 if parallel else 1)])
                    outbound = calls[start]["body"]
                    wire_parallel = not outbound["tool_choice"]["disable_parallel_tool_use"] if target == "messages" else outbound["parallel_tool_calls"]
                    check(f"outbound_controls_{source}_to_{target}_{stream}_{parallel}", wire_parallel == parallel and len(calls) == start + 1)
                    status, followup_raw, _ = data(path, followup(source, request, response))
                    if status == 200:
                        completed(source, followup_raw, stream)
                    check(f"tool_followup_{source}_to_{target}_{stream}_{parallel}", status == 200 and b"synthetic answer" in followup_raw and calls[-1]["has_result"])

    # Malformed ingress and unknown semantics reject without a transport call.
    for source in PROTOCOLS:
        request = body_for(source, "responses", "text-round")
        request["unknown_execution_control"] = True
        before = len(calls)
        status, _, _ = data(PROTOCOLS[source][0], request)
        check("unknown_ingress_" + source, status == 400 and len(calls) == before)
    before = len(calls)
    status, _, _ = data("/v1/responses", {"model": "model-responses", "input": [{"type": 7, "role": "user", "content": "text-round"}]})
    check("malformed_tag_before_upstream", status == 400 and len(calls) == before)
    for source in PROTOCOLS:
        for stream in [False, True]:
            restart()
            before = len(calls)
            status, raw, _ = data(PROTOCOLS[source][0], body_for(source, source, "serial-violation", stream, tools=True, parallel=False))
            check(f"serial_violation_{source}_{stream}", len(calls) == before + 1 and (status >= 400 if not stream else b"error" in raw) and b"response.completed" not in raw and b"message_stop" not in raw and b"data: [DONE]" not in raw)
            restart()
            before = len(calls)
            status, raw, _ = data(PROTOCOLS[source][0], body_for(source, source, "invalid-args", stream, tools=True))
            check(f"invalid_tool_arguments_{source}_{stream}", len(calls) == before + 1 and (status >= 400 if not stream else b"error" in raw) and b"response.completed" not in raw and b"message_stop" not in raw and b"data: [DONE]" not in raw)

    for source in PROTOCOLS:
        restart()
        before = len(calls)
        status, raw, _ = data(PROTOCOLS[source][0], body_for(source, "responses", "truncated-round", True))
        failures = [json.loads(line[6:]) for line in raw.decode().splitlines() if line.startswith("data: ") and line != "data: [DONE]"]
        if not (status == 200 and len([frame for frame in failures if "error" in frame or frame.get("type") in ["error", "response.failed"]]) == 1 and b"data: [DONE]" not in raw):
            raise AssertionError(("EOF terminal", source, status, raw.decode()))
        check("stream_eof_unique_failure_" + source, status == 200 and len([frame for frame in failures if "error" in frame or frame.get("type") in ["error", "response.failed"]]) == 1 and b"response.completed" not in raw and b"message_stop" not in raw and b"data: [DONE]" not in raw and len(calls) == before + 1)

    # Native reasoning, signatures, citations and history are preserved by their owning protocol.
    restart()
    for source, marker in [("chat", "thinking-round"), ("chat", "reasoning-only-round"), ("responses", "thinking-round"), ("messages", "signed-round"), ("responses", "citation-round"), ("messages", "citation-round")]:
        for stream in [False, True]:
            request = body_for(source, source, marker, stream)
            status, raw, _ = data(PROTOCOLS[source][0], request)
            response = completed(source, raw, stream) if status == 200 else {}
            check(f"native_metadata_{source}_{marker}_{stream}", status == 200 and (b"synthetic visible thought" in raw if "round" in marker and marker != "citation-round" else b"synthetic source" in raw))
            if source == "chat":
                if marker == "reasoning-only-round":
                    check(f"reasoning_only_null_content_{stream}", response["choices"][0]["message"]["content"] is None)
                request["messages"].append(response["choices"][0]["message"])
                request["messages"].append({"role": "user", "content": "next"})
            elif source == "messages":
                request["messages"].append({"role": "assistant", "content": response["content"]})
                request["messages"].append({"role": "user", "content": "next"})
            else:
                request["input"].extend(response["output"])
                request["input"].append({"role": "user", "content": "next"})
            status, _, _ = data(PROTOCOLS[source][0], request)
            check(f"native_metadata_replay_{source}_{marker}_{stream}", status == 200)
            if source == "chat" and marker == "reasoning-only-round":
                check(f"reasoning_only_replay_wire_preserved_{stream}", calls[-1]["body"]["messages"][-2] == request["messages"][-2])
    for stream in [False, True]:
        request = body_for("chat", "chat", "text-round", stream, tools=True)
        request["messages"].extend([
            {"role": "assistant", "content": None, "tool_calls": [{"id": "call-pending", "type": "function", "function": {"name": "echo", "arguments": '{"value":"pending"}'}}]},
            {"role": "assistant", "content": None, "reasoning_content": "thought before result"},
            {"role": "tool", "tool_call_id": "call-pending", "content": "done"},
        ])
        before = len(calls)
        status, _, _ = data("/v1/chat/completions", request)
        check(f"reasoning_only_cannot_overtake_tool_results_{stream}", status == 400 and len(calls) == before)
    for source in PROTOCOLS:
        restart()
        before = len(calls)
        status, raw, _ = data(PROTOCOLS[source][0], body_for(source, "chat", "usage-error-round", True))
        check("same_chunk_usage_error_terminal_" + source, status == 200 and b"error" in raw and b"response.completed" not in raw and b"message_stop" not in raw and b"data: [DONE]" not in raw and len(calls) == before + 1)
        identifier = calls[-1]["response_id"]
        deadline = time.monotonic() + 3
        while True:
            rows = [row for row in all_items("/admin/operations/billing?limit=100") if row["response_id"] == identifier]
            if rows or time.monotonic() >= deadline:
                break
            time.sleep(0.1)
        check("same_chunk_usage_error_keeps_evidence_" + source, len(rows) == 1 and rows[0]["input_tokens"] == 20 and rows[0]["output_tokens"] == 10 and rows[0]["cached_tokens"] == 6 and rows[0]["reasoning_tokens"] == 4 and rows[0]["usage_provenance"] == "measured")
    for source in ["chat", "responses"]:
        restart()
        before = len(calls)
        status, raw, _ = data(PROTOCOLS[source][0], body_for(source, "messages", "signed-round", True))
        check("signed_bridge_explicit_failure_" + source, status == 200 and b"error" in raw and b"response.completed" not in raw and b"data: [DONE]" not in raw and len(calls) == before + 1)

    for source in ["chat", "responses"]:
        restart()
        before = len(calls)
        status, raw, _ = data(PROTOCOLS[source][0], body_for(source, "messages", "signed-round"))
        check("signed_json_bridge_explicit_failure_" + source, status >= 400 and len(calls) == before + 1)
        identifier = calls[-1]["response_id"]
        deadline = time.monotonic() + 3
        while True:
            rows = [row for row in all_items("/admin/operations/billing?limit=100") if row["response_id"] == identifier]
            if rows or time.monotonic() >= deadline:
                break
            time.sleep(0.1)
        check("projection_failure_retains_decoded_json_usage_" + source, len(rows) == 1 and rows[0]["input_tokens"] == 7 and rows[0]["output_tokens"] == 10 and rows[0]["cache_read_tokens"] == 3 and rows[0]["cache_creation_tokens"] == 2 and rows[0]["usage_provenance"] == "measured")

    restart()
    before = len(calls)
    for source in ["chat", "responses"]:
        request = body_for(source, "messages", "text-round")
        request["reasoning_effort" if source == "chat" else "reasoning"] = "low" if source == "chat" else {"effort": "low"}
        status, _, _ = data(PROTOCOLS[source][0], request)
        check("non_equivalent_effort_rejected_" + source, status == 400 and len(calls) == before)
    status, _, _ = data("/v1/messages/count_tokens", {"model": "model-chat", "messages": [{"role": "user", "content": "text-round"}]})
    check("count_tokens_explicit_unsupported", status == 422 and len(calls) == before)
    status, raw, _ = data("/v1/messages/count_tokens", {}, key="synthetic-invalid")
    check("count_tokens_authentication_precedes_decode", status == 401 and b"input_tokens" not in raw and len(calls) == before)
    status, raw, _ = data("/v1/messages/count_tokens", {"model": "foreign-model", "messages": [{"role": "user", "content": "text-round"}]})
    check("count_tokens_model_visibility_precedes_unsupported", status == 404 and b"input_tokens" not in raw and len(calls) == before)

    stored_request = body_for("responses", "responses", "tool-round", tools=True, parallel=False)
    stored_request["store"] = True
    status, raw, _ = data("/v1/responses", stored_request)
    stored = json.loads(raw)
    check("stored_tool_root", status == 200 and len(tool_calls("responses", stored)) == 1)
    stored_id = stored["id"]
    status, denied, _ = data("/v1/responses/" + stored_id, method="GET", key=sibling_key)
    check("stored_owner_exact_key", status == 404 and stored_id.encode() not in denied)
    status, _, _ = data("/v1/responses/" + stored_id, method="GET")
    check("stored_public_get_before_restart", status == 200)
    restart()
    status, got, _ = data("/v1/responses/" + stored_id, method="GET")
    check("stored_public_get_after_real_process_restart", status == 200 and json.loads(got)["output"] == stored["output"])
    before = len(calls)
    continuation = {"model": "model-responses", "previous_response_id": stored_id, "input": [{"type": "function_call_output", "call_id": "call_local_0", "output": "synthetic tool result"}]}
    status, raw, _ = data("/v1/responses", continuation)
    check("stored_tool_continuation_after_restart", status == 200 and b"synthetic answer" in raw and len(calls) == before + 1 and calls[-1]["has_result"])
    check("continuation_exact_account_and_channel", calls[-1]["account"] == "a" and calls[-1]["protocol"] == "responses")
    # A completed text root can be compacted without an unresolved tool call.
    status, raw, _ = data("/v1/responses", {**body_for("responses", "responses", "text-round"), "store": True})
    text_id = json.loads(raw)["id"]
    status, compact_raw, _ = data("/v1/responses/compact", {"model": "model-responses", "previous_response_id": text_id, "stream": False})
    compact = json.loads(compact_raw)
    check("compact_public_success", status == 200 and compact["output"][0]["created_by"] == "cpar")
    restart()
    before = len(calls)
    status, raw, _ = data("/v1/responses", {"model": "model-responses", "input": [compact["output"][0], {"role": "user", "content": "next"}]})
    check("compact_after_real_process_restart", status == 200 and len(calls) == before + 1 and calls[-1]["account"] == "a")
    before = len(calls)
    valid_token = compact["output"][0]["encrypted_content"]
    token_prefix, token_payload = valid_token.split(".", 1)
    corrupted_token = token_prefix + "." + ("A" if token_payload[0] != "A" else "B") + token_payload[1:]
    for token in ["cpar_compact_v1.malformed", corrupted_token]:
        item = {**compact["output"][0], "encrypted_content": token}
        status, _, _ = data("/v1/responses", {"model": "model-responses", "input": [item, {"role": "user", "content": "next"}]})
        check("compact_malformed_or_corrupt_local_rejection", status >= 400 and len(calls) == before)
    status, _, _ = data("/v1/responses", {"model": "model-responses", "input": [compact["output"][0], {"role": "user", "content": "next"}]}, key=sibling_key)
    check("compact_owner_exact_key", status == 404 and len(calls) == before)
    for condition in ["expired", "corrupt"]:
        status, raw, _ = data("/v1/responses/compact", {"model": "model-responses", "previous_response_id": text_id, "stream": False})
        negative_compact = json.loads(raw)
        assert status == 200
        compact_id = negative_compact["output"][0]["encrypted_content"]
        with sqlite3.connect(state / "control.sqlite3") as database:
            compact_row = database.execute("SELECT created_at_ms, expires_at_ms, ciphertext FROM stored_response_compactions WHERE compact_id=?", (compact_id,)).fetchone()
            assert compact_row is not None
            changed_bytes = bytes([compact_row[2][0] ^ 1]) + compact_row[2][1:] if condition == "corrupt" else compact_row[2]
            changed_expiry = compact_row[0] + 1 if condition == "expired" else compact_row[1]
            database.execute("UPDATE stored_response_compactions SET expires_at_ms=?, ciphertext=? WHERE compact_id=?", (changed_expiry, changed_bytes, compact_id))
        restart()
        before = len(calls)
        status, _, _ = data("/v1/responses", {"model": "model-responses", "input": [negative_compact["output"][0], {"role": "user", "content": "next"}]})
        check("compact_" + condition + "_after_restart_local_rejection", status >= 400 and len(calls) == before)
    restart()

    # WebSocket exercises the real upgrade, wire framing and the shared execution path.
    for key, origin, expected in [(None, None, 401), (issued_key, "https://browser.invalid", 403)]:
        socket_client = WebSocket(key, origin)
        check("websocket_upgrade_auth_or_origin", socket_client.status == expected)
        socket_client.close()
    ws = WebSocket(issued_key)
    check("websocket_real_upgrade", ws.status == 101)
    request = body_for("responses", "responses", "parallel-round 商", tools=True, parallel=True, choice="required")
    before = len(calls)
    frames = ws.turn(request, fragmented=True)
    first = frames[-1]["response"]
    check("websocket_fragmented_utf8_parallel_tools", frames[-1]["type"] == "response.completed" and len(tool_calls("responses", first)) == 2 and len(calls) == before + 1)
    check("websocket_usage_measured", first["usage"]["cpar_usage"]["cached_tokens"] == 6 and first["usage"]["cpar_usage"]["reasoning_tokens"] == 4)
    before = len(calls)
    frames = ws.turn({"model": "model-responses", "previous_response_id": first["id"], "input": [{"type": "function_call_output", "call_id": identity, "output": "synthetic tool result"} for identity, _, _ in tool_calls("responses", first)]})
    check("websocket_tool_continuation_exact_account", frames[-1]["type"] == "response.completed" and len(calls) == before + 1 and calls[-1]["account"] == "a" and calls[-1]["has_result"])
    frames = ws.turn(body_for("responses", "responses", "thinking-round"))
    thinking_root = frames[-1]["response"]
    check("websocket_native_reasoning_hierarchy", thinking_root["output"][0]["summary"][0]["text"] == "synthetic visible thought" and thinking_root["output"][0]["id"] == "rs_local")
    frames = ws.turn({"model": "model-responses", "previous_response_id": thinking_root["id"], "input": [{"role": "user", "content": "next"}]})
    check("websocket_reasoning_continuation", frames[-1]["type"] == "response.completed" and calls[-1]["body"]["input"][1]["summary"][0]["text"] == "synthetic visible thought")
    ws.close()
    ws = WebSocket(issued_key)
    before = len(calls)
    frames = ws.turn({"model": "model-responses", "previous_response_id": first["id"], "input": "next"})
    check("websocket_roots_are_session_scoped", frames[-1]["type"] == "error" and len(calls) == before)
    ws.close()
    ws = WebSocket(issued_key)
    frames = ws.turn(body_for("responses", "responses", "duplicate-root first"))
    collision_root = frames[-1]["response"]
    frames = ws.turn(body_for("responses", "responses", "duplicate-root second"))
    check("websocket_cache_collision_cannot_acknowledge_success", frames[-1]["type"] == "error" and not any(frame["type"] == "response.completed" for frame in frames))
    before = len(calls)
    frames = ws.turn({"model": "model-responses", "previous_response_id": collision_root["id"], "input": "next"})
    check("websocket_collision_preserves_prior_root", frames[-1]["type"] == "response.completed" and len(calls) == before + 1 and "duplicate-root first" in json.dumps(calls[-1]["body"]))
    ws.close()
    for payload, opcode, expected in [(b"binary", 2, 1003), (b"\xff", 1, 1007)]:
        ws = WebSocket(issued_key)
        before = len(calls)
        ws.send(payload, opcode)
        actual_opcode, payload = ws.receive()
        assert actual_opcode == 8 and struct.unpack("!H", payload[:2])[0] == expected and len(calls) == before, ("websocket invalid wire", actual_opcode, payload.hex(), expected, before, len(calls))
        check("websocket_invalid_wire_before_upstream", True)
        ws.close()
    ws = WebSocket(issued_key)
    before = len(calls)
    for index in range(65):
        ws.send(b" ", opcode=1 if index == 0 else 0, final=False)
    opcode, payload = ws.receive()
    if opcode != 8 or struct.unpack("!H", payload[:2])[0] != 1009:
        raise AssertionError(("fragment bound close", opcode, payload.decode(errors="replace")))
    check("websocket_fragment_count_bound", opcode == 8 and struct.unpack("!H", payload[:2])[0] == 1009 and len(calls) == before)
    ws.close()

    restart()
    held_started.clear()
    held_closed.clear()
    ws = WebSocket(issued_key)
    ws.send({**body_for("responses", "responses", "held-round", stream=True), "type": "response.create"})
    check("websocket_held_upstream_started", held_started.wait(5))
    cancelled_response_ids.append(ws.wait_for_content())
    ws.close()
    check("websocket_close_cancels_real_upstream_socket", held_closed.wait(5))
    status, _, _ = data("/v1/responses", body_for("responses", "responses", "text-round"))
    check("websocket_close_releases_capacity", status == 200)
    restart()
    held_started.clear()
    held_closed.clear()
    ws = WebSocket(issued_key)
    ws.send({**body_for("responses", "responses", "held-round", stream=True), "type": "response.create"})
    check("websocket_pending_test_started", held_started.wait(5))
    cancelled_response_ids.append(ws.wait_for_content())
    before = len(calls)
    for _ in range(2):
        ws.send({"type": "response.create", "model": "model-responses", "input": "queued"})
    for _ in range(50):
        opcode, payload = ws.receive()
        if opcode == 8:
            break
    check("websocket_pending_turn_bound", opcode == 8 and struct.unpack("!H", payload[:2])[0] == 1008 and len(calls) == before)
    ws.close()
    check("websocket_pending_overflow_cancels_socket", held_closed.wait(5))

    restart()
    held_started.clear()
    held_closed.clear()
    connection = http.client.HTTPConnection("127.0.0.1", data_port, timeout=10)
    connection.request("POST", "/v1/responses", json.dumps(body_for("responses", "responses", "held-round", True)), {"Authorization": "Bearer " + issued_key, "Content-Type": "application/json"})
    downstream_socket = connection.sock
    response = connection.getresponse()
    check("sse_held_upstream_started", response.status == 200 and held_started.wait(5))
    response_id = None
    while True:
        line = response.readline()
        assert line
        if line.startswith(b"data: "):
            frame = json.loads(line[6:])
            if frame["type"] == "response.created":
                response_id = frame["response"]["id"]
            if frame["type"] == "response.output_text.delta":
                cancelled_response_ids.append(response_id)
                break
    # HTTPConnection can detach a Connection: close socket into HTTPResponse. Keep
    # the original transport handle so this probes a real disconnect, not only fp.close().
    downstream_socket.shutdown(socket.SHUT_RDWR)
    response.close()
    connection.close()
    check("sse_disconnect_cancels_real_upstream_socket", held_closed.wait(5))
    status, _, _ = data("/v1/responses", body_for("responses", "responses", "text-round"))
    check("sse_disconnect_releases_capacity", status == 200)

    # Persisted read models preserve source counts and matched attempts, including cancelled work.
    deadline = time.monotonic() + 10
    while True:
        ledger = all_items("/admin/operations/billing?limit=100")
        for identifier in cancelled_response_ids:
            if not any(row["response_id"] == identifier for row in ledger):
                break
        else:
            break
        assert time.monotonic() < deadline, "cancelled Usage materialization deadline"
        time.sleep(0.1)
    check("cancelled_known_usage_durable", all(any(row["response_id"] == identifier and row["input_tokens"] == 20 and row["output_tokens"] is None and row["cached_tokens"] == 6 and row["usage_provenance"] == "measured" and row["cost_confidence"] == "partial" for row in ledger) for identifier in cancelled_response_ids))
    check("ledger_no_duplicate_charging", len({(row["request_id"], row["response_id"]) for row in ledger}) == len(ledger))
    for accounting, expected in [("inclusive", 37), ("exclusive", 34)]:
        rows = [row for row in ledger if row["input_accounting"] == accounting and row["input_tokens"] is not None and row["output_tokens"] is not None]
        check("ledger_exact_source_rates_" + accounting, bool(rows) and all(row["cost_microunits"] == expected and row["cost_confidence"] == "exact" and row["usage_provenance"] == "measured" for row in rows))
    usage_page = api("GET", "/admin/operations/usage?allow_partial=true&limit=100")
    check("management_usage_source_provenance", bool(usage_page["items"]) and all(row["input_tokens"]["provenance"] == "measured" for row in usage_page["items"]))
    restart()
    persisted = all_items("/admin/operations/billing?limit=100")
    check("ledger_survives_real_process_restart", {row["ledger_id"] for row in persisted} == {row["ledger_id"] for row in ledger})
    with sqlite3.connect(state / "control.sqlite3") as database:
        events = [json.loads(row[0]) for row in database.execute("SELECT payload_json FROM gateway_event_log WHERE event_type IN ('attempt','usage')")]
        attempts = {event["attempt"]["attempt_id"]: event["attempt"] for event in events if "attempt" in event}
        usages = [event["usage"] for event in events if "usage" in event]
        check("durable_usage_exact_attempt_lineage", bool(usages) and all(usage["attempt_id"] in attempts and attempts[usage["attempt_id"]]["request_id"] == usage["request_id"] for usage in usages))
        check("stored_payloads_are_encrypted", all(b"synthetic" not in row[0] and b"signature" not in row[0] for row in database.execute("SELECT ciphertext FROM stored_responses")))

    # TTL and ciphertext failures are local even after reopening the gateway process.
    for marker in ["thinking-round", "citation-round"]:
        request = {**body_for("responses", "responses", marker), "store": True}
        status, raw, _ = data("/v1/responses", request)
        root_response = json.loads(raw)
        check("stored_native_metadata_success_" + marker, status == 200)
        restart()
        status, raw, _ = data("/v1/responses/" + root_response["id"], method="GET")
        check("stored_native_metadata_get_after_restart_" + marker, status == 200 and json.loads(raw)["output"] == root_response["output"])
        before = len(calls)
        status, raw, _ = data("/v1/responses", {"model": "model-responses", "previous_response_id": root_response["id"], "input": [{"role": "user", "content": "next"}]})
        actual_history = calls[-1]["body"].get("input")
        expected_history = root_response["output"] + [{"type": "message", "role": "user", "content": [{"type": "input_text", "text": "next"}]}]
        assert status == 200 and len(calls) == before + 1 and actual_history[1:] == expected_history, ("stored native replay", marker, status, len(calls) - before, actual_history, expected_history)
        check("stored_native_metadata_continue_after_restart_" + marker, True)
    for condition in ["expired", "corrupt"]:
        status, raw, _ = data("/v1/responses", {**body_for("responses", "responses", "text-round"), "store": True})
        identifier = json.loads(raw)["id"]
        with sqlite3.connect(state / "control.sqlite3") as database:
            if condition == "expired":
                database.execute("UPDATE stored_responses SET expires_at_ms=created_at_ms+1 WHERE response_id=?", (identifier,))
            else:
                ciphertext = database.execute("SELECT ciphertext FROM stored_responses WHERE response_id=?", (identifier,)).fetchone()[0]
                database.execute("UPDATE stored_responses SET ciphertext=? WHERE response_id=?", (bytes([ciphertext[0] ^ 1]) + ciphertext[1:], identifier))
        restart()
        before = len(calls)
        status, _, _ = data("/v1/responses/" + identifier, method="GET")
        check("stored_" + condition + "_local_get_rejection", status == (404 if condition == "expired" else 500) and len(calls) == before)
        status, _, _ = data("/v1/responses", {"model": "model-responses", "previous_response_id": identifier, "input": "next"})
        check("stored_" + condition + "_local_continuation_rejection", status >= 400 and len(calls) == before)
    draft = api("POST", f"/admin/config-versions/{scope}/fork", {"id": "local-retries", "description": "Synthetic bounded fallback"}, expected=201)
    scope, revision = draft["id"], draft["revision"]
    for target, candidate in fallback_candidates.items():
        api("PATCH", f"/admin/routes/route-{target}/candidates/{candidate['id']}", {**candidate, "enabled": True})
    publish()
    for source in PROTOCOLS:
        for stream in [False, True]:
            restart()
            before = len(calls)
            status, raw, _ = data(PROTOCOLS[source][0], body_for(source, source, "retry-round", stream))
            completed(source, raw, stream) if status == 200 else None
            actual_accounts = [call["account"] for call in calls[before:]]
            assert status == 200 and actual_accounts == ["a", "b"], ("early sibling retry", source, stream, status, actual_accounts, json.loads(raw).get("error", {}).get("code") if status != 200 else None)
            check("early_retry_healthy_fallback_" + source + "_" + str(stream), True)
            check("early_retry_distinct_connection_" + source + "_" + str(stream), [call["connection"] for call in calls[before:]] == ["primary", "fallback"])
            restart()
            before = len(calls)
            status, raw, _ = data(PROTOCOLS[source][0], body_for(source, source, "always-fail", stream))
            check("early_retry_finite_attempts_" + source + "_" + str(stream), status >= 400 and [call["account"] for call in calls[before:]] == ["a", "b"])
            restart()
            before = len(calls)
            status, _, _ = data(PROTOCOLS[source][0], body_for(source, source, "auth-round", stream))
            check("credential_rejection_not_transparently_retried_" + source + "_" + str(stream), status == 502 and len(calls) == before + 1 and calls[-1]["account"] == "a")
            status, _, _ = data(PROTOCOLS[source][0], body_for(source, source, "text-round", stream))
            check("credential_healthy_sibling_remains_eligible_" + source + "_" + str(stream), status == 200 and len(calls) == before + 2 and calls[-1]["account"] == "b" and calls[-1]["connection"] == "primary")
        restart()
        held_started.clear()
        held_closed.clear()
        before = len(calls)
        pending = socket.create_connection(("127.0.0.1", data_port), timeout=10)
        raw_body = json.dumps(body_for(source, source, "slow-bootstrap")).encode()
        pending.sendall((f"POST {PROTOCOLS[source][0]} HTTP/1.1\r\nHost: 127.0.0.1:{data_port}\r\nAuthorization: Bearer {issued_key}\r\nContent-Type: application/json\r\nContent-Length: {len(raw_body)}\r\n\r\n").encode() + raw_body)
        check("bootstrap_cancel_started_" + source, held_started.wait(5))
        pending.shutdown(socket.SHUT_RDWR)
        pending.close()
        check("bootstrap_cancel_closes_upstream_without_retry_" + source, held_closed.wait(2) and len(calls) == before + 1)
    restart()
    status, raw, _ = data("/v1/responses", {**body_for("responses", "responses", "text-round"), "store": True})
    bound_root = json.loads(raw)
    status, raw, _ = data("/v1/responses/compact", {"model": "model-responses", "previous_response_id": bound_root["id"], "stream": False})
    bound_compact = json.loads(raw)
    draft = api("POST", f"/admin/config-versions/{scope}/fork", {"id": "local-disabled", "description": "Synthetic lineage eligibility rejection"}, expected=201)
    scope, revision = draft["id"], draft["revision"]
    account = api("GET", "/admin/credentials/account-responses-a")
    api("PATCH", "/admin/credentials/account-responses-a/status", {"status": "disabled", "credential_revision": account["revision"]})
    publish()
    restart()
    before = len(calls)
    status, _, _ = data("/v1/responses/" + bound_root["id"], method="GET")
    check("stored_get_retains_history_after_lineage_ineligibility", status == 200 and len(calls) == before)
    status, _, _ = data("/v1/responses", {"model": "model-responses", "previous_response_id": bound_root["id"], "input": "next"})
    check("stored_ineligible_lineage_zero_fallback", status >= 400 and len(calls) == before)
    status, _, _ = data("/v1/responses", {"model": "model-responses", "input": [bound_compact["output"][0], {"role": "user", "content": "next"}]})
    check("compact_ineligible_lineage_zero_fallback", status >= 400 and len(calls) == before)
    draft = api("POST", f"/admin/config-versions/{scope}/fork", {"id": "local-budget", "description": "Synthetic cumulative bootstrap budget"}, expected=201)
    scope, revision = draft["id"], draft["revision"]
    account = api("GET", "/admin/credentials/account-responses-a")
    api("PATCH", "/admin/credentials/account-responses-a/status", {"status": "active", "credential_revision": account["revision"]})
    for source in PROTOCOLS:
        api("PATCH", "/admin/routes/route-" + source, {"id": "route-" + source, "policy": "smooth_weighted_round_robin", "max_attempts": 2, "bootstrap_timeout_ms": 150})
    publish()
    for source in PROTOCOLS:
        for stream in [False, True]:
            restart()
            before = len(calls)
            started = time.monotonic()
            status, _, _ = data(PROTOCOLS[source][0], body_for(source, source, "slow-bootstrap", stream))
            elapsed = time.monotonic() - started
            # A non-streaming attempt retains its existing bounded generation ceiling.
            # Its elapsed time must still close the cumulative budget to any fallback.
            # Streaming startup uses the strict remaining 150 ms route budget.
            check("cumulative_bootstrap_budget_" + source + "_" + str(stream), status >= 400 and elapsed < (1.5 if stream else 5) and len(calls) == before + 1)
            checks[-1]["elapsed_ms"] = round(elapsed * 1000)
    cpar_execution_evidence_http.final(globals(), execution_initial_ids)
    cpar_execution_evidence_http.rotations(globals())
    passed = True
    print(json.dumps({"status": "PASS", "checks": len(checks), "private_state": str(root)}))
finally:
    gateway.terminate()
    gateway.wait(timeout=30)
    provider.shutdown()
    fallback_provider.shutdown()
    log.close()
    receipt = {"status": "PASS" if passed else "FAIL", "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "source_sha": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(), "checks": checks, "provider_calls": [{key: call[key] for key in ["account", "connection", "protocol", "scenario", "stream", "has_result", "compact"]} for call in calls], "private_state": str(root), "scope": "local_synthetic_only"}
    (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
