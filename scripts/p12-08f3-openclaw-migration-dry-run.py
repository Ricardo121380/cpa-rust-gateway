#!/usr/bin/env python3
"""Rehearse an OpenClaw CPAR provider migration without touching live state."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
from typing import Any


class DryRunError(RuntimeError):
    """A bounded, value-free migration rehearsal failure."""


def read_secure_regular(path: Path) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise DryRunError("source_config_must_be_regular_file") from exc
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode):
            raise DryRunError("source_config_must_be_regular_file")
        if metadata.st_mode & 0o077:
            raise DryRunError("source_config_permissions_too_broad")
        with os.fdopen(descriptor, "rb", closefd=False) as handle:
            return handle.read()
    finally:
        os.close(descriptor)


def load_object(raw: bytes) -> dict[str, Any]:
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise DryRunError("source_config_invalid_json") from exc
    if not isinstance(value, dict):
        raise DryRunError("source_config_not_object")
    return value


def provider_at(config: dict[str, Any], provider_id: str) -> dict[str, Any]:
    models = config.get("models")
    providers = models.get("providers") if isinstance(models, dict) else None
    provider = providers.get(provider_id) if isinstance(providers, dict) else None
    if not isinstance(provider, dict):
        raise DryRunError("provider_not_found")
    return provider


def first_model(provider: dict[str, Any]) -> dict[str, Any]:
    models = provider.get("models")
    if not isinstance(models, list) or not models or not isinstance(models[0], dict):
        raise DryRunError("provider_model_not_found")
    model = models[0]
    if not isinstance(model.get("id"), str) or not model["id"]:
        raise DryRunError("provider_model_id_invalid")
    return model


def replace_model_reference(config: dict[str, Any], old_ref: str, new_ref: str) -> None:
    agents = config.get("agents")
    defaults = agents.get("defaults") if isinstance(agents, dict) else None
    if not isinstance(defaults, dict):
        raise DryRunError("agent_defaults_not_found")

    aliases = defaults.get("models")
    if not isinstance(aliases, dict) or old_ref not in aliases:
        raise DryRunError("active_model_alias_not_found")
    aliases[new_ref] = aliases.pop(old_ref)

    selection = defaults.get("model")
    if not isinstance(selection, dict):
        raise DryRunError("agent_model_selection_not_found")
    if selection.get("primary") == old_ref:
        selection["primary"] = new_ref
    fallbacks = selection.get("fallbacks")
    if isinstance(fallbacks, list):
        selection["fallbacks"] = [new_ref if item == old_ref else item for item in fallbacks]


def migrated_copy(source: dict[str, Any], provider_id: str) -> dict[str, Any]:
    # Round-trip through JSON so no reference in the source object can be mutated.
    candidate = json.loads(json.dumps(source))
    provider = provider_at(candidate, provider_id)
    model = first_model(provider)
    old_ref = f"{provider_id}/{model['id']}"
    new_model_id = "cpar-dry-run-alias"
    new_ref = f"{provider_id}/{new_model_id}"

    provider["baseUrl"] = "http://127.0.0.1:9/v1"
    provider["apiKey"] = "rgw_dry_run_not_a_credential"
    model["id"] = new_model_id
    model["name"] = "CPAR dry-run alias"
    replace_model_reference(candidate, old_ref, new_ref)
    return candidate


def protocol_projection(config: dict[str, Any], provider_id: str) -> tuple[Any, Any]:
    provider = provider_at(config, provider_id)
    model = first_model(provider)
    return provider.get("api"), model.get("api")


def validate(config_path: Path, state_dir: Path, validator: str) -> None:
    env = os.environ.copy()
    env["OPENCLAW_CONFIG_PATH"] = str(config_path)
    env["OPENCLAW_STATE_DIR"] = str(state_dir)
    try:
        result = subprocess.run(
            [validator, "config", "validate", "--json"],
            env=env,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise DryRunError("openclaw_validator_unavailable") from exc
    if result.returncode != 0:
        raise DryRunError("openclaw_validation_failed")
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise DryRunError("openclaw_validation_output_invalid") from exc
    if not isinstance(payload, dict) or payload.get("valid") is not True:
        raise DryRunError("openclaw_config_rejected")


def run(source: Path, provider_id: str, validator: str) -> None:
    original = read_secure_regular(source)
    source_object = load_object(original)
    provider_at(source_object, provider_id)
    source_protocol = protocol_projection(source_object, provider_id)

    with tempfile.TemporaryDirectory(prefix="cpar-openclaw-f3-") as temp_name:
        temp_dir = Path(temp_name)
        state_dir = temp_dir / "state"
        state_dir.mkdir(mode=0o700)
        config_copy = temp_dir / "openclaw.json"
        config_copy.write_bytes(original)
        config_copy.chmod(0o600)

        validate(config_copy, state_dir, validator)

        candidate = migrated_copy(source_object, provider_id)
        config_copy.write_text(json.dumps(candidate, indent=2) + "\n", encoding="utf-8")
        config_copy.chmod(0o600)
        validate(config_copy, state_dir, validator)

        reloaded = load_object(config_copy.read_bytes())
        migrated_provider = provider_at(reloaded, provider_id)
        migrated_model = first_model(migrated_provider)
        if not str(migrated_provider.get("apiKey", "")).startswith("rgw_"):
            raise DryRunError("migrated_key_shape_invalid")
        if migrated_model.get("id") != "cpar-dry-run-alias":
            raise DryRunError("migrated_alias_missing")
        if protocol_projection(reloaded, provider_id) != source_protocol:
            raise DryRunError("protocol_selection_changed")

        config_copy.write_bytes(original)
        config_copy.chmod(0o600)
        validate(config_copy, state_dir, validator)
        if config_copy.read_bytes() != original:
            raise DryRunError("temporary_rollback_mismatch")

    if read_secure_regular(source) != original:
        raise DryRunError("source_config_changed")

    print("baseline_validation=PASS")
    print("temporary_migration=PASS")
    print("protocol_selection_preserved=PASS")
    print("temporary_rollback=PASS")
    print("source_config_unchanged=PASS")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--provider-id", required=True)
    parser.add_argument("--validator", default=shutil.which("openclaw") or "openclaw")
    args = parser.parse_args()
    try:
        run(args.source.expanduser(), args.provider_id, args.validator)
    except (DryRunError, FileNotFoundError, PermissionError) as exc:
        label = str(exc) if isinstance(exc, DryRunError) else "source_config_unavailable"
        print(f"openclaw_migration_dry_run=FAIL category={label}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
