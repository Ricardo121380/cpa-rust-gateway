#!/usr/bin/env python3
"""Offline tests for the value-free G1 Messages SSE classifier."""

import importlib.util
import json
from pathlib import Path
import unittest


PATH = Path(__file__).with_name("p12-08g1-messages-sse-classifier.py")
SPEC = importlib.util.spec_from_file_location("p12_08g1_messages_sse_classifier", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ClassifierTest(unittest.TestCase):
    def test_complete_lifecycle_passes_without_retaining_values(self):
        result = MODULE.classify_events([
            {"type": "message_start", "message": {"id": "private", "usage": {"input_tokens": 3}}},
            {"type": "content_block_start", "content_block": {"text": "private"}},
            {"type": "content_block_delta", "delta": {"type": "text_delta", "text": "private"}},
            {"type": "content_block_stop"},
            {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 2}},
            {"type": "message_stop"},
        ])
        self.assertTrue(result["strict_lifecycle"])
        rendered = json.dumps(result)
        self.assertNotIn("private", rendered)
        self.assertNotIn("input_tokens", rendered)

    def test_missing_stop_error_and_unknown_shapes_fail_closed(self):
        for events in [
            [{"type": "message_start", "message": {"usage": {"input_tokens": 0}}}],
            [{"type": "error", "error": {"message": "private"}}],
            [{"type": "provider_private", "value": "private"}],
            [{"type": "content_block_delta", "delta": {"type": "private_delta"}}],
        ]:
            result = MODULE.classify_events(events)
            self.assertFalse(result["strict_lifecycle"])
            self.assertNotIn("private", json.dumps(result))


if __name__ == "__main__":
    unittest.main()
