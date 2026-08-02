#!/usr/bin/env python3
"""Run the fixed value-free P12-08G1 live tuple matrix once."""

from __future__ import annotations

import argparse
import http.client
import json
import os
from pathlib import Path
import stat
import sys
import time
from urllib.parse import urlsplit


MAX_BODY = 2 * 1024 * 1024
MAX_EVENT = 128 * 1024
MAX_EVENTS = 4096
ALIASES = {
    "chat": "p12-g1-codex-chat",
    "responses": "p12-g1-codex-responses",
    "messages": "p12-g1-codex-messages",
}


class ProbeFailure(RuntimeError):
    """A fixed safe live-probe failure category."""


def private_bytes(path: Path, limit: int) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ProbeFailure("private_input_unavailable") from error
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_mode & 0o077:
            raise ProbeFailure("private_input_admission")
        value = os.read(descriptor, limit + 1)
    finally:
        os.close(descriptor)
    if not value or len(value) > limit:
        raise ProbeFailure("private_input_shape")
    return value


def private_value(path: Path, limit: int) -> str:
    value = private_bytes(path, limit)
    if b"\n" in value or b"\r" in value:
        raise ProbeFailure("private_input_shape")
    try:
        return value.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ProbeFailure("private_input_encoding") from error


def loopback_endpoint(raw: str) -> tuple[str, int]:
    parsed = urlsplit(raw)
    if (
        parsed.scheme != "http"
        or parsed.hostname != "127.0.0.1"
        or parsed.path not in ("", "/")
        or parsed.query
        or parsed.fragment
        or parsed.username is not None
        or parsed.password is not None
    ):
        raise ProbeFailure("endpoint_admission")
    return parsed.hostname, parsed.port or 80


def request_body(protocol: str, *, stream: bool, tool: bool) -> bytes:
    if tool:
        instruction = "Call emit_probe exactly once with value set to ready and return no text."
    else:
        instruction = "Return one short plain text result."
    if protocol == "responses":
        value = {
            "model": ALIASES[protocol],
            "input": [{"type": "message", "role": "user", "content": instruction}],
            "max_output_tokens": 96,
            "stream": stream,
        }
        if tool:
            value.update({
                "tools": [{
                    "type": "function", "name": "emit_probe", "description": "Emit the probe result.",
                    "parameters": {"type": "object", "properties": {"value": {"type": "string"}},
                                   "required": ["value"], "additionalProperties": False},
                }],
                "tool_choice": "required",
            })
    elif protocol == "chat":
        value = {
            "model": ALIASES[protocol], "messages": [{"role": "user", "content": instruction}],
            "max_tokens": 96, "stream": stream,
        }
        if stream:
            value["stream_options"] = {"include_usage": True}
        if tool:
            value.update({
                "tools": [{"type": "function", "function": {
                    "name": "emit_probe", "description": "Emit the probe result.",
                    "parameters": {"type": "object", "properties": {"value": {"type": "string"}},
                                   "required": ["value"], "additionalProperties": False},
                }}],
                "tool_choice": "required",
            })
    elif protocol == "messages":
        value = {
            "model": ALIASES[protocol], "max_tokens": 96,
            "messages": [{"role": "user", "content": instruction}], "stream": stream,
        }
        if tool:
            value.update({
                "tools": [{
                    "name": "emit_probe", "description": "Emit the probe result.",
                    "input_schema": {"type": "object", "properties": {"value": {"type": "string"}},
                                     "required": ["value"], "additionalProperties": False},
                }],
                "tool_choice": {"type": "any"},
            })
    else:
        raise ProbeFailure("protocol_unknown")
    return json.dumps(value, separators=(",", ":")).encode("utf-8")


def read_json(response: http.client.HTTPResponse):
    raw = response.read(MAX_BODY + 1)
    if len(raw) > MAX_BODY:
        raise ProbeFailure("body_bound")
    try:
        return json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ProbeFailure("json_invalid") from error


def sse_values(response: http.client.HTTPResponse):
    total = 0
    count = 0
    while True:
        line = response.readline(MAX_EVENT + 1)
        total += len(line)
        if total > MAX_BODY:
            raise ProbeFailure("stream_body_bound")
        if len(line) > MAX_EVENT:
            raise ProbeFailure("stream_event_bound")
        if not line:
            return
        if not line.startswith(b"data:"):
            continue
        count += 1
        if count > MAX_EVENTS:
            raise ProbeFailure("stream_event_count")
        payload = line[5:].strip()
        if payload == b"[DONE]":
            yield "DONE"
            continue
        try:
            value = json.loads(payload)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ProbeFailure("stream_json_invalid") from error
        if not isinstance(value, dict):
            raise ProbeFailure("stream_event_invalid")
        yield value


def openai_usage(value, input_name: str, output_name: str) -> bool:
    if not isinstance(value, dict):
        return False
    incoming, outgoing, total = value.get(input_name), value.get(output_name), value.get("total_tokens")
    return all(isinstance(item, int) and not isinstance(item, bool) and item >= 0 for item in (incoming, outgoing, total)) and total == incoming + outgoing


def valid_tool(name, arguments) -> bool:
    if name != "emit_probe":
        return False
    try:
        value = json.loads(arguments) if isinstance(arguments, str) else arguments
    except json.JSONDecodeError:
        return False
    return value == {"value": "ready"}


def observe_responses_json(value, tool: bool) -> dict:
    if not isinstance(value, dict) or value.get("status") != "completed" or not isinstance(value.get("output"), list):
        raise ProbeFailure("responses_lifecycle")
    saw_text = False
    saw_tool = False
    for item in value["output"]:
        if not isinstance(item, dict):
            raise ProbeFailure("responses_output_shape")
        if item.get("type") == "message":
            content = item.get("content")
            saw_text = isinstance(content, list) and any(
                isinstance(part, dict) and part.get("type") == "output_text" and isinstance(part.get("text"), str) and bool(part["text"])
                for part in content
            )
        elif item.get("type") == "function_call":
            saw_tool = valid_tool(item.get("name"), item.get("arguments"))
    if (tool and not saw_tool) or (not tool and not saw_text):
        raise ProbeFailure("responses_semantics")
    usage = openai_usage(value.get("usage"), "input_tokens", "output_tokens")
    if not usage:
        raise ProbeFailure("responses_usage")
    return {"projection": "tool" if tool else "text", "usage_valid": True, "terminal": "completed"}


def observe_responses_stream(response, tool: bool) -> dict:
    started = False
    completed = False
    saw_text = False
    tool_name = None
    tool_arguments = ""
    usage_valid = False
    for event in sse_values(response):
        if event == "DONE":
            continue
        kind = event.get("type")
        if kind in ("response.created", "response.in_progress"):
            started = True
        elif kind == "response.output_text.delta" and isinstance(event.get("delta"), str):
            saw_text = saw_text or bool(event["delta"])
        elif kind == "response.output_item.added" and isinstance(event.get("item"), dict):
            if event["item"].get("type") == "function_call":
                tool_name = event["item"].get("name") or tool_name
        elif kind == "response.function_call_arguments.delta" and isinstance(event.get("delta"), str):
            tool_arguments += event["delta"]
        elif kind == "response.output_item.done" and isinstance(event.get("item"), dict):
            item = event["item"]
            if item.get("type") == "function_call":
                tool_name = item.get("name") or tool_name
                tool_arguments = item.get("arguments") or tool_arguments
        elif kind == "response.completed" and isinstance(event.get("response"), dict):
            completed = True
            usage_valid = openai_usage(event["response"].get("usage"), "input_tokens", "output_tokens")
        elif kind == "response.failed":
            raise ProbeFailure("responses_stream_failed")
    if not started or not completed or not usage_valid:
        raise ProbeFailure("responses_stream_lifecycle")
    if (tool and not valid_tool(tool_name, tool_arguments)) or (not tool and not saw_text):
        raise ProbeFailure("responses_stream_semantics")
    return {"projection": "tool" if tool else "text", "usage_valid": True, "terminal": "completed"}


def observe_chat_json(value, tool: bool) -> dict:
    choices = value.get("choices") if isinstance(value, dict) else None
    if not isinstance(choices, list) or len(choices) != 1 or not isinstance(choices[0], dict):
        raise ProbeFailure("chat_lifecycle")
    message = choices[0].get("message")
    if not isinstance(message, dict):
        raise ProbeFailure("chat_message")
    if tool:
        calls = message.get("tool_calls")
        valid = isinstance(calls, list) and len(calls) == 1 and isinstance(calls[0], dict)
        function = calls[0].get("function") if valid else None
        valid = valid and isinstance(function, dict) and valid_tool(function.get("name"), function.get("arguments"))
    else:
        valid = isinstance(message.get("content"), str) and bool(message["content"])
    if not valid or not openai_usage(value.get("usage"), "prompt_tokens", "completion_tokens"):
        raise ProbeFailure("chat_semantics_or_usage")
    return {"projection": "tool" if tool else "text", "usage_valid": True, "terminal": choices[0].get("finish_reason")}


def observe_chat_stream(response, tool: bool) -> dict:
    saw_text = False
    tool_name = None
    tool_arguments = ""
    finish = None
    usage_valid = False
    done = False
    for event in sse_values(response):
        if event == "DONE":
            done = True
            continue
        if "error" in event and not isinstance(event.get("choices"), list):
            raise ProbeFailure("chat_stream_error_frame")
        choices = event.get("choices")
        if isinstance(choices, list) and choices:
            choice = choices[0]
            delta = choice.get("delta") if isinstance(choice, dict) else None
            if isinstance(delta, dict):
                if isinstance(delta.get("content"), str):
                    saw_text = saw_text or bool(delta["content"])
                calls = delta.get("tool_calls")
                if isinstance(calls, list):
                    for call in calls:
                        function = call.get("function") if isinstance(call, dict) else None
                        if isinstance(function, dict):
                            tool_name = function.get("name") or tool_name
                            if isinstance(function.get("arguments"), str):
                                tool_arguments += function["arguments"]
            finish = choice.get("finish_reason") or finish
        if choices == []:
            usage_valid = openai_usage(event.get("usage"), "prompt_tokens", "completion_tokens")
    if not done:
        raise ProbeFailure("chat_stream_done_missing")
    if not finish:
        raise ProbeFailure("chat_stream_finish_missing")
    if not usage_valid:
        raise ProbeFailure("chat_stream_usage_invalid")
    if (tool and not valid_tool(tool_name, tool_arguments)) or (not tool and not saw_text):
        raise ProbeFailure("chat_stream_semantics")
    return {"projection": "tool" if tool else "text", "usage_valid": True, "terminal": finish}


def anthropic_usage(incoming, outgoing) -> bool:
    return all(isinstance(item, int) and not isinstance(item, bool) and item >= 0 for item in (incoming, outgoing))


def observe_messages_json(value, tool: bool) -> dict:
    if not isinstance(value, dict):
        raise ProbeFailure("messages_lifecycle")
    content = value.get("content")
    if value.get("type") != "message" or not isinstance(content, list):
        raise ProbeFailure("messages_lifecycle")
    if tool:
        blocks = [item for item in content if isinstance(item, dict) and item.get("type") == "tool_use"]
        valid = len(blocks) == 1 and valid_tool(blocks[0].get("name"), blocks[0].get("input"))
    else:
        valid = any(isinstance(item, dict) and item.get("type") == "text" and isinstance(item.get("text"), str) and bool(item["text"]) for item in content)
    usage = value.get("usage")
    usage_valid = isinstance(usage, dict) and anthropic_usage(usage.get("input_tokens"), usage.get("output_tokens"))
    if not valid or not usage_valid or not value.get("stop_reason"):
        raise ProbeFailure("messages_semantics_or_usage")
    return {"projection": "tool" if tool else "text", "usage_valid": True, "terminal": value["stop_reason"]}


def observe_messages_stream(response, tool: bool) -> dict:
    started = False
    stopped = False
    saw_text = False
    tool_name = None
    tool_arguments = ""
    input_tokens = None
    output_tokens = None
    terminal = None
    for event in sse_values(response):
        if event == "DONE":
            continue
        kind = event.get("type")
        if kind == "message_start":
            started = True
            message = event.get("message") or {}
            usage = message.get("usage") or {}
            input_tokens = usage.get("input_tokens")
        elif kind == "content_block_start":
            block = event.get("content_block") or {}
            if block.get("type") == "tool_use":
                tool_name = block.get("name")
        elif kind == "content_block_delta":
            delta = event.get("delta") or {}
            if delta.get("type") == "text_delta" and isinstance(delta.get("text"), str):
                saw_text = saw_text or bool(delta["text"])
            if delta.get("type") == "input_json_delta" and isinstance(delta.get("partial_json"), str):
                tool_arguments += delta["partial_json"]
        elif kind == "message_delta":
            delta = event.get("delta") or {}
            terminal = delta.get("stop_reason") or terminal
            usage = event.get("usage") or {}
            output_tokens = usage.get("output_tokens", output_tokens)
        elif kind == "message_stop":
            stopped = True
        elif kind == "error":
            raise ProbeFailure("messages_stream_failed")
    if not started or not stopped or not terminal or not anthropic_usage(input_tokens, output_tokens):
        raise ProbeFailure("messages_stream_lifecycle")
    if (tool and not valid_tool(tool_name, tool_arguments)) or (not tool and not saw_text):
        raise ProbeFailure("messages_stream_semantics")
    return {"projection": "tool" if tool else "text", "usage_valid": True, "terminal": terminal}


def request_once(host: str, port: int, key: str, protocol: str, *, stream: bool, tool: bool, timeout: float) -> dict:
    path = {"chat": "/v1/chat/completions", "responses": "/v1/responses", "messages": "/v1/messages"}[protocol]
    conn = http.client.HTTPConnection(host, port, timeout=timeout)
    try:
        conn.request("POST", path, request_body(protocol, stream=stream, tool=tool), {
            "Authorization": "Bearer " + key,
            "Content-Type": "application/json",
            "Accept": "text/event-stream" if stream else "application/json",
        })
        response = conn.getresponse()
        if response.status // 100 != 2:
            response.read(MAX_EVENT)
            raise ProbeFailure("http_" + str(response.status // 100) + "xx")
        content_type = (response.getheader("content-type") or "").split(";", 1)[0].strip().lower()
        if content_type != ("text/event-stream" if stream else "application/json"):
            raise ProbeFailure("content_type")
        if protocol == "responses":
            return observe_responses_stream(response, tool) if stream else observe_responses_json(read_json(response), tool)
        if protocol == "chat":
            return observe_chat_stream(response, tool) if stream else observe_chat_json(read_json(response), tool)
        return observe_messages_stream(response, tool) if stream else observe_messages_json(read_json(response), tool)
    except (OSError, http.client.HTTPException) as error:
        raise ProbeFailure("transport") from error
    finally:
        conn.close()


def write_receipt(path: Path, receipt: dict) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        json.dump(receipt, handle, indent=2, sort_keys=True)
        handle.write("\n")


def tuple_matrix() -> list[tuple[str, bool, bool]]:
    return [
        (protocol, stream, tool)
        for protocol in ("chat", "responses", "messages")
        for stream, tool in ((False, False), (True, False), (False, True), (True, True))
    ]


def resume_index(path: Path, tuples: list[tuple[str, bool, bool]]) -> tuple[int, list[dict]]:
    try:
        raw = private_bytes(path, MAX_BODY)
        receipt = json.loads(raw)
    except (ProbeFailure, json.JSONDecodeError) as error:
        raise ProbeFailure("resume_receipt_invalid") from error
    results = receipt.get("tuples") if isinstance(receipt, dict) else None
    if (
        not isinstance(receipt, dict)
        or receipt.get("schema_version") != 1
        or receipt.get("value_free") is not True
        or not isinstance(results, list)
        or not results
        or len(results) > len(tuples)
    ):
        raise ProbeFailure("resume_receipt_invalid")
    for index, result in enumerate(results):
        protocol, stream, tool = tuples[index]
        expected_mode = "tool" if tool else "text"
        if not isinstance(result, dict) or (
            result.get("protocol"), result.get("stream"), result.get("mode")
        ) != (protocol, stream, expected_mode):
            raise ProbeFailure("resume_receipt_sequence")
        expected_result = "FAIL" if index == len(results) - 1 else "PASS"
        if result.get("result") != expected_result:
            raise ProbeFailure("resume_receipt_state")
    return len(results) - 1, results[:-1]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", required=True)
    parser.add_argument("--key-file", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument("--resume-receipt", type=Path)
    args = parser.parse_args()
    try:
        host, port = loopback_endpoint(args.url)
        key = private_value(args.key_file, 8192)
    except ProbeFailure as error:
        print(f"p12-08g1-live=FAIL category={error}", file=sys.stderr)
        return 1

    tuples = tuple_matrix()
    results: list[dict] = []
    start_index = 0
    if args.resume_receipt:
        try:
            start_index, results = resume_index(args.resume_receipt, tuples)
        except ProbeFailure as error:
            print(f"p12-08g1-live=FAIL category={error}", file=sys.stderr)
            return 1
    failed = False
    sends = 0
    for protocol, stream, tool in tuples[start_index:]:
        sends += 1
        try:
            observation = request_once(host, port, key, protocol, stream=stream, tool=tool, timeout=args.timeout)
            results.append({"protocol": protocol, "stream": stream, "mode": "tool" if tool else "text", "result": "PASS", **observation})
        except ProbeFailure as error:
            results.append({"protocol": protocol, "stream": stream, "mode": "tool" if tool else "text", "result": "FAIL", "category": str(error)})
            failed = True
            break
    receipt = {
        "schema_version": 1,
        "value_free": True,
        "fixed_tuple_count": len(tuples),
        "attempted_tuple_count": len(results),
        "successful_tuple_count": sum(item["result"] == "PASS" for item in results),
        "network_send_count": sends,
        "resumed_from_failed_tuple": args.resume_receipt is not None,
        "stopped_on_first_failure": failed,
        "tuples": results,
    }
    try:
        write_receipt(args.out, receipt)
    except OSError:
        print("p12-08g1-live=FAIL category=receipt_write", file=sys.stderr)
        return 1
    print("p12-08g1-live=" + ("PASS" if not failed and len(results) == len(tuples) else "FAIL"))
    return 0 if not failed and len(results) == len(tuples) else 1


if __name__ == "__main__":
    raise SystemExit(main())
