#!/usr/bin/env python3
"""Emit the three private grok2api graph inputs to stdout as a NUL stream.

This server-side bridge deliberately has no diagnostic value output. It selects one enabled
Console Responses route that has the newest successful audit, verifies the Docker publication is
IPv4-loopback-only, reads the existing client key by reference, and writes base/key/model directly
to the graph helper's stdin.
"""

from __future__ import annotations

import json
from pathlib import Path
import sqlite3
import subprocess
import sys


DATABASE = Path("/opt/example-grok-adapter/data/backend.db")
KEY_FILE = Path("/opt/example-grok-adapter/client-api-key")


def fail(category: str) -> int:
    print(f"p12-10-grok2api-input=FAIL category={category}", file=sys.stderr)
    return 1


def main() -> int:
    try:
        inspected = json.loads(subprocess.check_output(["docker", "inspect", "grok2api"]))
        if not isinstance(inspected, list) or len(inspected) != 1:
            return fail("container_shape")
        bindings = []
        for entries in inspected[0]["NetworkSettings"]["Ports"].values():
            bindings.extend(entries or [])
        if len(bindings) != 1 or bindings[0].get("HostIp") != "127.0.0.1":
            return fail("publication_not_exact_loopback")
        port = int(bindings[0]["HostPort"])
        if not 1 <= port <= 65535:
            return fail("invalid_loopback_port")

        with sqlite3.connect(f"file:{DATABASE}?mode=ro", uri=True) as connection:
            row = connection.execute(
                """
                SELECT route.upstream_model
                  FROM model_routes AS route
             LEFT JOIN request_audits AS audit
                    ON audit.model_route_id = route.id
                   AND audit.status_code BETWEEN 200 AND 299
                 WHERE route.enabled = 1
                   AND route.provider = 'grok_console'
                   AND route.capability = 'responses'
              GROUP BY route.id, route.upstream_model
              ORDER BY MAX(audit.created_at) IS NULL, MAX(audit.created_at) DESC, route.id
                 LIMIT 1
                """
            ).fetchone()
        if row is None or not isinstance(row[0], str) or not row[0] or len(row[0]) > 256:
            return fail("eligible_route_missing")
        key = KEY_FILE.read_bytes().strip()
        if not key or len(key) > 8192 or b"\0" in key:
            return fail("client_key_invalid")
        model = row[0].encode("utf-8")
        base = f"http://127.0.0.1:{port}/v1".encode("ascii")
        sys.stdout.buffer.write(base + b"\0" + key + b"\0" + model + b"\0")
        return 0
    except (OSError, ValueError, KeyError, json.JSONDecodeError, sqlite3.Error, subprocess.SubprocessError):
        return fail("input_source_unavailable")


if __name__ == "__main__":
    raise SystemExit(main())
