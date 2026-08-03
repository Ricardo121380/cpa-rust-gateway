#!/usr/bin/env python3
"""Root-only grok2api export normalizer for the CPAR bounded stdin importer."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import sqlite3
import stat
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

CONFIG = Path("/opt/example-grok-adapter/config.yaml")
DATABASE = Path("/opt/example-grok-adapter/data/backend.db")
ADMIN_ROOT = "http://127.0.0.1:8000/api/admin/v1"
BUILD_CLIENT_ID = "b1a00492-073a-47ea-816f-4c329264a828"
BUILD_ISSUER = "https://auth.x.ai"
BUILD_SCOPE = "openid profile email offline_access grok-cli:access api:access"
BUILD_PROBE_MODEL = "grok-4.5-build-free"
ALLOWED_PROVIDERS = ("grok_build", "grok_console", "grok_web")


class ExportFailure(Exception):
    """A fixed-category failure whose text never includes source values."""


def fail(category: str) -> None:
    raise ExportFailure(category)


def direct_source_file(path: Path, *, root_owned: bool) -> None:
    if not path.is_absolute() or path.is_symlink():
        fail("unsafe_source_path")
    metadata = path.stat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_mode & 0o022
        or (root_owned and metadata.st_uid != 0)
    ):
        fail("unsafe_source_path")


def utc_ms(value: str | None) -> int | None:
    if not value:
        return None
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        fail("invalid_source_timestamp")
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    return int(parsed.timestamp() * 1000)


def json_request(path: str, *, body: dict[str, Any] | None = None, token: str | None = None) -> tuple[dict[str, Any], Any]:
    headers = {"Accept": "application/json"}
    encoded = None
    if body is not None:
        headers["Content-Type"] = "application/json"
        encoded = json.dumps(body, separators=(",", ":")).encode()
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(
        ADMIN_ROOT + path,
        data=encoded,
        headers=headers,
        method="POST" if body is not None else "GET",
    )
    try:
        with urllib.request.urlopen(request, timeout=15) as response:
            if response.status != 200:
                fail("admin_http_rejected")
            payload = json.load(response)
            return payload, response.headers
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError):
        fail("admin_http_unavailable")


def admin_credentials(config: dict[str, Any]) -> tuple[str, str]:
    try:
        bootstrap = config["bootstrapAdmin"]
        username = bootstrap["username"]
        password = bootstrap["password"]
    except (KeyError, TypeError):
        fail("admin_config_unavailable")
    if not isinstance(username, str) or not username or not isinstance(password, str) or not password:
        fail("admin_config_unavailable")
    return username, password


def login(config: dict[str, Any]) -> str:
    username, password = admin_credentials(config)
    payload, _ = json_request("/auth/login", body={"username": username, "password": password})
    try:
        token = payload["data"]["tokens"]["accessToken"]
    except (KeyError, TypeError):
        fail("admin_login_rejected")
    if not isinstance(token, str) or not token:
        fail("admin_login_rejected")
    return token


def source_rows(provider: str, limit: int) -> tuple[int, list[dict[str, Any]]]:
    uri = f"file:{DATABASE}?mode=ro"
    try:
        connection = sqlite3.connect(uri, uri=True)
        connection.row_factory = sqlite3.Row
        total = connection.execute(
            "SELECT count(*) FROM provider_accounts WHERE provider = ?", (provider,)
        ).fetchone()[0]
        rows = connection.execute(
            """
            SELECT p.identity_key, p.name, p.email, p.user_id, p.team_id,
                   p.auth_status, p.enabled, p.priority, p.max_concurrent,
                   p.cooldown_until, p.observed_model, c.expires_at, c.refresh_due_at
              FROM provider_accounts p
              JOIN account_credentials c ON c.account_id = p.id
             WHERE p.provider = ? AND p.enabled = 1 AND p.auth_status = 'active'
               AND (p.cooldown_until IS NULL OR p.cooldown_until <= CURRENT_TIMESTAMP)
               AND (? != 'grok_build' OR (c.expires_at IS NOT NULL AND c.expires_at > CURRENT_TIMESTAMP))
             ORDER BY p.id
             LIMIT ?
            """,
            (provider, provider, limit),
        ).fetchall()
    except sqlite3.Error:
        fail("source_database_unavailable")
    finally:
        if "connection" in locals():
            connection.close()
    return int(total), [dict(row) for row in rows]


def account_key(account: dict[str, Any]) -> tuple[str, str, str, str]:
    return tuple(str(account.get(name) or "") for name in ("name", "email", "user_id", "team_id"))


def canonical_credential(provider: str, exported: dict[str, Any]) -> str:
    if provider == "grok_build":
        client_id = str(exported.get("client_id") or BUILD_CLIENT_ID)
        document = {
            "access_token": exported.get("access_token"),
            "refresh_token": exported.get("refresh_token"),
            "expires_at": exported.get("expires_at"),
            "client_id": client_id,
            "issuer": BUILD_ISSUER,
            "scope": BUILD_SCOPE,
            "token_type": "Bearer",
        }
        if client_id != BUILD_CLIENT_ID or not all(
            isinstance(document[name], str) and document[name]
            for name in ("access_token", "refresh_token", "expires_at")
        ):
            fail("unsupported_build_credential")
        return json.dumps(document, separators=(",", ":"))
    if provider == "grok_console":
        token = exported.get("sso_token")
        if not isinstance(token, str) or not token:
            fail("unsupported_console_credential")
        return token
    fail("web_expiry_unavailable")


def transfer_records(provider: str, limit: int, exported: dict[str, Any], rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    accounts = exported.get("accounts")
    provider_marker_valid = (
        exported.get("provider") == provider
        if provider != "grok_build"
        else "provider" not in exported
    )
    if not provider_marker_valid or not isinstance(accounts, list):
        fail("invalid_export_document")
    indexed: dict[tuple[str, str, str, str], dict[str, Any]] = {}
    for account in accounts:
        if not isinstance(account, dict):
            fail("invalid_export_document")
        if provider == "grok_build" and account.get("provider") != provider:
            fail("invalid_export_document")
        key = account_key(account)
        if key in indexed:
            fail("ambiguous_export_identity")
        indexed[key] = account
    records = []
    for index, row in enumerate(rows):
        if provider == "grok_build" and row["observed_model"] != BUILD_PROBE_MODEL:
            fail("unsupported_build_model")
        exported_account = indexed.get(account_key(row))
        if exported_account is None:
            fail("source_export_mismatch")
        record = {
            "kind": "account",
            "source_ref": f"{provider}-{index}",
            "provider": provider,
            "identity_key": row["identity_key"],
            "credential": canonical_credential(provider, exported_account),
            "auth_status": "active",
            "enabled": True,
            "priority": int(row["priority"]),
            "weight": 1,
            "max_concurrency": int(row["max_concurrent"]),
            "refresh_due_at_ms": utc_ms(row["refresh_due_at"]),
            "quota_sync_due_at_ms": None,
            "cooldown_until_ms": utc_ms(row["cooldown_until"]),
        }
        records.append(record)
    if len(records) != limit:
        fail("insufficient_active_accounts")
    return records


def main() -> int:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("--provider", choices=ALLOWED_PROVIDERS, required=True)
    parser.add_argument("--limit", type=int, default=1)
    arguments = parser.parse_args()
    if os.geteuid() != 0:
        fail("root_required")
    if sys.stdout.isatty():
        fail("pipe_required")
    if arguments.limit < 1 or arguments.limit > 8:
        fail("invalid_limit")
    if arguments.provider == "grok_web":
        fail("web_expiry_unavailable")
    direct_source_file(CONFIG, root_owned=True)
    # The bind-mounted SQLite file is owned by the container's non-root uid. The fixed path must
    # still be a direct regular file and may not be group/world writable.
    direct_source_file(DATABASE, root_owned=False)
    try:
        import yaml  # type: ignore[import-not-found]
    except ImportError:
        fail("admin_config_unavailable")
    try:
        with CONFIG.open("rb") as handle:
            config = yaml.safe_load(handle)
    except (OSError, yaml.YAMLError):
        fail("admin_config_unavailable")
    if not isinstance(config, dict):
        fail("admin_config_unavailable")
    total, rows = source_rows(arguments.provider, arguments.limit)
    token = login(config)
    exported, headers = json_request(
        f"/accounts/export?provider={arguments.provider}", token=token
    )
    accounts = exported.get("accounts")
    try:
        header_count = int(headers["X-Exported-Accounts"])
    except (KeyError, TypeError, ValueError):
        fail("missing_export_count")
    if not isinstance(accounts, list) or total != header_count or total != len(accounts):
        fail("source_count_mismatch")
    records = transfer_records(arguments.provider, arguments.limit, exported, rows)
    for record in records:
        print(json.dumps(record, separators=(",", ":")), flush=True)
    print(
        f"source_export=PASS provider={arguments.provider} source_count={total} emitted_count={len(records)}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ExportFailure as error:
        print(f"source_export=FAIL category={error}", file=sys.stderr)
        raise SystemExit(1) from None
