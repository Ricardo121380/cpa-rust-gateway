"""Observe stock serve background controls at the actual loopback receiver.

Synthetic device grants, owned state, fixed-host TLS interposition and an OS
outbound deny are supplied by the existing process fixture. Explicit device
authorization seeds the account before counts are frozen; it is not a worker.
"""
import hashlib
import json
import os
import pathlib
import sqlite3
import subprocess
import sys
import time
import urllib.error
import urllib.request

os.umask(0o077)
out = pathlib.Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=True)
fixture = pathlib.Path(__file__).with_name("cpar_seo_ui_http.py")
client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
result = {"result": "FAIL", "scope": "stock process / public HTTP / synthetic TLS loopback",
          "real_provider_requests": 0, "cases": []}


def exercise(mode):
    case_out = out / mode
    case_out.mkdir()
    server_out = case_out / "server"
    server_out.mkdir()
    log = (case_out / "controller.log").open("wb")
    env = {**os.environ, "CPAR_UI_OPERATIONS": "1"}
    env.pop("CPAR_UI_BACKGROUND_PROVIDER_TASKS", None)
    if mode == "disabled":
        env["CPAR_UI_BACKGROUND_PROVIDER_TASKS"] = mode
    server = subprocess.Popen([sys.executable, str(fixture), str(server_out)],
                              stdout=log, stderr=log, env=env)
    case = {"mode": mode, "result": "FAIL", "checks": []}
    scope = revision = None

    def wait_for(predicate, timeout=30):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if server.poll() is not None:
                raise RuntimeError("owned fixture exited")
            try:
                value = predicate()
                if value:
                    return value
            except (OSError, ValueError, sqlite3.OperationalError):
                pass
            time.sleep(.1)
        raise AssertionError("controlled boundary did not reach expected state")

    def descriptor():
        return json.loads((server_out / "descriptor.json").read_text())

    def api(method, path, body=None, expected=200, extra=None):
        nonlocal revision
        headers = {"X-Management-Key": management, "Content-Type": "application/json",
                   **({"X-Config-Version": scope} if scope else {}),
                   **({"If-Match": revision} if revision and method != "GET" else {}),
                   **(extra or {})}
        request = urllib.request.Request(d["origin"] + path, method=method, headers=headers,
                                         data=None if body is None else json.dumps(body).encode())
        try:
            response = client.open(request, timeout=20)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            value = json.loads(response.read() or "null")
            code = value.get("error", {}).get("code") if isinstance(value, dict) else None
            assert response.status == expected, (method, path, response.status, code)
            if method != "GET" and response.headers.get("ETag"):
                revision = response.headers["ETag"]
            return value

    def counts():
        device = json.loads((server_out / "device-receives.json").read_text())
        provider = json.loads((server_out / "provider-receives.json").read_text())
        return {"oauth_refresh": device["refresh_requests"],
                "oauth_events": len(device["events"]),
                "catalog": sum(row["method"] == "GET" and row["path"] == "/v1/models" for row in provider),
                "inference": sum(row.get("protocol") == "grok-build-native" for row in provider)}

    def account():
        with sqlite3.connect(pathlib.Path(d["state"]) / "control.sqlite3") as db:
            row = db.execute("SELECT revision,refresh_due_at_ms,worker_claim_kind FROM grok_accounts WHERE provider='build'").fetchone()
        return {"revision": row[0], "refresh_due_at_ms": row[1], "claim": row[2]}

    def publish():
        assert api("POST", f"/admin/config-versions/{scope}/validate")["valid"]
        active = next(row for row in api("GET", "/admin/config-versions") if row["status"] == "active")
        event = max(row["id"] for row in api("GET", "/admin/audit-events") if row["action"] == "config_published")
        api("POST", f"/admin/config-versions/{scope}/publish", extra={
            "X-Expected-Active-Version": json.dumps(active["id"]),
            "X-Expected-Lifecycle-Event": str(event)})

    try:
        d = wait_for(descriptor)
        case.update(source_sha=d["source_sha"], binary_sha256=d["binary_sha256"],
                    initial_pid=d["gateway_pid"], outside_loopback_denied=d["outside_loopback_denied_by_os_sandbox"],
                    actual_serve_arguments=d["command"])
        assert case["outside_loopback_denied"]
        management = (pathlib.Path(d["root"]) / "credentials/management-key").read_text().strip()
        started = api("POST", "/admin/native-account-authorizations", {"name": "background-controlled"})

        def terminal():
            view = api("POST", f"/admin/native-account-authorizations/{started['session_id']}/poll")
            return view if view["state"] != "pending" else None

        saved = wait_for(terminal, 20)
        assert saved["state"] == "complete" and saved["runtime_applied"]
        # The receiver's value-free snapshot is flushed every 200 ms. Freeze only
        # after the completed explicit seed has reached that independent file.
        time.sleep(.5)
        case["seed_counts"] = wait_for(counts)
        active = next(row for row in api("GET", "/admin/config-versions") if row["status"] == "active")
        scope, revision = active["id"], active["revision"]
        draft = api("POST", f"/admin/config-versions/{scope}/fork", {
            "id": "background-active", "description": "Owned background Provider control acceptance"}, 201)
        scope, revision = draft["id"], draft["revision"]
        api("POST", "/admin/egress-policies", {"id": "background-fixed", "name": "Fixed Build under OS deny",
            "allowed_schemes": ["https"], "allowed_hosts": ["cli-chat-proxy.grok.com"], "allowed_ports": [443],
            "allowed_cidrs": ["127.0.0.1/32"], "redirect_mode": "deny", "max_redirects": 0}, 201)
        api("POST", "/admin/upstreams", {"id": "background-build", "name": "Controlled native Build",
            "kind": "grok-build-native", "enabled": True, "tags": [], "egress_policy_id": "background-fixed"}, 201)
        api("POST", "/admin/upstreams/background-build/endpoints", {"id": "background-endpoint",
            "adapter_id": "grok.build.responses", "api_format": "openai/responses",
            "base_url": "https://cli-chat-proxy.grok.com/v1", "inference_path": "/responses",
            "models_path": "/models", "transport": "https", "enabled": True}, 201)
        api("POST", "/admin/public-models", {"id": "background-model", "model_name": "grok-model",
            "display_name": "Controlled Build", "status": "active", "capabilities": {}}, 201)
        api("POST", "/admin/public-models/background-model/routes", {"id": "background-route",
            "policy": "smooth_weighted_round_robin", "max_attempts": 1, "bootstrap_timeout_ms": 10000}, 201)
        api("POST", "/admin/routes/background-route/candidates", {"id": "background-candidate",
            "endpoint_id": "background-endpoint", "upstream_model": "grok-model", "credential_scope": "all_active",
            "transform_mode": "canonical", "enabled": True, "priority": 0, "weight": 1,
            "capability_override": {"allow_unlisted_model": True}}, 201)
        api("POST", "/admin/access-groups", {"id": "background-group", "name": "Controlled owner",
            "status": "active", "limits": {}}, 201)
        api("POST", "/admin/access-groups/background-group/routes", {"route_id": "background-route", "enabled": True}, 201)
        key = api("POST", "/admin/client-keys", {"id": "background-owner", "access_group_id": "background-group",
            "status": "active", "expires_at_ms": None}, 201)["key"]
        publish()
        if mode == "default":
            wait_for(lambda: counts()["catalog"] >= 1)
        else:
            time.sleep(1)
            assert counts() == case["seed_counts"]
        case["after_first_publication"] = counts()
        before = account()
        (server_out / "e05-restart").touch()
        restarted = wait_for(lambda: descriptor() if descriptor()["restart_count"] == 1 else None)
        wait_for(lambda: api("GET", "/admin/config-versions"))
        assert restarted["gateway_pid"] != case["initial_pid"]
        assert restarted["prior_process"]["exit_code"] == 0
        if mode == "default":
            wait_for(lambda: account()["revision"] == before["revision"] + 1)
            wait_for(lambda: counts()["oauth_refresh"] == 1)
        else:
            time.sleep(1)
            assert account()["revision"] == before["revision"] and account()["refresh_due_at_ms"] == 0
            assert counts() == case["seed_counts"]
        case.update(restarted_pid=restarted["gateway_pid"], after_startup=counts(), startup_account=account())
        # Force only the owned synthetic row due, then publish a second generation.
        with sqlite3.connect(pathlib.Path(d["state"]) / "control.sqlite3") as db:
            db.execute("UPDATE grok_accounts SET refresh_due_at_ms=0 WHERE provider='build'")
            # Preserve the Catalog clock ordering and its default policy durations.
            # Two days makes this owned 24-hour refresh due, while its 72-hour
            # expiry remains in the future. Disabled mode has no Catalog rows.
            db.execute("UPDATE model_catalog_targets SET observed_at_ms=observed_at_ms-172800000, "
                       "stale_at_ms=stale_at_ms-172800000, refresh_due_at_ms=refresh_due_at_ms-172800000, "
                       "expires_at_ms=expires_at_ms-172800000")
        prior_catalog = counts()["catalog"]
        draft = api("POST", f"/admin/config-versions/{scope}/fork", {
            "id": "background-republished", "description": "Owned next runtime generation"}, 201)
        scope, revision = draft["id"], draft["revision"]
        publish()
        if mode == "default":
            wait_for(lambda: counts()["catalog"] > prior_catalog)
        else:
            assert counts() == case["seed_counts"]
        case["after_republication"] = counts()
        started_wait = time.monotonic()
        while time.monotonic() - started_wait < 65:
            assert server.poll() is None
            if mode == "disabled":
                assert counts() == case["seed_counts"]
            time.sleep(.2)
        case["observed_worker_interval_seconds"] = time.monotonic() - started_wait
        case["after_worker_interval"] = counts()
        if mode == "default":
            assert counts()["oauth_refresh"] == 2
        else:
            assert counts() == case["seed_counts"]
            assert account()["revision"] == before["revision"] and account()["refresh_due_at_ms"] == 0
        wait_for(lambda: any(row.get("account_id") == saved["account_id"] and row["active_leases"] == 0
                            for row in api("GET", "/admin/operations/provider-account-pools?limit=100")["items"]))
        request = urllib.request.Request(d["data_origin"] + "/v1/responses", method="POST",
            headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"},
            data=json.dumps({"model": "grok-model", "input": "controlled explicit request", "stream": False,
                "store": True, "parallel_tool_calls": False, "reasoning": {"effort": "low", "summary": "auto"},
                "tools": [{"type": "function", "name": "local_tool", "parameters": {"type": "object"}}]}).encode())
        with client.open(request, timeout=20) as response:
            assert response.status == 200 and json.loads(response.read())["status"] == "completed"
        wait_for(lambda: counts()["inference"] == 1)
        case["after_explicit_inference"] = counts()
        case["checks"] = ["startup refresh mode", "publication catalog mode", "republication subscription mode",
                          "actual one-minute worker mode", "explicit public inference retained", "owned restart and OS deny"]
        case["result"] = "PASS"
        print(json.dumps({"mode": mode, "result": "PASS", "receiver_counts": counts()}), flush=True)
    except Exception as error:
        case["error"] = str(error)
        raise
    finally:
        (server_out / "stop").touch()
        if server.poll() is None:
            server.wait(timeout=40)
        case["controller_exit_code"] = server.returncode
        log.close()
        result["cases"].append(case)
        (case_out / "receipt.json").write_text(json.dumps(case, indent=2) + "\n")


try:
    for mode in ["default", "disabled"]:
        exercise(mode)
    assert all(case["result"] == "PASS" and case["controller_exit_code"] == 0 for case in result["cases"])
    result["result"] = "PASS"
finally:
    result["driver_sha256"] = hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()
    (out / "background-provider-process.json").write_text(json.dumps(result, indent=2) + "\n")
