#!/usr/bin/env python3
"""No-network admission tests for the P12-08G1 graph helper."""

import importlib.util
import io
from pathlib import Path
import unittest


PATH = Path(__file__).with_name("p12-08g1-enter-production-graph.py")
SPEC = importlib.util.spec_from_file_location("p12_08g1_production_graph", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class Stdin:
    def __init__(self, value: bytes):
        self.buffer = io.BytesIO(value)


class Session:
    def call(self, method, path, payload, expect=201):
        self.payload = payload
        return {}


class Ledger:
    def record(self, key, value):
        self.key = key
        self.value = value


class ProductionGraphTest(unittest.TestCase):
    def read(self, value: bytes):
        previous = MODULE.sys.stdin
        MODULE.sys.stdin = Stdin(value)
        try:
            return MODULE.read_inputs()
        finally:
            MODULE.sys.stdin = previous

    def test_exact_private_input_normalizes_https_target(self):
        result = self.read(b"https://example.test/v1/\0opaque\0model\0")
        self.assertEqual(result, ("https://example.test/v1", "opaque", "model", "example.test", 443))

    def test_ambiguous_inputs_fail_closed(self):
        for value in (
            b"http://example.test/v1\0opaque\0model\0",
            b"https://user@example.test/v1\0opaque\0model\0",
            b"https://example.test/v1?q=x\0opaque\0model\0",
            b"https://example.test/v1\0opaque\0",
        ):
            with self.assertRaises(MODULE.ManagementError):
                self.read(value)

    def test_three_routes_freeze_the_required_transform_modes(self):
        self.assertEqual(set(MODULE.ALIASES), {"chat", "responses", "messages"})
        self.assertEqual(MODULE.MODE["chat"], "canonical")
        self.assertEqual(MODULE.MODE["responses"], "canonical")
        self.assertEqual(MODULE.MODE["messages"], "lossless_bridge")
        self.assertEqual(len(set(MODULE.ROUTES.values())), 3)
        self.assertEqual(len(set(MODULE.CANDIDATES.values())), 3)
        self.assertTrue(callable(MODULE.reactivate))

    def test_ledger_records_id_not_secret_payload(self):
        session, ledger = Session(), Ledger()
        MODULE.call(session, ledger, "credential_id", "POST", "/credentials", {
            "id": "credential", "secret": "not-retained",
        })
        self.assertEqual(ledger.value, "credential")
        self.assertNotIn("not-retained", repr(vars(ledger)))


if __name__ == "__main__":
    unittest.main()
