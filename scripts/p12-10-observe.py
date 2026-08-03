#!/usr/bin/env python3
"""Run the bounded, value-free P12-10 production observation window."""

from __future__ import annotations

import argparse
import json
import math
import os
import signal
import sqlite3
import ssl
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

SCHEMA = "cpa-rust-gateway-p12-10-observation-v1"
P1_OUTCOMES = (
    "required_quarantined",
    "write_failed",
    "required_queue_full",
    "sink_closed",
)
MAX_RESPONSE_BYTES = 512 * 1024


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--endpoint-file", required=True)
    parser.add_argument("--client-key-file", required=True)
    parser.add_argument("--model-file", required=True)
    parser.add_argument("--management-key-file", required=True)
    parser.add_argument("--database", required=True)
    parser.add_argument("--live-caddyfile", required=True)
    parser.add_argument("--expected-caddyfile", required=True)
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--duration-seconds", type=int, default=72 * 60 * 60)
    parser.add_argument("--interval-seconds", type=int, default=180)
    parser.add_argument("--minimum-successes", type=int, default=1250)
    return parser.parse_args()


def read_secret(path: str) -> str:
    value = Path(path).read_text(encoding="utf-8").strip()
    if not value:
        raise RuntimeError("required input file is empty")
    return value


def percentile(values: list[int], quantile: float) -> int | None:
    if not values:
        return None
    ordered = sorted(values)
    index = max(0, math.ceil(quantile * len(ordered)) - 1)
    return ordered[index]


def metrics_snapshot(url: str, management_key: str) -> dict[str, int]:
    request = urllib.request.Request(url, headers={"X-Management-Key": management_key})
    with urllib.request.urlopen(request, timeout=10) as response:
        text = response.read(256 * 1024).decode("utf-8")
    result = dict.fromkeys(P1_OUTCOMES, 0)
    for line in text.splitlines():
        if not line.startswith("gateway_observability_"):
            continue
        for outcome in P1_OUTCOMES:
            if f'outcome="{outcome}"' in line:
                result[outcome] += int(float(line.rsplit(None, 1)[1]))
    return result


def database_snapshot(path: str, baseline_ordinal: int | None = None) -> tuple[str, int, int]:
    uri = f"file:{Path(path).resolve()}?mode=ro"
    connection = sqlite3.connect(uri, uri=True, timeout=10)
    try:
        quick_check = connection.execute("PRAGMA quick_check").fetchone()[0]
        maximum = connection.execute(
            "SELECT COALESCE(MAX(event_ordinal), 0) FROM gateway_event_log"
        ).fetchone()[0]
        if baseline_ordinal is None:
            successes = 0
        else:
            successes = connection.execute(
                "SELECT COUNT(*) FROM gateway_event_log "
                "WHERE event_ordinal > ? AND event_type = 'attempt' "
                "AND json_extract(payload_json, '$.attempt.outcome') = 'succeeded'",
                (baseline_ordinal,),
            ).fetchone()[0]
        return str(quick_check), int(maximum), int(successes)
    finally:
        connection.close()


def synthetic_probe(endpoint: str, key: str, model: str) -> tuple[bool, int, int, int]:
    body = json.dumps(
        {
            "model": model,
            "messages": [{"role": "user", "content": "Reply with OK."}],
            "max_tokens": 8,
            "stream": False,
        },
        separators=(",", ":"),
    ).encode("utf-8")
    request = urllib.request.Request(
        endpoint.rstrip("/") + "/v1/chat/completions",
        data=body,
        headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"},
        method="POST",
    )
    started = time.monotonic_ns()
    try:
        with urllib.request.urlopen(request, timeout=90, context=ssl.create_default_context()) as response:
            first = response.read(1)
            first_byte = time.monotonic_ns()
            raw = first + response.read(MAX_RESPONSE_BYTES)
            ended = time.monotonic_ns()
            status = response.status
    except urllib.error.HTTPError as error:
        error.read(4096)
        ended = time.monotonic_ns()
        return False, error.code, (ended - started) // 1_000_000, (ended - started) // 1_000_000
    except (OSError, TimeoutError, urllib.error.URLError):
        ended = time.monotonic_ns()
        return False, 0, (ended - started) // 1_000_000, (ended - started) // 1_000_000
    try:
        parsed = json.loads(raw)
        valid = status == 200 and isinstance(parsed.get("choices"), list) and bool(parsed["choices"])
    except (UnicodeDecodeError, json.JSONDecodeError, AttributeError):
        valid = False
    return valid, status, (first_byte - started) // 1_000_000, (ended - started) // 1_000_000


def write_receipt(path: str, receipt: dict) -> None:
    destination = Path(path)
    temporary = destination.with_name(destination.name + ".tmp")
    payload = json.dumps(receipt, indent=2, sort_keys=True) + "\n"
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    try:
        os.write(descriptor, payload.encode("utf-8"))
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    os.replace(temporary, destination)


def utc_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def main() -> int:
    args = parse_args()
    if args.duration_seconds < 1 or args.interval_seconds < 30 or args.minimum_successes < 1:
        raise SystemExit("invalid bounded observation arguments")
    endpoint = read_secret(args.endpoint_file)
    if not endpoint.startswith("https://") or any(character.isspace() for character in endpoint):
        raise SystemExit("endpoint admission failed")
    key = read_secret(args.client_key_file)
    model = read_secret(args.model_file)
    management_key = read_secret(args.management_key_file)
    if Path(args.live_caddyfile).read_bytes() != Path(args.expected_caddyfile).read_bytes():
        raise SystemExit("production route is not the reviewed CPAR candidate")
    quick_check, baseline_ordinal, _ = database_snapshot(args.database)
    if quick_check != "ok":
        raise SystemExit("database preflight failed")
    baseline_metrics = metrics_snapshot(
        "http://127.0.0.1:18181/admin/observability/metrics", management_key
    )
    if any(baseline_metrics.values()):
        raise SystemExit("P1 counter preflight is not zero")

    started_wall = int(time.time())
    started_monotonic = time.monotonic()
    stopped = False

    def stop(_signum: int, _frame: object) -> None:
        nonlocal stopped
        stopped = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    attempts = successes = failures = consecutive_failures = 0
    ttfb: list[int] = []
    totals: list[int] = []
    status = "RUNNING"
    failure_reason: str | None = None

    while True:
        elapsed = int(time.monotonic() - started_monotonic)
        if stopped:
            status, failure_reason = "INCOMPLETE", "operator_stop"
        elif Path(args.live_caddyfile).read_bytes() != Path(args.expected_caddyfile).read_bytes():
            status, failure_reason = "FAILED_P1", "production_route_changed"
        quick_check, _, durable_successes = database_snapshot(args.database, baseline_ordinal)
        current_metrics = metrics_snapshot(
            "http://127.0.0.1:18181/admin/observability/metrics", management_key
        )
        metric_delta = {
            name: current_metrics[name] - baseline_metrics[name] for name in P1_OUTCOMES
        }
        if quick_check != "ok":
            status, failure_reason = "FAILED_P0", "database_integrity"
        elif any(value > 0 for value in metric_delta.values()):
            status, failure_reason = "FAILED_P1", "required_observability"

        if status == "RUNNING" and elapsed < args.duration_seconds:
            ok, _http_status, first_byte_ms, total_ms = synthetic_probe(endpoint, key, model)
            attempts += 1
            ttfb.append(first_byte_ms)
            totals.append(total_ms)
            if ok:
                successes += 1
                consecutive_failures = 0
            else:
                failures += 1
                consecutive_failures += 1
            if consecutive_failures >= 3 or (attempts >= 100 and failures * 100 > attempts):
                status, failure_reason = "FAILED_P1", "synthetic_error_rate"
        elif status == "RUNNING":
            status = "COMPLETED" if durable_successes >= args.minimum_successes else "INCOMPLETE"
            if status == "INCOMPLETE":
                failure_reason = "minimum_successes"

        receipt = {
            "schema_version": SCHEMA,
            "state": status,
            "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(started_wall)),
            "observed_at": utc_now(),
            "elapsed_seconds": elapsed,
            "required_duration_seconds": args.duration_seconds,
            "required_successes": args.minimum_successes,
            "durable_successes": durable_successes,
            "estimated_real_successes": max(0, durable_successes - successes),
            "synthetic_attempts": attempts,
            "synthetic_successes": successes,
            "synthetic_failures": failures,
            "synthetic_ttfb_ms": {
                "p50": percentile(ttfb, 0.50),
                "p95": percentile(ttfb, 0.95),
                "p99": percentile(ttfb, 0.99),
            },
            "synthetic_total_ms": {
                "p50": percentile(totals, 0.50),
                "p95": percentile(totals, 0.95),
                "p99": percentile(totals, 0.99),
            },
            "p1_counter_delta": metric_delta,
            "database_quick_check": quick_check,
            "failure_reason": failure_reason,
        }
        write_receipt(args.receipt, receipt)
        if status != "RUNNING":
            return 0 if status == "COMPLETED" else 1
        remaining = args.duration_seconds - int(time.monotonic() - started_monotonic)
        time.sleep(min(args.interval_seconds, max(1, remaining)))


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:  # noqa: BLE001 - fail closed without exposing input values
        print(f"p12-10 observation failed: {type(error).__name__}", file=sys.stderr)
        raise SystemExit(1) from None
