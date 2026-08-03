#!/usr/bin/env python3
"""Focused offline checks for the P12-10 observer helpers."""

import importlib.util
import json
import os
import tempfile
from pathlib import Path

SCRIPT = Path(__file__).with_name("p12-10-observe.py")
SPEC = importlib.util.spec_from_file_location("p12_10_observe", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

assert MODULE.percentile([], 0.95) is None
assert MODULE.percentile([4, 1, 3, 2], 0.50) == 2
assert MODULE.percentile([4, 1, 3, 2], 0.95) == 4

with tempfile.TemporaryDirectory() as directory:
    receipt = Path(directory) / "receipt.json"
    MODULE.write_receipt(str(receipt), {"schema_version": MODULE.SCHEMA, "state": "RUNNING"})
    assert json.loads(receipt.read_text(encoding="utf-8"))["state"] == "RUNNING"
    assert receipt.stat().st_mode & 0o077 == 0
    assert not Path(str(receipt) + ".tmp").exists()

for forbidden in ("endpoint", "client_key", "model", "request_body", "response_body"):
    assert forbidden not in {
        "schema_version",
        "state",
        "durable_successes",
        "synthetic_successes",
        "synthetic_failures",
        "p1_counter_delta",
    }

print("p12-10 observer test: ok")
