"""T12: historical binary creates synthetic state; current binary upgrades it.

Uses the Batch B public HTTP/TLS harness. All key bytes and SQLite snapshots stay
in its private temporary directory. Receipts contain only counts, hashes and
closed outcomes. The historical binaries are separate, immutable build inputs.
"""
import base64
import hashlib
import json
import os
import pathlib
import secrets
import shutil
import sqlite3
import subprocess
import sys
import time


NETWORK_PROFILE = '(version 1) (allow default) (deny network-outbound) (allow network-outbound (remote ip "localhost:*"))'


def sandbox_command(argv):
    return ["/usr/bin/sandbox-exec", "-p", NETWORK_PROFILE, *argv]


def verify(context):
    api, data, check = (context[name] for name in ("api", "data", "check"))
    root, state, credentials = (context[name] for name in ("root", "state", "credentials"))
    current = pathlib.Path(os.environ["CPAR_T12_CURRENT_BINARY"]).resolve()
    old_reader = pathlib.Path(os.environ["CPAR_T12_OLD_READER_BINARY"]).resolve()
    old = context["binary"]
    database_path = state / "control.sqlite3"
    active_database_path = database_path
    report = {"scope": "isolated_synthetic_only", "real_provider_calls": 0, "binary_sha256": {
        name: hashlib.sha256(binary.read_bytes()).hexdigest()
        for name, binary in (("schema32", old), ("pre_attempt_usage_schema34", old_reader), ("current", current))
    }, "snapshots": {}, "processes": []}
    report["processes"].append({"binary_sha256": report["binary_sha256"]["schema32"], "pid": context["gateway"].pid})
    (context["out"] / "t12.json").write_text(json.dumps(report, indent=2) + "\n")

    def save_report():
        report["status"] = "PASS"
        (context["out"] / "t12.json").write_text(json.dumps(report, indent=2) + "\n")

    def stopped():
        process = context["gateway"]
        process.terminate()
        process.wait(timeout=30)
        check("t12_owned_process_stopped_and_flushed", process.returncode == 0)
        with sqlite3.connect(active_database_path) as database:
            database.execute("PRAGMA wal_checkpoint(TRUNCATE)")

    def start(binary, options=(), state_directory=state, credential_directory=credentials):
        nonlocal active_database_path
        active_database_path = state_directory / "control.sqlite3"
        context["binary"] = binary
        command = sandbox_command([str(binary), "serve", "--state-dir", str(state_directory),
            "--credential-dir", str(credential_directory), "--data-listen", f"127.0.0.1:{context['data_port']}",
            "--management-listen", f"127.0.0.1:{context['admin_port']}", *options])
        context["gateway"] = subprocess.Popen(command, stdout=context["log"], stderr=context["log"],
            env={**os.environ, "SSL_CERT_FILE": str(context["ca"])})
        context["wait_ready"]()
        report["processes"].append({"binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            "pid": context["gateway"].pid})

    def schema():
        with sqlite3.connect(database_path) as database:
            return database.execute("SELECT max(version) FROM schema_migrations").fetchone()[0]

    tables = ("config_versions", "upstreams", "upstream_endpoints", "upstream_credentials",
        "endpoint_credential_bindings", "public_models", "model_routes", "route_candidates",
        "access_groups", "access_group_routes", "client_keys", "gateway_event_log",
        "billing_ledger_entries", "management_audit_events", "management_resource_audit_events",
        "grok_accounts", "native_account_identity_observations", "stored_responses")

    def snapshot(columns=None):
        result = {}
        with sqlite3.connect(database_path) as database:
            check("t12_sqlite_quick_and_foreign_key_checks", database.execute("PRAGMA quick_check").fetchone() == ("ok",)
                and not database.execute("PRAGMA foreign_key_check").fetchall())
            for table in tables:
                names = columns[table]["columns"] if columns else [row[1] for row in database.execute(f"PRAGMA table_info({table})")]
                assert names, table
                rows = database.execute(f"SELECT {','.join(names)} FROM {table}").fetchall()
                encoded = sorted(json.dumps([{"blob_sha256": hashlib.sha256(value).hexdigest()} if isinstance(value, bytes) else value
                    for value in row], separators=(",", ":")) for row in rows)
                result[table] = {"columns": names, "rows": len(rows), "sha256": hashlib.sha256(json.dumps(encoded).encode()).hexdigest()}
        return result

    def stored(stream=False):
        before = len(context["all_receives"])
        status, raw, _ = data("/v1/responses", {**context["body_for"]("responses", "responses", "text-round", stream), "store": True})
        response = context["completed"]("responses", raw, stream)
        check("t12_public_stored_json_or_sse_one_actual_receive", status == 200 and response["status"] == "completed"
            and len(context["all_receives"]) == before + 1)
        return response

    def readable(response):
        before = len(context["all_receives"])
        status, raw, _ = data("/v1/responses/" + response["id"], method="GET")
        check("t12_encrypted_owner_read_without_upstream", status == 200 and json.loads(raw)["output"] == response["output"]
            and len(context["all_receives"]) == before)
        status, _, _ = data("/v1/responses/" + response["id"], method="GET", key=context["sibling_key"])
        check("t12_same_group_foreign_client_key_cannot_read", status == 404 and len(context["all_receives"]) == before)

    def wait_ledger(response_ids):
        deadline = time.monotonic() + 15
        while True:
            items = context["all_items"]("/admin/operations/billing?limit=100")
            if response_ids <= {row["response_id"] for row in items}:
                return items
            assert time.monotonic() < deadline, "T12 real billing worker deadline"
            time.sleep(0.1)

    # Verify the exact OS network restriction before creating any native material.
    code = '''import socket,sys
with socket.create_connection(("127.0.0.1", int(sys.argv[1])), timeout=2): pass
try: socket.create_connection(("192.0.2.1",443),timeout=.5)
except PermissionError as error: assert error.errno == 1
else: raise AssertionError("non-loopback was not denied")
'''
    subprocess.run(sandbox_command([sys.executable, "-c", code, str(context["provider"].server_port)]), check=True,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    check("t12_os_denies_non_loopback_network", True)
    check("t12_historical_binary_created_schema32", schema() == 32)

    # Synthetic display identity is in an unsigned fixture token; it grants no authority.
    claims = base64.urlsafe_b64encode(json.dumps({"sub": "synthetic-t12-subject", "email": "t12@example.invalid",
        "name": "Synthetic T12", "client_id": "synthetic-client", "scope": "chat"}).encode()).decode().rstrip("=")
    token = "eyJhbGciOiJub25lIn0." + claims + ".synthetic"
    api("POST", "/admin/native-accounts/import", {"id": "t12-native", "channel": "grok.build",
        "secret": json.dumps({"access_token": "synthetic-t12-access", "id_token": token,
            "refresh_token": "synthetic-t12-refresh", "expires_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() + 3600))})}, expected=201)
    native = api("GET", "/admin/native-account-imports/t12-native")
    native_id = native["account_id"]
    account = next(row for row in api("GET", "/admin/native-accounts?limit=100")["items"] if row["id"] == native_id)
    check("t12_native_display_identity_observed", account["identity"]["email"] == "t12@example.invalid")
    api("POST", "/admin/native-accounts/" + native_id + "/identity", {"revision": account["revision"]})
    api("PATCH", "/admin/native-accounts/" + native_id, {"revision": account["revision"], "enabled": False})
    draft = api("POST", "/admin/config-versions/local-b/fork", {"id": "t12-existing", "description": "Synthetic existing disabled intent"}, expected=201)
    context["scope"], context["revision"] = draft["id"], draft["revision"]
    ordinary = api("GET", "/admin/credentials/account-responses-b")
    api("PATCH", "/admin/credentials/account-responses-b/status", {"status": "disabled", "credential_revision": ordinary["revision"]})
    context["publish"]()
    old_responses = [stored(False), stored(True)]
    # A separate old Chat request supplies bounded conflicting legacy history.
    status, _, _ = data("/v1/chat/completions", context["body_for"]("chat", "chat", "text-round"))
    check("t12_legacy_chat_request", status == 200)
    wait_ledger({response["id"] for response in old_responses})
    before = len(context["all_receives"])
    status, _, _ = data("/v1/responses", {"model": "model-responses", "input": "restricted"}, key=context["restricted_key"])
    check("t12_existing_client_key_permission_denied", status == 404 and len(context["all_receives"]) == before)
    stopped()
    with sqlite3.connect(database_path) as database:
        payload = json.loads(database.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='usage' "
            "AND json_extract(payload_json,'$.usage.response_id')=?", (context["calls"][-1]["response_id"],)).fetchone()[0])
        for index in (1, 2):
            unknown = json.loads(json.dumps(payload))
            unknown["usage"]["attempt_id"] = None
            unknown["usage"]["response_id"] = "t12-legacy-unknown-" + str(index)
            database.execute("INSERT INTO gateway_event_log(event_type,event_id,request_id,payload_json) VALUES('usage',?,?,?)",
                (unknown["usage"]["response_id"], unknown["usage"]["request_id"], json.dumps(unknown)))
        database.commit()
        database.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    baseline = snapshot()
    check("t12_nonempty_existing_identity_permissions_connections_history", all(row["rows"] > 0 for row in baseline.values()))
    report["snapshots"]["before_upgrade"] = baseline
    pre = root / "pre-upgrade-coherent"
    pre.mkdir()
    shutil.copytree(state, pre / "state")
    shutil.copytree(credentials, pre / "credentials")
    start(current)
    check("t12_in_place_schema32_to34", schema() == 34)
    after = snapshot(baseline)
    check("t12_upgrade_preserves_every_seeded_row_and_identity", after == baseline)
    report["snapshots"]["after_upgrade"] = after
    with sqlite3.connect(database_path) as database:
        evidence = [json.loads(row[0]) for row in database.execute("SELECT usage_evidence_json FROM billing_ledger_entries")]
        check("t12_schema33_legacy_ledger_provenance_stays_unknown", bool(evidence)
            and all(row == {"provenance": "unknown", "input_accounting": "unknown"} for row in evidence))
    ordinary = api("GET", "/admin/credentials/account-responses-b")
    native = next(row for row in api("GET", "/admin/native-accounts?limit=100")["items"] if row["id"] == native_id)
    check("t12_management_keeps_native_identity_and_both_disabled_intents", ordinary["status"] == "disabled"
        and native["enabled"] is False and native["identity"]["email"] == "t12@example.invalid")
    for response in old_responses:
        readable(response)
    usage = api("GET", "/admin/operations/usage?allow_partial=true&limit=100")
    check("t12_conflicting_legacy_history_explicitly_excluded", usage["excluded_usage_events"] >= 2 and usage["excluded_request_groups"] >= 1)
    with sqlite3.connect(database_path) as database:
        check("t12_unknown_history_does_not_fabricate_ledger", database.execute(
            "SELECT count(*) FROM billing_ledger_entries WHERE response_id LIKE 't12-legacy-unknown-%'").fetchone() == (0,))
        check("t12_legacy_attempt_sidecar_stays_unknown", database.execute("SELECT count(*) FROM gateway_attempt_execution").fetchone() == (0,))

    # Old schema rejection is tested against a stopped clone, never the active file.
    stopped()
    rejected_path = root / "old-schema-rejection.sqlite3"
    shutil.copyfile(database_path, rejected_path)
    rejected = subprocess.run([str(old), "admin", "audit", "--db", str(rejected_path), "--actor", "t12"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
    check("t12_old_schema32_binary_rejects_current_schema", rejected.returncode != 0
        and b"database migration history is not supported by this build" in rejected.stderr)
    report["old_schema_rejection"] = {"exit_code": rejected.returncode, "error": "unsupported_migration_history",
        "stderr_sha256": hashlib.sha256(rejected.stderr).hexdigest()}
    start(old_reader)
    api("GET", "/admin/operations/usage?provider_id=upstream-responses&allow_partial=true&limit=100")
    check("t12_schema34_pre_attempt_reader_accepts_legacy_usage", True)
    stopped()
    start(current)
    current_response = stored()
    wait_ledger({current_response["id"]})
    with sqlite3.connect(database_path) as database:
        identity, payload = database.execute("SELECT event_id,payload_json FROM gateway_event_log WHERE event_type='usage' "
            "AND json_extract(payload_json,'$.usage.response_id')=?", (current_response["id"],)).fetchone()
        usage_event = json.loads(payload)["usage"]
        check("t12_new_usage_has_exact_attempt_identity", identity == "usage-attempt:" + hashlib.sha256(usage_event["attempt_id"].encode()).hexdigest()
            and database.execute("SELECT request_id FROM gateway_event_log WHERE event_type='attempt' AND event_id=?",
                (usage_event["attempt_id"],)).fetchone() == (usage_event["request_id"],))
    stopped()
    replay = root / "old-reader-replay"
    shutil.copytree(state, replay / "state")
    shutil.copytree(credentials, replay / "credentials")
    replay_database = replay / "state" / "control.sqlite3"
    with sqlite3.connect(replay_database) as database:
        ordinal, source_payload = database.execute("SELECT event_ordinal,payload_json FROM gateway_event_log WHERE event_type='usage' "
            "AND json_extract(payload_json,'$.usage.response_id')=?", (current_response["id"],)).fetchone()
        # Remove only the clone's derived row/checkpoint to replay the genuine,
        # unchanged source event through the historical production billing worker.
        database.execute("DELETE FROM billing_ledger_entries WHERE response_id=?", (current_response["id"],))
        database.execute("UPDATE billing_materializer_checkpoints SET event_ordinal=?", (ordinal - 1,))
    before = len(context["all_receives"])
    start(old_reader, state_directory=replay / "state", credential_directory=replay / "credentials")
    api("GET", "/admin/operations/usage?provider_id=upstream-responses&allow_partial=true&limit=100")
    check("t12_old_usage_summary_is_not_source_identity_validation", True)
    deadline = time.monotonic() + 15
    while True:
        with sqlite3.connect(replay_database) as database:
            failed = database.execute("SELECT reason FROM billing_materializer_failures WHERE event_ordinal=? AND resolved_at_ms IS NULL", (ordinal,)).fetchone()
            if failed:
                check("t12_old_billing_reader_rejects_new_attempt_source_identity", failed == ("invalid_event",)
                    and database.execute("SELECT count(*) FROM billing_ledger_entries WHERE response_id=?", (current_response["id"],)).fetchone() == (0,)
                    and database.execute("SELECT payload_json FROM gateway_event_log WHERE event_ordinal=?", (ordinal,)).fetchone() == (source_payload,)
                    and len(context["all_receives"]) == before)
                break
        assert time.monotonic() < deadline, "T12 historical billing reader deadline"
        time.sleep(0.1)
    stopped()

    # Retain key1; explicitly select key2 for new writes. Binary rollback cannot
    # undo new identity semantics, even when all ciphertext keys are retained.
    directory = root / "master-keys"
    directory.mkdir()
    (directory / "1.key").write_bytes((credentials / "master-key").read_bytes())
    (directory / "2.key").write_bytes(secrets.token_bytes(32))
    options = ["--master-key-dir", str(directory), "--active-master-key-version", "2"]
    start(current, options)
    new_response = stored()
    wait_ledger({new_response["id"]})
    with sqlite3.connect(database_path) as database:
        check("t12_processed_unknown_history_still_has_no_fabricated_ledger", database.execute(
            "SELECT count(*) FROM billing_ledger_entries WHERE response_id LIKE 't12-legacy-unknown-%'").fetchone() == (0,))
    for response in [*old_responses, current_response, new_response]:
        readable(response)
    with sqlite3.connect(database_path) as database:
        check("t12_old_new_response_key_versions", database.execute("SELECT key_version FROM stored_responses WHERE response_id=?",
            (old_responses[0]["id"],)).fetchone() == (1,) and database.execute("SELECT key_version FROM stored_responses WHERE response_id=?",
            (new_response["id"],)).fetchone() == (2,))

    # Historical configuration restoration must require a new explicit review.
    draft = api("POST", "/admin/config-versions/t12-existing/fork", {"id": "t12-deletion", "description": "Synthetic reviewed deletion"}, expected=201)
    context["scope"], context["revision"] = draft["id"], draft["revision"]
    impact = api("GET", "/admin/credentials/account-responses-b/deletion-impact")
    api("DELETE", "/admin/credentials/account-responses-b", extra={"X-Deletion-Impact-Review": impact["review_token"]}, expected=204)
    context["publish"]()
    context["scope"] = "t12-existing"
    context["revision"] = api("GET", "/admin/config-versions/t12-existing")["revision"]
    restoration = api("GET", "/admin/config-versions/t12-existing/account-restoration")
    check("t12_historical_config_names_account_restoration", any(row["credential_id"] == "account-responses-b" for row in restoration["accounts"]))
    lifecycle = max(row["id"] for row in api("GET", "/admin/audit-events") if row["action"] in ("config_published", "config_rolled_back"))
    extra = {"X-Expected-Active-Version": json.dumps("t12-deletion"), "X-Expected-Lifecycle-Event": str(lifecycle),
        "If-Match": api("GET", "/admin/config-versions/t12-deletion")["revision"]}
    api("POST", "/admin/config-versions/rollback", extra=extra, expected=409)
    check("t12_old_configuration_cannot_silently_restore_account", True)
    api("POST", "/admin/config-versions/rollback", extra={**extra, "X-Account-Restoration-Review": restoration["review_token"]})
    check("t12_reviewed_restoration_keeps_disabled_intent", api("GET", "/admin/credentials/account-responses-b")["status"] == "disabled")

    stopped()
    post = root / "post-upgrade-preserved"
    post.mkdir()
    shutil.copytree(state, post / "state")
    shutil.copytree(credentials, post / "credentials")
    shutil.copytree(directory, post / "master-keys")
    post_snapshot = snapshot()
    report["snapshots"]["post_upgrade_preserved"] = post_snapshot
    check("t12_preserved_new_history_before_restore", post_snapshot["gateway_event_log"]["rows"] > baseline["gateway_event_log"]["rows"]
        and post_snapshot["stored_responses"]["rows"] > baseline["stored_responses"]["rows"])

    # Replace only this harness's owned, stopped temporary state. Both snapshots
    # remain private and retained; the post-upgrade history is never discarded.
    shutil.rmtree(state)
    shutil.copytree(pre / "state", state)
    shutil.rmtree(credentials)
    shutil.copytree(pre / "credentials", credentials)
    check("t12_coherent_prechange_restore_exact_nonempty_sets", snapshot() == baseline and schema() == 32)
    start(old)
    for response in old_responses:
        readable(response)
    check("t12_historical_binary_and_permissions_after_restore", api("GET", "/admin/credentials/account-responses-b")["status"] == "disabled")
    stopped()
    shutil.rmtree(state)
    shutil.copytree(post / "state", state)
    shutil.rmtree(credentials)
    shutil.copytree(post / "credentials", credentials)
    shutil.rmtree(directory)
    shutil.copytree(post / "master-keys", directory)
    check("t12_coherent_restore_includes_both_exact_key_files", all((directory / name).read_bytes() == (post / "master-keys" / name).read_bytes()
        for name in ("1.key", "2.key")))
    check("t12_restored_postchange_preserved_all_history", snapshot() == post_snapshot)
    start(current, options)
    for response in [*old_responses, current_response, new_response]:
        readable(response)
    before = len(context["all_receives"])
    status, _, _ = data("/v1/responses", {"model": "model-responses", "input": "restricted"}, key=context["restricted_key"])
    check("t12_restored_client_permissions_and_no_extra_receive", status == 404 and len(context["all_receives"]) == before)
    report.update({"upgrade": "PASS", "coherent_restore": "PASS", "binary_only_rollback": "UNSAFE",
        "schema_start": 32, "schema_end": 34, "history_preserved_privately": True,
        "restoring_prechange_state_excludes_later_writes_until_postchange_state_is_restored": True,
        "native_display_identity": "synthetic", "old_usage_summary_http": 200,
        "attempt_usage_old_billing_reader": "REJECTED_INVALID_EVENT",
        "upstream_receive_count": len(context["all_receives"]), "key_versions_retained": [1, 2]})
    save_report()
