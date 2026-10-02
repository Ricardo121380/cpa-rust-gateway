"""Runs inside private mount/network/PID namespaces; only safe assertions escape."""
import datetime
import hashlib
import json
import os
import pathlib
import socket
import sqlite3
import subprocess
import sys
import time
import urllib.error
import urllib.request

expected_sha, expected_binary_hash, expected_schema = sys.argv[1], sys.argv[2], int(sys.argv[3])
result = {"status": "FAIL", "expected_source_sha": expected_sha, "binary_sha256": hashlib.sha256(pathlib.Path("/gateway").read_bytes()).hexdigest(), "namespace": {"network": os.readlink("/proc/self/ns/net"), "mount": os.readlink("/proc/self/ns/mnt"), "pid": os.readlink("/proc/self/ns/pid")}, "uid": os.getuid(), "gid": os.getgid(), "production_state_visible": os.path.exists("/var/lib/cpa-rust-gateway"), "host_credentials_visible": os.path.exists("/etc/cpa-rust-gateway/credentials"), "external_routes": pathlib.Path("/proc/net/route").read_text().splitlines()[1:], "real_provider_calls": 0}
assert result["binary_sha256"] == expected_binary_hash
assert result["uid"] == 995 and result["gid"] == 986
capabilities = dict(line.split(":",1) for line in pathlib.Path("/proc/self/status").read_text().splitlines() if ":" in line)
assert int(capabilities["CapEff"].strip(),16) == 0 and int(capabilities["CapBnd"].strip(),16) == 0
assert capabilities["NoNewPrivs"].strip() == "1"
result["effective_and_bounding_capabilities_zero"] = True
result["no_new_privileges"] = True
assert not result["production_state_visible"] and not result["host_credentials_visible"] and not result["external_routes"]
with socket.socket() as probe:
    probe.settimeout(1)
    try:
        probe.connect(("192.0.2.1",443))
        raise AssertionError("external_network_not_blocked")
    except OSError as error:
        result["reserved_external_connect_blocked"] = error.errno in [101, 113, 1, 13]
assert result["reserved_external_connect_blocked"]
log = open("/work/gateway.private.log", "wb")
process = subprocess.Popen(["/gateway", "serve", "--state-dir", "/state", "--credential-dir", "/credentials", "--data-listen", "127.0.0.1:28180", "--management-listen", "127.0.0.1:28181"], stdout=log, stderr=log)
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
try:
    deadline = time.monotonic()+35
    while True:
        try:
            with opener.open("http://127.0.0.1:28180/healthz",timeout=1) as reply:
                assert reply.status == 200; result["healthz"] = "PASS"; break
        except Exception:
            if process.poll() is not None or time.monotonic() >= deadline:
                raise AssertionError("isolated_startup_failed_or_timed_out")
            time.sleep(0.1)
    key = pathlib.Path("/credentials/management-key").read_text().strip()
    http = {}
    headers = {"X-Management-Key":key}
    for route in ["/admin/system", "/admin/config-versions", "/admin/client-keys", "/admin/access-groups", "/admin/routes", "/admin/native-accounts?limit=100", "/admin/operations/account-pools?limit=100", "/admin/operations/provider-account-pools?limit=100"]:
        result["current_read_route"] = route
        request = urllib.request.Request("http://127.0.0.1:28181"+route, headers=headers)
        with opener.open(request,timeout=5) as reply:
            body = reply.read(4*1024*1024)
            data = json.loads(body)
            assert reply.status == 200
            http[route] = {"status":reply.status,"json_valid":True}
        if route == "/admin/config-versions":
            active = [item for item in data if item["status"] == "active"]
            assert len(active) == 1
            with sqlite3.connect("file:/state/control.sqlite3?mode=ro",uri=True) as database:
                expected_active = database.execute("SELECT id,revision FROM config_versions WHERE status='active'").fetchall()
            # This verifies the documented management "rev-N" projection only.
            # It supplies no actual execution identity or credential/config lease revision.
            assert [(identifier,"rev-"+str(revision)) for identifier,revision in expected_active] == [(active[0]["id"],active[0]["revision"])]
            headers["X-Config-Version"] = active[0]["id"]
            result["active_configuration_identity_and_management_revision_match_database"] = True
            result["execution_revision_inferred_from_management_etag"] = False
            http[route]["count"] = len(data)
        if isinstance(data,dict) and "items" in data:
            http[route]["count"] = len(data["items"])
            http[route]["no_more_pages"] = data.get("next_cursor") is None
            assert http[route]["no_more_pages"]
        if route == "/admin/system":
            build = data["build"]
            assert build["build_revision"] == expected_sha and build["schema_version"] == expected_schema
            assert build["build_target"] == "aarch64-unknown-linux-gnu" and build["rust_version"] == "1.97.1" and build["version"] == "0.1.0"
            result["actual_build"] = build
    result["protected_read_only_endpoints"] = http
    result.pop("current_read_route",None)
    result["isolated_listeners"] = ["127.0.0.1:28180", "127.0.0.1:28181"]
    result["status"] = "PASS"
except Exception as error:
    result["failure_class"] = type(error).__name__
    if isinstance(error,urllib.error.HTTPError): result["http_failure_code"] = error.code
    result["failure_category"] = "isolated_startup_or_protected_read_failed"
finally:
    process.terminate()
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        process.kill(); process.wait(); result["forced_kill"] = True; result["status"] = "FAIL"
    log.close()
    result["observed_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    result["isolated_process_stopped"] = process.poll() is not None
    pathlib.Path("/work/startup-receipt.json").write_text(json.dumps(result,indent=2)+"\n")
    print(json.dumps(result))
sys.exit(0 if result["status"] == "PASS" else 1)
