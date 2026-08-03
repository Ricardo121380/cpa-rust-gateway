#!/usr/bin/env python3
"""No-network tests for the P12-10 grok2api successor graph helper."""

import importlib.util
import io
from pathlib import Path
import unittest


PATH = Path(__file__).with_name("p12-10-enter-grok2api-graph.py")
SPEC = importlib.util.spec_from_file_location("p12_10_grok2api_graph", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class Stdin:
    def __init__(self, value: bytes):
        self.buffer = io.BytesIO(value)


class GraphTest(unittest.TestCase):
    def read(self, value: bytes):
        previous = MODULE.sys.stdin
        MODULE.sys.stdin = Stdin(value)
        try:
            return MODULE.read_inputs()
        finally:
            MODULE.sys.stdin = previous

    def test_accepts_https_codex_and_exact_http_loopback_grok(self):
        result = self.read(
            b"https://example.test/v1/\0codex-key\0codex-model\0"
            b"http://127.0.0.1:18000/v1/\0grok-key\0grok-model\0"
        )
        self.assertEqual(result["codex_base"], "https://example.test/v1")
        self.assertEqual(result["grok_base"], "http://127.0.0.1:18000/v1")
        self.assertEqual(result["grok_port"], 18000)

    def test_rejects_non_loopback_or_https_grok2api_targets(self):
        for target in (b"http://localhost:18000/v1", b"http://10.0.0.1:18000/v1", b"https://127.0.0.1:18000/v1"):
            value = b"https://example.test/v1\0a\0b\0" + target + b"\0c\0d\0"
            with self.assertRaises(MODULE.ManagementError):
                self.read(value)

    def test_freezes_six_distinct_routes_and_bridge_modes(self):
        self.assertEqual(len(MODULE.ALIASES), 6)
        self.assertEqual(len(set(MODULE.ALIASES.values())), 6)
        self.assertEqual(MODULE.MODE[("grok", "responses")], "canonical")
        self.assertEqual(MODULE.MODE[("grok", "chat")], "lossless_bridge")
        self.assertEqual(MODULE.MODE[("grok", "messages")], "lossless_bridge")
        self.assertTrue(callable(MODULE.rollback))
        self.assertTrue(callable(MODULE.reactivate))


if __name__ == "__main__":
    unittest.main()
