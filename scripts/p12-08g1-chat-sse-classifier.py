#!/usr/bin/env python3
"""Send one direct Chat SSE request and retain only structural, value-free evidence."""

from __future__ import annotations

import argparse
import http.client
import json
import os
from pathlib import Path
import stat
import sys
from urllib.parse import urlsplit


MAX_BODY = 2 * 1024 * 1024
MAX_EVENT = 128 * 1024


class ClassifierError(RuntimeError):
    pass


def read_private_json(path: Path) -> dict:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ClassifierError("private_config_unavailable") from error
    try:
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_mode & 0o077:
            raise ClassifierError("private_config_admission")
        raw = os.read(descriptor, MAX_BODY + 1)
    finally:
        os.close(descriptor)
    if not raw or len(raw) > MAX_BODY:
        raise ClassifierError("private_config_bound")
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ClassifierError("private_config_json") from error
    if not isinstance(value, dict):
        raise ClassifierError("private_config_shape")
    return value


def provider_values(config: dict, provider_id: str) -> tuple[str, str, str, str, int, str]:
    providers = config.get("models", {}).get("providers", {})
    provider = providers.get(provider_id) if isinstance(providers, dict) else None
    models = provider.get("models") if isinstance(provider, dict) else None
    if not isinstance(provider, dict) or not isinstance(models, list) or len(models) != 1 or not isinstance(models[0], dict):
        raise ClassifierError("provider_shape")
    base_url, key, model = provider.get("baseUrl"), provider.get("apiKey"), models[0].get("id")
    if not all(isinstance(item, str) and item for item in (base_url, key, model)):
        raise ClassifierError("provider_value_shape")
    parsed = urlsplit(base_url)
    if parsed.scheme != "https" or not parsed.hostname or parsed.query or parsed.fragment or parsed.username or parsed.password:
        raise ClassifierError("provider_endpoint_admission")
    base_path = parsed.path.rstrip("/")
    return key, model, parsed.hostname, base_path, parsed.port or 443, "/chat/completions"


def classify_stream(response: http.client.HTTPResponse) -> dict:
    total = 0
    sse_field_names: set[str] = set()
    sse_non_data_field_names: set[str] = set()
    sse_comment_line_count = 0
    event_count = 0
    done = False
    error_event = False
    choice_events = 0
    usage_events = 0
    root_keys: set[str] = set()
    choice_keys: set[str] = set()
    delta_keys: set[str] = set()
    finish_classes: set[str] = set()
    choice_message_classes: set[str] = set()
    choice_message_keys: set[str] = set()
    message_role_classes: set[str] = set()
    message_content_classes: set[str] = set()
    message_content_equals_delta: set[bool] = set()
    message_tool_calls_classes: set[str] = set()
    message_reasoning_content_classes: set[str] = set()
    message_refusal_classes: set[str] = set()
    reasoning_content_classes: set[str] = set()
    usage_with_choices_count = 0
    accumulated_content = ""
    response_ids: set[str] = set()
    object_classes: set[str] = set()
    choice_count_classes: set[str] = set()
    choice_index_classes: set[str] = set()
    logprobs_classes: set[str] = set()
    delta_content_classes: set[str] = set()
    message_on_finish_only: set[bool] = set()
    usage_keys: set[str] = set()
    prompt_detail_keys: set[str] = set()
    completion_detail_keys: set[str] = set()
    usage_total_consistent: set[bool] = set()
    usage_timing_classes: set[str] = set()
    object_timing_classes: set[str] = set()
    id_relation_classes: set[str] = set()
    baseline_id: str | None = None
    delta_role_classes: set[str] = set()
    delta_role_timing_classes: set[str] = set()
    delta_tool_calls_classes: set[str] = set()
    delta_refusal_classes: set[str] = set()
    unsupported_usage_detail_nonzero: set[str] = set()
    delta_role_occurrence_count = 0
    finish_event_count = 0
    finish_delta_keys: set[str] = set()
    finish_delta_content_relations: set[str] = set()
    finish_message_content_relations: set[str] = set()
    event_sequence: list[dict] = []

    def value_class(value) -> str:
        if value is None:
            return "null"
        if isinstance(value, str):
            return "empty_string" if not value else "nonempty_string"
        if isinstance(value, dict):
            return "empty_object" if not value else "nonempty_object"
        if isinstance(value, list):
            return "empty_array" if not value else "nonempty_array"
        return "other_type"
    while True:
        line = response.readline(MAX_EVENT + 1)
        total += len(line)
        if total > MAX_BODY or len(line) > MAX_EVENT:
            raise ClassifierError("stream_bound")
        if not line:
            break
        if line.startswith(b":"):
            sse_comment_line_count += 1
            sse_field_names.add("comment")
            continue
        if b":" in line:
            raw_field = line.split(b":", 1)[0]
            try:
                field = raw_field.decode("ascii")
            except UnicodeDecodeError:
                field = "non_ascii"
            if not field or any(character not in "abcdefghijklmnopqrstuvwxyz" for character in field):
                field = "invalid"
            sse_field_names.add(field)
            if field != "data":
                sse_non_data_field_names.add(field)
        if not line.startswith(b"data:"):
            continue
        payload = line[5:].strip()
        if payload == b"[DONE]":
            done = True
            continue
        event_count += 1
        try:
            value = json.loads(payload)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ClassifierError("stream_json") from error
        if not isinstance(value, dict):
            raise ClassifierError("stream_event_shape")
        root_keys.update(str(key) for key in value)
        choices = value.get("choices")
        only_choice = choices[0] if isinstance(choices, list) and len(choices) == 1 and isinstance(choices[0], dict) else None
        event_finish = only_choice is not None and only_choice.get("finish_reason") is not None
        timing = "finish" if event_finish else "nonfinish"
        event_id = value.get("id")
        if isinstance(event_id, str):
            response_ids.add(event_id)
            relation = "first" if baseline_id is None else "same" if event_id == baseline_id else "different"
            id_relation_classes.add(f"{timing}_{relation}")
            if baseline_id is None:
                baseline_id = event_id
        object_value = value.get("object")
        object_class = (
            "chat_chunk" if object_value == "chat.completion.chunk" else
            "chat_completion" if object_value == "chat.completion" else
            value_class(object_value)
        )
        object_classes.add(object_class)
        object_timing_classes.add(f"{timing}_{object_class}")
        error_event = error_event or "error" in value
        event_record = {
            "ordinal": event_count,
            "timing": timing,
            "object_class": object_class,
            "id_relation": relation if isinstance(event_id, str) else "non_string",
            "usage_class": value_class(value.get("usage")) if "usage" in value else "absent",
            "choice_count": "zero" if choices == [] else "one" if isinstance(choices, list) and len(choices) == 1 else "other",
        }
        if isinstance(choices, list):
            choice_count_classes.add("zero" if not choices else "one" if len(choices) == 1 else "many")
        usage = value.get("usage")
        if isinstance(usage, dict):
            usage_keys.update(str(key) for key in usage)
            prompt_details = usage.get("prompt_tokens_details")
            completion_details = usage.get("completion_tokens_details")
            if isinstance(prompt_details, dict):
                prompt_detail_keys.update(str(key) for key in prompt_details)
                audio = prompt_details.get("audio_tokens")
                if isinstance(audio, int) and not isinstance(audio, bool) and audio != 0:
                    unsupported_usage_detail_nonzero.add("prompt_audio_tokens")
            if isinstance(completion_details, dict):
                completion_detail_keys.update(str(key) for key in completion_details)
                for field in ("audio_tokens", "accepted_prediction_tokens", "rejected_prediction_tokens"):
                    item = completion_details.get(field)
                    if isinstance(item, int) and not isinstance(item, bool) and item != 0:
                        unsupported_usage_detail_nonzero.add(field)
            incoming, outgoing, total_tokens = (
                usage.get("prompt_tokens"), usage.get("completion_tokens"), usage.get("total_tokens")
            )
            integers = all(isinstance(item, int) and not isinstance(item, bool) and item >= 0
                           for item in (incoming, outgoing, total_tokens))
            usage_total_consistent.add(bool(integers and total_tokens == incoming + outgoing))
        if choices == [] and isinstance(value.get("usage"), dict):
            usage_events += 1
        if not isinstance(choices, list):
            continue
        if choices and isinstance(value.get("usage"), dict):
            usage_with_choices_count += 1
        for choice in choices:
            if not isinstance(choice, dict):
                raise ClassifierError("choice_shape")
            choice_events += 1
            choice_keys.update(str(key) for key in choice)
            index = choice.get("index")
            choice_index_classes.add("zero" if index == 0 and not isinstance(index, bool) else
                                     "nonzero" if isinstance(index, int) and not isinstance(index, bool) else
                                     value_class(index))
            if "logprobs" in choice:
                logprobs_classes.add(value_class(choice.get("logprobs")))
            content_before_delta = accumulated_content
            delta = choice.get("delta")
            if isinstance(delta, dict):
                event_record["delta_keys"] = sorted(str(key) for key in delta)
                delta_keys.update(str(key) for key in delta)
                if "role" in delta:
                    delta_role_occurrence_count += 1
                    role = delta.get("role")
                    role_class = "assistant" if role == "assistant" else value_class(role)
                    delta_role_classes.add(role_class)
                    delta_role_timing_classes.add(f"{'finish' if choice.get('finish_reason') is not None else 'nonfinish'}_{role_class}")
                    event_record["delta_role_class"] = role_class
                content = delta.get("content")
                if "content" in delta:
                    delta_content_classes.add(value_class(content))
                    event_record["delta_content_class"] = value_class(content)
                if isinstance(content, str):
                    accumulated_content += content
                if "reasoning_content" in delta:
                    reasoning_content_classes.add(value_class(delta.get("reasoning_content")))
                    event_record["reasoning_content_class"] = value_class(delta.get("reasoning_content"))
                if "tool_calls" in delta:
                    delta_tool_calls_classes.add(value_class(delta.get("tool_calls")))
                if "refusal" in delta:
                    delta_refusal_classes.add(value_class(delta.get("refusal")))
            reason = choice.get("finish_reason")
            if reason is None:
                finish_classes.add("null")
            elif reason in ("stop", "length", "tool_calls"):
                finish_classes.add(reason)
            elif isinstance(reason, str):
                finish_classes.add("other_string")
            else:
                finish_classes.add("other_type")
            if reason is not None:
                finish_event_count += 1
                if isinstance(delta, dict):
                    finish_delta_keys.update(str(key) for key in delta)
                    content = delta.get("content")
                    if "content" not in delta:
                        finish_delta_content_relations.add("absent")
                    elif content is None:
                        finish_delta_content_relations.add("null")
                    elif content == "":
                        finish_delta_content_relations.add("empty")
                    elif isinstance(content, str) and content == content_before_delta:
                        finish_delta_content_relations.add("equals_prior_full")
                    elif isinstance(content, str) and content_before_delta.endswith(content):
                        finish_delta_content_relations.add("equals_prior_suffix")
                    elif isinstance(content, str) and content.endswith(content_before_delta):
                        finish_delta_content_relations.add("contains_prior_prefix")
                    elif isinstance(content, str):
                        finish_delta_content_relations.add("other_string")
                    else:
                        finish_delta_content_relations.add("other_type")
            if isinstance(usage, dict):
                usage_timing_classes.add("finish" if reason is not None else "nonfinish")
            if "message" in choice:
                event_record["message_class"] = value_class(choice.get("message"))
                message_on_finish_only.add(reason is not None)
                message = choice.get("message")
                choice_message_classes.add(value_class(message))
                if isinstance(message, dict):
                    choice_message_keys.update(str(key) for key in message)
                    if "role" in message:
                        role = message.get("role")
                        message_role_classes.add(
                            "assistant" if role == "assistant" else value_class(role)
                        )
                    if "content" in message:
                        content = message.get("content")
                        message_content_classes.add(value_class(content))
                        if isinstance(content, str):
                            message_content_equals_delta.add(content == accumulated_content)
                            before = content == content_before_delta
                            after = content == accumulated_content
                            finish_message_content_relations.add(
                                "equals_before_and_after" if before and after else
                                "equals_before" if before else
                                "equals_after" if after else
                                "other"
                            )
                    if "tool_calls" in message:
                        message_tool_calls_classes.add(value_class(message.get("tool_calls")))
                    if "reasoning_content" in message:
                        message_reasoning_content_classes.add(
                            value_class(message.get("reasoning_content"))
                        )
                    if "refusal" in message:
                        message_refusal_classes.add(value_class(message.get("refusal")))
        event_sequence.append(event_record)
    compatible = done and bool({"stop", "length", "tool_calls"} & finish_classes) and not error_event
    return {
        "event_count": event_count,
        "sse_field_names": sorted(sse_field_names),
        "sse_non_data_field_names": sorted(sse_non_data_field_names),
        "sse_comment_line_count": sse_comment_line_count,
        "done_present": done,
        "error_event_present": error_event,
        "choice_event_count": choice_events,
        "usage_event_count": usage_events,
        "finish_classes": sorted(finish_classes),
        "choice_message_classes": sorted(choice_message_classes),
        "choice_message_keys": sorted(choice_message_keys),
        "message_role_classes": sorted(message_role_classes),
        "message_content_classes": sorted(message_content_classes),
        "message_content_equals_prior_delta": sorted(message_content_equals_delta),
        "message_tool_calls_classes": sorted(message_tool_calls_classes),
        "message_reasoning_content_classes": sorted(message_reasoning_content_classes),
        "message_refusal_classes": sorted(message_refusal_classes),
        "reasoning_content_classes": sorted(reasoning_content_classes),
        "usage_with_choices_count": usage_with_choices_count,
        "response_id_consistent": len(response_ids) == 1,
        "object_classes": sorted(object_classes),
        "choice_count_classes": sorted(choice_count_classes),
        "choice_index_classes": sorted(choice_index_classes),
        "logprobs_classes": sorted(logprobs_classes),
        "delta_content_classes": sorted(delta_content_classes),
        "message_on_finish_only": sorted(message_on_finish_only),
        "usage_keys": sorted(usage_keys),
        "prompt_token_detail_keys": sorted(prompt_detail_keys),
        "completion_token_detail_keys": sorted(completion_detail_keys),
        "usage_total_consistent": sorted(usage_total_consistent),
        "usage_timing_classes": sorted(usage_timing_classes),
        "object_timing_classes": sorted(object_timing_classes),
        "id_relation_classes": sorted(id_relation_classes),
        "delta_role_classes": sorted(delta_role_classes),
        "delta_role_timing_classes": sorted(delta_role_timing_classes),
        "delta_tool_calls_classes": sorted(delta_tool_calls_classes),
        "delta_refusal_classes": sorted(delta_refusal_classes),
        "unsupported_usage_detail_nonzero": sorted(unsupported_usage_detail_nonzero),
        "delta_role_occurrence_count": delta_role_occurrence_count,
        "finish_event_count": finish_event_count,
        "finish_delta_keys": sorted(finish_delta_keys),
        "finish_delta_content_relations": sorted(finish_delta_content_relations),
        "finish_message_content_relations": sorted(finish_message_content_relations),
        "event_sequence": event_sequence,
        "root_keys": sorted(root_keys),
        "choice_keys": sorted(choice_keys),
        "delta_keys": sorted(delta_keys),
        "strict_decoder_compatible": compatible,
    }


def write_receipt(path: Path, value: dict) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", required=True, type=Path)
    parser.add_argument("--provider-id", required=True)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--timeout", type=float, default=120.0)
    args = parser.parse_args()
    try:
        key, model, host, base_path, port, suffix = provider_values(read_private_json(args.config), args.provider_id)
        body = json.dumps({
            "model": model,
            "messages": [{"role": "user", "content": "Return one short plain text result."}],
            "max_tokens": 96,
            "stream": True,
            "stream_options": {"include_usage": True},
        }, separators=(",", ":")).encode()
        connection = http.client.HTTPSConnection(host, port, timeout=args.timeout)
        try:
            connection.request("POST", base_path + suffix, body, {
                "Authorization": "Bearer " + key,
                "Content-Type": "application/json",
                "Accept": "text/event-stream",
            })
            response = connection.getresponse()
            if response.status // 100 != 2:
                response.read(MAX_EVENT)
                raise ClassifierError("http_" + str(response.status // 100) + "xx")
            content_type = (response.getheader("content-type") or "").split(";", 1)[0].strip().lower()
            if content_type != "text/event-stream":
                raise ClassifierError("content_type")
            receipt = {"schema_version": 1, "value_free": True, "single_send": True, **classify_stream(response)}
        finally:
            connection.close()
        write_receipt(args.out, receipt)
    except (ClassifierError, OSError, http.client.HTTPException) as error:
        category = str(error) if isinstance(error, ClassifierError) else "transport"
        print(f"p12-08g1-chat-classifier=FAIL category={category}", file=sys.stderr)
        return 1
    print("p12-08g1-chat-classifier=PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
