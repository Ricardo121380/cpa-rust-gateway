#!/usr/bin/env python3
"""Enters the P12-06 Kiro configuration graph on the management listener and publishes it.

Runs on the server, against `127.0.0.1:18181` only. Reads every secret from a file path and never
prints one: the management key, the Kiro `ksk_` value and the issued client key are handled by
reference. The issued client key is written to a caller-named file with 0600, because the API
returns it exactly once (`docs/p12-rollout-runbook.md` step 12) and there is no recovery path.

It maintains the ledger §2 of the runbook mandates: endpoints, credentials, aliases, routes and
candidates have no list endpoint, so every id is recorded to a JSON ledger as it is created. The
ledger is the only way back to those rows.

The graph is Kiro-only and deliberately minimal -- one upstream, one endpoint, one credential, one
public model, one route, one candidate, one access group, one client key. P12-06 needs a channel
that works, not a production topology.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

MANAGEMENT_BASE_DEFAULT = "http://127.0.0.1:18181"
# Kiro's endpoint URL is derived by the gateway from the endpoint kind and API Region; the stored
# `base_url` + `inference_path` must denote exactly that URL or composition fails closed (see
# `p12_kiro_endpoint_shape`). These constants are therefore not free parameters: they are the CLI
# shape for us-east-1, which is what the kiro-rs credential in use is provisioned for.
KIRO_BASE_URL = "https://runtime.us-east-1.kiro.dev"
KIRO_INFERENCE_PATH = "/"
KIRO_ADAPTER_ID = "kiro.messages"
KIRO_API_FORMAT = "anthropic/messages"


class ManagementError(Exception):
    """A management call did not return the expected status."""


class Ledger:
    """Records every created id, since most graph resources have no read endpoint."""

    def __init__(self, path: str) -> None:
        self.path = path
        self.entries: dict[str, object] = {}

    def record(self, key: str, value: object) -> None:
        self.entries[key] = value
        self.flush()

    def flush(self) -> None:
        with open(self.path, "w", encoding="utf-8") as handle:
            json.dump(self.entries, handle, indent=2, sort_keys=True)
            handle.write("\n")


class Management:
    """One authenticated management session that tracks the current revision for If-Match."""

    def __init__(self, key: str, config_version: str | None = None, base: str = MANAGEMENT_BASE_DEFAULT) -> None:
        self.key = key
        self.config_version = config_version
        self.revision: str | None = None
        self.base = base

    def call(
        self,
        method: str,
        path: str,
        payload: dict | None = None,
        expect: int = 201,
        send_config_version: bool = True,
        send_if_match: bool = True,
    ) -> dict:
        headers = {"x-management-key": self.key, "content-type": "application/json"}
        if send_config_version and self.config_version:
            headers["x-config-version"] = self.config_version
        if send_if_match and self.revision:
            headers["if-match"] = self.revision
        body = None if payload is None else json.dumps(payload).encode("utf-8")
        request = urllib.request.Request(
            self.base + path, data=body, method=method, headers=headers
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:  # noqa: S310 - loopback
                status = response.status
                raw = response.read()
                etag = response.headers.get("etag")
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", "replace")[:400]
            raise ManagementError(f"{method} {path} -> {error.code}: {detail}") from error
        except OSError as error:
            raise ManagementError(f"{method} {path} -> {type(error).__name__}") from error
        if status != expect:
            raise ManagementError(f"{method} {path} -> {status}, expected {expect}")
        # The ETag of a successful mutation is the *next* If-Match value; tracking it here is what
        # lets a whole sequence run without re-reading the version between every step.
        if etag:
            self.revision = etag.strip('"')
        return json.loads(raw) if raw else {}


def read_secret(path: str) -> str:
    with open(path, "r", encoding="utf-8") as handle:
        value = handle.read().strip()
    if not value:
        raise ManagementError(f"{path} is empty")
    return value


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("--management-key-file", required=True)
    parser.add_argument("--kiro-key-file", help="File holding the ksk_ value")
    parser.add_argument("--version-id", default="p12-06-kiro")
    parser.add_argument("--upstream-model", default="claude-sonnet-5")
    parser.add_argument("--public-model", default="claude-sonnet-5")
    parser.add_argument("--ledger", help="Where to write the id ledger")
    parser.add_argument("--client-key-out", help="Where to write the issued key")
    parser.add_argument("--publish", action="store_true", help="Publish after validating")
    # The management listener is loopback-only by construction, so this override cannot expose
    # anything; it exists so the sequence can be rehearsed against a scratch gateway on another
    # port before it is run against the real state directory.
    parser.add_argument("--management-base", default=MANAGEMENT_BASE_DEFAULT)
    parser.add_argument(
        "--explain-only",
        action="store_true",
        help="Only read the route explanation, for use after the post-publish restart",
    )
    parser.add_argument(
        "--finish",
        action="store_true",
        help="Validate and publish an already-entered graph, after one service restart",
    )
    args = parser.parse_args()
    if args.explain_only and args.finish:
        parser.error("--explain-only and --finish are separate phases")
    if not (args.explain_only or args.finish):
        missing = [
            name
            for name in ("kiro_key_file", "ledger", "client_key_out")
            if getattr(args, name) is None
        ]
        if missing:
            parser.error(
                "entering the graph requires " + ", ".join(f"--{n.replace('_', '-')}" for n in missing)
            )
    return args


def enter_graph(args: argparse.Namespace) -> int:
    management_key = read_secret(args.management_key_file)
    kiro_key = read_secret(args.kiro_key_file)
    if not kiro_key.startswith("ksk_"):
        # The gateway composes only the headless API-key family; an OAuth credential would fail
        # closed at composition. Catch that here where the message can say why.
        print("enter-graph: the Kiro credential must be a ksk_ API key", file=sys.stderr)
        return 2

    ledger = Ledger(args.ledger)
    session = Management(management_key, base=args.management_base)
    ids = {
        "config_version": args.version_id,
        "egress_policy": "p12-06-kiro-egress",
        "upstream": "p12-06-kiro-upstream",
        "endpoint": "p12-06-kiro-endpoint",
        "credential": "p12-06-kiro-credential",
        "public_model": "p12-06-kiro-model",
        "route": "p12-06-kiro-route",
        "candidate": "p12-06-kiro-candidate",
        "access_group": "p12-06-kiro-group",
        "client_key": "p12-06-kiro-client-key",
    }

    version = session.call(
        "POST",
        "/admin/config-versions",
        {"id": ids["config_version"], "description": "P12-06 Kiro channel verification"},
        send_config_version=False,
    )
    session.config_version = ids["config_version"]
    session.revision = version.get("revision")
    ledger.record("config_version_id", ids["config_version"])
    ledger.record("initial_revision", session.revision)

    # The egress policy names exactly the derived Kiro host and nothing else. `allowed_cidrs` is
    # left empty on purpose: an empty list means "no CIDR restriction beyond the built-in denies",
    # and pinning Kiro's AWS addresses would break on their next rotation while adding nothing --
    # the host allow-list is already the binding constraint.
    session.call(
        "POST",
        "/admin/egress-policies",
        {
            "id": ids["egress_policy"],
            "name": "P12-06 Kiro egress",
            "allowed_schemes": ["https"],
            "allowed_hosts": ["runtime.us-east-1.kiro.dev"],
            "allowed_ports": [443],
            "allowed_cidrs": [],
            "redirect_mode": "deny",
            "max_redirects": 0,
        },
    )
    ledger.record("egress_policy_id", ids["egress_policy"])

    session.call(
        "POST",
        "/admin/upstreams",
        {
            "id": ids["upstream"],
            "name": "Kiro CLI us-east-1",
            "kind": "kiro",
            "enabled": True,
            "tags": ["p12-06"],
            "egress_policy_id": ids["egress_policy"],
        },
    )
    ledger.record("upstream_id", ids["upstream"])

    session.call(
        "POST",
        f"/admin/upstreams/{ids['upstream']}/endpoints",
        {
            "id": ids["endpoint"],
            "adapter_id": KIRO_ADAPTER_ID,
            "api_format": KIRO_API_FORMAT,
            "base_url": KIRO_BASE_URL,
            "inference_path": KIRO_INFERENCE_PATH,
            "transport": "https",
            "enabled": True,
        },
    )
    ledger.record("endpoint_id", ids["endpoint"])

    # `kind` must be "bearer": the runtime accepts no other credential kind
    # (`validate_p12_credential_bindings`). The Kiro adapter reads the secret bytes and imports them
    # as an API key, so the stored kind describes the storage shape, not the Kiro auth family.
    session.call(
        "POST",
        f"/admin/upstreams/{ids['upstream']}/credentials",
        {
            "id": ids["credential"],
            "kind": "bearer",
            "secret": kiro_key,
            "status": "active",
        },
    )
    ledger.record("credential_id", ids["credential"])

    session.call(
        "POST",
        f"/admin/endpoints/{ids['endpoint']}/credential-bindings",
        {
            "credential_id": ids["credential"],
            "enabled": True,
            "priority": 0,
            "weight": 1,
            # One concurrent request. The account is shared with the user's live kiro-rs, so the
            # measurement must not be able to burst against it.
            "concurrency": 1,
        },
    )
    ledger.record("binding", f"{ids['endpoint']}::{ids['credential']}")

    session.call(
        "POST",
        "/admin/public-models",
        {
            "id": ids["public_model"],
            "model_name": args.public_model,
            "status": "active",
            "display_name": args.public_model,
            # Empty for the same reason the Candidate needs `allow_unlisted_model`: a Public Model's
            # capability map is a *requirement* the effective Candidate profile must satisfy, and
            # P12 injects an empty profile for every Endpoint. Asserting `streaming` here would fail
            # the whole Version as `candidate_capability_mismatch`. Streaming still works -- it is a
            # property of the request, not something the graph has to advertise.
            "capabilities": {},
        },
    )
    ledger.record("public_model_id", ids["public_model"])

    session.call(
        "POST",
        f"/admin/public-models/{ids['public_model']}/routes",
        {
            "id": ids["route"],
            "policy": "smooth_weighted_round_robin",
            # One attempt. Retries all happen before the first byte, so they cannot corrupt a
            # measurement -- but they would silently mask an upstream failure that P12-06 needs to
            # see and count.
            "max_attempts": 1,
            "bootstrap_timeout_ms": 15000,
        },
    )
    ledger.record("route_id", ids["route"])

    session.call(
        "POST",
        f"/admin/routes/{ids['route']}/candidates",
        {
            "id": ids["candidate"],
            "endpoint_id": ids["endpoint"],
            "upstream_model": args.upstream_model,
            "credential_scope": "all_active",
            "transform_mode": "canonical",
            "enabled": True,
            "priority": 0,
            "weight": 1,
            # P12 injects an *empty* capability profile for every Endpoint on purpose: until a
            # controlled request has proven a capability, the graph must not advertise it, and the
            # deployment performs no catalog import at all. So a Candidate naming any real upstream
            # model is `catalog_model_not_eligible` unless it carries this exact override, which the
            # runtime accepts only as the sole key (`has_p12_unlisted_model_override`). Asserting
            # `tools` or `streaming` here would instead fail as capability escalation.
            "capability_override": {"allow_unlisted_model": True},
        },
    )
    ledger.record("candidate_id", ids["candidate"])
    session.call(
        "POST",
        "/admin/access-groups",
        {
            "id": ids["access_group"],
            "name": "P12-06 Kiro measurement",
            "status": "active",
            "limits": {},
        },
    )
    ledger.record("access_group_id", ids["access_group"])

    session.call(
        "POST",
        f"/admin/access-groups/{ids['access_group']}/routes",
        {"route_id": ids["route"], "enabled": True},
        expect=201,
    )
    ledger.record("access_group_route", f"{ids['access_group']}::{ids['route']}")

    issued = session.call(
        "POST",
        "/admin/client-keys",
        {
            "id": ids["client_key"],
            "access_group_id": ids["access_group"],
            "status": "active",
        },
    )
    # The full key is returned exactly once and has no recovery path, so write it before anything
    # else can fail. 0600 via O_CREAT|O_EXCL: refuse to clobber an existing file, which would
    # destroy a key issued by an earlier run.
    secret = issued.get("key")
    if not secret:
        raise ManagementError(
            f"client key response carried no key; fields: {sorted(issued)}"
        )
    descriptor = os.open(args.client_key_out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        handle.write(secret)
    ledger.record("client_key_id", ids["client_key"])
    # The prefix comes from the response, not from splitting the secret: it is the server's own
    # non-secret identifier and the value the Caddy split matches on. Deriving it by hand would
    # risk writing part of the secret into the ledger if the format ever changed.
    ledger.record("client_key_prefix", issued.get("prefix"))

    print(
        "enter-graph: graph entered. The new Endpoint identity has no capability profile in the\n"
        "             running process, so validate would fail with\n"
        "             missing_endpoint_capability_profile. Restart the service, then run --finish."
    )
    return 0


def finish(args: argparse.Namespace) -> int:
    """Validates and publishes an already-entered graph, after the service has been restarted.

    This is a separate phase because the Route Compiler builds its Endpoint capability table once,
    at process start, from the Endpoints already in the database. A draft that introduces a brand-new
    Endpoint identity therefore cannot validate in the process that created it: the compiler has no
    profile for that id and fails with `missing_endpoint_capability_profile`. One restart makes the
    Endpoint visible to a freshly built compiler, which is the same restart-after-publication
    lifecycle the deployment already has, so this splits the sequence rather than working around it.
    """
    session = Management(
        read_secret(args.management_key_file),
        config_version=args.version_id,
        base=args.management_base,
    )
    ledger = Ledger(args.ledger) if args.ledger else None
    route_id = "p12-06-kiro-route"

    session.call("POST", f"/admin/routes/{route_id}/validate", {}, expect=200)
    validated = session.call(
        "POST",
        f"/admin/config-versions/{args.version_id}/validate",
        {},
        expect=200,
    )
    if ledger:
        ledger.record("version_validate", validated)
    if not validated.get("valid"):
        print(f"finish: version did not validate: {validated}", file=sys.stderr)
        return 1

    if not args.publish:
        print("finish: validated but not published (pass --publish)")
        return 0

    # `validate` returns no ETag, so re-read the version for the revision `publish` demands in
    # If-Match. A stale If-Match returns 409 without partial mutation, so guessing is not safe.
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
        raise ManagementError("config version carried no revision before publish")
    published = session.call(
        "POST",
        f"/admin/config-versions/{args.version_id}/publish",
        {},
        expect=200,
        send_config_version=False,
    )
    if ledger:
        ledger.record("published", published)
    # `explain` is deliberately not called here either: it reads the composed runtime, which is
    # replaced only when the service restarts onto the newly published version. Use --explain-only.
    print("finish: published; restart cpa-rust-gateway, then run --explain-only")
    return 0


def explain_only(args: argparse.Namespace) -> int:
    """Reads the route explanation after the service has restarted onto the published version."""
    session = Management(
        read_secret(args.management_key_file),
        config_version=args.version_id,
        base=args.management_base,
    )
    explain = session.call(
        "GET",
        f"/admin/routes/p12-06-kiro-route/explain?requested_model={args.public_model}"
        "&protocol=anthropic_messages",
        None,
        expect=200,
        send_if_match=False,
    )
    print(json.dumps(explain, indent=2, sort_keys=True))
    return 0


def main() -> int:
    args = parse_args()
    try:
        if args.explain_only:
            return explain_only(args)
        if args.finish:
            return finish(args)
        return enter_graph(args)
    except ManagementError as error:
        print(f"enter-graph: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
