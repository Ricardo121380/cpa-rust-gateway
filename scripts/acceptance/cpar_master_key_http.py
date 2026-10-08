"""B-05 public restart checks sharing the existing private loopback harness."""
import json
import os
import secrets
import sqlite3
import subprocess


def verify(context):
    data, api, check, restart = (context[name] for name in ("data", "api", "check", "restart"))
    root, state, credentials = (context[name] for name in ("root", "state", "credentials"))
    calls = context["calls"]
    database_path = state / "control.sqlite3"

    def stored():
        status, raw, _ = data("/v1/responses", {**context["body_for"]("responses", "responses", "text-round"), "store": True})
        check("b05_stored_write", status == 200)
        return json.loads(raw)

    def compact(identifier):
        status, raw, _ = data("/v1/responses/compact", {"model": "model-responses", "previous_response_id": identifier, "stream": False})
        check("b05_compact_write", status == 200)
        return json.loads(raw)["output"][0]

    def versions(response_id, item, expected):
        with sqlite3.connect(database_path) as database:
            response_version = database.execute("SELECT key_version FROM stored_responses WHERE response_id=?", (response_id,)).fetchone()
            compact_version = database.execute("SELECT key_version FROM stored_response_compactions WHERE compact_id=?", (item["encrypted_content"],)).fetchone()
        check("b05_response_and_compact_active_version_" + str(expected), response_version == (expected,) and compact_version == (expected,))

    old = stored()
    old_compact = compact(old["id"])
    versions(old["id"], old_compact, 1)
    directory = root / "master-keys"
    directory.mkdir()
    old_key = (credentials / "master-key").read_bytes()
    (directory / "1.key").write_bytes(old_key)
    (directory / "2.key").write_bytes(secrets.token_bytes(32))
    # Remove the legacy input to prove explicit mode never silently consults it.
    (credentials / "master-key").unlink()
    context["serve_options"] = ["--master-key-dir", str(directory), "--active-master-key-version", "2"]
    previous_pid = context["gateway"].pid
    restart()
    check("b05_actual_process_restarted_with_versioned_configuration", context["gateway"].pid != previous_pid)
    status, raw, headers = data("/v1/responses/" + old["id"], method="GET")
    no_store = next((value for name, value in headers.items() if name.lower() == "cache-control"), None)
    check("b05_old_record_public_read", status == 200 and json.loads(raw)["output"] == old["output"] and no_store == "no-store")
    before = len(calls)
    status, _, _ = data("/v1/responses", {"model": "model-responses", "input": [old_compact, {"role": "user", "content": "next"}]})
    check("b05_old_compact_public_read", status == 200 and len(calls) == before + 1 and calls[-1]["account"] == "a")
    new = stored()
    new_compact = compact(new["id"])
    versions(new["id"], new_compact, 2)
    # Real management persistence must use the same active version as serving writes.
    draft = api("POST", "/admin/config-versions/local-b/fork", {"id": "b05-draft", "description": "Synthetic versioned-key write"}, expected=201)
    context["scope"], context["revision"] = draft["id"], draft["revision"]
    api("POST", "/admin/upstreams", {"id": "b05-upstream", "name": "Synthetic B05", "kind": "openai-compatible", "enabled": True, "tags": [], "egress_policy_id": "local-policy"}, expected=201)
    context["keys"]["c"] = "synthetic-" + secrets.token_hex(24)
    api("POST", "/admin/upstreams/b05-upstream/account-import", {"id": "b05-new-account", "channel": "openai-compatible", "secret": context["keys"]["c"]}, expected=201)
    with sqlite3.connect(database_path) as database:
        check("b05_management_active_version", database.execute("SELECT key_version FROM upstream_credentials WHERE config_version_id=? AND id=?", (draft["id"], "b05-new-account")).fetchone() == (2,))
    context["scope"] = "local-b"
    restart()
    for response in (old, new):
        status, _, _ = data("/v1/responses/" + response["id"], method="GET")
        check("b05_retained_and_active_versions_after_second_restart", status == 200)

    before = len(calls)
    status, missing, _ = data("/v1/responses/missing-b05", method="GET")
    check("b05_missing_not_found", status == 404)
    for method in ("GET", "DELETE"):
        status, foreign, _ = data("/v1/responses/" + old["id"], method=method, key=context["sibling_key"])
        check("b05_foreign_owner_same_not_found_" + method, status == 404 and foreign == missing)
    status, _, _ = data("/v1/responses/" + old["id"], method="GET", key="synthetic-invalid")
    check("b05_authentication_precedes_read", status == 401)
    status, _, _ = data("/v1/responses", {"model": "model-responses", "input": [old_compact, {"role": "user", "content": "next"}]}, key=context["sibling_key"])
    check("b05_compact_foreign_owner_no_upstream", status == 404 and len(calls) == before)
    with sqlite3.connect(database_path) as database:
        for table, column, identifier in (("stored_responses", "response_id", new["id"]), ("stored_response_compactions", "compact_id", new_compact["encrypted_content"])):
            blob = database.execute(f"SELECT ciphertext FROM {table} WHERE {column}=?", (identifier,)).fetchone()[0]
            database.execute(f"UPDATE {table} SET ciphertext=? WHERE {column}=?", (bytes([blob[0] ^ 1]) + blob[1:], identifier))
    status, _, _ = data("/v1/responses/" + new["id"], method="GET")
    check("b05_corrupt_response_fails_closed", status >= 400 and len(calls) == before)
    status, _, _ = data("/v1/responses", {"model": "model-responses", "input": [new_compact, {"role": "user", "content": "next"}]})
    check("b05_corrupt_compact_no_upstream", status >= 400 and len(calls) == before)

    # Failed startup uses other ports and an independent state directory; the owned
    # running gateway remains untouched. No test key bytes enter a log or receipt.
    invalid_state = root / "invalid-state"
    invalid_state.mkdir()
    base = [str(context["binary"]), "serve", "--state-dir", str(invalid_state), "--credential-dir", str(credentials), "--data-listen", "127.0.0.1:" + str(context["port"]()), "--management-listen", "127.0.0.1:" + str(context["port"]())]
    def rejected(name, extra):
        result = subprocess.run(base + extra, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15, env={**os.environ, "SSL_CERT_FILE": str(context["ca"])})
        check("b05_reject_" + name, result.returncode != 0 and len(calls) == before)
    for active in ("0", "-1", "01", "4294967296", "missing", "3"):
        rejected("invalid_active_" + active, ["--master-key-dir", str(directory), "--active-master-key-version", active])
    rejected("missing_active_option", ["--master-key-dir", str(directory)])
    rejected("missing_directory_option", ["--active-master-key-version", "2"])
    rejected("duplicate_version_option", context["serve_options"] + ["--active-master-key-version", "2"])
    (directory / "01.key").write_bytes(secrets.token_bytes(32))
    rejected("duplicate_noncanonical_version", context["serve_options"])
    (directory / "01.key").unlink()
    (directory / "3.key").symlink_to(directory / "2.key")
    rejected("symlink_key", context["serve_options"])
    (directory / "3.key").unlink()
    (directory / "3.key").write_bytes(b"invalid-test-length")
    rejected("wrong_key_length", context["serve_options"])
    (directory / "3.key").unlink()
    # A retained active configuration still references old credential envelopes.
    # Omitting its key must stop composition rather than expose partial serving.
    context["gateway"].terminate()
    context["gateway"].wait(timeout=30)
    (directory / "1.key").unlink()
    invalid_old = subprocess.run([str(context["binary"]), "serve", "--state-dir", str(state), "--credential-dir", str(credentials), "--data-listen", "127.0.0.1:" + str(context["port"]()), "--management-listen", "127.0.0.1:" + str(context["port"]()), *context["serve_options"]], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15, env={**os.environ, "SSL_CERT_FILE": str(context["ca"])})
    check("b05_missing_old_key_rejects_runtime_before_serving", invalid_old.returncode != 0 and len(calls) == before)
    # Explicit management updates create a key-2-only active configuration. This
    # lets public storage demonstrate a missing old key independently of startup.
    # These are intentional synthetic management writes, not implicit rotation.
    (directory / "1.key").write_bytes(old_key)
    restart()
    context["scope"] = draft["id"]
    with sqlite3.connect(database_path) as database:
        accounts = database.execute("SELECT id, kind FROM upstream_credentials WHERE config_version_id=?", (draft["id"],)).fetchall()
    for identifier, kind in accounts:
        material = context["keys"]["c" if identifier == "b05-new-account" else "a" if identifier.endswith("-a") else "b"]
        api("PATCH", "/admin/credentials/" + identifier, {"id": identifier, "kind": kind, "secret": material, "status": "active"})
    endpoint = api("GET", "/admin/endpoints/endpoint-responses")
    api("POST", "/admin/upstreams/b05-upstream/endpoints", {**{key: endpoint[key] for key in ("adapter_id", "api_format", "base_url", "inference_path", "models_path", "transport", "enabled")}, "id": "b05-endpoint"}, expected=201)
    api("POST", "/admin/endpoints/b05-endpoint/credential-bindings", {"credential_id": "b05-new-account", "enabled": True, "priority": 0, "weight": 1, "concurrency": 1}, expected=201)
    api("POST", "/admin/public-models", {"id": "b05-model", "model_name": "model-b05", "display_name": "Synthetic B05", "status": "active", "capabilities": {}}, expected=201)
    api("POST", "/admin/public-models/b05-model/routes", {"id": "b05-route", "policy": "smooth_weighted_round_robin", "max_attempts": 1, "bootstrap_timeout_ms": 20000}, expected=201)
    api("POST", "/admin/routes/b05-route/candidates", {"id": "b05-candidate", "endpoint_id": "b05-endpoint", "upstream_model": "model-b05", "credential_scope": "all_active", "transform_mode": "canonical_bridge", "enabled": True, "priority": 0, "weight": 1, "capability_override": {"allow_unlisted_model": True, "stored_responses": True, "response_compaction": True}}, expected=201)
    api("POST", "/admin/access-groups/local-group/routes", {"route_id": "b05-route", "enabled": True}, expected=201)
    context["publish"]()
    before = len(calls)
    status, raw, _ = data("/v1/responses", {"model": "model-b05", "input": "text-round", "store": True})
    active_response = json.loads(raw)
    check("b05_new_management_key_consumed_by_runtime", status == 200 and len(calls) == before + 1 and calls[-1]["account"] == "c")
    status, raw, _ = data("/v1/responses/compact", {"model": "model-b05", "previous_response_id": active_response["id"], "stream": False})
    check("b05_new_management_compact", status == 200)
    active_compact = json.loads(raw)["output"][0]
    context["gateway"].terminate()
    context["gateway"].wait(timeout=30)
    (directory / "1.key").unlink()
    restart()
    before = len(calls)
    status, _, _ = data("/v1/responses/" + old["id"], method="GET")
    check("b05_missing_old_response_key_public_failure", status >= 400 and len(calls) == before)
    status, _, _ = data("/v1/responses", {"model": "model-responses", "input": [old_compact, {"role": "user", "content": "next"}]})
    check("b05_missing_old_compact_key_public_failure", status >= 400 and len(calls) == before)
    status, foreign, _ = data("/v1/responses/" + old["id"], method="GET", key=context["sibling_key"])
    check("b05_missing_old_key_foreign_owner_stays_not_found", status == 404 and foreign == missing)
    status, _, _ = data("/v1/responses/" + active_response["id"], method="GET")
    check("b05_current_key_response_remains_readable", status == 200)
    status, _, _ = data("/v1/responses", {"model": "model-b05", "input": [active_compact, {"role": "user", "content": "next"}]})
    check("b05_current_key_compact_after_restart", status == 200 and len(calls) == before + 1 and calls[-1]["account"] == "c")
