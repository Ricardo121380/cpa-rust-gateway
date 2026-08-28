#!/usr/bin/env python3
"""Enter and publish one isolated CPAR native Grok Console route.

This is the Console counterpart of ``p12-10h-bind-native-grok.py``.  It mutates only the
explicitly supplied loopback management listener and never reads or prints a provider secret.
The imported Console account pool is compiled by the gateway itself; this helper only creates the
route graph and writes the one-time client key to a mode-0600 file.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import sys
from urllib.parse import quote


def load_shared():
    path = Path(__file__).with_name("p12-06-enter-kiro-graph.py")
    spec = importlib.util.spec_from_file_location("p12_10h_graph_shared", path)
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

VERSION_ID = "p12-10h-native-grok-console-v1"
UPSTREAM_ID = "p12-10h-native-grok-console-upstream"
ENDPOINT_ID = "p12-10h-native-grok-console-endpoint"
PUBLIC_MODEL_ID = "p12-10h-native-grok-console-model"
ROUTE_ID = "p12-10h-native-grok-console-route"
CANDIDATE_ID = "p12-10h-native-grok-console-candidate"
ACCESS_GROUP_ID = "p12-10h-native-grok-console-group"
CLIENT_KEY_ID = "p12-10h-native-grok-console-client-key"
EGRESS_ID = "p12-10h-native-grok-console-egress"

CONSOLE_BASE_URL = "https://console.x.ai"
CONSOLE_INFERENCE_PATH = "/v1/responses"
CONSOLE_HOST = "console.x.ai"
PUBLIC_MODEL = "grok-cpar-console"
# This is a fixed model name from the reviewed Console catalog.  The source account's observed
# probe model is embedded in its imported envelope and is not read by this graph helper.
UPSTREAM_MODEL = "grok-4.20-0309"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--management-key-file", required=True)
    parser.add_argument("--management-base", required=True)
    parser.add_argument("--parent-version-id", required=True)
    parser.add_argument("--version-id", default=VERSION_ID)
    parser.add_argument("--ledger", required=True)
    parser.add_argument("--client-key-out", required=True)
    actions = parser.add_mutually_exclusive_group()
    actions.add_argument("--finish", action="store_true")
    actions.add_argument("--explain-only", action="store_true")
    return parser.parse_args()


def call(session, ledger, key: str, method: str, path: str, payload: dict, expect: int = 201):
    result = session.call(method, path, payload, expect=expect)
    ledger.record(key, payload.get("id", True))
    return result


def enter(args: argparse.Namespace) -> int:
    ledger = Ledger(args.ledger)
    session = Management(read_secret(args.management_key_file), base=args.management_base)
    version = session.call(
        "POST",
        "/admin/config-versions",
        {
            "id": args.version_id,
            "parent_id": args.parent_version_id,
            "description": "P12-10H isolated native Grok Console route",
        },
        send_config_version=False,
    )
    session.config_version = args.version_id
    session.revision = version.get("revision")
    ledger.record("config_version_id", args.version_id)
    ledger.record("initial_revision", session.revision)

    call(session, ledger, "egress_policy_id", "POST", "/admin/egress-policies", {
        "id": EGRESS_ID,
        "name": "P12-10H native Grok Console egress",
        "allowed_schemes": ["https"],
        "allowed_hosts": [CONSOLE_HOST],
        "allowed_ports": [443],
        "allowed_cidrs": [],
        "redirect_mode": "deny",
        "max_redirects": 0,
    })
    call(session, ledger, "upstream_id", "POST", "/admin/upstreams", {
        "id": UPSTREAM_ID,
        "name": "P12-10H native Grok Console",
        "kind": "grok-console-native",
        "enabled": True,
        "tags": ["p12-10h", "native-grok", "console"],
        "egress_policy_id": EGRESS_ID,
    })
    call(session, ledger, "endpoint_id", "POST", f"/admin/upstreams/{UPSTREAM_ID}/endpoints", {
        "id": ENDPOINT_ID,
        "adapter_id": "grok.console.responses",
        "api_format": "openai/responses",
        "base_url": CONSOLE_BASE_URL,
        "inference_path": CONSOLE_INFERENCE_PATH,
        "transport": "https",
        "enabled": True,
    })
    call(session, ledger, "public_model_id", "POST", "/admin/public-models", {
        "id": PUBLIC_MODEL_ID,
        "model_name": PUBLIC_MODEL,
        "status": "active",
        "display_name": PUBLIC_MODEL,
        "capabilities": {},
    })
    call(session, ledger, "route_id", "POST", f"/admin/public-models/{PUBLIC_MODEL_ID}/routes", {
        "id": ROUTE_ID,
        "policy": "smooth_weighted_round_robin",
        "max_attempts": 1,
        "bootstrap_timeout_ms": 15000,
    })
    call(session, ledger, "candidate_id", "POST", f"/admin/routes/{ROUTE_ID}/candidates", {
        "id": CANDIDATE_ID,
        "endpoint_id": ENDPOINT_ID,
        "upstream_model": UPSTREAM_MODEL,
        "credential_scope": "all_active",
        "transform_mode": "canonical_bridge",
        "enabled": True,
        "priority": 0,
        "weight": 1,
        # Console's reviewed catalog can expose private reasoning.  Keep the same explicit
        # narrowing used by the native Build route so an OpenAI Chat client is never selected for
        # a response that could contain unrepresentable private-reasoning events.
        "capability_override": {"allow_unlisted_model": True, "reasoning": False},
    })
    call(session, ledger, "access_group_id", "POST", "/admin/access-groups", {
        "id": ACCESS_GROUP_ID,
        "name": "P12-10H native Grok Console staging clients",
        "status": "active",
        "limits": {},
    })
    call(session, ledger, "access_group_route", "POST", f"/admin/access-groups/{ACCESS_GROUP_ID}/routes", {
        "route_id": ROUTE_ID,
        "enabled": True,
    })
    issued = session.call("POST", "/admin/client-keys", {
        "id": CLIENT_KEY_ID,
        "access_group_id": ACCESS_GROUP_ID,
        "status": "active",
    })
    key = issued.get("key")
    if not isinstance(key, str) or not key:
        raise ManagementError("client key response omitted one-time value")
    descriptor = os.open(args.client_key_out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        handle.write(key)
    ledger.record("client_key_id", CLIENT_KEY_ID)
    ledger.record("client_key_prefix", issued.get("prefix"))
    print("p12-10h-native-grok-console=ENTERED")
    return 0


def finish(args: argparse.Namespace) -> int:
    session = Management(
        read_secret(args.management_key_file),
        config_version=args.version_id,
        base=args.management_base,
    )
    session.call("POST", f"/admin/routes/{ROUTE_ID}/validate", {}, expect=200)
    current = session.call(
        "GET",
        f"/admin/config-versions/{args.version_id}",
        None,
        expect=200,
        send_config_version=False,
        send_if_match=False,
    )
    session.revision = current.get("revision")
    if not session.revision:
        raise ManagementError("native Grok Console graph omitted revision before validation")
    validated = session.call("POST", f"/admin/config-versions/{args.version_id}/validate", {}, expect=200)
    if validated.get("valid") is not True:
        raise ManagementError("native Grok Console graph validation failed")
    session.call(
        "POST", f"/admin/config-versions/{args.version_id}/publish", {}, expect=200,
        send_config_version=False,
    )
    print("p12-10h-native-grok-console=PUBLISHED")
    return 0


def explain_only(args: argparse.Namespace) -> int:
    session = Management(
        read_secret(args.management_key_file), config_version=args.version_id, base=args.management_base
    )
    result = session.call(
        "GET",
        f"/admin/routes/{ROUTE_ID}/explain?requested_model={quote(PUBLIC_MODEL)}"
        "&protocol=openai_responses",
        None,
        expect=200,
        send_if_match=False,
    )
    candidates = result.get("candidates")
    selected = (
        isinstance(candidates, list)
        and len(candidates) == 1
        and candidates[0].get("decision") == "selected"
    )
    print(json.dumps({
        "single_selected": selected,
        "candidate_count": len(candidates) if isinstance(candidates, list) else 0,
    }, sort_keys=True))
    return 0 if selected else 1


def main() -> int:
    args = parse_args()
    try:
        if args.finish:
            return finish(args)
        if args.explain_only:
            return explain_only(args)
        return enter(args)
    except (ManagementError, OSError, ValueError) as error:
        print(f"p12-10h-native-grok-console=FAIL category={type(error).__name__}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
