#!/usr/bin/env python3
"""Offline tests for the value-free G1 Responses JSON classifier."""

import importlib.util
import json
from pathlib import Path
import unittest


PATH = Path(__file__).with_name("p12-08g1-responses-json-classifier.py")
SPEC = importlib.util.spec_from_file_location("p12_08g1_responses_classifier", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ClassifierTest(unittest.TestCase):
    def test_strict_text_response_passes_without_retaining_values(self):
        result = MODULE.classify_json({
            "id": "private-id", "object": "response", "status": "completed", "error": None,
            "output": [{
                "id": "private-item", "type": "message", "role": "assistant", "status": "completed",
                "content": [{"type": "output_text", "text": "private-text", "annotations": []}],
            }],
            "usage": {"input_tokens": 1, "output_tokens": 2, "total_tokens": 3,
                      "input_tokens_details": {"cached_tokens": 0},
                      "output_tokens_details": {"reasoning_tokens": 0}},
        })
        self.assertTrue(result["strict_decoder_compatible"])
        self.assertEqual(result["first_failed_gate"], "none")
        rendered = json.dumps(result)
        self.assertNotIn("private-id", rendered)
        self.assertNotIn("private-item", rendered)
        self.assertNotIn("private-text", rendered)

    def test_extra_root_field_is_the_first_closed_failure(self):
        result = MODULE.classify_json({
            "id": "id", "object": "response", "status": "completed",
            "output": [{"id": "item", "type": "message", "role": "assistant",
                        "content": [{"type": "output_text", "text": "text"}]}],
            "provider_private": "private-value",
        })
        self.assertFalse(result["strict_decoder_compatible"])
        self.assertEqual(result["first_failed_gate"], "root_fields")
        self.assertEqual(result["root_extra_keys"], ["provider_private"])
        self.assertNotIn("private-value", json.dumps(result))

    def test_message_and_usage_failures_are_value_free(self):
        result = MODULE.classify_json({
            "id": "id", "object": "response", "status": "completed",
            "output": [{"id": "item", "type": "message", "role": "assistant",
                        "content": [{"type": "output_text", "text": "secret", "extra": 1}]}],
            "usage": {"input_tokens": 1, "output_tokens": 2, "total_tokens": 9,
                      "output_tokens_details": {"reasoning_tokens": 0, "extra": 1}},
        })
        self.assertFalse(result["strict_decoder_compatible"])
        self.assertEqual(result["first_failed_gate"], "usage")
        self.assertEqual(result["usage"]["output_detail_keys"], ["extra", "reasoning_tokens"])
        self.assertEqual(result["output_items"][0]["content_parts"][0]["extra_keys"], ["extra"])
        self.assertNotIn("secret", json.dumps(result))


if __name__ == "__main__":
    unittest.main()
