#!/usr/bin/env python3
"""Authorized live public-protocol checks using an existing Pi CPAR client key.

Receipts exclude credentials and prompt/response bodies. No configuration, account,
client-key, service or release mutation is performed. The operator records the
observed runtime commit and active configuration; this is not a deployment check.
"""
import argparse
import copy
import json
import os
from pathlib import Path
import time
import urllib.error
import urllib.request


def payload(protocol, model, streaming, tool):
    result = {"model": model, "stream": streaming}
    prompt = "Call lookup with value 1." if tool else "Reply with OK."
    schema = {"type": "object", "properties": {"value": {"type": "integer"}}, "required": ["value"], "additionalProperties": False}
    if protocol == "responses":
        result.update(input=prompt, max_output_tokens=64)
        if tool:
            result["tools"] = [{"type": "function", "name": "lookup", "description": "Return the supplied value.", "parameters": schema}]
    elif protocol == "chat":
        result.update(messages=[{"role": "user", "content": prompt}], max_tokens=64)
        if tool:
            result["tools"] = [{"type": "function", "function": {"name": "lookup", "description": "Return the supplied value.", "parameters": schema}}]
    else:
        result.update(messages=[{"role": "user", "content": prompt}], max_tokens=64)
        if tool:
            result["tools"] = [{"name": "lookup", "description": "Return the supplied value.", "input_schema": schema}]
    return result


def decode_stream(protocol, raw):
    events = []
    for line in raw.decode("utf-8").splitlines():
        if line.startswith("data:"):
            value = line[5:].strip()
            if value and value != "[DONE]":
                events.append(json.loads(value))
    if any("error" in event or event.get("type") in ["response.failed", "error"] for event in events):
        return None
    if protocol == "responses":
        for event in reversed(events):
            if event.get("type") == "response.completed":
                return event.get("response")
        return None
    if protocol == "chat":
        text, thought, calls, finish, usage = "", "", {}, None, None
        for event in events:
            usage = event.get("usage") or usage
            for choice in event.get("choices", []):
                delta = choice.get("delta", {})
                text += delta.get("content") or ""
                thought += delta.get("reasoning_content") or ""
                finish = choice.get("finish_reason") or finish
                for call in delta.get("tool_calls", []):
                    target = calls.setdefault(call["index"], {"id": "", "type": "function", "function": {"name": "", "arguments": ""}})
                    target["id"] = call.get("id") or target["id"]
                    target["function"]["name"] += call.get("function", {}).get("name") or ""
                    target["function"]["arguments"] += call.get("function", {}).get("arguments") or ""
        if finish is None:
            return None
        message = {"role": "assistant", "content": text or None}
        if thought:
            message["reasoning_content"] = thought
        if calls:
            message["tool_calls"] = [calls[index] for index in sorted(calls)]
        return {"choices": [{"message": message, "finish_reason": finish}], "usage": usage}
    blocks, arguments, finish, usage, stopped = {}, {}, None, {}, False
    for event in events:
        kind = event.get("type")
        if kind == "message_start":
            usage.update(event["message"].get("usage") or {})
        elif kind == "content_block_start":
            blocks[event["index"]] = copy.deepcopy(event["content_block"])
        elif kind == "content_block_delta":
            index, delta = event["index"], event["delta"]
            for field in ["text", "thinking", "signature"]:
                if field in delta:
                    blocks[index][field] = blocks[index].get(field, "") + delta[field]
            if "partial_json" in delta:
                arguments[index] = arguments.get(index, "") + delta["partial_json"]
        elif kind == "message_delta":
            finish = event.get("delta", {}).get("stop_reason") or finish
            usage.update(event.get("usage") or {})
        elif kind == "message_stop":
            stopped = True
    for index, argument in arguments.items():
        blocks[index]["input"] = json.loads(argument)
    return {"content": [blocks[index] for index in sorted(blocks)], "stop_reason": finish, "usage": usage} if stopped and finish else None


def observe(protocol, value):
    if protocol == "responses":
        if value.get("status") != "completed":
            raise ValueError("incomplete_response")
        calls = [(item["call_id"], item["name"], json.loads(item["arguments"])) for item in value.get("output", []) if item.get("type") == "function_call"]
        text = "".join(part.get("text", "") for item in value.get("output", []) if item.get("type") == "message" for part in item.get("content", []) if part.get("type") == "output_text")
    elif protocol == "chat":
        choice = value["choices"][0]
        if choice.get("finish_reason") not in ["stop", "tool_calls"]:
            raise ValueError("incomplete_response")
        message = choice["message"]
        calls = [(item["id"], item["function"]["name"], json.loads(item["function"]["arguments"])) for item in message.get("tool_calls", [])]
        text = message.get("content") or ""
    else:
        if value.get("stop_reason") not in ["end_turn", "tool_use"]:
            raise ValueError("incomplete_response")
        calls = [(item["id"], item["name"], item["input"]) for item in value.get("content", []) if item.get("type") == "tool_use"]
        text = "".join(item.get("text", "") for item in value.get("content", []) if item.get("type") == "text")
    if len({identity for identity, _, _ in calls}) != len(calls) or any(not identity or name != "lookup" or arguments != {"value": 1} for identity, name, arguments in calls):
        raise ValueError("invalid_tool_call")
    return text, calls


def followup(protocol, first, answer, calls):
    result = copy.deepcopy(first)
    if protocol == "responses":
        history = [{"role": "user", "content": first["input"]}] if isinstance(first["input"], str) else copy.deepcopy(first["input"])
        result["input"] = [*history, *answer["output"], *[{"type": "function_call_output", "call_id": identity, "output": "1"} for identity, _, _ in calls]]
    elif protocol == "chat":
        result["messages"] += [answer["choices"][0]["message"], *[{"role": "tool", "tool_call_id": identity, "content": "1"} for identity, _, _ in calls]]
    else:
        result["messages"] += [{"role": "assistant", "content": answer["content"]}, {"role": "user", "content": [{"type": "tool_result", "tool_use_id": identity, "content": "1"} for identity, _, _ in calls]}]
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pi-models", type=Path, required=True)
    parser.add_argument("--provider", required=True)
    parser.add_argument("--runtime-commit", required=True)
    parser.add_argument("--config-version", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--model", action="append", help="Restrict verification to named, currently visible models.")
    args = parser.parse_args()
    provider = json.loads(args.pi_models.read_text())["providers"][args.provider]
    key = provider["apiKey"]
    if key.startswith("$"):
        key = os.environ.get(key[1:], "")
    if not key:
        raise SystemExit("existing authentication unavailable")
    receipt = {"schema_version": 1, "layer": "LIVE_EXISTING_RUNTIME", "runtime_commit": args.runtime_commit, "config_version": args.config_version, "value_free": True, "constraint_enforcement": "UNVERIFIED_EXISTING_RUNTIME", "rows": []}
    paths = {"responses": "/responses", "chat": "/chat/completions", "messages": "/messages"}
    models_request = urllib.request.Request(provider["baseUrl"] + "/models", headers={"Authorization": "Bearer " + key})
    with urllib.request.urlopen(models_request, timeout=20) as response:
        models = [item["id"] for item in json.load(response)["data"]]
    if args.model:
        if not set(args.model).issubset(models):
            raise SystemExit("requested model unavailable in the authenticated current view")
        models = args.model
    args.out.parent.mkdir(parents=True, exist_ok=True)

    def call(protocol, body, round_number, tool):
        started = time.monotonic()
        row = {"model": body["model"], "protocol": protocol, "streaming": body["stream"], "tool_requested": tool, "round": round_number, "observed_at_ms": time.time_ns() // 1_000_000, "requested_output_limit": 64}
        result, calls = None, []
        request = urllib.request.Request(provider["baseUrl"] + paths[protocol], data=json.dumps(body, separators=(",", ":")).encode(), headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"}, method="POST")
        try:
            with urllib.request.urlopen(request, timeout=90) as response:
                row["http_status"] = response.status
                row["request_id"] = response.headers.get("x-request-id")
                raw = response.read(2 * 1024 * 1024 + 1)
            if len(raw) > 2 * 1024 * 1024:
                raise ValueError("response_size")
            result = decode_stream(protocol, raw) if body["stream"] else json.loads(raw)
            if result is None:
                raise ValueError("missing_success_terminal")
            text, calls = observe(protocol, result)
            row.update(result="PASS" if (calls if tool and round_number == 1 else text) else "FAIL", category="tool_call" if calls else "text" if text else "empty_output", tool_call_count=len(calls), usage={k: v for k, v in (result.get("usage") or {}).items() if k in ["input_tokens", "output_tokens", "prompt_tokens", "completion_tokens", "total_tokens"] and isinstance(v, int)})
        except urllib.error.HTTPError as error:
            row.update(http_status=error.code, result="FAIL", category="http_error")
            row["request_id"] = error.headers.get("x-request-id")
            raw = error.read(8192)
            try:
                code = json.loads(raw).get("error", {}).get("code")
                if isinstance(code, str) and code.isalnum() and len(code) < 80:
                    row["gateway_error_code"] = code
            except (ValueError, AttributeError):
                pass
        except (ValueError, KeyError, TypeError, urllib.error.URLError, TimeoutError):
            row.update(result="FAIL", category="response_or_transport")
        row["duration_ms"] = round((time.monotonic() - started) * 1000)
        receipt["rows"].append(row)
        args.out.write_text(json.dumps(receipt, indent=2) + "\n")
        print(json.dumps({k: row[k] for k in ["model", "protocol", "streaming", "round", "result", "category", "duration_ms"]}), flush=True)
        return result, calls, row["result"] == "PASS"

    for model in models:
        for protocol in paths:
            for streaming in [False, True]:
                call(protocol, payload(protocol, model, streaming, False), 1, False)
                first = payload(protocol, model, streaming, True)
                answer, calls, passed = call(protocol, first, 1, True)
                if passed and calls:
                    call(protocol, followup(protocol, first, answer, calls), 2, True)
    receipt["completed"] = True
    args.out.write_text(json.dumps(receipt, indent=2) + "\n")


if __name__ == "__main__":
    import sys
    if sys.argv[1:2] == ["--new-build"]:
        from cpar_new_build_acceptance import main as new_build_main
        raise SystemExit(new_build_main(sys.argv[2:]))
    main()
