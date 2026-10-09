"""E-04 missing cells on the stock gateway/TLS/SQLite harness; no real Provider."""
import hashlib
import json
import sqlite3
import time


def run(context):
    api, check = context["api"], context["check"]
    # Keep this matrix within the serving build's total binding concurrency budget.
    # The earlier batch graph already fills it; an independent empty version is required.
    draft = api("POST", "/admin/config-versions",
                {"id": "seo-isolation", "description": "E-04 public isolation cells"}, expected=201)
    context["scope"], context["revision"] = draft["id"], draft["revision"]
    api("POST", "/admin/egress-policies", {"id":"local-policy","name":"Matrix loopback only",
        "allowed_schemes":["https"],"allowed_hosts":["127.0.0.1"],
        "allowed_ports":[context["provider"].server_port],"allowed_cidrs":["127.0.0.1/32"],
        "redirect_mode":"deny","max_redirects":0},expected=201)
    for group in ("local-group","restricted-group"):
        api("POST","/admin/access-groups",{"id":group,"name":group,"status":"active","limits":{}},expected=201)
    for group, field, key_id in (("local-group","issued_key","seo-client"),("restricted-group","restricted_key","seo-restricted-client")):
        context[field]=api("POST","/admin/client-keys",{"id":key_id,"access_group_id":group,"status":"active","expires_at_ms":None},expected=201)["key"]
    identities = {}

    def upstream(name):
        owner = "seo-" + name
        api("POST", "/admin/upstreams", {"id": owner, "name": owner, "kind": "openai-compatible",
            "enabled": True, "tags": [], "egress_policy_id": "local-policy"}, expected=201)
        account = api("POST", f"/admin/upstreams/{owner}/account-import",
            {"id": owner + "-account", "channel": "openai-compatible", "secret": context["keys"]["a"]}, expected=201)
        return owner, account["id"]

    def endpoint(owner, account, name, protocol):
        _, adapter, api_format = context["PROTOCOLS"][protocol]
        target = "seo-" + name
        api("POST", f"/admin/upstreams/{owner}/endpoints", {"id": target, "adapter_id": adapter,
            "api_format": api_format, "base_url": f'https://127.0.0.1:{context["provider"].server_port}/v1',
            "inference_path": "/" + target + context["PROTOCOLS"][protocol][0][3:],
            "models_path": None, "transport": "https", "enabled": True}, expected=201)
        api("POST", f"/admin/endpoints/{target}/credential-bindings", {"credential_id": account,
            "enabled": True, "priority": 0, "weight": 1, "concurrency": 2}, expected=201)
        return target

    def model(owner, account, target, name, protocol, wire_model=None):
        name = "seo-" + name
        wire_model = wire_model or name
        api("POST", "/admin/public-models", {"id": name, "model_name": name, "display_name": name,
            "status": "active", "capabilities": {}}, expected=201)
        api("POST", f"/admin/public-models/{name}/routes", {"id": name + "-route",
            "policy": "smooth_weighted_round_robin", "max_attempts": 1, "bootstrap_timeout_ms": 5000}, expected=201)
        api("POST", f"/admin/routes/{name}-route/candidates", {"id": name + "-candidate",
            "endpoint_id": target, "upstream_model": wire_model, "credential_scope": "all_active",
            "transform_mode": "canonical_bridge", "enabled": True, "priority": 0, "weight": 1,
            "capability_override": {"allow_unlisted_model": True}}, expected=201)
        api("POST", "/admin/access-groups/local-group/routes", {"route_id": name + "-route", "enabled": True}, expected=201)
        identities[name] = {"provider_id": owner, "endpoint_id": target, "credential_id": account,
            "credential_revision": 0, "route_candidate_id": name + "-candidate", "upstream_model": wire_model,
            "protocol": protocol}
        return name

    owner, account = upstream("connections")
    a = endpoint(owner, account, "connection-a", "responses")
    # A published Upstream admits one Endpoint per API format, so use two legal connections.
    b = endpoint(owner, account, "connection-b", "chat")
    connection_a = model(owner, account, a, "connection-a", "responses", "seo-connection-model")
    connection_b = model(owner, account, b, "connection-b", "chat", "seo-connection-model")
    owner, account = upstream("models")
    target = endpoint(owner, account, "model-connection", "responses")
    model_a = model(owner, account, target, "model-a", "responses")
    model_b = model(owner, account, target, "model-b", "responses")
    owner, account = upstream("shared")
    shared = {}
    for protocol in ("responses", "messages", "chat"):
        target = endpoint(owner, account, "shared-" + protocol, protocol)
        shared[protocol] = model(owner, account, target, "shared-" + protocol, protocol)
    context["publish"]()
    accepted_revision = api("GET", "/admin/config-versions/seo-isolation")["revision"]
    before_pid = context["gateway"].pid
    rows = []

    def invoke(name, model_name, marker, success, receives, stream=False, key=None):
        identity = identities[model_name]
        protocol = identity["protocol"]
        body = context["body_for"](protocol, protocol, marker, stream)
        body["model"] = model_name
        with sqlite3.connect(context["state"] / "control.sqlite3") as db:
            ordinal = db.execute("SELECT coalesce(max(event_ordinal),0) FROM gateway_event_log").fetchone()[0]
        before = len(context["all_receives"])
        status, raw, headers = context["data"](context["PROTOCOLS"][protocol][0], body, key=key)
        delta = len(context["all_receives"]) - before
        check("seo_" + name + "_public_status", status == 200 if success else status >= 400)
        check("seo_" + name + "_all_receives", delta == receives)
        check("seo_" + name + "_no_reset", context["gateway"].pid == before_pid)
        record = {"name": name, "expected_identity": identity, "status": status,
            "stream": stream, "receive_before": before, "receive_after": before + delta,
            "raw_public_response_sha256": hashlib.sha256(raw).hexdigest(), "attempts": []}
        request_id = {k.lower(): v for k, v in headers.items()}.get("x-request-id")
        # Early upstream failures need not return a correlation header. Match the single
        # serialized public request through the append-only event sequence instead.
        if key is None:
            for _ in range(200):
                with sqlite3.connect(context["state"] / "control.sqlite3") as db:
                    events = db.execute("SELECT event_type,request_id,payload_json FROM gateway_event_log "
                        "WHERE event_ordinal>? AND event_type IN ('request','request_finished') ORDER BY event_ordinal",(ordinal,)).fetchall()
                if any(event[0] == "request_finished" for event in events):
                    break
                time.sleep(0.025)
            started = [json.loads(event[2])["request"] for event in events if event[0] == "request"]
            check("seo_" + name + "_single_public_request",len(started)==1 and started[0]["requested_model"]==model_name
                and started[0]["streaming"]==stream and started[0]["client_key_id"]=="seo-client")
            if request_id:
                check("seo_" + name + "_header_correlation", request_id==started[0]["request_id"])
            request_id=started[0]["request_id"]
            record["public_request"]=started[0]
            record["public_terminal"]=[json.loads(event[2])["request_finished"] for event in events if event[0]=="request_finished"]
        record["request_id"] = request_id
        if receives:
            check("seo_" + name + "_request_id", bool(request_id))
            for _ in range(200):
                with sqlite3.connect(context["state"] / "control.sqlite3") as db:
                    actual = db.execute("SELECT a.payload_json,e.evidence_json,e.attempt_payload_sha256 "
                        "FROM gateway_event_log a JOIN gateway_attempt_execution e ON e.attempt_id=a.event_id "
                        "WHERE a.event_type='attempt' AND a.request_id=?", (request_id,)).fetchall()
                if actual:
                    break
                time.sleep(0.025)
            check("seo_" + name + "_single_attempt", len(actual) == 1)
            payload, evidence_json, checksum = actual[0]
            attempt, execution = json.loads(payload)["attempt"], json.loads(evidence_json)
            check("seo_" + name + "_payload_hash", hashlib.sha256(payload.encode()).hexdigest() == checksum)
            check("seo_" + name + "_actual_lease", attempt["upstream_id"]==identity["provider_id"] and all(attempt[field] == identity[field] for field in
                ("endpoint_id", "credential_id", "route_candidate_id", "upstream_model")))
            check("seo_" + name + "_exact_configuration", execution["config_version_id"] == "seo-isolation"
                and execution["config_revision"] == int(accepted_revision.removeprefix("rev-"))
                and execution["credential_revision"] == 0 and execution["egress"] == {"kind": "direct"})
            check("seo_" + name + "_actual_wire", context["calls"][-1]["body"]["model"] == identity["upstream_model"]
                and context["calls"][-1]["protocol"] == protocol)
            record["attempts"].append({"attempt": attempt, "execution": execution,
                "attempt_payload_sha256": checksum})
        elif request_id:
            with sqlite3.connect(context["state"] / "control.sqlite3") as db:
                count=db.execute("SELECT count(*) FROM gateway_event_log WHERE event_type='attempt' AND request_id=?",(request_id,)).fetchone()[0]
            check("seo_"+name+"_zero_started_attempts",count==0)
        rows.append(record)
        (context["out"] / "seo-isolation.json").write_text(json.dumps({"status": "IN_PROGRESS", "rows": rows}, indent=2) + "\n")

    invoke("same_account_connection_failure", connection_a, "auth-round", False, 1)
    invoke("same_account_other_connection_usable", connection_b, "text-round", True, 1, True)
    invoke("failed_binding_remains_closed", connection_a, "text-round", False, 0)
    invoke("model_permanent_failure", model_a, "seo-missing-model", False, 1)
    invoke("same_binding_other_model_usable", model_b, "text-round", True, 1)
    invoke("permanent_failure_has_no_retained_block", model_a, "text-round", True, 1, True)
    invoke("binding_quota_failure", model_a, "seo-quota", False, 1)
    invoke("binding_quota_blocks_other_model", model_b, "text-round", False, 0)
    invoke("shared_responses_endpoint_failure", shared["responses"], "always-fail", False, 1)
    invoke("shared_chat_after_responses_failure", shared["chat"], "text-round", True, 1)
    invoke("shared_messages_endpoint_failure", shared["messages"], "always-fail", False, 1)
    invoke("shared_chat_after_messages_failure", shared["chat"], "text-round", True, 1, True)
    invoke("misused_ungranted_client_key", shared["chat"], "text-round", False, 0, key=context["restricted_key"])
    invoke("misused_unknown_client_key", shared["chat"], "text-round", False, 0, key="rgw_invalid-synthetic-key")
    check("seo_fault_probe_configuration_unchanged", api("GET", "/admin/config-versions/seo-isolation")["revision"] == accepted_revision)
    report = {"status": "PASS", "scope": "actual stock gateway serve, public HTTP, synthetic loopback TLS",
        "gateway_pid": before_pid, "config_version_id": "seo-isolation", "config_revision": accepted_revision,
        "rows": rows, "all_upstream_receive_count": len(context["all_receives"]),
        "limits": ["HTTP quota is binding-wide; it intentionally blocks both models on that binding",
            "404 has no retained runtime state; this does not claim a public model-circuit trigger",
            "no process restart, publication or recovery action between each fault and probe"]}
    (context["out"] / "seo-isolation.json").write_text(json.dumps(report, indent=2) + "\n")
