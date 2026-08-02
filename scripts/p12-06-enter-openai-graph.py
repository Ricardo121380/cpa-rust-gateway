#!/usr/bin/env python3
"""Enter and publish the P12-06 OpenAI-compatible differential graph.

The enter phase reads exactly three NUL-delimited values from stdin: HTTPS base URL, effective
Bearer, and upstream model. Values never appear in argv or output. Finish and Explain are separate
post-restart phases because endpoint capability profiles are assembled at process start.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import sys
from urllib.parse import quote, urlsplit


def load_shared():
    path = Path(__file__).with_name("p12-06-enter-kiro-graph.py")
    spec = importlib.util.spec_from_file_location("p12_06_graph_shared", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load shared graph helper")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


shared = load_shared()
Management = shared.Management
ManagementError = shared.ManagementError
Ledger = shared.Ledger
read_secret = shared.read_secret

IDS = {
    "egress_policy": "p12-06-openai-egress",
    "upstream": "p12-06-openai-upstream",
    "endpoint": "p12-06-openai-endpoint",
    "credential": "p12-06-openai-credential",
    "public_model": "p12-06-openai-model",
    "route": "p12-06-openai-route",
    "candidate": "p12-06-openai-candidate",
    "access_group": "p12-06-openai-group",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--management-key-file", required=True)
    parser.add_argument("--version-id", default="p12-06-openai-v1")
    parser.add_argument("--parent-version-id")
    parser.add_argument("--public-model", default="p12-openai-differential")
    parser.add_argument("--client-key-id", default="p12-06-openai-client-key")
    parser.add_argument("--ledger")
    parser.add_argument("--client-key-out")
    parser.add_argument("--management-base", default=shared.MANAGEMENT_BASE_DEFAULT)
    parser.add_argument("--finish", action="store_true")
    parser.add_argument("--publish", action="store_true")
    parser.add_argument("--explain-only", action="store_true")
    args = parser.parse_args()
    if args.finish and args.explain_only:
        parser.error("--finish and --explain-only are separate phases")
    if args.parent_version_id == args.version_id:
        parser.error("parent and successor version ids must differ")
    if not (args.finish or args.explain_only):
        missing = [name for name in ("ledger", "client_key_out") if not getattr(args, name)]
        if missing:
            parser.error("enter requires " + ", ".join("--" + x.replace("_", "-") for x in missing))
    return args


def read_inputs() -> tuple[str, str, str, str, int]:
    raw = sys.stdin.buffer.read(131073)
    if len(raw) > 131072:
        raise ManagementError("enter input exceeds bound")
    parts = raw.split(b"\0")
    if parts and parts[-1] == b"":
        parts.pop()
    if len(parts) != 3:
        raise ManagementError("enter requires three NUL-delimited values")
    try:
        base_url, bearer, upstream_model = (part.decode("utf-8") for part in parts)
    except UnicodeDecodeError as error:
        raise ManagementError("enter input is not UTF-8") from error
    parsed = urlsplit(base_url)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
        or not bearer
        or not upstream_model
        or len(bearer) > 8192
        or len(upstream_model) > 256
    ):
        raise ManagementError("enter input failed closed validation")
    port = parsed.port or 443
    base_path = parsed.path.rstrip("/")
    normalized = f"https://{parsed.hostname}" + (f":{port}" if port != 443 else "") + base_path
    return normalized, bearer, upstream_model, parsed.hostname, port


def call(session, ledger, key: str, method: str, path: str, payload: dict, expect: int = 201):
    result = session.call(method, path, payload, expect=expect)
    ledger.record(key, payload.get("id", True))
    return result


def enter(args: argparse.Namespace) -> int:
    base_url, bearer, upstream_model, host, port = read_inputs()
    ledger = Ledger(args.ledger)
    session = Management(read_secret(args.management_key_file), base=args.management_base)
    version_payload: dict[str, object] = {
        "id": args.version_id,
        "description": "P12-06 OpenAI-compatible live differential",
    }
    if args.parent_version_id:
        version_payload["parent_id"] = args.parent_version_id
    version = session.call("POST", "/admin/config-versions", version_payload, send_config_version=False)
    session.config_version = args.version_id
    session.revision = version.get("revision")
    ledger.record("config_version_id", args.version_id)
    ledger.record("initial_revision", session.revision)

    call(session, ledger, "egress_policy_id", "POST", "/admin/egress-policies", {
        "id": IDS["egress_policy"], "name": "P12-06 OpenAI differential egress",
        "allowed_schemes": ["https"], "allowed_hosts": [host], "allowed_ports": [port],
        "allowed_cidrs": [], "redirect_mode": "deny", "max_redirects": 0,
    })
    call(session, ledger, "upstream_id", "POST", "/admin/upstreams", {
        "id": IDS["upstream"], "name": "P12-06 OpenAI-compatible upstream",
        "kind": "openai-compatible", "enabled": True, "tags": ["p12-06"],
        "egress_policy_id": IDS["egress_policy"],
    })
    call(session, ledger, "endpoint_id", "POST", f"/admin/upstreams/{IDS['upstream']}/endpoints", {
        "id": IDS["endpoint"], "adapter_id": "openai-compatible.responses",
        "api_format": "openai/responses", "base_url": base_url,
        "inference_path": "/responses", "transport": "https", "enabled": True,
    })
    call(session, ledger, "credential_id", "POST", f"/admin/upstreams/{IDS['upstream']}/credentials", {
        "id": IDS["credential"], "kind": "bearer", "secret": bearer, "status": "active",
    })
    call(session, ledger, "binding", "POST", f"/admin/endpoints/{IDS['endpoint']}/credential-bindings", {
        "credential_id": IDS["credential"], "enabled": True, "priority": 0, "weight": 1,
        "concurrency": 1,
    })
    call(session, ledger, "public_model_id", "POST", "/admin/public-models", {
        "id": IDS["public_model"], "model_name": args.public_model, "status": "active",
        "display_name": args.public_model, "capabilities": {},
    })
    call(session, ledger, "route_id", "POST", f"/admin/public-models/{IDS['public_model']}/routes", {
        "id": IDS["route"], "policy": "smooth_weighted_round_robin", "max_attempts": 1,
        "bootstrap_timeout_ms": 15000,
    })
    call(session, ledger, "candidate_id", "POST", f"/admin/routes/{IDS['route']}/candidates", {
        "id": IDS["candidate"], "endpoint_id": IDS["endpoint"],
        "upstream_model": upstream_model, "credential_scope": "all_active",
        "transform_mode": "canonical", "enabled": True, "priority": 0, "weight": 1,
        "capability_override": {"allow_unlisted_model": True},
    })
    call(session, ledger, "access_group_id", "POST", "/admin/access-groups", {
        "id": IDS["access_group"], "name": "P12-06 OpenAI differential",
        "status": "active", "limits": {},
    })
    call(session, ledger, "access_group_route", "POST", f"/admin/access-groups/{IDS['access_group']}/routes", {
        "route_id": IDS["route"], "enabled": True,
    }, expect=201)
    issued = session.call("POST", "/admin/client-keys", {
        "id": args.client_key_id, "access_group_id": IDS["access_group"], "status": "active",
    })
    secret = issued.get("key")
    if not isinstance(secret, str) or not secret:
        raise ManagementError("client key response omitted the one-time key")
    descriptor = os.open(args.client_key_out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        handle.write(secret)
    ledger.record("client_key_id", args.client_key_id)
    ledger.record("client_key_prefix", issued.get("prefix"))
    print("openai-graph: entered; restart, then run --finish --publish")
    return 0


def finish(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), config_version=args.version_id, base=args.management_base)
    ledger = Ledger(args.ledger, load_existing=True) if args.ledger else None
    session.call("POST", f"/admin/routes/{IDS['route']}/validate", {}, expect=200)
    validated = session.call("POST", f"/admin/config-versions/{args.version_id}/validate", {}, expect=200)
    if not validated.get("valid"):
        raise ManagementError("config version did not validate")
    if ledger:
        ledger.record("version_validate", validated)
    if not args.publish:
        print("openai-graph: validated but not published")
        return 0
    current = session.call("GET", f"/admin/config-versions/{args.version_id}", None, expect=200,
                           send_config_version=False, send_if_match=False)
    session.revision = current.get("revision")
    if not session.revision:
        raise ManagementError("config version omitted revision")
    published = session.call("POST", f"/admin/config-versions/{args.version_id}/publish", {}, expect=200,
                             send_config_version=False)
    if ledger:
        ledger.record("published", published)
    print("openai-graph: published; restart, then run --explain-only")
    return 0


def explain(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), config_version=args.version_id, base=args.management_base)
    result = session.call(
        "GET", f"/admin/routes/{IDS['route']}/explain?requested_model={quote(args.public_model, safe='')}&protocol=openai_responses",
        None, expect=200, send_if_match=False,
    )
    candidates = result.get("candidates")
    selected = isinstance(candidates, list) and len(candidates) == 1 and candidates[0].get("decision") == "selected"
    print(json.dumps({"single_selected_candidate": selected}, sort_keys=True))
    return 0 if selected else 1


def main() -> int:
    args = parse_args()
    try:
        if args.explain_only:
            return explain(args)
        if args.finish:
            return finish(args)
        return enter(args)
    except (ManagementError, OSError, ValueError) as error:
        print(f"openai-graph: {type(error).__name__}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
