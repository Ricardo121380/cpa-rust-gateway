"""Output-limit regression at the existing real-process HTTP/TLS boundary."""
import hashlib
import json
import os
import re
import secrets
import sqlite3
import subprocess
import sys
import time

import cpar_build_extensions as extensions
import cpar_build_extensions_http as fixture
import cpar_new_build_acceptance as oracle


def provider_answer(body):
    """Synthetic upstream reads actual history; the formal oracle judges the result."""
    history = body.get("input", body.get("messages", []))
    text = json.dumps(history)
    choice = body.get("tool_choice")
    tools = choice == "required" or isinstance(choice, dict) and choice.get("type") in ("any", "tool")
    if tools:
        return True, ["call_" + secrets.token_hex(8)], ['{"value":1}'], "", "lookup"
    token = re.search(r"ACCEPT:[0-9a-f]{24}", text)
    answer = token[0] if token else "synthetic answer"
    if "Return exactly ACCEPT:" in text:
        # Do not manufacture a result when conversion lost the actual tool output.
        assert isinstance(history, list)
        results = [item["output"] for item in history if item.get("type") == "function_call_output"]
        results += [item["content"] for item in history if item.get("role") == "tool"]
        results += [block["content"] for item in history if isinstance(item.get("content"), list)
                    for block in item["content"] if block.get("type") == "tool_result"]
        assert results and all(value == "1" for value in results)
        answer += ":" + results[-1]
    return False, [], [], answer, "lookup"


def event_counts(state):
    with sqlite3.connect("file:" + str(state / "control.sqlite3") + "?mode=ro", uri=True) as database:
        return dict(database.execute("SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type"))


def verify(context):
    assert os.environ.get("CPAR_EXTENSIONS_ONLY") == "1", "single-attempt, disabled-background, loopback sandbox required"
    body = {"model": "model-responses", "messages": [{"role": "user", "content": "synthetic output limit"}],
            "max_completion_tokens": 4096, "stream": False}
    before = event_counts(context["state"])
    receives = len(context["all_receives"])
    status, raw, headers = context["data"]("/v1/chat/completions", body)
    time.sleep(.2)
    after = event_counts(context["state"])
    delta = {name: after.get(name, 0) - before.get(name, 0) for name in ("request", "request_finished", "attempt", "usage")}
    receipt = {"layer": "LOCAL_SIMULATED", "request": body, "http_status": status,
               "response_sha256": hashlib.sha256(raw).hexdigest(), "response_bytes": len(raw),
               "wire_request_id_present": any(key.lower() == "x-request-id" for key in headers),
               "events": delta, "upstream_receives": len(context["all_receives"]) - receives,
               "real_provider_requests": 0}
    if status != 200:
        receipt["error"] = json.loads(raw)
    (context["out"] / "public-reproduction.json").write_text(json.dumps(receipt, indent=2) + "\n")
    context["check"]("chat_completion_limit_public_json", status == 200 and receipt["upstream_receives"] == 1)
    context["check"]("chat_completion_limit_actual_upstream_value", context["calls"][-1]["body"]["max_output_tokens"] == 4096)
    boundaries(context)
    basic_matrix(context)


def boundaries(context):
    rows = []
    for target in ("chat", "responses"):
        for stream in (False, True):
            cases = [("missing", {}, True)]
            for field in ("max_completion_tokens", "max_tokens"):
                cases += [(field + "_" + label, {field: value}, accepted) for label, value, accepted in [
                    ("one", 1, True), ("formal", 4096, True), ("u64_max", 2**64 - 1, True),
                    ("null", None, False), ("zero", 0, False), ("negative", -1, False),
                    ("fraction", 1.5, False), ("float", 4096.0, False), ("string", "4096", False),
                    ("bool", True, False), ("array", [], False), ("object", {}, False),
                    ("overflow", 2**64, False)]]
            cases += [("dual_" + str(value), {"max_completion_tokens": 4096, "max_tokens": value}, False)
                      for value in (4096, 8192, None)]
            cases += [("foreign_" + field, {"max_completion_tokens": 4096, field: 4096}, False)
                      for field in ("max_output_tokens", "openai.responses.max_output_tokens", "anthropic.messages.max_tokens")]
            for label, limits, accepted in cases:
                body = {"model": "model-" + target, "messages": [{"role": "user", "content": "synthetic boundary"}],
                        "stream": stream, **limits}
                if stream:
                    body["stream_options"] = {"include_usage": True}
                before, receives = event_counts(context["state"]), len(context["all_receives"])
                status, raw, _ = context["data"]("/v1/chat/completions", body)
                time.sleep(.1)
                after = event_counts(context["state"])
                delta = {name: after.get(name, 0) - before.get(name, 0) for name in ("request", "request_finished", "attempt", "usage")}
                received = len(context["all_receives"]) - receives
                row = {"case": label, "target": target, "stream": stream, "controls": limits,
                       "http_status": status, "events": delta, "upstream_receives": received}
                rows.append(row)
                (context["out"] / "parameter-boundaries.json").write_text(json.dumps(rows, indent=2) + "\n")
                context["check"](f"limit_{target}_{stream}_{label}", status == (200 if accepted else 400)
                                 and received == int(accepted) and delta["attempt"] == int(accepted)
                                 and delta["usage"] == int(accepted))
                if accepted:
                    actual = context["calls"][-1]["body"]
                    expected = limits if target == "chat" else ({"max_output_tokens": next(iter(limits.values()))} if limits else {})
                    observed = {key: actual[key] for key in ("max_tokens", "max_completion_tokens", "max_output_tokens") if key in actual}
                    context["check"]("limit_exact_wire_" + label, observed == expected)
                    answer = oracle.decode_stream("chat", raw) if stream else json.loads(raw)
                    oracle.native_usage("chat", answer)
                    row["observed_controls"] = observed
    # The reverse shared projection must use the modern total-output field too.
    for stream in (False, True):
        before = len(context["all_receives"])
        status, raw, _ = context["data"]("/v1/responses", {"model": "model-chat", "input": "synthetic reverse",
                                                                  "max_output_tokens": 4096, "stream": stream})
        context["check"]("responses_to_chat_modern_limit_" + str(stream), status == 200
                         and len(context["all_receives"]) == before + 1
                         and context["calls"][-1]["body"].get("max_completion_tokens") == 4096
                         and "max_tokens" not in context["calls"][-1]["body"])
        oracle.native_usage("responses", oracle.decode_stream("responses", raw) if stream else json.loads(raw))
    (context["out"] / "parameter-boundaries.json").write_text(json.dumps(rows, indent=2) + "\n")


def basic_matrix(context):
    revisions = set(match.decode() for match in re.findall(rb"gateway-release-revision=([0-9a-f]{40})", context["binary"].read_bytes()))
    assert len(revisions) == 1, "exact embedded source required"
    source = revisions.pop()
    private = context["root"] / "basic"
    private.mkdir(mode=0o700)
    public = context["out"] / "basic"
    public.mkdir()
    probe = 'import socket\ntry: socket.create_connection(("192.0.2.1",443),timeout=.5)\nexcept PermissionError as error: assert error.errno == 1\nelse: raise AssertionError("non-loopback was not denied")\n'
    subprocess.run(context["cpar_t12_upgrade_http"].sandbox_command([sys.executable, "-c", probe]), check=True)
    context["check"]("output_limit_os_denies_non_loopback", True)
    matrix = []
    for protocol in ("chat", "messages", "responses"):
        for mode in ("json", "sse"):
            label = protocol + "-" + mode
            receives = len(context["all_receives"])
            pin = context["api"]("POST", "/admin/operations/channel-pin", {
                "provider_id": "upstream-responses", "channel_id": "endpoint-responses", "route_id": "route-responses",
                "credential_id": "account-responses-a", "requested_model": "model-responses",
                "protocol": {"chat": "openai_chat_completions", "responses": "openai_responses", "messages": "anthropic_messages"}[protocol],
                "mode": mode, "client_key_id": "local-key", "credential_revision": 0})
            oracle.write_private_bundle(private / (label + "-pin.json"), pin)
            context["check"]("basic_pin_" + label, pin["outcome"] == "succeeded" and pin["attempt_count"] == 1
                             and len(context["all_receives"]) == receives + 1)
            runtime = fixture.observe_runtime(context, source, "owned-output-limit-fixture")
            parameters = {{"chat": "max_completion_tokens", "messages": "max_tokens", "responses": "max_output_tokens"}[protocol]: 4096}
            if protocol == "chat" and mode == "sse":
                parameters["stream_options"] = {"include_usage": True}
            plan = oracle.freeze_from_pin({"layer": "LOCAL_SIMULATED", "source_sha": source,
                "target": {"protocol": protocol, "mode": mode, "public_model": "model-responses", "response_model": "model-responses",
                           "parameters": parameters}, "collector": {"local_state_dir": str(context["state"]), "binary": str(context["binary"])}}, runtime, pin)
            plan_file, runtime_file, bundle_file = (private / (label + suffix) for suffix in ("-plan.json", "-runtime.json", "-bundle.json"))
            oracle.write_private_bundle(plan_file, plan)
            oracle.write_private_bundle(runtime_file, {"runtime": runtime})
            before = len(context["all_receives"])
            calls_before = len(context["calls"])
            command = [sys.executable, str(context["repo"] / "scripts/acceptance/cpar-batch-c-live.py"), "--new-build"]
            completed = subprocess.run([*command, "run", "--execute", "--plan", str(plan_file), "--runtime", str(runtime_file),
                "--base-url", f"http://127.0.0.1:{context['data_port']}/v1", "--private-out", str(bundle_file),
                "--out", str(public / (label + ".json"))], capture_output=True, text=True,
                env={**os.environ, "CPAR_ACCEPTANCE_CLIENT_KEY": context["issued_key"]}, timeout=180)
            receipt = json.loads((public / (label + ".json")).read_text())
            row = {"combination": label, "basic_status": receipt.get("basic_status"), "formal_status": receipt["status"],
                   "parameters": parameters, "pin_receives": 1, "basic_receives": len(context["all_receives"]) - before,
                   "returncode": completed.returncode, "bundle_sha256": hashlib.sha256(bundle_file.read_bytes()).hexdigest()}
            matrix.append(row)
            (context["out"] / "matrix.json").write_text(json.dumps(matrix, indent=2) + "\n")
            context["check"]("formal_basic_" + label, row["basic_status"] == "PASS" and row["basic_receives"] == 9)
            actual = context["calls"][calls_before:]
            context["check"]("basic_actual_4096_" + label, len(actual) == 9 and all(
                call["body"].get("max_output_tokens") == 4096 and "max_completion_tokens" not in call["body"]
                and "max_tokens" not in call["body"] for call in actual))
            audited = subprocess.run([*command, "audit", "--bundle", str(bundle_file), "--out", str(public / (label + "-audit.json"))],
                                     capture_output=True, text=True, timeout=30)
            audit = json.loads((public / (label + "-audit.json")).read_text())
            context["check"]("independent_formal_audit_" + label, audit["basic_status"] == "PASS" and audited.returncode == completed.returncode)
            # Keep only synthetic upstream request bodies and exact IDs in the local private evidence.
            oracle.write_private_bundle(private / (label + "-upstream.json"), actual)
            row["upstream_sha256"] = hashlib.sha256((private / (label + "-upstream.json")).read_bytes()).hexdigest()
    (context["out"] / "matrix.json").write_text(json.dumps(matrix, indent=2) + "\n")
