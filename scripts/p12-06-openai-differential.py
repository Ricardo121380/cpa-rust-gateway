#!/usr/bin/env python3
"""Run a value-free live differential over two loopback OpenAI Responses endpoints."""

from __future__ import annotations

import argparse
import http.client
import json
import math
import os
import statistics
import time
from urllib.parse import urlsplit

MAX_BODY = 2 * 1024 * 1024
MAX_EVENT = 128 * 1024


class ProbeFailure(Exception):
    pass


def args():
    parser = argparse.ArgumentParser()
    for arm in ("incumbent", "candidate"):
        parser.add_argument(f"--{arm}-url", required=True)
        parser.add_argument(f"--{arm}-key-file", required=True)
        parser.add_argument(f"--{arm}-model-file", required=True)
    parser.add_argument("--samples", type=int, default=10)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--out", required=True)
    parser.add_argument("--timeout", type=float, default=120.0)
    value = parser.parse_args()
    if not 1 <= value.samples <= 20 or not 0 <= value.warmup <= 3:
        parser.error("samples must be 1..20 and warmup 0..3")
    return value


def private_file(path: str, limit: int) -> str:
    stat = os.lstat(path)
    if not os.path.isfile(path) or os.path.islink(path) or stat.st_mode & 0o077:
        raise ProbeFailure("private input admission failed")
    with open(path, encoding="utf-8") as handle:
        value = handle.read(limit + 1).strip()
    if not value or len(value) > limit or "\n" in value or "\r" in value:
        raise ProbeFailure("private input shape failed")
    return value


def endpoint(raw: str) -> tuple[str, int, str]:
    parsed = urlsplit(raw)
    if parsed.scheme != "http" or parsed.hostname != "127.0.0.1" or parsed.query or parsed.fragment:
        raise ProbeFailure("only exact loopback HTTP endpoints are admitted")
    if parsed.path != "/v1/responses":
        raise ProbeFailure("endpoint path must be /v1/responses")
    return parsed.hostname, parsed.port or 80, parsed.path


def body(model: str, *, stream: bool, tool: bool = False) -> bytes:
    request = {
        "model": model,
        "input": [{"type": "message", "role": "user", "content": "Return only the short requested result."}],
        "max_output_tokens": 96,
        "stream": stream,
    }
    if tool:
        request["input"][0]["content"] = "Call emit_probe exactly once with value set to ready; do not answer in text."
        request["tools"] = [{
            "type": "function", "name": "emit_probe", "description": "Emit the probe result.",
            "parameters": {"type": "object", "properties": {"value": {"type": "string"}},
                           "required": ["value"], "additionalProperties": False},
        }]
        request["tool_choice"] = "auto"
    return json.dumps(request, separators=(",", ":")).encode()


def usage_shape(usage) -> dict:
    if not isinstance(usage, dict):
        return {"present": False}
    input_tokens = usage.get("input_tokens")
    output_tokens = usage.get("output_tokens")
    total_tokens = usage.get("total_tokens")
    valid = all(isinstance(x, int) and not isinstance(x, bool) and x >= 0 for x in (input_tokens, output_tokens))
    input_details = usage.get("input_tokens_details") or {}
    output_details = usage.get("output_tokens_details") or {}
    return {
        "present": True, "nonnegative": valid,
        "total_present": isinstance(total_tokens, int) and not isinstance(total_tokens, bool),
        "conserved": valid and total_tokens == input_tokens + output_tokens,
        "cache_read_present": isinstance(input_details, dict) and isinstance(input_details.get("cached_tokens"), int),
        "cache_creation_present": isinstance(input_details, dict) and isinstance(input_details.get("cache_creation_tokens"), int),
        "reasoning_present": isinstance(output_details, dict) and isinstance(output_details.get("reasoning_tokens"), int),
    }


def collapse(markers: list[str]) -> list[str]:
    result = []
    for marker in markers:
        if result and result[-1] == marker and marker in ("text_delta", "tool_delta"):
            continue
        result.append(marker)
    return result


def observe_item(item, markers: list[str], tool_expected: bool) -> bool:
    if not isinstance(item, dict):
        raise ProbeFailure("output item is not an object")
    kind = item.get("type")
    if kind == "message":
        if tool_expected:
            raise ProbeFailure("tool response contained a message")
        content = item.get("content")
        if not isinstance(content, list) or not content:
            raise ProbeFailure("message content is missing")
        text_parts = [part for part in content if isinstance(part, dict) and part.get("type") == "output_text"]
        if not text_parts or any(not isinstance(part.get("text"), str) for part in text_parts):
            raise ProbeFailure("message output_text structure failed")
        markers.append("message_start")
        markers.extend("text_delta" for _ in text_parts)
        markers.append("message_end")
        return False
    if kind == "function_call":
        if not tool_expected:
            raise ProbeFailure("text response contained a tool call")
        markers.extend(["tool_start", "tool_delta", "tool_end"])
        try:
            arguments = json.loads(item.get("arguments", ""))
        except (TypeError, json.JSONDecodeError) as error:
            raise ProbeFailure("tool arguments are not complete JSON") from error
        valid = (
            item.get("name") == "emit_probe"
            and isinstance(item.get("call_id"), str)
            and arguments == {"value": "ready"}
        )
        if tool_expected and not valid:
            raise ProbeFailure("tool structure failed")
        return True
    if kind == "reasoning":
        return False
    raise ProbeFailure("unknown output item type")


def request_once(url: str, key: str, model: str, *, stream: bool, tool: bool, timeout: float) -> dict:
    host, port, path = endpoint(url)
    conn = http.client.HTTPConnection(host, port, timeout=timeout)
    started = time.monotonic()
    try:
        conn.request("POST", path, body(model, stream=stream, tool=tool), {
            "Authorization": "Bearer " + key, "Content-Type": "application/json",
            "Accept": "text/event-stream" if stream else "application/json",
        })
        response = conn.getresponse()
        ttfb = time.monotonic() - started
        if response.status // 100 != 2:
            response.read(MAX_EVENT)
            raise ProbeFailure("http_" + str(response.status // 100) + "xx")
        content_type = (response.getheader("content-type") or "").split(";", 1)[0].strip().lower()
        expected_type = "text/event-stream" if stream else "application/json"
        if content_type != expected_type:
            raise ProbeFailure("unexpected content type")
        if stream:
            result = observe_stream(response, tool, started, timeout)
        else:
            raw = response.read(MAX_BODY + 1)
            if len(raw) > MAX_BODY:
                raise ProbeFailure("body exceeds bound")
            try:
                value = json.loads(raw)
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise ProbeFailure("invalid JSON body") from error
            result = observe_json(value, tool)
        result["ttfb_ms"] = round(ttfb * 1000, 3)
        result["total_ms"] = round((time.monotonic() - started) * 1000, 3)
        return result
    except (OSError, http.client.HTTPException) as error:
        raise ProbeFailure("transport failure") from error
    finally:
        conn.close()


def observe_json(value, tool_expected: bool) -> dict:
    if not isinstance(value, dict) or value.get("status") != "completed" or not isinstance(value.get("output"), list):
        raise ProbeFailure("nonstream lifecycle failed")
    markers = ["response_start"]
    saw_tool = False
    for item in value["output"]:
        item_is_tool = observe_item(item, markers, tool_expected)
        if item_is_tool and saw_tool:
            raise ProbeFailure("duplicate tool call")
        saw_tool = saw_tool or item_is_tool
    markers.append("response_end")
    if tool_expected and not saw_tool:
        raise ProbeFailure("tool call missing")
    return {"projection": collapse(markers), "usage": usage_shape(value.get("usage")), "terminal": "tool_use" if saw_tool else "end_turn"}


def observe_stream(response, tool_expected: bool, started: float, timeout: float | None = None) -> dict:
    markers = []
    usage = {"present": False}
    first_semantic = None
    first_text = None
    delta_times = []
    saw_tool = False
    total_bytes = 0
    event_count = 0
    output_tokens = None
    while True:
        if timeout is not None and time.monotonic() - started > timeout:
            raise ProbeFailure("stream total timeout")
        line = response.readline(MAX_EVENT + 1)
        total_bytes += len(line)
        if total_bytes > MAX_BODY:
            raise ProbeFailure("stream exceeds total bound")
        if len(line) > MAX_EVENT:
            raise ProbeFailure("SSE event exceeds bound")
        if not line:
            break
        if not line.startswith(b"data:"):
            continue
        event_count += 1
        if event_count > 4096:
            raise ProbeFailure("stream exceeds event bound")
        payload = line[5:].strip()
        if payload == b"[DONE]":
            continue
        try:
            event = json.loads(payload)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ProbeFailure("invalid SSE JSON") from error
        if not isinstance(event, dict):
            raise ProbeFailure("invalid SSE event")
        kind = event.get("type")
        if kind in ("response.created", "response.in_progress"):
            if "response_start" not in markers: markers.append("response_start")
        elif kind == "response.output_item.added":
            if first_semantic is None: first_semantic = time.monotonic() - started
            item = event.get("item") or {}
            if item.get("type") == "message":
                if tool_expected: raise ProbeFailure("tool response contained a message")
                markers.append("message_start")
            elif item.get("type") == "function_call":
                if not tool_expected: raise ProbeFailure("text response contained a tool call")
                markers.append("tool_start")
        elif kind == "response.output_text.delta":
            if not isinstance(event.get("delta"), str) or not event["delta"]:
                raise ProbeFailure("text delta structure failed")
            now = time.monotonic() - started
            if first_semantic is None: first_semantic = now
            if first_text is None: first_text = now
            markers.append("text_delta"); delta_times.append(now)
        elif kind == "response.function_call_arguments.delta":
            if not isinstance(event.get("delta"), str):
                raise ProbeFailure("tool delta structure failed")
            if first_semantic is None: first_semantic = time.monotonic() - started
            markers.append("tool_delta")
        elif kind == "response.output_item.done":
            item = event.get("item") or {}
            if item.get("type") == "message": markers.append("message_end")
            elif item.get("type") == "function_call":
                if saw_tool: raise ProbeFailure("duplicate tool call")
                scratch=[]; saw_tool = observe_item(item, scratch, tool_expected); markers.append("tool_end")
        elif kind == "response.completed":
            response_value = event.get("response") or {}
            raw_usage = response_value.get("usage")
            usage = usage_shape(raw_usage)
            if isinstance(raw_usage, dict) and isinstance(raw_usage.get("output_tokens"), int):
                output_tokens = raw_usage["output_tokens"]
            markers.append("response_end")
        elif kind == "response.failed":
            raise ProbeFailure("stream failed")
    markers = collapse(markers)
    if not markers or markers[0] != "response_start" or markers[-1] != "response_end" or first_semantic is None:
        raise ProbeFailure("stream did not complete")
    if tool_expected and not saw_tool:
        raise ProbeFailure("tool call missing")
    gaps = [round((b - a) * 1000, 3) for a, b in zip(delta_times, delta_times[1:])]
    return {"projection": markers, "usage": usage, "terminal": "tool_use" if saw_tool else "end_turn",
            "first_semantic_ms": round(first_semantic * 1000, 3),
            "first_text_ms": None if first_text is None else round(first_text * 1000, 3),
            "_output_tokens": output_tokens, "inter_delta_ms": gaps}


def summary(values: list[float]) -> dict | None:
    if not values: return None
    ordered = sorted(values)
    pick = lambda q: round(ordered[min(len(ordered)-1, math.ceil(q*len(ordered))-1)], 3)
    return {"n": len(values), "p50": pick(.5), "p95": pick(.95), "p99": pick(.99), "mean": round(statistics.fmean(values), 3)}


def public_observation(value: dict) -> dict:
    """Remove measurement-only fields before an observation enters durable evidence."""
    return {key: item for key, item in value.items() if not key.startswith("_")}


def usage_valid(value: dict | None) -> bool:
    return bool(
        value
        and value.get("present")
        and value.get("nonnegative")
        and value.get("total_present")
        and value.get("conserved")
    )


def evaluate_differential(left: dict, right: dict) -> dict:
    """Compare only cross-arm invariants approved by CR-P12-06-008."""
    comparable = all(
        arm["successful_samples"] == arm["requested_samples"]
        and arm["projection_consistent"]
        for arm in (left, right)
    )

    def equal_mode(mode: str, field: str) -> bool:
        return (
            "failure" not in left[mode]
            and "failure" not in right[mode]
            and left[mode].get(field) == right[mode].get(field)
        )

    def usage_mode_valid(mode: str) -> bool:
        return all(
            "failure" not in arm[mode] and usage_valid(arm[mode].get("usage"))
            for arm in (left, right)
        )

    return {
        "stream_projection_equal": comparable and left["stream_projection"] == right["stream_projection"],
        "stream_terminal_equal": comparable and left["stream_terminal"] == right["stream_terminal"],
        "stream_usage_invariants_valid": comparable
        and left["stream_usage_invariants_valid"]
        and right["stream_usage_invariants_valid"],
        "nonstream_projection_equal": equal_mode("nonstream", "projection"),
        "nonstream_terminal_equal": equal_mode("nonstream", "terminal"),
        "nonstream_usage_invariants_valid": usage_mode_valid("nonstream"),
        "tool_projection_equal": equal_mode("tool_stream", "projection"),
        "tool_terminal_equal": equal_mode("tool_stream", "terminal"),
        "tool_usage_invariants_valid": usage_mode_valid("tool_stream"),
    }


def main() -> int:
    a = args()
    arms = {}
    for label in ("incumbent", "candidate"):
        arms[label] = {
            "url": getattr(a, label + "_url"),
            "key": private_file(getattr(a, label + "_key_file"), 8192),
            "model": private_file(getattr(a, label + "_model_file"), 256),
            "samples": [], "failures": [],
        }
    for index in range(a.warmup + a.samples):
        order = ("incumbent", "candidate") if index % 2 == 0 else ("candidate", "incumbent")
        for label in order:
            arm = arms[label]
            try: sample = request_once(arm["url"], arm["key"], arm["model"], stream=True, tool=False, timeout=a.timeout)
            except ProbeFailure as error: arm["failures"].append(str(error)); continue
            if index >= a.warmup: arm["samples"].append(sample)
    for label, arm in arms.items():
        for mode, stream, tool in (("nonstream", False, False), ("tool_stream", True, True)):
            try: arm[mode] = request_once(arm["url"], arm["key"], arm["model"], stream=stream, tool=tool, timeout=a.timeout)
            except ProbeFailure as error: arm[mode] = {"failure": str(error)}
    report_arms = {}
    for label, arm in arms.items():
        samples = arm["samples"]
        report_arms[label] = {
            "requested_samples": a.samples, "successful_samples": len(samples), "failures": arm["failures"],
            "projection_consistent": bool(samples) and len({json.dumps(x["projection"]) for x in samples}) == 1,
            "stream_projection": samples[0]["projection"] if samples else None,
            "stream_usage_shape": samples[0]["usage"] if samples else None,
            "stream_terminal": samples[0]["terminal"] if samples else None,
            "stream_usage_invariants_valid": bool(samples) and all(usage_valid(x.get("usage")) for x in samples),
            "metrics_ms": {k: summary([x[k] for x in samples if x.get(k) is not None]) for k in ("ttfb_ms","first_semantic_ms","first_text_ms","total_ms")},
            "inter_delta_ms": summary([v for x in samples for v in x.get("inter_delta_ms",[])]),
            "output_tokens_per_second": summary([
                round(x["_output_tokens"] * 1000 / (x["total_ms"] - x["first_text_ms"]), 3)
                for x in samples
                if isinstance(x.get("_output_tokens"), int) and x.get("first_text_ms") is not None
                and x["total_ms"] > x["first_text_ms"]
            ]),
            "nonstream": public_observation(arm["nonstream"]),
            "tool_stream": public_observation(arm["tool_stream"]),
        }
    left, right = report_arms["incumbent"], report_arms["candidate"]
    report = {"schema_version": 2, "value_free": True, "arms": report_arms,
              "differential": evaluate_differential(left, right)}
    descriptor = os.open(a.out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2, sort_keys=True); handle.write("\n")
    modes_valid = all(
        arm["stream_usage_invariants_valid"]
        and "failure" not in arm["nonstream"]
        and usage_valid(arm["nonstream"].get("usage"))
        and "failure" not in arm["tool_stream"]
        and usage_valid(arm["tool_stream"].get("usage"))
        for arm in (left, right)
    )
    return 0 if modes_valid and all(report["differential"].values()) else 1


if __name__ == "__main__":
    raise SystemExit(main())
