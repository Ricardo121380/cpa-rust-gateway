#!/usr/bin/env python3
"""Merges per-round P12-06 measurements into one paired summary.

Reads the `round-N-<arm>.json` files the comparison runner writes and reports, per arm, the metrics
`CR-P12-06-003` names: first-token latency, TTFB, total duration, output token rate and inter-token
delay quantiles.

The comparison is *paired*: round N of both arms ran seconds apart against the same upstream with
the same prompt, so per-round differences are reported alongside the aggregate. An aggregate alone
can hide that one arm simply ran during a slower window.

Emits timings and token counts only -- no prompt, no response text, no credential.
"""

from __future__ import annotations

import argparse
import json
import statistics
from pathlib import Path

METRICS = (
    ("first_semantic_ms", "first_semantic_event"),
    ("first_text_ms", "first_text_delta"),
    ("ttfb_ms", "ttfb"),
    ("total_ms", "total"),
)


def quantile(values: list[float], fraction: float) -> float | None:
    """Nearest-rank quantile: always an observed value, never an interpolated invention."""
    if not values:
        return None
    ordered = sorted(values)
    rank = max(0, min(len(ordered) - 1, int(round(fraction * (len(ordered) - 1)))))
    return round(ordered[rank], 3)


def summarise(values: list[float]) -> dict | None:
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


def token_rate(sample: dict) -> float | None:
    """Output tokens per second over the post-first-token window (see the measurement harness)."""
    tokens = sample.get("output_tokens")
    first_text = sample.get("first_text_ms")
    total = sample.get("total_ms")
    if tokens is None or first_text is None or total is None:
        return None
    window = total - first_text
    return round(tokens * 1000.0 / window, 3) if window > 0 else None


def load_rounds(in_dir: Path) -> dict[str, dict[int, dict]]:
    """Returns {arm: {round_index: sample}} for every round file that holds a successful sample.

    The arm name comes from the report's own `label`, not from the filename. Parsing the filename
    would make the arm identity depend on a naming convention two scripts have to agree on, and a
    silent mismatch there would misattribute one implementation's latency to the other.
    """
    arms: dict[str, dict[int, dict]] = {}
    for path in sorted(in_dir.glob("round-*.json")):
        try:
            round_index = int(path.stem.split("-")[1])
        except (IndexError, ValueError):
            continue
        report = json.loads(path.read_text(encoding="utf-8"))
        samples = report.get("samples") or []
        arm = report.get("label")
        if not samples or not arm:
            continue
        arms.setdefault(arm, {})[round_index] = samples[0]
    return arms


def main() -> int:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("--in-dir", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    in_dir = Path(args.in_dir)
    arms = load_rounds(in_dir)
    if not arms:
        print("summarise: no successful rounds found")
        return 1

    per_arm: dict[str, dict] = {}
    for arm, rounds in arms.items():
        samples = [rounds[index] for index in sorted(rounds)]
        inter = [gap for sample in samples for gap in sample.get("inter_delta_ms", [])]
        gaps = [
            sample["first_text_ms"] - sample["first_semantic_ms"]
            for sample in samples
            if sample.get("first_text_ms") is not None
            and sample.get("first_semantic_ms") is not None
        ]
        rates = [rate for rate in (token_rate(sample) for sample in samples) if rate is not None]
        per_arm[arm] = {
            "successful_rounds": sorted(rounds),
            "metrics_ms": {
                label: summarise([
                    sample[field] for sample in samples if sample.get(field) is not None
                ])
                for field, label in METRICS
            }
            | {
                "thinking_gap_first_text_minus_first_semantic": summarise(gaps),
                "inter_token_delta": summarise(inter),
            },
            "output_token_rate_per_second": summarise(rates),
            "output_tokens": summarise([
                sample["output_tokens"]
                for sample in samples
                if sample.get("output_tokens") is not None
            ]),
            "input_tokens": summarise([
                sample["input_tokens"]
                for sample in samples
                if sample.get("input_tokens") is not None
            ]),
            "stop_reasons": sorted({str(sample.get("stop_reason")) for sample in samples}),
        }

    # Paired per-round deltas, computed only for rounds where BOTH arms succeeded. An unpaired
    # aggregate difference can be produced entirely by upstream drift between the two windows.
    paired: dict = {}
    arm_names = sorted(arms)
    if len(arm_names) == 2:
        left, right = arm_names
        shared = sorted(set(arms[left]) & set(arms[right]))
        deltas: dict[str, list[float]] = {label: [] for _, label in METRICS}
        rate_deltas: list[float] = []
        for index in shared:
            for field, label in METRICS:
                left_value = arms[left][index].get(field)
                right_value = arms[right][index].get(field)
                if left_value is not None and right_value is not None:
                    deltas[label].append(left_value - right_value)
            left_rate = token_rate(arms[left][index])
            right_rate = token_rate(arms[right][index])
            if left_rate is not None and right_rate is not None:
                rate_deltas.append(left_rate - right_rate)
        paired = {
            "left": left,
            "right": right,
            "note": f"positive means {left} was slower / higher than {right} in the same round",
            "paired_rounds": shared,
            "delta_ms": {label: summarise(values) for label, values in deltas.items()},
            "delta_output_token_rate_per_second": summarise(rate_deltas),
        }

    report = {
        "arms": per_arm,
        "paired_comparison": paired,
        "not_a_differential": (
            "The incumbent CPA has no Kiro channel, so this is functional verification plus a "
            "latency comparison against kiro-rs, the reference implementation for the same "
            "upstream. It is not the incumbent differential."
        ),
    }
    Path(args.out).write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"summarise: {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
