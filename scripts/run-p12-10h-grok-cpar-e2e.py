#!/usr/bin/env python3
"""Run bounded, value-free Grok calls through the CPAR HTTP entrypoint.

The harness deliberately invokes curl for every request. It reads endpoint, client key and model
only from private files, refuses a non-Grok visible model before inference, stops at the first
failure, and writes only counts/categories to the receipt.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
from urllib.parse import urlsplit


MAX_INPUT_BYTES = 8 * 1024
MAX_RESPONSE_BYTES = 512 * 1024
MAX_ITERATIONS = 1000
GROK_MARKERS = ("grok", "xai")
PROTOCOLS = ("responses", "chat", "messages")


class ProbeFailure(RuntimeError):
    """A stable, value-free failure category."""


def private_value(path: Path, limit: int) -> str:
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
    if not value or len(value) > limit or b"\n" in value or b"\r" in value:
        raise ProbeFailure("private_input_shape")
    try:
        return value.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ProbeFailure("private_input_encoding") from error


def endpoint_admission(raw: str, allow_public_https: bool) -> tuple[str, str]:
    parsed = urlsplit(raw)
    if (
        parsed.scheme not in ("http", "https")
        or parsed.path not in ("", "/")
        or parsed.query
        or parsed.fragment
        or parsed.username is not None
        or parsed.password is not None
        or parsed.hostname is None
    ):
        raise ProbeFailure("endpoint_admission")
    loopback = parsed.hostname in ("127.0.0.1", "localhost", "::1")
    if parsed.scheme == "http" and not loopback:
        raise ProbeFailure("non_loopback_http")
    if parsed.scheme == "https" and not loopback and not allow_public_https:
        raise ProbeFailure("public_https_not_explicitly_allowed")
    return raw.rstrip("/"), parsed.scheme


def is_grok_label(value: str) -> bool:
    lowered = value.lower()
    return any(marker in lowered for marker in GROK_MARKERS)


def curl_escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\r", "\\r")


def fixed_body(protocol: str, model: str, stream: bool) -> bytes:
    prompt = "Reply with OK."
    if protocol == "responses":
        value = {
            "model": model,
            "input": [{"type": "message", "role": "user", "content": prompt}],
            "max_output_tokens": 8,
            "stream": stream,
        }
    elif protocol == "chat":
        value = {
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 8,
            "stream": stream,
        }
        if stream:
            value["stream_options"] = {"include_usage": True}
    elif protocol == "messages":
        value = {
            "model": model,
            "max_tokens": 8,
            "messages": [{"role": "user", "content": prompt}],
            "stream": stream,
        }
    else:
        raise ProbeFailure("protocol_unknown")
    encoded = json.dumps(value, separators=(",", ":")).encode("utf-8")
    if len(encoded) > MAX_INPUT_BYTES:
        raise ProbeFailure("request_bound")
    return encoded


def curl_once(
    endpoint: str,
    key: str,
    body_path: Path,
    response_path: Path,
    protocol: str,
    stream: bool,
    timeout: float,
) -> tuple[int, str]:
    path = {"responses": "/v1/responses", "chat": "/v1/chat/completions", "messages": "/v1/messages"}[protocol]
    accept = "text/event-stream" if stream else "application/json"
    command = [
        "curl",
        "--silent",
        "--show-error",
        "--noproxy",
        "*",
        "--max-time",
        str(timeout),
        "--output",
        str(response_path),
        "--write-out",
        "%{http_code}\\t%{content_type}",
        "--config",
        "-",
    ]
    config = "\n".join(
        (
            f'url = "{curl_escape(endpoint + path)}"',
            'request = "POST"',
            f'header = "Authorization: Bearer {curl_escape(key)}"',
            'header = "Content-Type: application/json"',
            f'header = "Accept: {accept}"',
            f'data-binary = "@{curl_escape(str(body_path))}"',
        )
    ) + "\n"
    try:
        completed = subprocess.run(
            command,
            check=False,
            capture_output=True,
            input=config,
            text=True,
            timeout=timeout + 5,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProbeFailure("curl_transport") from error
    if completed.returncode != 0:
        raise ProbeFailure("curl_transport")
    try:
        status_text, content_type = completed.stdout.strip().split("\t", 1)
        status = int(status_text)
    except (ValueError, TypeError) as error:
        raise ProbeFailure("curl_status") from error
    if not response_path.is_file() or response_path.stat().st_size > MAX_RESPONSE_BYTES:
        raise ProbeFailure("response_bound")
    return status, content_type.split(";", 1)[0].strip().lower()


def curl_models(endpoint: str, key: str, response_path: Path, timeout: float) -> tuple[int, str]:
    command = [
        "curl",
        "--silent",
        "--show-error",
        "--noproxy",
        "*",
        "--max-time",
        str(timeout),
        "--output",
        str(response_path),
        "--write-out",
        "%{http_code}\\t%{content_type}",
        "--config",
        "-",
    ]
    config = "\n".join(
        (
            f'url = "{curl_escape(endpoint + "/v1/models")}"',
            'request = "GET"',
            f'header = "Authorization: Bearer {curl_escape(key)}"',
            'header = "Accept: application/json"',
        )
    ) + "\n"
    try:
        completed = subprocess.run(
            command,
            check=False,
            capture_output=True,
            input=config,
            text=True,
            timeout=timeout + 5,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProbeFailure("curl_transport") from error
    if completed.returncode != 0:
        raise ProbeFailure("curl_transport")
    try:
        status_text, content_type = completed.stdout.strip().split("\t", 1)
        status = int(status_text)
    except (ValueError, TypeError) as error:
        raise ProbeFailure("curl_status") from error
    if not response_path.is_file() or response_path.stat().st_size > MAX_RESPONSE_BYTES:
        raise ProbeFailure("response_bound")
    return status, content_type.split(";", 1)[0].strip().lower()


def parse_json(path: Path) -> object:
    try:
        return json.loads(path.read_bytes())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ProbeFailure("json_invalid") from error


def validate_models(path: Path, selected_model: str) -> None:
    value = parse_json(path)
    rows = value.get("data") if isinstance(value, dict) else None
    if not isinstance(rows, list):
        raise ProbeFailure("models_shape")
    labels = [row.get("id") for row in rows if isinstance(row, dict) and isinstance(row.get("id"), str)]
    grok_labels = [label for label in labels if is_grok_label(label)]
    if not grok_labels:
        raise ProbeFailure("grok_route_missing")
    if selected_model not in labels:
        raise ProbeFailure("selected_model_not_visible")
    if not is_grok_label(selected_model):
        raise ProbeFailure("selected_model_not_grok")


def validate_json(path: Path, protocol: str) -> None:
    value = parse_json(path)
    if not isinstance(value, dict):
        raise ProbeFailure("json_shape")
    if protocol == "responses":
        if not isinstance(value.get("output"), list) or value.get("status") not in ("completed", "in_progress"):
            raise ProbeFailure("responses_semantics")
    elif protocol == "chat":
        choices = value.get("choices")
        if not isinstance(choices, list) or not choices or not isinstance(choices[0], dict):
            raise ProbeFailure("chat_semantics")
    else:
        if value.get("type") != "message" or not isinstance(value.get("content"), list):
            raise ProbeFailure("messages_semantics")


def validate_sse(path: Path, protocol: str) -> None:
    try:
        lines = path.read_bytes().splitlines()
    except OSError as error:
        raise ProbeFailure("stream_read") from error
    if not lines:
        raise ProbeFailure("stream_empty")
    events = []
    done = False
    for line in lines:
        if not line.startswith(b"data:"):
            continue
        payload = line[5:].strip()
        if payload == b"[DONE]":
            done = True
            continue
        try:
            event = json.loads(payload)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ProbeFailure("stream_json_invalid") from error
        if isinstance(event, dict):
            events.append(event)
    if not events:
        raise ProbeFailure("stream_no_events")
    kinds = {event.get("type") for event in events}
    if protocol == "responses":
        terminal = "response.completed" in kinds or "response.done" in kinds
    elif protocol == "messages":
        terminal = "message_stop" in kinds or done
    else:
        terminal = done or any(isinstance(event.get("choices"), list) for event in events)
    if not terminal:
        raise ProbeFailure("stream_not_terminal")


def write_receipt(path: Path, receipt: dict) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        json.dump(receipt, handle, indent=2, sort_keys=True)
        handle.write("\n")


def write_blocked_receipt(path: Path, category: str, iterations: int) -> None:
    write_receipt(
        path,
        {
            "schema_version": 1,
            "value_free": True,
            "state": "BLOCKED",
            "iterations_requested": iterations,
            "attempted_calls": 0,
            "successful_calls": 0,
            "stopped_on_first_failure": True,
            "protocol_counts": {protocol: 0 for protocol in PROTOCOLS},
            "failure_categories": [category],
            "upstream_request": "not_sent",
        },
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--endpoint-file", required=True, type=Path)
    parser.add_argument("--client-key-file", required=True, type=Path)
    parser.add_argument("--model-file", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--iterations", type=int, default=100)
    parser.add_argument("--timeout-seconds", type=float, default=90.0)
    parser.add_argument("--allow-public-https", action="store_true")
    args = parser.parse_args()
    if args.iterations < 1 or args.iterations > MAX_ITERATIONS or args.timeout_seconds <= 0:
        print("cpar_grok_e2e=FAIL category=invalid_bounds", file=sys.stderr)
        return 1

    try:
        endpoint_raw = private_value(args.endpoint_file, MAX_INPUT_BYTES)
        endpoint, _scheme = endpoint_admission(endpoint_raw, args.allow_public_https)
        key = private_value(args.client_key_file, MAX_INPUT_BYTES)
        model = private_value(args.model_file, MAX_INPUT_BYTES)
    except ProbeFailure as error:
        try:
            write_blocked_receipt(args.receipt, str(error), args.iterations)
        except OSError:
            print("cpar_grok_e2e=FAIL category=receipt_write", file=sys.stderr)
            return 1
        print(f"cpar_grok_e2e=FAIL category={error}", file=sys.stderr)
        return 1

    successful = 0
    attempted = 0
    categories: list[str] = []
    protocol_counts = {protocol: 0 for protocol in PROTOCOLS}
    with tempfile.TemporaryDirectory(prefix="cpar-grok-e2e-") as temporary:
        root = Path(temporary)
        models_path = root / "models.json"
        body_path = root / "request.json"
        response_path = root / "response.bin"
        body_path.write_bytes(b"{}")
        os.chmod(body_path, stat.S_IRUSR | stat.S_IWUSR)
        try:
            status, content_type = curl_models(endpoint, key, models_path, args.timeout_seconds)
            if status // 100 != 2:
                raise ProbeFailure("models_http_status")
            if content_type != "application/json":
                raise ProbeFailure("models_content_type")
            validate_models(models_path, model)
            print("cpar_grok_e2e=PASS target=models_preflight")
        except ProbeFailure as error:
            try:
                write_blocked_receipt(args.receipt, str(error), args.iterations)
            except OSError:
                print("cpar_grok_e2e=FAIL category=receipt_write", file=sys.stderr)
                return 1
            print(f"cpar_grok_e2e=FAIL category={error}", file=sys.stderr)
            return 1

        for index in range(args.iterations):
            protocol = PROTOCOLS[index % len(PROTOCOLS)]
            stream = (index // len(PROTOCOLS)) % 2 == 1
            body_path.write_bytes(fixed_body(protocol, model, stream))
            response_path.unlink(missing_ok=True)
            attempted += 1
            protocol_counts[protocol] += 1
            try:
                status, content_type = curl_once(
                    endpoint,
                    key,
                    body_path,
                    response_path,
                    protocol,
                    stream,
                    args.timeout_seconds,
                )
                if status // 100 != 2:
                    raise ProbeFailure("http_" + str(status // 100) + "xx")
                expected_type = "text/event-stream" if stream else "application/json"
                if content_type != expected_type:
                    raise ProbeFailure("content_type")
                if stream:
                    validate_sse(response_path, protocol)
                else:
                    validate_json(response_path, protocol)
                successful += 1
            except ProbeFailure as error:
                categories.append(str(error))
                break

    receipt = {
        "schema_version": 1,
        "value_free": True,
        "iterations_requested": args.iterations,
        "attempted_calls": attempted,
        "successful_calls": successful,
        "stopped_on_first_failure": bool(categories),
        "protocol_counts": protocol_counts,
        "failure_categories": categories,
        "upstream_request": "sent_via_cpar",
    }
    try:
        write_receipt(args.receipt, receipt)
    except OSError:
        print("cpar_grok_e2e=FAIL category=receipt_write", file=sys.stderr)
        return 1
    if successful != args.iterations:
        print(f"cpar_grok_e2e=FAIL category={categories[0] if categories else 'incomplete'}", file=sys.stderr)
        return 1
    print(f"cpar_grok_e2e=COMPLETE calls={successful} upstream_request=sent_via_cpar")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
