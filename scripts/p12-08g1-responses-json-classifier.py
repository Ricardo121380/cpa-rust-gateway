#!/usr/bin/env python3
"""Send one direct Responses JSON request and retain only value-free structure."""

from __future__ import annotations

import argparse
import http.client
import json
import os
from pathlib import Path
import ssl
import sys
from urllib.parse import urlsplit


MAX_BODY = 2 * 1024 * 1024
MAX_OUTPUT_ITEMS = 64
MAX_IDENTIFIER_BYTES = 256
ROOT_FIELDS = {
    "background", "created_at", "error", "id", "incomplete_details", "instructions",
    "max_output_tokens", "max_tool_calls", "metadata", "model", "object", "output",
    "parallel_tool_calls", "previous_response_id", "prompt_cache_key", "reasoning",
    "safety_identifier", "service_tier", "status", "store", "temperature", "text",
    "tool_choice", "tools", "top_logprobs", "top_p", "truncation", "usage", "user",
}
ITEM_FIELDS = {
    "message": {"content", "id", "role", "status", "type"},
    "reasoning": {"content", "encrypted_content", "id", "status", "summary", "type"},
    "function_call": {"arguments", "call_id", "id", "name", "status", "type"},
}
USAGE_FIELDS = {
    "input_tokens", "input_tokens_details", "output_tokens", "output_tokens_details",
    "total_tokens",
}


class ClassifierError(RuntimeError):
    pass


def value_class(value) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, str):
        return "empty_string" if not value else "nonempty_string"
    if isinstance(value, list):
        return "empty_array" if not value else "nonempty_array"
    if isinstance(value, dict):
        return "empty_object" if not value else "nonempty_object"
    if isinstance(value, int):
        return "nonnegative_integer" if value >= 0 else "negative_integer"
    return "other_type"


def identifier_valid(value) -> bool:
    return isinstance(value, str) and bool(value) and len(value.encode("utf-8")) <= MAX_IDENTIFIER_BYTES


def completed_or_absent(value, present: bool) -> bool:
    return not present or value == "completed"


def classify_usage(value) -> tuple[dict, bool]:
    if value is None:
        return {"class": "null", "keys": [], "extra_keys": []}, True
    if not isinstance(value, dict):
        return {"class": value_class(value), "keys": [], "extra_keys": []}, False
    keys = set(value)
    result = {
        "class": value_class(value),
        "keys": sorted(keys),
        "extra_keys": sorted(keys - USAGE_FIELDS),
        "input_detail_keys": [],
        "output_detail_keys": [],
        "integer_fields_valid": True,
        "total_consistent": True,
    }
    valid = not result["extra_keys"]
    numbers = {}
    for field in ("input_tokens", "output_tokens", "total_tokens"):
        if field in value:
            item = value[field]
            field_valid = isinstance(item, int) and not isinstance(item, bool) and item >= 0
            result["integer_fields_valid"] = result["integer_fields_valid"] and field_valid
            valid = valid and field_valid
            if field_valid:
                numbers[field] = item
    if "total_tokens" in numbers:
        consistent = (
            "input_tokens" in numbers and "output_tokens" in numbers
            and numbers["total_tokens"] == numbers["input_tokens"] + numbers["output_tokens"]
        )
        result["total_consistent"] = consistent
        valid = valid and consistent
    for parent, child, result_key in (
        ("input_tokens_details", "cached_tokens", "input_detail_keys"),
        ("output_tokens_details", "reasoning_tokens", "output_detail_keys"),
    ):
        if parent not in value or value[parent] is None:
            continue
        details = value[parent]
        if not isinstance(details, dict):
            valid = False
            continue
        result[result_key] = sorted(details)
        if set(details) - {child}:
            valid = False
        if child in details and not (
            isinstance(details[child], int) and not isinstance(details[child], bool)
            and details[child] >= 0
        ):
            valid = False
    return result, valid


def classify_json(value) -> dict:
    if not isinstance(value, dict):
        return {
            "json_shape": value_class(value),
            "strict_decoder_compatible": False,
            "first_failed_gate": "root_object",
        }
    gates: list[tuple[str, bool]] = []
    root_keys = set(value)
    gates.append(("root_fields", not (root_keys - ROOT_FIELDS)))
    gates.append(("object", value.get("object") == "response"))
    gates.append(("error", "error" not in value or value.get("error") is None))
    gates.append(("status", value.get("status") in ("completed", "incomplete")))
    gates.append(("response_id", identifier_valid(value.get("id"))))
    usage, usage_valid = classify_usage(value.get("usage")) if "usage" in value else (
        {"class": "absent", "keys": [], "extra_keys": []}, True
    )
    gates.append(("usage", usage_valid))
    output = value.get("output")
    output_valid = isinstance(output, list) and 0 < len(output) <= MAX_OUTPUT_ITEMS
    gates.append(("output", output_valid))
    items = []
    emitted_content = False
    if isinstance(output, list):
        for item in output:
            record = {
                "class": value_class(item), "type_class": "absent", "keys": [],
                "extra_keys": [], "status_class": "absent", "id_valid": False,
            }
            item_valid = isinstance(item, dict)
            if isinstance(item, dict):
                item_type = item.get("type")
                record["type_class"] = item_type if item_type in ITEM_FIELDS else value_class(item_type)
                record["keys"] = sorted(item)
                allowed = ITEM_FIELDS.get(item_type)
                record["extra_keys"] = sorted(set(item) - allowed) if allowed else sorted(item)
                record["status_class"] = (
                    "absent" if "status" not in item else
                    "completed" if item.get("status") == "completed" else value_class(item.get("status"))
                )
                record["id_valid"] = identifier_valid(item.get("id"))
                item_valid = bool(allowed) and not record["extra_keys"] and record["id_valid"]
                item_valid = item_valid and completed_or_absent(item.get("status"), "status" in item)
                if item_type == "message":
                    record["role_class"] = "assistant" if item.get("role") == "assistant" else value_class(item.get("role"))
                    content = item.get("content")
                    record["content_class"] = value_class(content)
                    record["content_count"] = len(content) if isinstance(content, list) else None
                    parts = []
                    if isinstance(content, list):
                        for part in content:
                            part_record = {"class": value_class(part), "keys": [], "extra_keys": []}
                            part_valid = isinstance(part, dict)
                            if isinstance(part, dict):
                                part_record["keys"] = sorted(part)
                                part_record["extra_keys"] = sorted(set(part) - {"annotations", "logprobs", "text", "type"})
                                part_record["type_class"] = "output_text" if part.get("type") == "output_text" else value_class(part.get("type"))
                                part_record["text_class"] = value_class(part.get("text"))
                                part_record["annotations_class"] = value_class(part.get("annotations")) if "annotations" in part else "absent"
                                part_record["logprobs_class"] = value_class(part.get("logprobs")) if "logprobs" in part else "absent"
                                part_valid = (
                                    not part_record["extra_keys"] and part.get("type") == "output_text"
                                    and isinstance(part.get("text"), str)
                                    and ("annotations" not in part or part.get("annotations") == [])
                                    and ("logprobs" not in part or part.get("logprobs") in (None, []))
                                )
                                emitted_content = emitted_content or isinstance(part.get("text"), str)
                            parts.append(part_record)
                            item_valid = item_valid and part_valid
                    else:
                        item_valid = False
                    item_valid = item_valid and item.get("role") == "assistant"
                    record["content_parts"] = parts
                elif item_type == "reasoning":
                    record["encrypted_content_class"] = value_class(item.get("encrypted_content")) if "encrypted_content" in item else "absent"
                    reasoning_emitted = False
                    for field, expected in (("summary", "summary_text"), ("content", "reasoning_text")):
                        parts = item.get(field)
                        record[field + "_class"] = value_class(parts) if field in item else "absent"
                        record[field + "_part_keys"] = []
                        if field not in item:
                            continue
                        if not isinstance(parts, list):
                            item_valid = False
                            continue
                        for part in parts:
                            if not isinstance(part, dict):
                                item_valid = False
                                continue
                            record[field + "_part_keys"] = sorted(set(record[field + "_part_keys"]) | set(part))
                            part_valid = set(part) <= {"text", "type"} and part.get("type") == expected and isinstance(part.get("text"), str)
                            item_valid = item_valid and part_valid
                            reasoning_emitted = reasoning_emitted or part_valid
                    item_valid = item_valid and ("encrypted_content" not in item or item.get("encrypted_content") is None) and reasoning_emitted
                    emitted_content = emitted_content or reasoning_emitted
                elif item_type == "function_call":
                    record["call_id_valid"] = identifier_valid(item.get("call_id"))
                    record["name_valid"] = identifier_valid(item.get("name"))
                    record["arguments_class"] = value_class(item.get("arguments"))
                    item_valid = item_valid and record["call_id_valid"] and record["name_valid"] and isinstance(item.get("arguments"), str)
                    emitted_content = emitted_content or item_valid
            items.append(record)
            gates.append(("output_item", item_valid))
    gates.append(("emitted_content", emitted_content))
    first_failed = next((name for name, passed in gates if not passed), "none")
    return {
        "json_shape": "object",
        "root_keys": sorted(root_keys),
        "root_extra_keys": sorted(root_keys - ROOT_FIELDS),
        "object_class": "response" if value.get("object") == "response" else value_class(value.get("object")),
        "status_class": value.get("status") if value.get("status") in ("completed", "incomplete") else value_class(value.get("status")),
        "error_class": value_class(value.get("error")) if "error" in value else "absent",
        "response_id_valid": identifier_valid(value.get("id")),
        "output_class": value_class(output),
        "output_count": len(output) if isinstance(output, list) else None,
        "output_items": items,
        "usage": usage,
        "gate_results": {name: passed for name, passed in gates},
        "strict_decoder_compatible": all(passed for _, passed in gates),
        "first_failed_gate": first_failed,
    }


def read_inputs() -> tuple[str, str, str]:
    raw = sys.stdin.buffer.read(131073)
    if len(raw) > 131072:
        raise ClassifierError("input_bound")
    parts = raw.split(b"\0")
    if parts and parts[-1] == b"":
        parts.pop()
    if len(parts) != 3:
        raise ClassifierError("input_shape")
    try:
        values = tuple(part.decode("utf-8") for part in parts)
    except UnicodeDecodeError as error:
        raise ClassifierError("input_encoding") from error
    if not all(values):
        raise ClassifierError("input_empty")
    return values


def write_receipt(path: Path, value: dict) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--timeout", type=float, default=120.0)
    args = parser.parse_args()
    try:
        base_url, bearer, model = read_inputs()
        parsed = urlsplit(base_url)
        if parsed.scheme != "https" or not parsed.hostname or parsed.query or parsed.fragment or parsed.username or parsed.password:
            raise ClassifierError("endpoint_admission")
        path = parsed.path.rstrip("/") + "/responses"
        body = json.dumps({
            "model": model,
            "input": [{"type": "message", "role": "user", "content": "Return one short plain text result."}],
            "max_output_tokens": 96,
            "stream": False,
        }, separators=(",", ":")).encode("utf-8")
        connection = http.client.HTTPSConnection(parsed.hostname, parsed.port or 443, timeout=args.timeout, context=ssl.create_default_context())
        connection.request("POST", path, body=body, headers={
            "accept": "application/json", "authorization": "Bearer " + bearer,
            "content-type": "application/json",
        })
        response = connection.getresponse()
        raw = response.read(MAX_BODY + 1)
        if len(raw) > MAX_BODY:
            raise ClassifierError("body_bound")
        status_class = "2xx" if 200 <= response.status < 300 else "non_2xx"
        content_type = response.getheader("content-type", "").split(";", 1)[0].strip().lower()
        receipt = {
            "schema_version": 1, "value_free": True, "request_count": 1, "retry_count": 0,
            "status_class": status_class,
            "content_type_class": "json" if content_type == "application/json" else "other",
            "cc_switch_access": "read_only", "cc_switch_modified": False,
        }
        if status_class == "2xx" and content_type == "application/json":
            try:
                value = json.loads(raw)
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise ClassifierError("response_json") from error
            receipt.update(classify_json(value))
        write_receipt(args.out, receipt)
        print("p12-08g1-responses-classifier=" + ("PASS" if receipt.get("strict_decoder_compatible") else "FAIL"))
        return 0 if receipt.get("strict_decoder_compatible") else 1
    except (ClassifierError, OSError, http.client.HTTPException):
        print("p12-08g1-responses-classifier=FAIL category=classifier_error", file=sys.stderr)
        return 1
    finally:
        if "connection" in locals():
            connection.close()


if __name__ == "__main__":
    raise SystemExit(main())
