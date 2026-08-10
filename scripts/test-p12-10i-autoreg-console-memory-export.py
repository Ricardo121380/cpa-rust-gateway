#!/usr/bin/env python3
"""Offline contract tests for the bounded Autoreg Console SSO source adapter."""

from __future__ import annotations

import base64
import importlib.util
import json
import os
import pathlib
import sqlite3
import tempfile
import time
import unittest


SCRIPT = pathlib.Path(__file__).with_name("p12-10i-autoreg-console-memory-export.py")
SPEC = importlib.util.spec_from_file_location("autoreg_console_export", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def token(session_id: str = "fixture-session") -> str:
    def part(value: object) -> str:
        return base64.urlsafe_b64encode(json.dumps(value, separators=(",", ":")).encode()).decode().rstrip("=")

    return f"{part({'alg': 'none'})}.{part({'session_id': session_id})}.fixture-signature"


class ExportContractTests(unittest.TestCase):
    def fixture(self, *, session_token: str | None = None) -> tuple[pathlib.Path, pathlib.Path, pathlib.Path]:
        directory = pathlib.Path(tempfile.mkdtemp())
        source_root = directory / "source"
        task = source_root / "apps/console/runtime/tasks/task_67"
        (task / "sso").mkdir(parents=True)
        (task / "keys").mkdir()
        database = source_root / "apps/console/runtime/console.db"
        database.parent.mkdir(parents=True, exist_ok=True)
        connection = sqlite3.connect(database)
        connection.execute(
            "CREATE TABLE tasks (id INTEGER PRIMARY KEY,status TEXT,target_count INTEGER,"
            "completed_count INTEGER,failed_count INTEGER,last_email TEXT,task_dir TEXT)"
        )
        connection.execute(
            "INSERT INTO tasks VALUES (67,'completed',1,1,0,'fixture@example.test',?)",
            ("/workspace/apps/console/runtime/tasks/task_67",),
        )
        connection.commit()
        connection.close()
        raw_token = session_token or token()
        (task / "sso/task_67_w0.txt").write_text(raw_token + "\n", encoding="utf-8")
        session = {
            "email": "fixture@example.test",
            "cookies": [
                {"name": "sso", "value": raw_token},
                {"name": "sso-rw", "value": raw_token},
            ],
        }
        (task / "keys/auth-sessions.jsonl").write_text(
            json.dumps(session, separators=(",", ":")) + "\n", encoding="utf-8"
        )
        return source_root, database, task

    def test_valid_task_is_normalized_to_one_console_credential(self) -> None:
        source_root, database, task = self.fixture()
        value, selected_task = MODULE._read_task(
            source_root, database, 67, max_age_hours=1, require_root=False
        )
        self.assertEqual(value, token())
        self.assertEqual(selected_task, pathlib.Path(os.path.abspath(task)))
        self.assertEqual(MODULE._normalize_model("Console/grok-4.20-0309"), "grok-4.20-0309")

    def test_stale_source_fails_without_echoing_token(self) -> None:
        source_root, database, task = self.fixture()
        sso = task / "sso/task_67_w0.txt"
        os.utime(sso, (time.time() - 8 * 86400, time.time() - 8 * 86400))
        with self.assertRaises(MODULE.ExportFailure) as context:
            MODULE._read_task(source_root, database, 67, max_age_hours=1, require_root=False)
        self.assertEqual(str(context.exception), "stale_sso_source")
        self.assertNotIn(token(), str(context.exception))

    def test_symlink_and_ambiguous_sources_fail_closed(self) -> None:
        source_root, database, task = self.fixture()
        original = task / "sso/task_67_w0.txt"
        link = task / "sso/other.txt"
        link.symlink_to(original)
        with self.assertRaisesRegex(MODULE.ExportFailure, "ambiguous_sso_source"):
            MODULE._read_task(source_root, database, 67, max_age_hours=1, require_root=False)
        link.unlink()
        original.unlink()
        original.symlink_to(task / "keys/auth-sessions.jsonl")
        with self.assertRaisesRegex(MODULE.ExportFailure, "unsafe_source_path"):
            MODULE._read_task(source_root, database, 67, max_age_hours=1, require_root=False)

    def test_identity_unknown_fields_and_model_namespace_fail_closed(self) -> None:
        source_root, database, task = self.fixture()
        session_path = task / "keys/auth-sessions.jsonl"
        session = json.loads(session_path.read_text())
        session["unexpected"] = True
        session_path.write_text(json.dumps(session) + "\n")
        with self.assertRaisesRegex(MODULE.ExportFailure, "invalid_session_document"):
            MODULE._read_task(source_root, database, 67, max_age_hours=1, require_root=False)
        for value in ("Build/grok-4.20-0309", "Web/grok-4.20-0309", "grok-unknown"):
            with self.assertRaisesRegex(MODULE.ExportFailure, "unsupported_console_model"):
                MODULE._normalize_model(value)

    def test_token_shape_rejects_auth_claims(self) -> None:
        payload = base64.urlsafe_b64encode(
            json.dumps({"session_id": "x", "email": "secret@example.test"}).encode()
        ).decode().rstrip("=")
        bad = f"a.{payload}.c"
        with self.assertRaisesRegex(MODULE.ExportFailure, "invalid_console_sso"):
            MODULE._parse_sso(bad.encode())


if __name__ == "__main__":
    unittest.main()
