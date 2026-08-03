#!/usr/bin/env python3
"""Publish the production Codex graph plus a loopback-only grok2api channel.

The management API treats ``parent_id`` as lineage, not inheritance, so this helper deliberately
re-enters the complete production graph. Secrets arrive as six NUL-delimited stdin fields and are
never printed or retained in the value-free ledger.
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
    spec = importlib.util.spec_from_file_location("p12_10_graph_shared", path)
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

VERSION_ID = "p12-10-codex-grok2api-v2"
CHANNELS = ("codex", "grok")
PROTOCOLS = ("chat", "responses", "messages")
ALIASES = {
    ("codex", "chat"): "p12-g1-codex-chat",
    ("codex", "responses"): "p12-g1-codex-responses",
    ("codex", "messages"): "p12-g1-codex-messages",
    ("grok", "chat"): "p12-grok-chat",
    ("grok", "responses"): "p12-grok-responses",
    ("grok", "messages"): "p12-grok-messages",
}
MODE = {
    ("codex", "chat"): "canonical",
    ("codex", "responses"): "canonical",
    ("codex", "messages"): "lossless_bridge",
    ("grok", "chat"): "lossless_bridge",
    ("grok", "responses"): "canonical",
    ("grok", "messages"): "lossless_bridge",
}
INBOUND_PROTOCOL = {
    "chat": "openai_chat_completions",
    "responses": "openai_responses",
    "messages": "anthropic_messages",
}


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


def normalize_https(value: str) -> tuple[str, str, int]:
    parsed = urlsplit(value)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
    ):
        raise ManagementError("external target failed closed validation")
    port = parsed.port or 443
    path = parsed.path.rstrip("/")
    normalized = f"https://{parsed.hostname}" + (f":{port}" if port != 443 else "") + path
    return normalized, parsed.hostname, port


def normalize_loopback(value: str) -> tuple[str, int]:
    parsed = urlsplit(value)
    if (
        parsed.scheme != "http"
        or parsed.hostname != "127.0.0.1"
        or parsed.port is None
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
    ):
        raise ManagementError("grok2api target must be exact IPv4 loopback HTTP")
    path = parsed.path.rstrip("/")
    return f"http://127.0.0.1:{parsed.port}{path}", parsed.port


def read_inputs() -> dict[str, object]:
    raw = sys.stdin.buffer.read(262145)
    if len(raw) > 262144:
        raise ManagementError("enter input exceeds bound")
    parts = raw.split(b"\0")
    if parts and parts[-1] == b"":
        parts.pop()
    if len(parts) != 6:
        raise ManagementError("enter requires six NUL-delimited values")
    try:
        codex_base, codex_key, codex_model, grok_base, grok_key, grok_model = (
            part.decode("utf-8") for part in parts
        )
    except UnicodeDecodeError as error:
        raise ManagementError("enter input is not UTF-8") from error
    if not all((codex_key, codex_model, grok_key, grok_model)):
        raise ManagementError("enter input contains an empty value")
    if max(len(codex_key), len(grok_key)) > 8192 or max(len(codex_model), len(grok_model)) > 256:
        raise ManagementError("enter input exceeds field bounds")
    codex_base, codex_host, codex_port = normalize_https(codex_base)
    grok_base, grok_port = normalize_loopback(grok_base)
    return {
        "codex_base": codex_base,
        "codex_host": codex_host,
        "codex_port": codex_port,
        "codex_key": codex_key,
        "codex_model": codex_model,
        "grok_base": grok_base,
        "grok_port": grok_port,
        "grok_key": grok_key,
        "grok_model": grok_model,
    }


def call(session, ledger, key: str, method: str, path: str, payload: dict, expect: int = 201):
    result = session.call(method, path, payload, expect=expect)
    ledger.record(key, payload.get("id", True))
    return result


def add_upstream(session, ledger, channel: str, values: dict[str, object]) -> None:
    is_grok = channel == "grok"
    base = str(values[f"{channel}_base"])
    port = int(values[f"{channel}_port"])
    policy_id = f"p12-10-{channel}-egress"
    upstream_id = f"p12-10-{channel}-upstream"
    allowed_hosts = ["127.0.0.1"] if is_grok else [str(values["codex_host"])]
    call(session, ledger, f"{channel}_egress_policy_id", "POST", "/admin/egress-policies", {
        "id": policy_id,
        "name": f"P12-10 {channel} exact egress",
        "allowed_schemes": ["http" if is_grok else "https"],
        "allowed_hosts": allowed_hosts,
        "allowed_ports": [port],
        "allowed_cidrs": ["127.0.0.1/32"] if is_grok else [],
        "redirect_mode": "deny",
        "max_redirects": 0,
    })
    call(session, ledger, f"{channel}_upstream_id", "POST", "/admin/upstreams", {
        "id": upstream_id,
        "name": f"P12-10 {channel} OpenAI-compatible",
        "kind": "openai-compatible",
        "enabled": True,
        "tags": ["p12-10", channel, "loopback-proxy" if is_grok else "external"],
        "egress_policy_id": policy_id,
    })
    endpoint_kinds = ("responses",) if is_grok else ("chat", "responses")
    for kind in endpoint_kinds:
        adapter = "openai-compatible.responses" if kind == "responses" else "openai-compatible.chat-completions"
        api_format = "openai/responses" if kind == "responses" else "openai/chat-completions"
        path = "/responses" if kind == "responses" else "/chat/completions"
        call(session, ledger, f"{channel}_{kind}_endpoint_id", "POST", f"/admin/upstreams/{upstream_id}/endpoints", {
            "id": f"p12-10-{channel}-{kind}-endpoint",
            "adapter_id": adapter,
            "api_format": api_format,
            "base_url": base,
            "inference_path": path,
            "transport": "http" if is_grok else "https",
            "enabled": True,
        })
    credential_id = f"p12-10-{channel}-credential"
    call(session, ledger, f"{channel}_credential_id", "POST", f"/admin/upstreams/{upstream_id}/credentials", {
        "id": credential_id,
        "kind": "bearer",
        "secret": str(values[f"{channel}_key"]),
        "status": "active",
    })
    for kind in endpoint_kinds:
        endpoint_id = f"p12-10-{channel}-{kind}-endpoint"
        call(session, ledger, f"{channel}_{kind}_binding", "POST", f"/admin/endpoints/{endpoint_id}/credential-bindings", {
            "credential_id": credential_id,
            "enabled": True,
            "priority": 0,
            "weight": 1,
            "concurrency": 1,
        })


def enter(args: argparse.Namespace) -> int:
    values = read_inputs()
    ledger = Ledger(args.ledger)
    session = Management(read_secret(args.management_key_file), base=args.management_base)
    version = session.call("POST", "/admin/config-versions", {
        "id": VERSION_ID,
        "parent_id": args.parent_version_id,
        "description": "P12-10 production Codex plus loopback grok2api graph",
    }, send_config_version=False)
    session.config_version = VERSION_ID
    session.revision = version.get("revision")
    ledger.record("config_version_id", VERSION_ID)
    ledger.record("initial_revision", session.revision)

    for channel in CHANNELS:
        add_upstream(session, ledger, channel, values)

    for channel in CHANNELS:
        for protocol in PROTOCOLS:
            alias = ALIASES[(channel, protocol)]
            model_id = f"p12-10-{channel}-{protocol}-model"
            route_id = f"p12-10-{channel}-{protocol}-route"
            candidate_id = f"p12-10-{channel}-{protocol}-candidate"
            endpoint_kind = "responses" if channel == "grok" or protocol != "chat" else "chat"
            call(session, ledger, f"{channel}_{protocol}_public_model_id", "POST", "/admin/public-models", {
                "id": model_id, "model_name": alias, "status": "active", "display_name": alias,
                "capabilities": {},
            })
            call(session, ledger, f"{channel}_{protocol}_route_id", "POST", f"/admin/public-models/{model_id}/routes", {
                "id": route_id, "policy": "smooth_weighted_round_robin", "max_attempts": 1,
                "bootstrap_timeout_ms": 15000,
            })
            call(session, ledger, f"{channel}_{protocol}_candidate_id", "POST", f"/admin/routes/{route_id}/candidates", {
                "id": candidate_id,
                "endpoint_id": f"p12-10-{channel}-{endpoint_kind}-endpoint",
                "upstream_model": str(values[f"{channel}_model"]),
                "credential_scope": "all_active",
                "transform_mode": MODE[(channel, protocol)],
                "enabled": True, "priority": 0, "weight": 1,
                "capability_override": {"allow_unlisted_model": True},
            })

    group_id = "p12-10-production-group"
    call(session, ledger, "access_group_id", "POST", "/admin/access-groups", {
        "id": group_id, "name": "P12-10 production clients", "status": "active", "limits": {},
    })
    for channel in CHANNELS:
        for protocol in PROTOCOLS:
            route_id = f"p12-10-{channel}-{protocol}-route"
            call(session, ledger, f"{channel}_{protocol}_access_route", "POST", f"/admin/access-groups/{group_id}/routes", {
                "route_id": route_id, "enabled": True,
            })
    issued = session.call("POST", "/admin/client-keys", {
        "id": "p12-10-production-client-key", "access_group_id": group_id, "status": "active",
    })
    secret = issued.get("key")
    if not isinstance(secret, str) or not secret:
        raise ManagementError("client key response omitted one-time value")
    descriptor = os.open(args.client_key_out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        handle.write(secret)
    ledger.record("client_key_id", "p12-10-production-client-key")
    ledger.record("client_key_prefix", issued.get("prefix"))
    print("p12-10-grok2api-graph=ENTERED")
    return 0


def finish(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), config_version=VERSION_ID, base=args.management_base)
    for channel in CHANNELS:
        for protocol in PROTOCOLS:
            session.call("POST", f"/admin/routes/p12-10-{channel}-{protocol}-route/validate", {}, expect=200)
    validated = session.call("POST", f"/admin/config-versions/{VERSION_ID}/validate", {}, expect=200)
    if validated.get("valid") is not True:
        raise ManagementError("config version validation failed")
    current = session.call("GET", f"/admin/config-versions/{VERSION_ID}", None, expect=200,
                           send_config_version=False, send_if_match=False)
    session.revision = current.get("revision")
    if not session.revision:
        raise ManagementError("config version omitted revision")
    session.call("POST", f"/admin/config-versions/{VERSION_ID}/publish", {}, expect=200,
                 send_config_version=False)
    print("p12-10-grok2api-graph=PUBLISHED")
    return 0


def explain(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), config_version=VERSION_ID, base=args.management_base)
    passed = []
    for channel in CHANNELS:
        for protocol in PROTOCOLS:
            route = f"p12-10-{channel}-{protocol}-route"
            alias = ALIASES[(channel, protocol)]
            path = f"/admin/routes/{route}/explain?requested_model={quote(alias, safe='')}&protocol={INBOUND_PROTOCOL[protocol]}"
            result = session.call("GET", path, None, expect=200, send_if_match=False)
            candidates = result.get("candidates")
            passed.append(isinstance(candidates, list) and len(candidates) == 1 and candidates[0].get("decision") == "selected")
    print(json.dumps({"all_routes_single_selected": all(passed), "route_count": len(passed)}, sort_keys=True))
    return 0 if all(passed) else 1


def rollback(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), base=args.management_base)
    active = session.call("GET", f"/admin/config-versions/{VERSION_ID}", None, expect=200,
                          send_config_version=False, send_if_match=False)
    if active.get("status") != "active" or not active.get("revision"):
        raise ManagementError("P12-10 version is not active")
    session.revision = active["revision"]
    result = session.call("POST", "/admin/config-versions/rollback", {}, expect=200,
                          send_config_version=False)
    if result.get("replaced_config_version_id") != VERSION_ID:
        raise ManagementError("rollback did not replace P12-10 version")
    print("p12-10-grok2api-graph=ROLLED_BACK")
    return 0


def reactivate(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), base=args.management_base)
    predecessor = session.call("GET", f"/admin/config-versions/{args.parent_version_id}", None,
                               expect=200, send_config_version=False, send_if_match=False)
    if predecessor.get("status") != "active" or not predecessor.get("revision"):
        raise ManagementError("predecessor version is not active")
    session.revision = predecessor["revision"]
    result = session.call("POST", "/admin/config-versions/rollback", {}, expect=200,
                          send_config_version=False)
    if result.get("active_config_version_id") != VERSION_ID:
        raise ManagementError("reactivation did not select P12-10 version")
    print("p12-10-grok2api-graph=REACTIVATED")
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
        print(f"p12-10-grok2api-graph=FAIL category={type(error).__name__}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
