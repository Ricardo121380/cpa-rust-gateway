#!/usr/bin/env python3
"""Measures per-request streaming latency against one Anthropic-Messages SSE endpoint.

P12-06 needs client-side timing because the gateway exposes no latency histogram: its Prometheus
surface is seven counters and `ManagementRequestAttempt` carries no timing by design (plan gap 8).
So the only honest place to measure first-token latency is where a client would see it.

The measurement sits *outside* the split layer on purpose -- it talks to a data plane directly, so
no proxy hop is attributed to the gateway.

Emitted values are timings and token counts only. No prompt, no response text, and no credential
ever enters the output: the report has to be publishable next to the code.

Usage:
  p12-06-measure-stream.py --url URL --key-file PATH --model NAME --prompt-file PATH
                           [--samples N] [--warmup N] [--label NAME] [--out PATH]

The key is read from a file rather than argv so it cannot appear in `ps` output.
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
import time
import urllib.error
import urllib.request

# One connection per sample, deliberately. A pooled connection would hide TLS/TCP setup from the
# first sample only, making sample 1 an outlier and the rest unrepresentative of a fresh client.
# Every sample therefore pays the same setup cost and the arms stay comparable.
READ_CHUNK_BYTES = 1


class SampleFailure(Exception):
    """One sample did not produce a complete, well-formed stream."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("--url", required=True, help="Full /v1/messages URL")
    parser.add_argument("--key-file", required=True, help="File holding the x-api-key value")
    parser.add_argument("--model", required=True, help="Model name to request")
    parser.add_argument("--prompt-file", required=True, help="File holding the user prompt text")
    parser.add_argument("--samples", type=int, default=10)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--max-tokens", type=int, default=512)
    parser.add_argument("--label", default="unlabelled")
    parser.add_argument("--out", default="-")
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument("--sleep-between", type=float, default=2.0)
    return parser.parse_args()


def read_one_line(path: str) -> str:
    with open(path, "r", encoding="utf-8") as handle:
        return handle.read().strip()


def build_body(model: str, prompt: str, max_tokens: int) -> bytes:
    return json.dumps(
        {
            "model": model,
            "max_tokens": max_tokens,
            "stream": True,
            "messages": [{"role": "user", "content": prompt}],
        }
    ).encode("utf-8")


def measure_once(
    url: str, key: str, body: bytes, timeout: float
) -> dict[str, float | int | None]:
    """Streams one request, timestamping the boundaries P12-06 reports on.

    Four distinct boundaries are recorded because they answer different questions and routinely
    differ by seconds on a thinking model:

    `ttfb_ms`        first response byte, i.e. the head. Upstream has accepted the request.
    `first_semantic` first semantic SSE event (`message_start`). This is the BL-05 transparent
                     retry boundary: before it a failed attempt may still be retried invisibly,
                     after it the response is committed to this attempt.
    `first_text_ms`  first `content_block_delta`. This is what a human calls "the first character",
                     and on a thinking model it can trail `message_start` by a long pause.
    `total_ms`       stream termination.
    """
    request = urllib.request.Request(
        url,
        data=body,
        method="POST",
        headers={
            "content-type": "application/json",
            "accept": "text/event-stream",
            "x-api-key": key,
            "anthropic-version": "2023-06-01",
        },
    )
    started = time.monotonic()
    ttfb: float | None = None
    first_semantic: float | None = None
    first_text: float | None = None
    delta_times: list[float] = []
    output_tokens: int | None = None
    input_tokens: int | None = None
    stop_reason: str | None = None
    saw_stop = False
    buffer = b""

    # `urlopen` returns once the head is available, so the clock here is genuinely time-to-first-
    # byte and not merely time-to-connect.
    try:
        response = urllib.request.urlopen(request, timeout=timeout)  # noqa: S310 - loopback only
    except urllib.error.HTTPError as error:
        raise SampleFailure(f"http {error.code}") from error
    except OSError as error:
        raise SampleFailure(f"transport: {type(error).__name__}") from error

    with response:
        if response.status != 200:
            raise SampleFailure(f"http {response.status}")
        while True:
            chunk = response.read(READ_CHUNK_BYTES)
            now = time.monotonic()
            if ttfb is None:
                ttfb = now - started
            if not chunk:
                break
            buffer += chunk
            while b"\n\n" in buffer:
                record, buffer = buffer.split(b"\n\n", 1)
                payload = parse_sse_record(record)
                if payload is None:
                    continue
                kind = payload.get("type")
                if kind is None:
                    continue
                if first_semantic is None:
                    first_semantic = now - started
                if kind == "content_block_delta":
                    if first_text is None:
                        first_text = now - started
                    delta_times.append(now - started)
                elif kind == "message_start":
                    usage = payload.get("message", {}).get("usage", {})
                    input_tokens = usage.get("input_tokens", input_tokens)
                elif kind == "message_delta":
                    usage = payload.get("usage", {})
                    output_tokens = usage.get("output_tokens", output_tokens)
                    if usage.get("input_tokens") is not None:
                        input_tokens = usage["input_tokens"]
                    stop_reason = payload.get("delta", {}).get("stop_reason", stop_reason)
                elif kind == "message_stop":
                    saw_stop = True
                elif kind == "error":
                    raise SampleFailure("stream error event")

    total = time.monotonic() - started
    if not saw_stop:
        raise SampleFailure("stream ended without message_stop")
    if first_semantic is None:
        raise SampleFailure("no semantic event")
    return {
        "ttfb_ms": to_ms(ttfb),
        "first_semantic_ms": to_ms(first_semantic),
        "first_text_ms": to_ms(first_text),
        "total_ms": to_ms(total),
        "output_tokens": output_tokens,
        "input_tokens": input_tokens,
        "stop_reason": stop_reason,
        "delta_count": len(delta_times),
        "inter_delta_ms": [
            to_ms(later - earlier)
            for earlier, later in zip(delta_times, delta_times[1:])
        ],
    }


def parse_sse_record(record: bytes) -> dict | None:
    """Returns the JSON object of one SSE record, or None for keepalives and comments."""
    data_lines = []
    for raw_line in record.split(b"\n"):
        line = raw_line.strip()
        if not line or line.startswith(b":"):
            continue
        if line.startswith(b"data:"):
            data_lines.append(line[len(b"data:") :].strip())
    if not data_lines:
        return None
    try:
        parsed = json.loads(b"\n".join(data_lines).decode("utf-8"))
    except (json.JSONDecodeError, UnicodeDecodeError):
        return None
    return parsed if isinstance(parsed, dict) else None


def to_ms(value: float | None) -> float | None:
    return None if value is None else round(value * 1000.0, 3)


def quantile(values: list[float], fraction: float) -> float | None:
    """Nearest-rank quantile.

    Deliberately not interpolated: an interpolated P99 of nine samples invents a number that no
    request ever produced. Nearest-rank always returns an observed value, which is what a latency
    claim should rest on. The sample count travels with every summary so a reader can see how much
    a given quantile is worth.
    """
    if not values:
        return None
    ordered = sorted(values)
    rank = max(0, min(len(ordered) - 1, int(round(fraction * (len(ordered) - 1)))))
    return round(ordered[rank], 3)


def summarise(field: str, samples: list[dict]) -> dict | None:
    values = [sample[field] for sample in samples if sample.get(field) is not None]
    if not values:
        return None
    return {
        "n": len(values),
        "min": round(min(values), 3),
        "p50": quantile(values, 0.50),
        "p95": quantile(values, 0.95),
        "p99": quantile(values, 0.99),
        "max": round(max(values), 3),
        "mean": round(statistics.fmean(values), 3),
    }


def token_rate_per_second(sample: dict) -> float | None:
    """Output tokens per second over the post-first-token interval.

    The denominator is `total - first_text`, not `total`. Using total would fold the upstream's
    thinking pause into the rate and understate steady-state throughput -- the exact conflation
    `CR-P12-06-003` requires this report to avoid. First-token latency is reported separately, so
    nothing is hidden by excluding it here.

    `first_text` is the divisor rather than `first_semantic` because tokens are what is being
    measured, and the first token arrives with the first `content_block_delta`.
    """
    output_tokens = sample.get("output_tokens")
    first_text = sample.get("first_text_ms")
    total = sample.get("total_ms")
    if output_tokens is None or first_text is None or total is None:
        return None
    window_ms = total - first_text
    if window_ms <= 0.0:
        return None
    return round(output_tokens * 1000.0 / window_ms, 3)


def build_report(args: argparse.Namespace, samples: list[dict], failures: list[str]) -> dict:
    rates = [rate for rate in (token_rate_per_second(s) for s in samples) if rate is not None]
    inter_delta = [gap for sample in samples for gap in sample.get("inter_delta_ms", [])]
    thinking_gaps = [
        round(sample["first_text_ms"] - sample["first_semantic_ms"], 3)
        for sample in samples
        if sample.get("first_text_ms") is not None
        and sample.get("first_semantic_ms") is not None
    ]
    return {
        "label": args.label,
        "url": args.url,
        "model": args.model,
        "max_tokens": args.max_tokens,
        "requested_samples": args.samples,
        "successful_samples": len(samples),
        "failures": failures,
        "metrics_ms": {
            "ttfb": summarise("ttfb_ms", samples),
            "first_semantic_event": summarise("first_semantic_ms", samples),
            "first_text_delta": summarise("first_text_ms", samples),
            "total": summarise("total_ms", samples),
            "thinking_gap_first_text_minus_first_semantic": summarise(
                "gap", [{"gap": gap} for gap in thinking_gaps]
            ),
            "inter_token_delta": summarise(
                "gap", [{"gap": gap} for gap in inter_delta]
            ),
        },
        "output_token_rate_per_second": summarise(
            "rate", [{"rate": rate} for rate in rates]
        ),
        "output_tokens": summarise("output_tokens", samples),
        "input_tokens": summarise("input_tokens", samples),
        "stop_reasons": sorted({
            str(sample.get("stop_reason")) for sample in samples
        }),
        "samples": samples,
    }


def main() -> int:
    args = parse_args()
    key = read_one_line(args.key_file)
    if not key:
        print("measure: key file is empty", file=sys.stderr)
        return 2
    with open(args.prompt_file, "r", encoding="utf-8") as handle:
        prompt = handle.read()
    if not prompt.strip():
        print("measure: prompt file is empty", file=sys.stderr)
        return 2
    body = build_body(args.model, prompt, args.max_tokens)

    # Warm-up samples are discarded, not merged: the first request against a fresh process pays
    # DNS resolution and TLS handshake costs to the upstream that later requests reuse, and folding
    # that into the summary would misreport steady-state latency as if every request paid it.
    for _ in range(max(0, args.warmup)):
        try:
            measure_once(args.url, key, body, args.timeout)
        except SampleFailure as failure:
            print(f"measure: warmup failed: {failure}", file=sys.stderr)
        time.sleep(args.sleep_between)

    samples: list[dict] = []
    failures: list[str] = []
    for index in range(args.samples):
        try:
            samples.append(measure_once(args.url, key, body, args.timeout))
        except SampleFailure as failure:
            # A failure is recorded rather than retried. Retrying until N successes accumulate
            # would report the latency of a channel that never fails, which is not the channel
            # under test; the failure count belongs in the evidence.
            failures.append(str(failure))
            print(f"measure: sample {index + 1} failed: {failure}", file=sys.stderr)
        if index + 1 < args.samples:
            time.sleep(args.sleep_between)

    report = build_report(args, samples, failures)
    rendered = json.dumps(report, indent=2, sort_keys=True)
    if args.out == "-":
        print(rendered)
    else:
        with open(args.out, "w", encoding="utf-8") as handle:
            handle.write(rendered + "\n")
        print(f"measure: wrote {args.out} ({len(samples)}/{args.samples} samples)")
    return 0 if samples else 1


if __name__ == "__main__":
    sys.exit(main())
