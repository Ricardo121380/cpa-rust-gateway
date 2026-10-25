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
    token_fields = ("input_tokens", "output_tokens", "reasoning_tokens", "cache_read_tokens", "cache_creation_tokens", "cached_tokens")

    def check_totals(label, totals, selected):
        expected_counts = {
            "requests": len({r["request_id"] for r in selected}),
            "attempts": sum(r["attempt_id"] is not None for r in selected),
            "usage_observations": sum(r["usage_observed"] for r in selected),
            "failed_usage_observations": sum(r["usage_observed"] and r["usage_outcome"] == "failed" for r in selected),
            "cancelled_usage_observations": sum(r["usage_observed"] and r["usage_outcome"] == "cancelled" for r in selected),
            "requests_without_terminal": len({r["request_id"] for r in selected if r["request_outcome"] is None}),
        }
        check(label + "_counts", all(totals[field] == value for field, value in expected_counts.items()))
        for field in token_fields:
            observed = [r for r in selected if r[field] is not None]
            expected = {"total": sum(r[field] for r in observed) if observed else None,
                        "observed": len(observed), "missing": len(selected) - len(observed),
                        **{p: sum(r["provenance"] == p for r in observed) for p in ("measured", "estimated", "unknown")}}
            expected["confidence"] = ("unknown" if not observed or expected["estimated"] or expected["unknown"]
                                      else "partial" if expected["missing"] else "exact")
            if field == "input_tokens":
                accounting = {r["input_accounting"] for r in observed}
                if len(accounting) != 1 or "unknown" in accounting:
                    expected.update(total=None, confidence="unknown")
            check(label + "_" + field, totals[field] == expected)
        statuses = {status: sum(r["cost_status"] == status for r in selected)
                    for status in ("exact", "partial", "unknown", "unpriced", "pending", "unavailable")}
        amounts = [r["cost_microunits"] for r in selected if r["cost_microunits"] is not None]
        amount = sum(amounts) if amounts else None
        status = ("unknown" if not selected else "exact" if statuses["exact"] == len(selected)
                  else "partial" if amounts else "unpriced" if statuses["unpriced"] == len(selected)
                  else "pending" if statuses["pending"] else "unavailable" if statuses["unavailable"] else "unknown")
        check(label + "_cost", totals["known_cost_microunits"] == amount
              and totals["cost_records"] == statuses and totals["cost_status"] == status)

    def csv_matches(actual, expected):
        return len(actual) == len(expected) and all(
            set(cells) == set(row) and all(
                json.loads(cells[field]) == value if isinstance(value, (dict, list))
                else cells[field] == ("" if value is None else str(value).lower() if isinstance(value, bool) else str(value))
                for field, value in row.items()) for cells, row in zip(actual, expected))

    # Independent oracle: select from the already checked full detail set, then compare a
    # new backend-filtered capture and every page/export. Source writes are paused here.
    def check_filtered(label, patch):
        query = {**filters, **patch}
        expected = [row for row in details
                    if query["from_ms"] <= row["occurred_at_ms"] <= query["to_ms"]
                    and all(row[field] == query[field] for field in
                            ("model", "protocol", "provider_id", "channel_id", "account_id",
                             "client_key_id", "access_group_id") if field in query)]
        scoped = api("POST", "/admin/operations/usage-snapshots", query, expected=201)
        scope_path = "/admin/operations/usage-snapshots/" + scoped["snapshot_id"]
        summary = scoped["summary"]
        check(label + "_row_count", scoped["detail_rows"] == len(expected))
        check_totals(label + "_summary", summary, expected)
        check(label + "_all_detail_pages", pages(scope_path + "/rows?limit=1") == expected)
        check(label + "_jsonl_details", pages(scope_path + "/export?format=jsonl&limit=2", True) == expected)
        scoped_buckets = pages(scope_path + "/rows?grain=buckets&limit=1")
        grouped = {}
        for row in expected:
            bucket_start = query["from_ms"] + ((row["occurred_at_ms"] - query["from_ms"]) // query["bucket_ms"]) * query["bucket_ms"]
            grouped.setdefault(bucket_start, []).append(row)
        check(label + "_bucket_count", len(scoped_buckets) == len(grouped))
        for bucket, (start_ms, selected) in zip(scoped_buckets, sorted(grouped.items())):
            check(label + "_bucket_boundaries", bucket["from_ms"] == start_ms
                  and bucket["to_ms"] == min(start_ms + query["bucket_ms"] - 1, query["to_ms"]))
            check_totals(label + "_bucket_" + str(start_ms), bucket, selected)
        check(label + "_jsonl_buckets", pages(scope_path + "/export?format=jsonl&grain=buckets&limit=1", True) == scoped_buckets)
        check(label + "_csv_details", csv_matches(pages(scope_path + "/export?format=csv&limit=1", True), expected))
        check(label + "_csv_buckets", csv_matches(pages(scope_path + "/export?format=csv&grain=buckets&limit=1", True), scoped_buckets))
        api("DELETE", scope_path, expected=204)

    for field in ("model", "protocol", "provider_id", "channel_id", "account_id"):
        for index, value in enumerate(sorted({r[field] for r in details if r[field] is not None})):
            check_filtered("snapshot_filter_" + field + "_" + str(index), {field: value})
        absent = "anthropic_messages" if field == "protocol" else "absent"
        check_filtered("snapshot_filter_" + field + "_absent", {field: absent})
    check_filtered("snapshot_filter_key_absent", {"client_key_id": "absent"})
    check_filtered("snapshot_filter_group_absent", {"access_group_id": "absent"})
    first_time = min(row["occurred_at_ms"] for row in details)
    last_time = max(row["occurred_at_ms"] for row in details)
    check_filtered("snapshot_time_lower_upper_inclusive", {"from_ms": first_time, "to_ms": first_time})
    check_filtered("snapshot_time_before", {"from_ms": first_time - 1, "to_ms": first_time - 1})
    check_filtered("snapshot_time_after", {"from_ms": last_time + 1, "to_ms": last_time + 1})
    combined = {field: details[0][field] for field in ("model", "protocol", "provider_id", "channel_id", "account_id")}
    check_filtered("snapshot_filters_combined", {**combined, "from_ms": first_time, "to_ms": first_time})
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
