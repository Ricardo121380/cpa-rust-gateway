#!/usr/bin/env python3
"""Apply the Oracle-only Autoreg safety envelope before the stack starts.

The Jakarta project tree is copied byte-for-byte, so its config can still point
at the Jakarta proxy ports or at the legacy CPA management API.  This small,
dependency-free pre-start hook makes the Oracle boundary explicit and idempotent:
the browser dependencies use the Oracle loopback sidecars and registration
credentials are written only to the mounted, dedicated auth directory.  It never
prints or rewrites secret fields.
"""

from __future__ import annotations

import json
import os
import shutil
import tempfile
from pathlib import Path


CONFIG_PATH = Path(
    os.environ.get("AUTOREG_ORACLE_CONFIG", "/opt/example-autoreg/project/config.json")
)
BACKUP_PATH = Path(
    os.environ.get(
        "AUTOREG_ORACLE_CONFIG_PREIMAGE",
        "/var/backups/autoreg-oracle/oracle-config-pre-oracle-policy.json",
    )
)


def main() -> None:
    if not CONFIG_PATH.is_file():
        raise SystemExit(f"missing Autoreg config: {CONFIG_PATH}")
    with CONFIG_PATH.open(encoding="utf-8") as handle:
        config = json.load(handle)
    if not isinstance(config, dict):
        raise SystemExit("Autoreg config must be a JSON object")

    changed = False

    def set_value(mapping: dict, key: str, value: object) -> None:
        nonlocal changed
        if mapping.get(key) != value:
            mapping[key] = value
            changed = True

    cpa = config.setdefault("cpa", {})
    if not isinstance(cpa, dict):
        raise SystemExit("Autoreg cpa config must be an object")
    set_value(cpa, "prefer", "dir")
    set_value(cpa, "using_api", False)
    set_value(cpa, "auth_dir", "/opt/example-legacy-gateway/cpa/auths")

    # These keys are consumed by the migrated browser runner.  Keep the values
    # loopback-only so a copied Jakarta config cannot send the Oracle canary to a
    # stale port or a public listener.
    set_value(config, "proxy", "http://127.0.0.1:24080")
    set_value(config, "browser_proxy", "http://127.0.0.1:24080")
    set_value(config, "clearance_proxy", "http://127.0.0.1:24080")
    set_value(config, "flaresolverr_url", "http://127.0.0.1:28191")

    api = config.setdefault("api", {})
    if not isinstance(api, dict):
        raise SystemExit("Autoreg api config must be an object")
    # grok2api is not a migration destination or an upstream for Oracle Autoreg.
    set_value(api, "mode", "disabled")

    if changed:
        BACKUP_PATH.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        if not BACKUP_PATH.exists():
            shutil.copy2(CONFIG_PATH, BACKUP_PATH)
            os.chmod(BACKUP_PATH, 0o600)
        fd, tmp_name = tempfile.mkstemp(
            prefix="autoreg-oracle-config.", suffix=".tmp", dir=CONFIG_PATH.parent
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as handle:
                json.dump(config, handle, ensure_ascii=False, indent=2)
                handle.write("\n")
            os.chmod(tmp_name, 0o600)
            os.replace(tmp_name, CONFIG_PATH)
        finally:
            if os.path.exists(tmp_name):
                os.unlink(tmp_name)
    print(f"autoreg_oracle_config=ok changed={str(changed).lower()}")


if __name__ == "__main__":
    main()
