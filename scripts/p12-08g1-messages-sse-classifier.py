#!/usr/bin/env python3
"""Classify one loopback Messages SSE lifecycle without retaining response values."""

from __future__ import annotations

import argparse
import http.client
import importlib.util
import json
import os
from pathlib import Path
import sys


LIVE_PATH = Path(__file__).with_name("p12-08g1-live-e2e.py")
SPEC = importlib.util.spec_from_file_location("p12_08g1_live", LIVE_PATH)
LIVE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LIVE)
ALLOWED_EVENTS = {
    "message_start", "content_block_start", "content_block_delta", "content_block_stop",
    "message_delta", "message_stop", "ping", "error",
}
ALLOWED_DELTAS = {"text_delta", "thinking_delta", "input_json_delta"}
ALLOWED_STOPS = {
    "end_turn", "tool_use", "max_tokens", "stop_sequence", "refusal",
    "model_context_window_exceeded", "pause_turn",
}
MAX_EVENTS = 512


class ClassifierError(RuntimeError):
    pass


def nonnegative_integer(value) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def classify_events(events: list[dict]) -> dict:
    event_types = []
    delta_types = []
    message_start = False
    message_stop = False
    error_frame = False
    input_usage_valid = False
    output_usage_valid = False
    stop_reason_class = "absent"
    unknown_event = False
    unknown_delta = False
    for event in events:
        kind = event.get("type") if isinstance(event, dict) else None
        safe_kind = kind if kind in ALLOWED_EVENTS else "unknown"
        event_types.append(safe_kind)
        unknown_event = unknown_event or safe_kind == "unknown"
        if kind == "message_start":
            message_start = True
            message = event.get("message") if isinstance(event.get("message"), dict) else {}
            usage = message.get("usage") if isinstance(message.get("usage"), dict) else {}
            input_usage_valid = nonnegative_integer(usage.get("input_tokens"))
        elif kind == "content_block_delta":
            delta = event.get("delta") if isinstance(event.get("delta"), dict) else {}
            delta_kind = delta.get("type")
            safe_delta = delta_kind if delta_kind in ALLOWED_DELTAS else "unknown"
            delta_types.append(safe_delta)
            unknown_delta = unknown_delta or safe_delta == "unknown"
        elif kind == "message_delta":
            delta = event.get("delta") if isinstance(event.get("delta"), dict) else {}
            reason = delta.get("stop_reason")
            stop_reason_class = reason if reason in ALLOWED_STOPS else "unknown"
            usage = event.get("usage") if isinstance(event.get("usage"), dict) else {}
            output_usage_valid = nonnegative_integer(usage.get("output_tokens"))
        elif kind == "message_stop":
            message_stop = True
        elif kind == "error":
            error_frame = True
    return {
        "event_count": len(events),
        "event_types": event_types,
        "delta_types": delta_types,
        "message_start": message_start,
        "message_stop": message_stop,
        "error_frame": error_frame,
        "input_usage_valid": input_usage_valid,
        "output_usage_valid": output_usage_valid,
        "stop_reason_class": stop_reason_class,
        "unknown_event": unknown_event,
        "unknown_delta": unknown_delta,
        "strict_lifecycle": (
            message_start and message_stop and not error_frame and input_usage_valid
            and output_usage_valid and stop_reason_class in ALLOWED_STOPS
            and not unknown_event and not unknown_delta
        ),
    }


def write_receipt(path: Path, value: dict) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", required=True)
    parser.add_argument("--key-file", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--timeout", type=float, default=120.0)
    args = parser.parse_args()
    try:
        host, port = LIVE.loopback_endpoint(args.url)
        key = LIVE.private_value(args.key_file, 8192)
        connection = http.client.HTTPConnection(host, port, timeout=args.timeout)
        connection.request(
            "POST", "/v1/messages",
            LIVE.request_body("messages", stream=True, tool=False),
            {
                "Authorization": "Bearer " + key,
                "Content-Type": "application/json",
                "Accept": "text/event-stream",
            },
        )
        response = connection.getresponse()
        status_class = "2xx" if response.status // 100 == 2 else "non_2xx"
        content_type = (response.getheader("content-type") or "").split(";", 1)[0].strip().lower()
        events = []
        if status_class == "2xx" and content_type == "text/event-stream":
            for event in LIVE.sse_values(response):
                if event == "DONE":
                    continue
                if len(events) >= MAX_EVENTS:
                    raise ClassifierError("event_bound")
                events.append(event)
        receipt = {
            "schema_version": 1,
            "value_free": True,
            "request_count": 1,
            "retry_count": 0,
            "status_class": status_class,
            "content_type_class": "sse" if content_type == "text/event-stream" else "other",
            **classify_events(events),
        }
        write_receipt(args.out, receipt)
        print("p12-08g1-messages-sse-classifier=" + ("PASS" if receipt["strict_lifecycle"] else "FAIL"))
        return 0 if receipt["strict_lifecycle"] else 1
    except (ClassifierError, OSError, http.client.HTTPException, LIVE.ProbeFailure):
        print("p12-08g1-messages-sse-classifier=FAIL category=classifier_error", file=sys.stderr)
        return 1
    finally:
        if "connection" in locals():
            connection.close()


if __name__ == "__main__":
    raise SystemExit(main())
