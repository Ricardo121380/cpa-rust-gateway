#!/usr/bin/env python3
"""Offline contract tests for the P12-10G source normalizer."""

from __future__ import annotations

import importlib.util
import json
import pathlib
import unittest


SCRIPT = pathlib.Path(__file__).with_name("p12-10g-grok2api-memory-export.py")
SPEC = importlib.util.spec_from_file_location("p12_10g_export", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ExportContractTests(unittest.TestCase):
    def test_build_and_console_normalize_without_source_values_in_diagnostics(self) -> None:
        build_export = {
            "provider": "grok_build",
            "accounts": [{
                "provider": "grok_build",
                "name": "a", "email": "", "user_id": "", "team_id": "",
                "client_id": MODULE.BUILD_CLIENT_ID,
                "access_token": "fixture-access", "refresh_token": "fixture-refresh",
                "expires_at": "2099-01-01T00:00:00Z",
            }],
        }
        build_export.pop("provider")
        row = {
            "identity_key": "fixture-identity", "name": "a", "email": "",
            "user_id": "", "team_id": "", "priority": 1, "max_concurrent": 2,
            "refresh_due_at": None, "cooldown_until": None,
        }
        records = MODULE.transfer_records("grok_build", 1, build_export, [row])
        credential = json.loads(records[0]["credential"])
        self.assertEqual(credential["issuer"], MODULE.BUILD_ISSUER)
        self.assertEqual(records[0]["identity_key"], "fixture-identity")

        console_export = {
            "provider": "grok_console",
            "accounts": [{
                "name": "a", "email": "", "user_id": "", "team_id": "",
                "sso_token": "fixture-console",
            }],
        }
        console = MODULE.transfer_records("grok_console", 1, console_export, [row])
        self.assertEqual(console[0]["credential"], "fixture-console")

    def test_web_and_ambiguous_identity_fail_closed(self) -> None:
        with self.assertRaisesRegex(MODULE.ExportFailure, "web_expiry_unavailable"):
            MODULE.canonical_credential("grok_web", {"sso_token": "fixture"})
        duplicate = {
            "provider": "grok_console",
            "accounts": [
                {"name": "a", "sso_token": "one"},
                {"name": "a", "sso_token": "two"},
            ],
        }
        with self.assertRaisesRegex(MODULE.ExportFailure, "ambiguous_export_identity"):
            MODULE.transfer_records("grok_console", 1, duplicate, [])


if __name__ == "__main__":
    unittest.main()
