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
from unittest import mock


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
    @staticmethod
    def session_bytes(raw_token: str, expiries: list[int | float]) -> bytes:
        cookies = []
        for index, (domain, name) in enumerate(
            (domain, name)
            for domain in (".x.ai", ".grok.com")
            for name in ("sso", "sso-rw")
        ):
            cookies.append(
                {
                    "name": name,
                    "value": raw_token,
                    "domain": domain,
                    "expires": expiries[index],
                    "httpOnly": True,
                    "path": "/",
                    "sameSite": "Lax",
                    "secure": True,
                }
            )
        return (
            json.dumps(
                {"email": "fixture@example.test", "cookies": cookies},
                separators=(",", ":"),
            ).encode()
            + b"\n"
        )

    def fixture(
        self,
        *,
        session_token: str | None = None,
        target_count: int = 1,
        completed_count: int = 1,
        cookie_overrides: dict[tuple[str, str], dict[str, object]] | None = None,
    ) -> tuple[pathlib.Path, pathlib.Path, pathlib.Path]:
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
            "INSERT INTO tasks VALUES (67,'completed',?,?,0,'fixture@example.test',?)",
            (target_count, completed_count, "/workspace/apps/console/runtime/tasks/task_67"),
        )
        connection.commit()
        connection.close()
        raw_token = session_token or token()
        (task / "sso/task_67_w0.txt").write_text(raw_token + "\n", encoding="utf-8")
        expiry = int((time.time() + 48 * 3600) * 1000)
        session = {
            "email": "fixture@example.test",
            "cookies": [
                {
                    "name": name,
                    "value": raw_token,
                    "domain": domain,
                    "expires": expiry,
                    "httpOnly": True,
                    "path": "/",
                    "sameSite": "Lax",
                    "secure": True,
                }
                for domain in (".x.ai", ".grok.com")
                for name in ("sso", "sso-rw")
            ],
        }
        for key, override in (cookie_overrides or {}).items():
            for cookie in session["cookies"]:
                if (cookie["domain"], cookie["name"]) == key:
                    cookie.update(override)
        (task / "keys/auth-sessions.jsonl").write_text(
            json.dumps(session, separators=(",", ":")) + "\n", encoding="utf-8"
        )
        return source_root, database, task

    def test_valid_task_is_normalized_to_one_console_credential(self) -> None:
        source_root, database, task = self.fixture()
        value, identity, refresh_due, selected_task = MODULE._read_task(
            source_root, database, 67, max_age_hours=1, require_root=False
        )
        self.assertEqual(value, token())
        self.assertEqual(identity, "fixture@example.test")
        self.assertGreater(refresh_due, int(time.time() * 1000))
        self.assertEqual(selected_task, pathlib.Path(os.path.abspath(task)))
        self.assertEqual(MODULE._normalize_model("Console/grok-4.20-0309"), "grok-4.20-0309")
        self.assertEqual(MODULE._normalize_model("Console/grok-build-0.1"), "grok-build-0.1")

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

    def test_task_must_be_exactly_one_completed_account(self) -> None:
        for target_count, completed_count in ((2, 2), (1, 0), (2, 1)):
            source_root, database, _ = self.fixture(
                target_count=target_count, completed_count=completed_count
            )
            with self.assertRaisesRegex(MODULE.ExportFailure, "task_not_eligible"):
                MODULE._read_task(source_root, database, 67, max_age_hours=1, require_root=False)

    def test_cookie_security_and_expiry_are_required(self) -> None:
        cases = (
            ((".x.ai", "sso"), {"secure": False}),
            ((".grok.com", "sso-rw"), {"domain": "attacker.invalid"}),
            ((".x.ai", "sso"), {"expires": 1}),
        )
        for key, override in cases:
            source_root, database, _ = self.fixture(cookie_overrides={key: override})
            with self.assertRaisesRegex(
                MODULE.ExportFailure,
                "(?:invalid_session_cookie|expired_session_cookie|session_identity_mismatch)",
            ):
                MODULE._read_task(source_root, database, 67, max_age_hours=1, require_root=False)

    def test_refresh_deadline_uses_earliest_cookie_and_clamps_to_now(self) -> None:
        now_ms = 2_000_000_000_000
        raw_token = token()
        with mock.patch.object(MODULE.time, "time", return_value=now_ms / 1000):
            future_due = MODULE._parse_session(
                self.session_bytes(
                    raw_token,
                    [
                        (now_ms + 72 * 3600 * 1000) / 1000,
                        now_ms + 48 * 3600 * 1000,
                        now_ms + 96 * 3600 * 1000,
                        now_ms + 60 * 3600 * 1000,
                    ],
                ),
                raw_token,
                "fixture@example.test",
            )
            self.assertEqual(future_due, now_ms + 24 * 3600 * 1000)
            immediate_due = MODULE._parse_session(
                self.session_bytes(raw_token, [now_ms + 12 * 3600 * 1000] * 4),
                raw_token,
                "fixture@example.test",
            )
            self.assertEqual(immediate_due, now_ms)

    def test_stdout_must_be_a_fifo(self) -> None:
        with tempfile.NamedTemporaryFile() as output:
            original = MODULE.sys.stdout
            try:
                MODULE.sys.stdout = output  # type: ignore[assignment]
                with self.assertRaisesRegex(MODULE.ExportFailure, "pipe_required"):
                    MODULE._require_pipe_stdout()
            finally:
                MODULE.sys.stdout = original
        read_descriptor, write_descriptor = os.pipe()
        try:
            with os.fdopen(write_descriptor, "w") as writer:
                original = MODULE.sys.stdout
                try:
                    MODULE.sys.stdout = writer
                    MODULE._require_pipe_stdout()
                finally:
                    MODULE.sys.stdout = original
        finally:
            os.close(read_descriptor)

    def test_token_shape_rejects_auth_claims(self) -> None:
        payload = base64.urlsafe_b64encode(
            json.dumps({"session_id": "x", "email": "secret@example.test"}).encode()
        ).decode().rstrip("=")
        bad = f"a.{payload}.c"
        with self.assertRaisesRegex(MODULE.ExportFailure, "invalid_console_sso"):
            MODULE._parse_sso(bad.encode())


if __name__ == "__main__":
    unittest.main()
