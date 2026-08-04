#!/usr/bin/env python3
"""Focused offline checks for the CPAR Grok curl E2E harness."""

import importlib.util
import json
from pathlib import Path
import tempfile


SCRIPT = Path(__file__).with_name("run-p12-10h-grok-cpar-e2e.py")
SPEC = importlib.util.spec_from_file_location("p12_10h_grok_cpar_e2e", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

assert MODULE.endpoint_admission("http://127.0.0.1:18180", False)[0] == "http://127.0.0.1:18180"
try:
    MODULE.endpoint_admission("http://example.test", False)
except MODULE.ProbeFailure as error:
    assert str(error) == "non_loopback_http"
else:
    raise AssertionError("public HTTP endpoint must fail closed")

assert MODULE.is_grok_label("grok-test")
assert not MODULE.is_grok_label("codex-test")
assert json.loads(MODULE.fixed_body("responses", "grok-test", False))["stream"] is False
assert json.loads(MODULE.fixed_body("chat", "grok-test", True))["stream"] is True
assert json.loads(MODULE.fixed_body("messages", "grok-test", False))["stream"] is False

with tempfile.TemporaryDirectory() as directory:
    path = Path(directory) / "models.json"
    path.write_text(json.dumps({"data": [{"id": "codex-test"}]}), encoding="utf-8")
    try:
        MODULE.validate_models(path, "codex-test")
    except MODULE.ProbeFailure as error:
        assert str(error) == "grok_route_missing"
    else:
        raise AssertionError("a model catalog without Grok must fail closed")

print("p12-10h Grok CPAR E2E test: ok")
