#!/usr/bin/env python3
"""Create, publish, explain, or roll back the bounded P12-08G1 Codex graph."""

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
    spec = importlib.util.spec_from_file_location("p12_08g1_graph_shared", path)
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

VERSION_ID = "p12-08g1-codex-v2"
ALIASES = {
    "chat": "p12-g1-codex-chat",
    "responses": "p12-g1-codex-responses",
    "messages": "p12-g1-codex-messages",
}
ROUTES = {name: f"p12-08g1-{name}-route" for name in ALIASES}
CANDIDATES = {name: f"p12-08g1-{name}-candidate" for name in ALIASES}
MODE = {"chat": "canonical", "responses": "canonical", "messages": "lossless_bridge"}
PROTOCOL = {"chat": "openai_chat_completions", "responses": "openai_responses", "messages": "anthropic_messages"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--management-key-file", required=True)
    parser.add_argument("--parent-version-id")
    parser.add_argument("--ledger")
    parser.add_argument("--client-key-out")
    parser.add_argument("--management-base", default=shared.MANAGEMENT_BASE_DEFAULT)
    actions = parser.add_mutually_exclusive_group()
    actions.add_argument("--finish", action="store_true")
    actions.add_argument("--explain-only", action="store_true")
    actions.add_argument("--rollback", action="store_true")
    actions.add_argument("--reactivate", action="store_true")
    args = parser.parse_args()
    if args.reactivate and not args.parent_version_id:
        parser.error("--reactivate requires --parent-version-id")
    if not (args.finish or args.explain_only or args.rollback or args.reactivate):
        missing = [name for name in ("parent_version_id", "ledger", "client_key_out") if not getattr(args, name)]
        if missing:
            parser.error("enter requires " + ", ".join("--" + name.replace("_", "-") for name in missing))
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
    version = session.call(
        "POST",
        "/admin/config-versions",
        {"id": VERSION_ID, "parent_id": args.parent_version_id, "description": "P12-08G1 current Krill graph"},
        send_config_version=False,
    )
    session.config_version = VERSION_ID
    session.revision = version.get("revision")
    ledger.record("config_version_id", VERSION_ID)
    ledger.record("initial_revision", session.revision)

    call(session, ledger, "egress_policy_id", "POST", "/admin/egress-policies", {
        "id": "p12-08g1-egress", "name": "P12-08G1 exact egress", "allowed_schemes": ["https"],
        "allowed_hosts": [host], "allowed_ports": [port], "allowed_cidrs": [],
        "redirect_mode": "deny", "max_redirects": 0,
    })
    call(session, ledger, "upstream_id", "POST", "/admin/upstreams", {
        "id": "p12-08g1-upstream", "name": "P12-08G1 Codex-compatible", "kind": "openai-compatible",
        "enabled": True, "tags": ["p12-08g1"], "egress_policy_id": "p12-08g1-egress",
    })
    for name, adapter, api_format, inference_path in (
        ("chat", "openai-compatible.chat-completions", "openai/chat-completions", "/chat/completions"),
        ("responses", "openai-compatible.responses", "openai/responses", "/responses"),
    ):
        call(session, ledger, f"{name}_endpoint_id", "POST", "/admin/upstreams/p12-08g1-upstream/endpoints", {
            "id": f"p12-08g1-{name}-endpoint", "adapter_id": adapter, "api_format": api_format,
            "base_url": base_url, "inference_path": inference_path, "transport": "https", "enabled": True,
        })
    call(session, ledger, "credential_id", "POST", "/admin/upstreams/p12-08g1-upstream/credentials", {
        "id": "p12-08g1-credential", "kind": "bearer", "secret": bearer, "status": "active",
    })
    for endpoint in ("chat", "responses"):
        call(session, ledger, f"{endpoint}_binding", "POST", f"/admin/endpoints/p12-08g1-{endpoint}-endpoint/credential-bindings", {
            "credential_id": "p12-08g1-credential", "enabled": True, "priority": 0, "weight": 1, "concurrency": 1,
        })

    for name, alias in ALIASES.items():
        endpoint = "chat" if name == "chat" else "responses"
        call(session, ledger, f"{name}_public_model_id", "POST", "/admin/public-models", {
            "id": f"p12-08g1-{name}-model", "model_name": alias, "status": "active",
            "display_name": alias, "capabilities": {},
        })
        call(session, ledger, f"{name}_route_id", "POST", f"/admin/public-models/p12-08g1-{name}-model/routes", {
            "id": ROUTES[name], "policy": "smooth_weighted_round_robin", "max_attempts": 1,
            "bootstrap_timeout_ms": 15000,
        })
        call(session, ledger, f"{name}_candidate_id", "POST", f"/admin/routes/{ROUTES[name]}/candidates", {
            "id": CANDIDATES[name], "endpoint_id": f"p12-08g1-{endpoint}-endpoint",
            "upstream_model": upstream_model, "credential_scope": "all_active", "transform_mode": MODE[name],
            "enabled": True, "priority": 0, "weight": 1, "capability_override": {"allow_unlisted_model": True},
        })

    call(session, ledger, "access_group_id", "POST", "/admin/access-groups", {
        "id": "p12-08g1-group", "name": "P12-08G1 controlled clients", "status": "active", "limits": {},
    })
    for name in ALIASES:
        call(session, ledger, f"{name}_access_route", "POST", "/admin/access-groups/p12-08g1-group/routes", {
            "route_id": ROUTES[name], "enabled": True,
        })
    issued = session.call("POST", "/admin/client-keys", {
        "id": "p12-08g1-client-key", "access_group_id": "p12-08g1-group", "status": "active",
    })
    secret = issued.get("key")
    if not isinstance(secret, str) or not secret:
        raise ManagementError("client key response omitted one-time value")
    descriptor = os.open(args.client_key_out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        handle.write(secret)
    ledger.record("client_key_id", "p12-08g1-client-key")
    ledger.record("client_key_prefix", issued.get("prefix"))
    print("p12-08g1-graph=ENTERED")
    return 0


def finish(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), config_version=VERSION_ID, base=args.management_base)
    for route in ROUTES.values():
        session.call("POST", f"/admin/routes/{route}/validate", {}, expect=200)
    validated = session.call("POST", f"/admin/config-versions/{VERSION_ID}/validate", {}, expect=200)
    if validated.get("valid") is not True:
        raise ManagementError("config version validation failed")
    current = session.call("GET", f"/admin/config-versions/{VERSION_ID}", None, expect=200,
                           send_config_version=False, send_if_match=False)
    session.revision = current.get("revision")
    if not session.revision:
        raise ManagementError("config version omitted revision")
    session.call("POST", f"/admin/config-versions/{VERSION_ID}/publish", {}, expect=200, send_config_version=False)
    print("p12-08g1-graph=PUBLISHED")
    return 0


def explain(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), config_version=VERSION_ID, base=args.management_base)
    results = {}
    for name, route in ROUTES.items():
        path = f"/admin/routes/{route}/explain?requested_model={quote(ALIASES[name], safe='')}&protocol={PROTOCOL[name]}"
        result = session.call("GET", path, None, expect=200, send_if_match=False)
        candidates = result.get("candidates")
        results[name] = isinstance(candidates, list) and len(candidates) == 1 and candidates[0].get("decision") == "selected"
    print(json.dumps({"all_routes_single_selected": all(results.values()), "route_count": len(results)}, sort_keys=True))
    return 0 if all(results.values()) else 1


def rollback(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), base=args.management_base)
    active = session.call("GET", f"/admin/config-versions/{VERSION_ID}", None, expect=200,
                          send_config_version=False, send_if_match=False)
    if active.get("status") != "active" or not active.get("revision"):
        raise ManagementError("G1 version is not active")
    session.revision = active["revision"]
    result = session.call("POST", "/admin/config-versions/rollback", {}, expect=200, send_config_version=False)
    if result.get("replaced_config_version_id") != VERSION_ID:
        raise ManagementError("rollback did not replace G1 version")
    print("p12-08g1-graph=ROLLED_BACK")
    return 0


def reactivate(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), base=args.management_base)
    predecessor = session.call("GET", f"/admin/config-versions/{args.parent_version_id}", None, expect=200,
                               send_config_version=False, send_if_match=False)
    if predecessor.get("status") != "active" or not predecessor.get("revision"):
        raise ManagementError("predecessor version is not active")
    session.revision = predecessor["revision"]
    result = session.call("POST", "/admin/config-versions/rollback", {}, expect=200, send_config_version=False)
    if result.get("active_config_version_id") != VERSION_ID:
        raise ManagementError("reactivation did not select G1 version")
    print("p12-08g1-graph=REACTIVATED")
    return 0


def main() -> int:
    args = parse_args()
    try:
        if args.finish:
            return finish(args)
        if args.explain_only:
            return explain(args)
        if args.rollback:
            return rollback(args)
        if args.reactivate:
            return reactivate(args)
        return enter(args)
    except (ManagementError, OSError, ValueError) as error:
        print(f"p12-08g1-graph=FAIL category={type(error).__name__}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
