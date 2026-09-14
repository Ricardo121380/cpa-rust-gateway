"""Plan/apply exact legacy-alias retirement through management CAS; never calls providers.

Run against an isolated production copy first. The plan is bound to the full
visible routing/permission graph; an interrupted apply is never replayed.
Credentials stay in the existing local credential directory, not the receipt.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import urllib.error
import urllib.parse
import urllib.request

LEGACY = frozenset(("p12-chatgpt-go", "grok-cpar-build", "grok-cpar-console",
                    "krill-cpar-chat", "krill-cpar-messages", "krill-cpar-responses"))


class Management:
    def __init__(self, base, origin, directory):
        self.base, self.scope, self.revision = base.rstrip("/"), None, None
        directory = Path(directory)
        self.headers = {"Origin": origin, "Content-Type": "application/json",
                        "X-Management-Key": (directory / "management-key").read_text().strip(),
                        "X-Management-CSRF-Token": (directory / "management-csrf").read_text().strip()}
        self.http = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def request(self, path, method="GET", body=None, extra=None):
        headers = dict(self.headers)
        if self.scope:
            headers["X-Config-Version"] = self.scope
        if method != "GET" and self.revision:
            headers["If-Match"] = self.revision
        headers.update(extra or {})
        request = urllib.request.Request(self.base + path, method=method, headers=headers,
                                        data=None if body is None else json.dumps(body).encode())
        try:
            response = self.http.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            raise RuntimeError(f"management HTTP {error.code}; inspect the draft, do not replay") from None
        with response:
            raw = response.read(8_000_001)
            if len(raw) > 8_000_000:
                raise RuntimeError("management response exceeds bound")
            if method != "GET" and response.headers.get("ETag"):
                self.revision = response.headers["ETag"]
            return json.loads(raw) if raw else None

    def pages(self, path):
        items, cursor, revision = [], None, None
        while True:
            query = {"limit": 100}
            if cursor:
                query["cursor"] = cursor
            page = self.request(path + "?" + urllib.parse.urlencode(query))
            if revision is not None and revision != page["revision"]:
                raise RuntimeError("enumeration changed")
            revision = page["revision"]
            items.extend(page["items"])
            cursor = page["next_cursor"]
            if len(items) > 10000 or (len(items) == 10000 and cursor):
                raise RuntimeError("inventory exceeds bound")
            if not cursor:
                return items


def graph(api):
    groups = api.request("/admin/access-groups")
    result = {"models": api.request("/admin/public-models"),
              "upstreams": api.request("/admin/upstreams"),
              "endpoints": api.pages("/admin/endpoints"),
              "routes": api.pages("/admin/routes"),
              "candidates": api.pages("/admin/route-candidates"),
              "aliases": api.pages("/admin/model-aliases"),
              "groups": groups, "keys": [{k: row.get(k) for k in ("id", "access_group_id", "prefix", "status", "expires_at_ms")}
                  for row in api.request("/admin/client-keys")]}
    result["grants"] = {g["id"]: api.request("/admin/access-groups/" +
        urllib.parse.quote(g["id"], safe="") + "/routes") for g in groups}
    return result


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def inspect(api):
    active = next(v for v in api.request("/admin/config-versions") if v["status"] == "active")
    api.scope, api.revision = active["id"], active["revision"]
    before = graph(api)
    selected = [a for a in before["aliases"] if a["alias"] in LEGACY]
    if {a["alias"] for a in selected} != LEGACY:
        raise RuntimeError("expected exactly the six approved legacy aliases")
    if any(m["model_name"] in LEGACY for m in before["models"]):
        raise RuntimeError("legacy names are still canonical models; alias retirement is not applicable")
    # Bracket enumeration with the active version/revision, in addition to page cursors.
    current = next(v for v in api.request("/admin/config-versions") if v["status"] == "active")
    if current != active:
        raise RuntimeError("active configuration changed during inspection")
    return active, before, selected


def run(api, expected=None, target=None, upstream_names=None, catalog_paths=None):
    active, before, selected = inspect(api)
    upstream_names = upstream_names or {}
    catalog_paths = catalog_paths or {}
    if not set(catalog_paths).issubset({e["id"] for e in before["endpoints"]}) or any(not isinstance(p, str) or not p.startswith("/") or p.startswith("//") or "?" in p or "#" in p for p in catalog_paths.values()):
        raise RuntimeError("invalid reviewed endpoint catalog paths")
    if not set(upstream_names).issubset({u["id"] for u in before["upstreams"]}):
        raise RuntimeError("unknown upstream in reviewed name mapping")
    if any(not isinstance(n, str) or not n.strip() or len(n) > 128 for n in upstream_names.values()):
        raise RuntimeError("invalid formal upstream name")
    changes = [{"id": u["id"], "from": u["name"], "to": upstream_names[u["id"]]}
               for u in before["upstreams"] if u["id"] in upstream_names and u["name"] != upstream_names[u["id"]]]
    plan = {"source": active["id"], "revision": active["revision"],
            "fingerprint": digest(before), "remove": sorted(LEGACY), "upstream_names": changes, "catalog_paths": catalog_paths, "provider_calls": 0}
    if expected is None:
        return plan
    if plan != expected or not target:
        raise RuntimeError("plan changed or target missing")
    if any(v["id"] == target for v in api.request("/admin/config-versions")):
        raise RuntimeError("target already exists; inspect it, do not replay")
    draft = api.request("/admin/config-versions/" + urllib.parse.quote(active["id"], safe="") + "/fork",
                        "POST", {"id": target, "description": "Retire six legacy model aliases"})
    api.scope, api.revision = target, draft["revision"]
    for upstream in before["upstreams"]:
        if upstream["id"] in upstream_names and upstream["name"] != upstream_names[upstream["id"]]:
            api.request("/admin/upstreams/" + urllib.parse.quote(upstream["id"], safe=""), "PATCH",
                        {**upstream, "name": upstream_names[upstream["id"]]})
    for endpoint in before["endpoints"]:
        if endpoint["id"] in catalog_paths and endpoint.get("models_path") != catalog_paths[endpoint["id"]]:
            fields = {key: endpoint[key] for key in ("id", "adapter_id", "api_format", "base_url", "inference_path", "models_path", "transport", "enabled")}
            fields["models_path"] = catalog_paths[endpoint["id"]]
            api.request("/admin/endpoints/" + urllib.parse.quote(endpoint["id"], safe=""), "PATCH", fields)
    for alias in selected:
        api.request("/admin/public-models/" + urllib.parse.quote(alias["public_model_id"], safe="") +
                    "/aliases", "DELETE", {"alias": alias["alias"]})
    expected_graph = {**before, "aliases": [a for a in before["aliases"] if a["alias"] not in LEGACY]}
    expected_graph["upstreams"] = [{**u, "name": upstream_names.get(u["id"], u["name"])} for u in before["upstreams"]]
    expected_graph["endpoints"] = [{**e, "models_path": catalog_paths.get(e["id"], e.get("models_path"))} for e in before["endpoints"]]
    if graph(api) != expected_graph:
        raise RuntimeError("draft changed beyond approved aliases; not publishing")
    valid = api.request("/admin/config-versions/" + target + "/validate", "POST", {})
    if not valid["valid"]:
        raise RuntimeError("draft validation failed; not publishing")
    events = api.request("/admin/audit-events")
    event = max([e["id"] for e in events if e["action"] in ("config_published", "config_rolled_back")] + [0])
    receipt = api.request("/admin/config-versions/" + target + "/publish", "POST", {},
                          {"X-Expected-Active-Version": json.dumps(active["id"]),
                           "X-Expected-Lifecycle-Event": str(event)})
    if receipt["active_config_version_id"] != target or graph(api) != expected_graph:
        raise RuntimeError("published readback differs; inspect before any further change")
    return {**plan, "published": target, "preserved_graph": True}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--origin", required=True)
    parser.add_argument("--credential-dir", required=True)
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--apply-plan")
    parser.add_argument("--target")
    parser.add_argument("--upstream-names", help="Reviewed JSON object of exact upstream IDs to formal names")
    parser.add_argument("--catalog-paths", help="Reviewed exact endpoint ID to models path mapping")
    args = parser.parse_args()
    expected = json.loads(Path(args.apply_plan).read_text()) if args.apply_plan else None
    names = json.loads(Path(args.upstream_names).read_text()) if args.upstream_names else None
    paths = json.loads(Path(args.catalog_paths).read_text()) if args.catalog_paths else None
    result = run(Management(args.base, args.origin, args.credential_dir), expected, args.target, names, paths)
    descriptor = os.open(args.receipt, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(descriptor, "w") as output:
        json.dump(result, output, indent=2)
    print("Alias retirement receipt saved; provider calls: 0")
