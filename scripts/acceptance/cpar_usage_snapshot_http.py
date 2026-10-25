"""Issue #72: production composition, loopback upstream and actual management HTTP only."""
import concurrent.futures
import csv
import http.client
import io
import json
import socket
import time
import urllib.error
import urllib.parse
import urllib.request


def verify(c):
    api, data, body_for, check = (c[name] for name in ("api", "data", "body_for", "check"))
    start = int(time.time() * 1000) - 1000
    for key in (c["issued_key"], c["sibling_key"]):
        status, _, _ = data("/v1/responses", body_for("responses", "responses", "text-round"), key=key)
        check("snapshot_public_success", status == 200)
    status, raw, _ = data("/v1/chat/completions", body_for("chat", "chat", "usage-error-round", True))
    check("snapshot_failed_stream", status == 200 and b"malformed" not in raw and b"error" in raw)
    c["held_started"].clear()
    c["held_closed"].clear()
    conn = http.client.HTTPConnection("127.0.0.1", c["data_port"], timeout=10)
    conn.request("POST", "/v1/responses", json.dumps(body_for("responses", "responses", "held-round", True)),
                 {"Authorization": "Bearer " + c["issued_key"], "Content-Type": "application/json"})
    transport = conn.sock
    response = conn.getresponse()
    check("snapshot_cancel_started", response.status == 200 and c["held_started"].wait(5))
    while True:
        line = response.readline()
        assert line
        if b"response.output_text.delta" in line:
            break
    transport.shutdown(socket.SHUT_RDWR)
    response.close()
    conn.close()
    check("snapshot_cancel_closed_upstream", c["held_closed"].wait(5))
    draft = api("POST", f"/admin/config-versions/{c['scope']}/fork", {"id": "usage-retry", "description": "Synthetic usage retry"}, expected=201)
    c["scope"], c["revision"] = draft["id"], draft["revision"]
    candidate = c["fallback_candidates"]["responses"]
    api("PATCH", "/admin/routes/route-responses/candidates/" + candidate["id"], {**candidate, "enabled": True})
    c["publish"]()
    c["restart"]()
    before = len(c["calls"])
    status, _, _ = data("/v1/responses", body_for("responses", "responses", "retry-round"))
    check("snapshot_retry_two_attempts", status == 200 and len(c["calls"]) == before + 2)

    filters = {"from_ms": start, "to_ms": int(time.time() * 1000) + 60_000, "bucket_ms": 1000,
               "client_key_id": "local-key", "access_group_id": "local-group"}
    deadline = time.monotonic() + 15
    while True:
        snap = api("POST", "/admin/operations/usage-snapshots", filters, expected=201)
        prefix = "/admin/operations/usage-snapshots/" + snap["snapshot_id"]
        if snap["summary"]["requests"] == 4 and snap["summary"]["cost_records"]["pending"] == 0:
            break
        api("DELETE", prefix, expected=204)
        assert time.monotonic() < deadline, "snapshot materialization deadline"
        time.sleep(.1)
    check("snapshot_distinct_request_attempt_counts", snap["summary"]["requests"] == 4 and snap["summary"]["attempts"] == 5)
    check("snapshot_failed_cancel_usage", snap["summary"]["failed_usage_observations"] == 1 and snap["summary"]["cancelled_usage_observations"] == 1)
    check("snapshot_unpriced_fallback", snap["summary"]["cost_records"]["unpriced"] == 1)

    def pages(path, export=False):
        result, cursor = [], None
        for _ in range(100):
            suffix = "&cursor=" + urllib.parse.quote(cursor, safe="") if cursor else ""
            if export:
                with c["client"].open(urllib.request.Request(c["base"] + path + suffix, headers=c["headers"]), timeout=10) as response:
                    raw = response.read().decode()
                    cursor = response.headers.get("X-Next-Cursor")
                    result.extend(list(csv.DictReader(io.StringIO(raw))) if "format=csv" in path else [json.loads(line) for line in raw.splitlines()])
            else:
                page = api("GET", path + suffix)
                result.extend(page["items"])
                cursor = page["next_cursor"]
            if not cursor:
                return result
        raise AssertionError("bounded pagination")

    details = pages(prefix + "/rows?limit=1")
    check("snapshot_full_filter_not_page_sum", len(details) == 5 and len({r["request_id"] for r in details}) == 4
          and all(r["client_key_id"] == "local-key" for r in details))
    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
        pending = pool.submit(data, "/v1/chat/completions", body_for("chat", "chat", "text-round"))
        jsonl = pages(prefix + "/export?format=jsonl&limit=1", True)
        csv_rows = pages(prefix + "/export?format=csv&limit=2", True)
        check("snapshot_concurrent_public_write", pending.result()[0] == 200)
    check("snapshot_export_exact_rows", jsonl == details and len(csv_rows) == len(details))
    check("snapshot_export_totals", sum(int(r["input_tokens"]) for r in csv_rows if r["input_tokens"])
          == snap["summary"]["input_tokens"]["total"] and sum(int(r["cost_microunits"]) for r in csv_rows if r["cost_microunits"])
          == snap["summary"]["known_cost_microunits"])
    buckets = pages(prefix + "/rows?grain=buckets&limit=1")
    check("snapshot_bucket_export", pages(prefix + "/export?grain=buckets&format=jsonl&limit=1", True) == buckets)
    check("snapshot_bucket_attempt_totals", sum(r["attempts"] for r in buckets) == snap["summary"]["attempts"])
    check("snapshot_metadata_frozen", api("GET", prefix) == snap)
    # A disconnected download has no offset mutation; the same page can be fetched again.
    conn = http.client.HTTPConnection("127.0.0.1", c["admin_port"], timeout=10)
    conn.request("GET", prefix + "/export?format=jsonl&limit=1", headers=c["headers"])
    conn.getresponse().close()
    conn.close()
    check("snapshot_disconnect_retry", pages(prefix + "/export?format=jsonl&limit=1", True) == details)
    api("GET", prefix + "/export?limit=101", expected=400)
    api("GET", prefix + "/export?client_key_id=sibling-key", expected=400)
    api("POST", "/admin/operations/usage-snapshots", {**filters, "to_ms": filters["from_ms"] - 1}, expected=400)
    check("snapshot_invalid_scope_and_limits", True)
    sibling = api("POST", "/admin/operations/usage-snapshots", {**filters, "client_key_id": "sibling-key"}, expected=201)
    check("snapshot_cross_key_isolation", sibling["summary"]["requests"] == 1 and sibling["summary"]["input_tokens"]["total"] == 20)
    api("DELETE", prefix, expected=204)
    api("GET", prefix + "/export", expected=410)
    check("snapshot_cancel_invalidation", True)
    sibling_path = "/admin/operations/usage-snapshots/" + sibling["snapshot_id"]
    c["restart"]()
    api("GET", sibling_path, expected=410)
    check("snapshot_restart_invalidation", True)
