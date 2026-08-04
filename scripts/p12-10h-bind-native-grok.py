#!/usr/bin/env python3
"""Enter and publish one isolated CPAR native Grok Build route.

The route deliberately has no ordinary control-plane Credential row.  The gateway's native
account-pool compiler supplies the imported Build account at process bootstrap.  This helper only
mutates the explicitly supplied management listener and writes the one-time client key to a
0600 file; it never reads or prints a provider credential.
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

VERSION_ID = "p12-10h-native-grok-build"
UPSTREAM_ID = "p12-10h-native-grok-upstream"
ENDPOINT_ID = "p12-10h-native-grok-endpoint"
PUBLIC_MODEL_ID = "p12-10h-native-grok-model"
ROUTE_ID = "p12-10h-native-grok-route"
CANDIDATE_ID = "p12-10h-native-grok-candidate"
ACCESS_GROUP_ID = "p12-10h-native-grok-group"
CLIENT_KEY_ID = "p12-10h-native-grok-client-key"
EGRESS_ID = "p12-10h-native-grok-egress"

BUILD_BASE_URL = "https://cli-chat-proxy.grok.com/v1"
BUILD_INFERENCE_PATH = "/responses"
BUILD_HOST = "cli-chat-proxy.grok.com"
PUBLIC_MODEL = "grok-cpar-build"
UPSTREAM_MODEL = "grok-4.5"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--management-key-file", required=True)
    # A staging graph must never silently fall back to the production management listener.
    # Requiring the caller to name the isolated loopback listener makes the deployment boundary
    # explicit at every invocation.
    parser.add_argument("--management-base", required=True)
    parser.add_argument("--parent-version-id", required=True)
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
            "id": VERSION_ID,
            "parent_id": args.parent_version_id,
            "description": "P12-10H isolated native Grok Build route",
        },
        send_config_version=False,
    )
    session.config_version = VERSION_ID
    session.revision = version.get("revision")
    ledger.record("config_version_id", VERSION_ID)
    ledger.record("initial_revision", session.revision)

    call(session, ledger, "egress_policy_id", "POST", "/admin/egress-policies", {
        "id": EGRESS_ID,
        "name": "P12-10H native Grok Build egress",
        "allowed_schemes": ["https"],
        "allowed_hosts": [BUILD_HOST],
        "allowed_ports": [443],
        "allowed_cidrs": [],
        "redirect_mode": "deny",
        "max_redirects": 0,
    })
    call(session, ledger, "upstream_id", "POST", "/admin/upstreams", {
        "id": UPSTREAM_ID,
        "name": "P12-10H native Grok Build",
        "kind": "grok-build-native",
        "enabled": True,
        "tags": ["p12-10h", "native-grok"],
        "egress_policy_id": EGRESS_ID,
    })
    call(session, ledger, "endpoint_id", "POST", f"/admin/upstreams/{UPSTREAM_ID}/endpoints", {
        "id": ENDPOINT_ID,
        "adapter_id": "grok.build.responses",
        "api_format": "openai/responses",
        "base_url": BUILD_BASE_URL,
        "inference_path": BUILD_INFERENCE_PATH,
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
        # The management API accepts its stable public spelling here and maps it to
        # the runtime's endpoint-binding scope internally.
        "credential_scope": "all_active",
        # One native Responses endpoint serves same-protocol Canonical requests and the reviewed
        # cross-protocol lossless bridge matrix. Reasoning is narrowed at this public route so a
        # Chat client can never receive an unrepresentable private-reasoning event.
        "transform_mode": "canonical_bridge",
        "enabled": True,
        "priority": 0,
        "weight": 1,
        "capability_override": {"allow_unlisted_model": True, "reasoning": False},
    })
    call(session, ledger, "access_group_id", "POST", "/admin/access-groups", {
        "id": ACCESS_GROUP_ID,
        "name": "P12-10H native Grok Build staging clients",
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
    print("p12-10h-native-grok=ENTERED")
    return 0


def finish(args: argparse.Namespace) -> int:
    session = Management(
        read_secret(args.management_key_file),
        config_version=VERSION_ID,
        base=args.management_base,
    )
    session.call("POST", f"/admin/routes/{ROUTE_ID}/validate", {}, expect=200)
    validated = session.call("POST", f"/admin/config-versions/{VERSION_ID}/validate", {}, expect=200)
    if validated.get("valid") is not True:
        raise ManagementError("native Grok graph validation failed")
    current = session.call("GET", f"/admin/config-versions/{VERSION_ID}", None, expect=200,
                           send_config_version=False, send_if_match=False)
    session.revision = current.get("revision")
    if not session.revision:
        raise ManagementError("native Grok graph omitted revision")
    session.call("POST", f"/admin/config-versions/{VERSION_ID}/publish", {}, expect=200,
                 send_config_version=False)
    print("p12-10h-native-grok=PUBLISHED")
    return 0


def explain_only(args: argparse.Namespace) -> int:
    session = Management(read_secret(args.management_key_file), config_version=VERSION_ID,
                         base=args.management_base)
    result = session.call(
        "GET",
        f"/admin/routes/{ROUTE_ID}/explain?requested_model={quote(PUBLIC_MODEL)}"
        "&protocol=openai_responses",
        None,
        expect=200,
        send_if_match=False,
    )
    candidates = result.get("candidates")
    selected = isinstance(candidates, list) and len(candidates) == 1 and candidates[0].get("decision") == "selected"
    print(json.dumps({"single_selected": selected, "candidate_count": len(candidates) if isinstance(candidates, list) else 0}, sort_keys=True))
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
        print(f"p12-10h-native-grok=FAIL category={type(error).__name__}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
