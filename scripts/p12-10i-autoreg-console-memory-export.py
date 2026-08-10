#!/usr/bin/env python3
"""Export one Autoreg Console SSO through the CPAR bounded stdin contract.

The Autoreg project is intentionally not a CPAR runtime dependency.  This bridge is a
root-only, one-way source adapter: it reads one completed task's authenticated session,
normalizes it to the existing ``grok_console`` NDJSON envelope, and writes the credential
only to stdout so the caller can pipe it directly into ``gateway admin grok-import``.
No source path, account identity, cookie, or token is written to diagnostics.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import errno
import json
import math
import os
import sqlite3
import stat
import sys
import time
from pathlib import Path
from typing import Any


DEFAULT_SOURCE_ROOT = Path("/opt/example-autoreg/project")
DEFAULT_DATABASE = DEFAULT_SOURCE_ROOT / "apps/console/runtime/console.db"
MAX_TOKEN_BYTES = 16 * 1024
MAX_SESSION_BYTES = 256 * 1024
MAX_TASK_ID_BYTES = 64
MAX_IDENTITY_BYTES = 1024
MAX_AGE_HOURS = 168
ALLOWED_CONSOLE_MODELS = frozenset(
    {
        "grok-4.3",
        "grok-4.20-0309",
        "grok-4.20-0309-reasoning",
        "grok-4.20-0309-non-reasoning",
        "grok-4.20-multi-agent-0309",
        "grok-build-0.1",
    }
)
ALLOWED_SESSION_KEYS = frozenset({"email", "cookies"})
ALLOWED_COOKIE_KEYS = frozenset(
    {"domain", "expires", "httpOnly", "name", "path", "sameSite", "secure", "value"}
)
ALLOWED_COOKIE_DOMAINS = frozenset({".x.ai", ".grok.com"})
COOKIE_NAMES = frozenset({"sso", "sso-rw"})
REFRESH_LEAD_MS = 24 * 60 * 60 * 1000


class ExportFailure(Exception):
    """A fixed-category failure that never contains source values."""


def fail(category: str) -> None:
    raise ExportFailure(category)


def _direct_path(path: Path, *, directory: bool, require_root: bool) -> os.stat_result:
    if not path.is_absolute():
        fail("unsafe_source_path")
    try:
        metadata = os.lstat(path)
    except OSError:
        fail("source_unavailable")
    if stat.S_ISLNK(metadata.st_mode):
        fail("unsafe_source_path")
    if (directory and not stat.S_ISDIR(metadata.st_mode)) or (
        not directory and not stat.S_ISREG(metadata.st_mode)
    ):
        fail("unsafe_source_path")
    if metadata.st_mode & 0o022 or (require_root and metadata.st_uid != 0):
        fail("unsafe_source_path")
    return metadata


def _read_direct_file(
    path: Path,
    *,
    require_root: bool,
    maximum: int,
    expected: os.stat_result | None = None,
) -> bytes:
    """Read one regular file with O_NOFOLLOW and an fstat/read bound."""

    if not path.is_absolute():
        fail("unsafe_source_path")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail("unsafe_source_path" if error.errno == errno.ELOOP else "source_unavailable")
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode):
            fail("unsafe_source_path")
        if expected is not None and (
            metadata.st_dev != expected.st_dev or metadata.st_ino != expected.st_ino
        ):
            fail("source_changed")
        if metadata.st_mode & 0o022 or (require_root and metadata.st_uid != 0):
            fail("unsafe_source_path")
        if metadata.st_size > maximum:
            fail("source_too_large")
        chunks: list[bytes] = []
        total = 0
        try:
            while True:
                chunk = os.read(descriptor, min(8192, maximum + 1 - total))
                if not chunk:
                    break
                chunks.append(chunk)
                total += len(chunk)
                if total > maximum:
                    fail("source_too_large")
        except OSError:
            fail("source_unavailable")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def _safe_child(parent: Path, child: Path) -> Path:
    """Resolve a database-declared path without allowing it outside source root."""

    try:
        root = Path(os.path.abspath(parent))
        candidate = Path(os.path.abspath(child))
        if os.path.commonpath((str(root), str(candidate))) != str(root):
            fail("unsafe_source_path")
        relative_parts = candidate.relative_to(root).parts
        current = root
        for part in relative_parts:
            current /= part
            if os.path.lexists(current) and stat.S_ISLNK(os.lstat(current).st_mode):
                fail("unsafe_source_path")
    except (OSError, ValueError):
        fail("unsafe_source_path")
    return candidate


def _normalize_task_path(source_root: Path, database_task_dir: Any) -> Path:
    if not isinstance(database_task_dir, str) or not database_task_dir.startswith("/workspace/"):
        fail("unsupported_task_path")
    relative = database_task_dir.removeprefix("/workspace/")
    if not relative or "\x00" in relative:
        fail("unsupported_task_path")
    return _safe_child(source_root, source_root / relative)


def _normalize_model(value: str) -> str:
    if not isinstance(value, str):
        fail("unsupported_console_model")
    model = value.strip()
    if model.casefold().startswith("console/"):
        model = model.split("/", 1)[1].strip()
    elif any(model.casefold().startswith(prefix) for prefix in ("build/", "web/")):
        fail("unsupported_console_model")
    if model not in ALLOWED_CONSOLE_MODELS:
        fail("unsupported_console_model")
    return model


def _parse_sso(raw: bytes) -> str:
    if not raw or len(raw) > MAX_TOKEN_BYTES or b"\x00" in raw:
        fail("invalid_console_sso")
    try:
        token = raw.decode("utf-8", "strict").strip()
    except UnicodeDecodeError:
        fail("invalid_console_sso")
    if token.startswith("sso="):
        token = token[4:]
    if not token or any(ord(char) < 0x20 or ord(char) == 0x7F for char in token):
        fail("invalid_console_sso")
    if ";" in token or "\n" in token or "\r" in token:
        fail("invalid_console_sso")
    parts = token.split(".")
    if len(parts) < 2:
        fail("invalid_console_sso")
    try:
        payload = json.loads(base64.urlsafe_b64decode(parts[1] + "=" * (-len(parts[1]) % 4)))
    except (ValueError, binascii.Error, UnicodeDecodeError, json.JSONDecodeError):
        fail("invalid_console_sso")
    if not isinstance(payload, dict) or not isinstance(payload.get("session_id"), str):
        fail("invalid_console_sso")
    if any(key in payload for key in ("access_token", "refresh_token", "password", "email")):
        fail("invalid_console_sso")
    return token


def _normalize_identity(value: Any) -> str:
    if not isinstance(value, str) or value.strip() != value:
        fail("invalid_source_identity")
    normalized = value.casefold()
    if (
        not normalized
        or len(normalized.encode("utf-8")) > MAX_IDENTITY_BYTES
        or any(ord(char) < 0x20 or ord(char) == 0x7F for char in normalized)
    ):
        fail("invalid_source_identity")
    return normalized


def _cookie_expiry_ms(value: Any, now_ms: int) -> int:
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        fail("invalid_session_expiry")
    try:
        numeric = float(value)
    except (OverflowError, TypeError, ValueError):
        fail("invalid_session_expiry")
    if not math.isfinite(numeric):
        fail("invalid_session_expiry")
    seconds = numeric / 1000 if numeric > 10_000_000_000 else numeric
    expiry_ms = int(seconds * 1000)
    if expiry_ms > 2**63 - 1:
        fail("invalid_session_expiry")
    if expiry_ms <= now_ms + 5 * 60 * 1000:
        fail("expired_session_cookie")
    return expiry_ms


def _parse_session(raw: bytes, token: str, expected_email: str) -> int:
    if len(raw) > MAX_SESSION_BYTES:
        fail("source_too_large")
    matches = 0
    refresh_due_ms: int | None = None
    now_ms = int(time.time() * 1000)
    for line in raw.splitlines():
        try:
            document = json.loads(line)
        except (UnicodeDecodeError, json.JSONDecodeError):
            fail("invalid_session_document")
        if not isinstance(document, dict) or set(document) - ALLOWED_SESSION_KEYS:
            fail("invalid_session_document")
        if not isinstance(document.get("cookies"), list):
            fail("invalid_session_document")
        if _normalize_identity(document.get("email")) != expected_email:
            continue
        cookies: dict[tuple[str, str], tuple[str, int]] = {}
        for cookie in document["cookies"]:
            if not isinstance(cookie, dict) or set(cookie) - ALLOWED_COOKIE_KEYS:
                fail("invalid_session_document")
            name, value, domain = cookie.get("name"), cookie.get("value"), cookie.get("domain")
            if name not in COOKIE_NAMES or domain not in ALLOWED_COOKIE_DOMAINS:
                continue
            if not isinstance(value, str) or cookie.get("secure") is not True or cookie.get("httpOnly") is not True:
                fail("invalid_session_cookie")
            if cookie.get("path") != "/":
                fail("invalid_session_cookie")
            expiry_ms = _cookie_expiry_ms(cookie.get("expires"), now_ms)
            key = (domain, name)
            if key in cookies:
                fail("ambiguous_session_cookie")
            cookies[key] = (value, expiry_ms)
        required = {
            (domain, name)
            for domain in (".x.ai", ".grok.com")
            for name in COOKIE_NAMES
        }
        if required.issubset(cookies) and all(cookies[key][0] == token for key in required):
            matches += 1
            candidate_due = min(cookies[key][1] for key in required) - REFRESH_LEAD_MS
            # Schedule refresh before the earliest cookie expires.  If the
            # lead time has already elapsed, make it immediately due; do not
            # turn every otherwise-valid future session into an immediate
            # refresh job.
            refresh_due_ms = max(now_ms, candidate_due)
    if matches != 1 or refresh_due_ms is None:
        fail("session_identity_mismatch")
    return refresh_due_ms


def _read_task(
    source_root: Path,
    database: Path,
    task_id: int,
    *,
    max_age_hours: int,
    require_root: bool = True,
) -> tuple[str, str, int, Path]:
    database_metadata = _direct_path(database, directory=False, require_root=require_root)
    try:
        connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
        row = connection.execute(
            "SELECT status,target_count,completed_count,failed_count,last_email,task_dir "
            "FROM tasks WHERE id = ?",
            (task_id,),
        ).fetchone()
        try:
            current_database = os.lstat(database)
        except OSError:
            fail("source_changed")
        if (
            current_database.st_dev != database_metadata.st_dev
            or current_database.st_ino != database_metadata.st_ino
        ):
            fail("source_changed")
    except sqlite3.Error:
        fail("source_database_unavailable")
    finally:
        if "connection" in locals():
            connection.close()
    if row is None:
        fail("task_unavailable")
    status, target, completed, failed, email, task_dir = row
    if (
        status != "completed"
        or target != 1
        or completed != 1
        or failed != 0
        or not isinstance(email, str)
    ):
        fail("task_not_eligible")
    identity = _normalize_identity(email)
    if len(str(task_id).encode()) > MAX_TASK_ID_BYTES:
        fail("task_unavailable")
    directory = _normalize_task_path(source_root, task_dir)
    _direct_path(directory, directory=True, require_root=require_root)
    sso_directory = directory / "sso"
    _direct_path(sso_directory, directory=True, require_root=require_root)
    entries = []
    try:
        with os.scandir(sso_directory) as iterator:
            for entry in iterator:
                if entry.name.endswith(".txt"):
                    entries.append(entry.name)
    except OSError:
        fail("source_unavailable")
    if len(entries) != 1:
        fail("ambiguous_sso_source")
    sso_path = _safe_child(source_root, sso_directory / entries[0])
    metadata = _direct_path(sso_path, directory=False, require_root=require_root)
    age = time.time() - metadata.st_mtime
    if age < -300 or age > max_age_hours * 3600:
        fail("stale_sso_source")
    token = _parse_sso(
        _read_direct_file(
            sso_path,
            require_root=require_root,
            maximum=MAX_TOKEN_BYTES,
            expected=metadata,
        )
    )
    session_path = directory / "keys" / "auth-sessions.jsonl"
    session_metadata = _direct_path(session_path, directory=False, require_root=require_root)
    session_raw = _read_direct_file(
        session_path,
        require_root=require_root,
        maximum=MAX_SESSION_BYTES,
        expected=session_metadata,
    )
    refresh_due_ms = _parse_session(session_raw, token, identity)
    return token, identity, refresh_due_ms, directory


def _require_pipe_stdout() -> None:
    try:
        mode = os.fstat(sys.stdout.fileno()).st_mode
    except (AttributeError, OSError):
        fail("pipe_required")
    if not stat.S_ISFIFO(mode):
        fail("pipe_required")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, default=DEFAULT_SOURCE_ROOT)
    parser.add_argument("--database", type=Path, default=DEFAULT_DATABASE)
    parser.add_argument("--task-id", type=int, required=True)
    parser.add_argument("--probe-model", required=True)
    parser.add_argument("--max-age-hours", type=int, default=MAX_AGE_HOURS)
    arguments = parser.parse_args()
    if os.geteuid() != 0:
        fail("root_required")
    _require_pipe_stdout()
    if arguments.task_id < 1 or not 1 <= arguments.max_age_hours <= 24 * 30:
        fail("invalid_arguments")
    source_root = arguments.source_root
    _direct_path(source_root, directory=True, require_root=True)
    model = _normalize_model(arguments.probe_model)
    token, identity, refresh_due_ms, _directory = _read_task(
        source_root, arguments.database, arguments.task_id, max_age_hours=arguments.max_age_hours
    )
    # The source identity is carried only through the root-only pipe and is hashed immediately
    # by CPAR; it is never rendered in diagnostics or persisted as text.
    source_ref = f"autoreg-console-task-{arguments.task_id}"
    record = {
        "kind": "account",
        "source_ref": source_ref,
        "provider": "grok_console",
        "identity_key": identity,
        "credential": json.dumps({"sso_token": token, "probe_model": model}, separators=(",", ":")),
        "auth_status": "active",
        "enabled": True,
        "priority": 101,
        "weight": 1,
        "max_concurrency": 1,
        "refresh_due_at_ms": refresh_due_ms,
        "quota_sync_due_at_ms": None,
        "cooldown_until_ms": None,
    }
    print(json.dumps(record, separators=(",", ":")), flush=True)
    print(
        "source_export=PASS provider=grok_console source_count=1 emitted_count=1 "
        "model_source=allowlisted_pending_native_probe",
        file=sys.stderr,
        flush=True,
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ExportFailure as error:
        print(f"source_export=FAIL category={error}", file=sys.stderr)
        raise SystemExit(1) from None
