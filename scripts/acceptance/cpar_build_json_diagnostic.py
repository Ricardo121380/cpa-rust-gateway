"""The authorized one-Pin/one-Chat diagnostic policy; no network or credential access."""
import hashlib
import json
import os
import re


def require(condition, category):
    if not condition:
        raise ValueError(category)


def new_budget():
    return {
        "schema_version": 1, "kind": "chat_json_502_diagnostic",
        "historical_inference_requests": 35,
        "historical_gateway_pin_confirmed_sends": 34,
        "new_pin_intents": 0, "new_basic_intents": 0,
        "new_inference_limit": 2, "new_pin_limit": 1, "new_basic_limit": 1,
        "cumulative_inference_limit": 37, "stopped": False,
        "pin_status": "NOT_RUN", "chat_status": "NOT_RUN",
        "provider_receive_count": None, "actual_cost": None,
        "retries": 0, "fallbacks": 0, "refresh_reauthorization": 0,
    }


def validate_budget(budget):
    fixed = new_budget()
    for key in ("schema_version", "kind", "historical_inference_requests",
                "historical_gateway_pin_confirmed_sends", "new_inference_limit",
                "new_pin_limit", "new_basic_limit", "cumulative_inference_limit",
                "retries", "fallbacks", "refresh_reauthorization"):
        require(budget.get(key) == fixed[key], "diagnostic_budget_changed")
    for key in ("new_pin_intents", "new_basic_intents"):
        require(type(budget.get(key)) is int and 0 <= budget[key] <= 1, "diagnostic_intent_limit")
    require(type(budget.get("stopped")) is bool, "diagnostic_stop_unknown")
    require(budget.get("provider_receive_count") is None and budget.get("actual_cost") is None,
            "independent_provider_evidence_unavailable")
    require(budget["new_basic_intents"] == 0 or budget["new_pin_intents"] == 1,
            "diagnostic_chat_without_pin")


def reserve_pin(budget):
    validate_budget(budget)
    require(not budget["stopped"] and budget["new_pin_intents"] == budget["new_basic_intents"] == 0
            and budget["pin_status"] == budget["chat_status"] == "NOT_RUN", "diagnostic_pin_replay")
    budget.update(new_pin_intents=1, stopped=True, pin_status="UNKNOWN")


def finish_pin(budget, verified_pass):
    validate_budget(budget)
    require(budget["stopped"] and budget["new_pin_intents"] == 1
            and budget["new_basic_intents"] == 0 and budget["pin_status"] == "UNKNOWN", "diagnostic_pin_replay")
    budget.update(pin_status="PASS" if verified_pass else "FAIL", stopped=not verified_pass)


def reserve_chat(budget):
    validate_budget(budget)
    require(not budget["stopped"] and budget["new_pin_intents"] == 1 and budget["pin_status"] == "PASS"
            and budget["new_basic_intents"] == 0 and budget["chat_status"] == "NOT_RUN", "diagnostic_chat_replay")
    budget.update(new_basic_intents=1, stopped=True, chat_status="UNKNOWN")


def finish_chat(budget, status):
    validate_budget(budget)
    require(budget["stopped"] and budget["new_basic_intents"] == 1 and budget["chat_status"] == "UNKNOWN"
            and status in ("REPRODUCED", "NOT_REPRODUCED", "UNKNOWN"), "diagnostic_chat_replay")
    budget.update(chat_status=status, stopped=True)


def save_budget(path, budget):
    """Caller holds the owned epoch's exclusive execution lock. Latch survives a crash."""
    validate_budget(budget)
    temporary = path.with_suffix(".tmp")
    with temporary.open("w") as output:
        os.chmod(temporary, 0o600)
        json.dump(budget, output, indent=2)
        output.flush()
        os.fsync(output.fileno())
    temporary.replace(path)
    descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def validate_request(body):
    require(isinstance(body, dict) and set(body) == {"model", "messages", "stream", "max_completion_tokens"}
            and body["model"] == "grok-4.5" and body["stream"] is False
            and type(body["max_completion_tokens"]) is int and body["max_completion_tokens"] == 4096,
            "diagnostic_request_semantics_changed")
    messages = body["messages"]
    require(isinstance(messages, list) and len(messages) == 1
            and set(messages[0]) == {"role", "content"} and messages[0]["role"] == "user"
            and re.fullmatch(r"Remember ACCEPT:[0-9a-f]{24}\. Reply READY\.", messages[0]["content"] or ""),
            "diagnostic_request_not_original_synthetic_case")


def native_usage_summary(native):
    """Mirror Build's four source counters; absence never becomes zero."""
    value = native.get("usage")
    if value is None:
        return None
    require(isinstance(value, dict), "diagnostic_native_usage_shape")
    def counter(object, name):
        count = object.get(name)
        require(count is None or type(count) is int and 0 <= count <= 18446744073709551615,
                "diagnostic_native_usage_counter")
        return count
    def details(name):
        object = value.get(name)
        require(object is None or isinstance(object, dict), "diagnostic_native_usage_details")
        return object or {}
    summary = {"provenance": "measured", "input_accounting": "inclusive",
               "input_tokens": counter(value, "input_tokens"), "output_tokens": counter(value, "output_tokens"),
               "reasoning_tokens": counter(details("output_tokens_details"), "reasoning_tokens"),
               "cached_tokens": counter(details("input_tokens_details"), "cached_tokens"),
               "cache_read_tokens": None, "cache_creation_tokens": None}
    return summary if any(summary[name] is not None for name in summary if name.endswith("tokens")) else None


def verify_usage_lineage(native, request_id, attempt, usages, ledger):
    """Check decoded source, exact event identity and all nullable ledger counters."""
    if attempt["outcome"] != "succeeded" or attempt["retry_decision"] != "completed":
        require(not usages and not ledger, "diagnostic_usage_decode_unconfirmed")
        return None
    expected = native_usage_summary(native)
    require(len(usages) == len(ledger) == int(expected is not None), "diagnostic_usage_count_mismatch")
    if expected is None:
        return None
    event_id, usage = usages[0]
    entry = ledger[0]
    require(usage["request_id"] == request_id and usage["attempt_id"] == attempt["attempt_id"]
            and usage["response_id"] == native["id"] and usage["usage"] == expected,
            "diagnostic_usage_source_or_identity_mismatch")
    require(entry["source_event_id"] == event_id and entry["request_id"] == request_id
            and entry["response_id"] == native["id"] and entry["account_id"] == attempt["credential_id"]
            and entry["model"] == "grok-4.5", "diagnostic_ledger_source_or_identity_mismatch")
    require(all(entry[name] == value for name, value in expected.items() if name.endswith("tokens"))
            and json.loads(entry["usage_evidence_json"]) == {name: expected[name] for name in ("provenance", "input_accounting")},
            "diagnostic_ledger_usage_mismatch")
    return expected


def binding(plan, runtime, base_url):
    material = {"kind": "chat_json_502_diagnostic", "epoch": plan["epoch"],
                "source_sha": plan["source_sha"], "public_request_sha256": plan["public_request_sha256"],
                "runtime": {key: runtime[key] for key in ("instance", "pid", "process_start", "artifact_sha256")},
                "target": plan["target"], "base_url": base_url, "max_inference_intents": 2}
    return hashlib.sha256(json.dumps(material, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def verify_qualification(formal, plan, runtime, base_url):
    require(plan["kind"] == "chat_json_502_diagnostic" and plan["layer"] == "LIVE_NEW_BUILD" and base_url == "http://127.0.0.1:19180/v1",
            "diagnostic_instance_changed")
    formal.verify_runtime(runtime, plan["source_sha"], "LIVE_NEW_BUILD")
    formal.verify_target(plan["target"])
    formal.verify_current_declaration(plan["target"], runtime)
    formal.verify_execution_inventory(plan, runtime, base_url)
    target = plan["target"]
    require(target["channel"] == "grok.build" and target["protocol"] == "chat" and target["mode"] == "json"
            and target["public_model"] == target["response_model"] == target["upstream_model"] == "grok-4.5"
            and target["parameters"] == {"max_completion_tokens":4096}, "diagnostic_target_changed")
    approval = plan["authorization"]
    require(approval["status"] == "APPROVED" and approval["basis"] == "user_explicit"
            and approval["max_inference_requests"] == 2 and approval["max_attempts"] == 1
            and approval["allow_fallback"] is False and approval["binding_sha256"] == binding(plan,runtime,base_url),
            "diagnostic_authorization_changed")
    require(approval["cost_policy"] == "UNLIMITED_USER_EXPLICIT" and approval["budget_limit"] is None
            and approval["cost_authorization_evidence_sha256"] == "77980e82623fa9913bd0623803f1a0f6b33857335127ad96fde5064f0ebaaa0f",
            "diagnostic_cost_policy_changed")
