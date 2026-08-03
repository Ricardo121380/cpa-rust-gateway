#!/usr/bin/env python3
"""Offline contract tests for the P12-10G source normalizer."""

from __future__ import annotations

import importlib.util
import json
import pathlib
import sqlite3
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).with_name("p12-10g-grok2api-memory-export.py")
SPEC = importlib.util.spec_from_file_location("p12_10g_export", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ExportContractTests(unittest.TestCase):
    def test_console_prefers_the_most_recent_verified_success(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            database = pathlib.Path(directory) / "source.sqlite3"
            connection = sqlite3.connect(database)
            connection.executescript("""
                CREATE TABLE provider_accounts (
                    id INTEGER PRIMARY KEY, identity_key TEXT, name TEXT, email TEXT,
                    user_id TEXT, team_id TEXT, provider TEXT, auth_status TEXT,
                    enabled INTEGER, priority INTEGER, max_concurrent INTEGER,
                    cooldown_until TEXT, observed_model TEXT
                );
                CREATE TABLE account_credentials (
                    account_id INTEGER, expires_at TEXT, refresh_due_at TEXT
                );
                CREATE TABLE request_audits (
                    account_id INTEGER, status_code INTEGER, created_at TEXT,
                    model_upstream_model TEXT
                );
                INSERT INTO provider_accounts VALUES
                    (1, 'older', 'a', '', '', '', 'grok_console', 'active', 1, 1, 1, NULL, NULL),
                    (2, 'newer', 'b', '', '', '', 'grok_console', 'active', 1, 1, 1, NULL, NULL);
                INSERT INTO account_credentials VALUES (1, NULL, NULL), (2, NULL, NULL);
                INSERT INTO request_audits VALUES
                    (1, 200, '2026-01-01T00:00:00Z', 'fixture-older'),
                    (2, 200, '2026-02-01T00:00:00Z', 'fixture-newer');
            """)
            connection.commit()
            connection.close()
            original = MODULE.DATABASE
            MODULE.DATABASE = database
            try:
                total, rows = MODULE.source_rows("grok_console", 1)
            finally:
                MODULE.DATABASE = original
            self.assertEqual(total, 2)
            self.assertEqual(rows[0]["identity_key"], "newer")
            self.assertEqual(rows[0]["probe_model"], "fixture-newer")

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
            "observed_model": MODULE.BUILD_PROBE_MODEL,
            "probe_model": "Console/fixture-console-model",
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
        console_credential = json.loads(console[0]["credential"])
        self.assertEqual(console_credential["sso_token"], "fixture-console")
        self.assertEqual(console_credential["probe_model"], "fixture-console-model")

        row["probe_model"] = "Build/fixture-console-model"
        with self.assertRaisesRegex(MODULE.ExportFailure, "unsupported_console_credential"):
            MODULE.transfer_records("grok_console", 1, console_export, [row])

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

    def test_build_rejects_an_unobserved_probe_model(self) -> None:
        exported = {
            "accounts": [{
                "provider": "grok_build", "name": "a", "email": "",
                "user_id": "", "team_id": "", "client_id": MODULE.BUILD_CLIENT_ID,
                "access_token": "fixture-access", "refresh_token": "fixture-refresh",
                "expires_at": "2099-01-01T00:00:00Z",
            }],
        }
        row = {
            "identity_key": "fixture-identity", "name": "a", "email": "",
            "user_id": "", "team_id": "", "priority": 1, "max_concurrent": 1,
            "refresh_due_at": None, "cooldown_until": None,
            "observed_model": "unsupported-fixture-model",
        }
        with self.assertRaisesRegex(MODULE.ExportFailure, "unsupported_build_model"):
            MODULE.transfer_records("grok_build", 1, exported, [row])


if __name__ == "__main__":
    unittest.main()
