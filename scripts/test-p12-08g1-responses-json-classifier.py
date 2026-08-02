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
    def test_duplicate_json_names_are_counted_without_retaining_names_or_values(self):
        value, counts = MODULE.decode_json_with_duplicate_counts(
            b'{"private":"first","nested":{"secret":1,"secret":2},"private":"last"}'
        )
        self.assertEqual(value["private"], "last")
        self.assertEqual(counts["duplicate_json_object_count"], 2)
        self.assertEqual(counts["duplicate_json_name_occurrence_count"], 2)
        self.assertFalse(counts["duplicate_json_names_absent"])
        rendered = json.dumps(counts)
        self.assertNotIn("private", rendered)
        self.assertNotIn("secret", rendered)
        self.assertNotIn("first", rendered)
        self.assertNotIn("last", rendered)

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
        self.assertEqual(result["root_extra_shapes"]["provider_private"]["class"], "nonempty_string")
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
        self.assertFalse(result["usage"]["output_extra_shapes"]["extra"]["zero"])
        self.assertEqual(result["output_items"][0]["content_parts"][0]["extra_keys"], ["extra"])
        self.assertNotIn("secret", json.dumps(result))

    def test_known_extra_relations_retain_classes_not_values(self):
        result = MODULE.classify_json({
            "id": "id", "object": "response", "status": "completed",
            "created_at": 10, "completed_at": 11, "moderation": None,
            "output": [{"id": "item", "type": "message", "role": "assistant",
                        "phase": "final_answer", "metadata": {},
                        "content": [{"type": "output_text", "text": "secret"}]}],
        })
        self.assertTrue(result["completed_at_not_before_created_at"])
        self.assertEqual(result["root_extra_shapes"]["moderation"]["class"], "null")
        self.assertTrue(result["output_items"][0]["phase_is_final_answer"])
        self.assertEqual(result["output_items"][0]["extra_shapes"]["metadata"]["keys"], [])
        self.assertNotIn("final_answer", json.dumps(result["output_items"][0]["extra_shapes"]))
        self.assertNotIn("secret", json.dumps(result))

    def test_deep_zero_and_turn_relations_are_boolean_only(self):
        result = MODULE.classify_json({
            "id": "id", "object": "response", "status": "completed",
            "frequency_penalty": 0.0, "prompt_cache_retention": "24h",
            "tool_usage": {"web_search": {"count": 0}, "image_gen": {"count": 0}},
            "output": [{"id": "item", "type": "message", "role": "assistant",
                        "metadata": {"turn_id": "private-turn"},
                        "internal_chat_message_metadata_passthrough": {"turn_id": "private-turn"},
                        "content": [{"type": "output_text", "text": "secret"}]}],
        })
        self.assertTrue(result["root_extra_shapes"]["frequency_penalty"]["zero"])
        self.assertTrue(result["prompt_cache_retention_known"])
        self.assertTrue(result["tool_usage_all_numeric_leaves_zero"])
        self.assertTrue(result["message_turn_ids_equal_and_valid"])
        self.assertNotIn("private-turn", json.dumps(result))


if __name__ == "__main__":
    unittest.main()
