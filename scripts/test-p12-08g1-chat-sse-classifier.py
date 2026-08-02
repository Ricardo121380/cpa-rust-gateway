#!/usr/bin/env python3
"""Offline structural tests for the G1 direct Chat SSE classifier."""

import importlib.util
import io
import json
from pathlib import Path
import unittest


PATH = Path(__file__).with_name("p12-08g1-chat-sse-classifier.py")
SPEC = importlib.util.spec_from_file_location("p12_08g1_chat_classifier", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def wire(*events):
    data = bytearray()
    for event in events:
        payload = event if isinstance(event, str) else json.dumps(event, separators=(",", ":"))
        data.extend(b"data: " + payload.encode() + b"\n\n")
    return io.BytesIO(bytes(data))


class ClassifierTest(unittest.TestCase):
    def test_compatible_stream_records_only_structure(self):
        result = MODULE.classify_stream(wire(
            {"id": "private", "object": "chat.completion.chunk", "choices": [{"index": 0, "delta": {"content": "private"}, "finish_reason": None}]},
            {"id": "private", "object": "chat.completion.chunk", "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]},
            {"id": "private", "object": "chat.completion.chunk", "choices": [], "usage": {}},
            "[DONE]",
        ))
        self.assertTrue(result["strict_decoder_compatible"])
        self.assertNotIn("private", json.dumps(result))
        self.assertEqual(result["choice_message_classes"], [])
        self.assertEqual(result["reasoning_content_classes"], [])

    def test_missing_finish_is_classified_without_guessing(self):
        result = MODULE.classify_stream(wire(
            {"id": "private", "object": "chat.completion.chunk", "choices": [{"index": 0, "delta": {"content": "private"}, "finish_reason": None}]},
            {"id": "private", "object": "chat.completion.chunk", "choices": [], "usage": {}},
            "[DONE]",
        ))
        self.assertFalse(result["strict_decoder_compatible"])
        self.assertEqual(result["finish_classes"], ["null"])

    def test_extra_fields_are_reduced_to_value_classes(self):
        result = MODULE.classify_stream(wire(
            {"id": "private", "object": "chat.completion.chunk", "choices": [{
                "index": 0, "delta": {"content": "private"}, "finish_reason": None,
            }]},
            {"id": "private", "object": "chat.completion.chunk", "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant", "content": "private", "tool_calls": None,
                    "reasoning_content": "", "refusal": None,
                },
                "delta": {"reasoning_content": ""}, "finish_reason": "stop",
            }], "usage": {"prompt_tokens": 1}},
            "[DONE]",
        ))
        self.assertEqual(result["choice_message_classes"], ["nonempty_object"])
        self.assertEqual(
            result["choice_message_keys"],
            ["content", "reasoning_content", "refusal", "role", "tool_calls"],
        )
        self.assertEqual(result["message_role_classes"], ["assistant"])
        self.assertEqual(result["message_content_classes"], ["nonempty_string"])
        self.assertEqual(result["message_content_equals_prior_delta"], [True])
        self.assertEqual(result["message_tool_calls_classes"], ["null"])
        self.assertEqual(result["message_reasoning_content_classes"], ["empty_string"])
        self.assertEqual(result["message_refusal_classes"], ["null"])
        self.assertEqual(result["reasoning_content_classes"], ["empty_string"])
        self.assertEqual(result["usage_with_choices_count"], 1)


if __name__ == "__main__":
    unittest.main()
