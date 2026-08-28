#!/usr/bin/env python3
"""No-network regression tests for the P12-06 live differential classifier."""

import importlib.util
import io
import json
import copy
from pathlib import Path
import unittest


PATH = Path(__file__).with_name("p12-06-openai-differential.py")
SPEC = importlib.util.spec_from_file_location("p12_06_openai_differential", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class FakeResponse(io.BytesIO):
    pass


def event(value):
    return b"data: " + json.dumps(value, separators=(",", ":")).encode() + b"\n\n"


class DifferentialTest(unittest.TestCase):
    def test_nonstream_text_and_tool_project_without_values(self):
        text = MODULE.observe_json({
            "status": "completed", "output": [{"type": "message", "content": [
                {"type": "output_text", "text": "secret-text"},
            ]}],
            "usage": {"input_tokens": 2, "output_tokens": 3, "total_tokens": 5},
        }, False)
        self.assertEqual(text["projection"], ["response_start", "message_start", "text_delta", "message_end", "response_end"])
        tool = MODULE.observe_json({
            "status": "completed",
            "output": [{"type": "function_call", "name": "emit_probe", "call_id": "opaque", "arguments": "{\"value\":\"ready\"}"}],
            "usage": {"input_tokens": 2, "output_tokens": 3, "total_tokens": 5},
        }, True)
        self.assertEqual(tool["terminal"], "tool_use")
        self.assertNotIn("ready", json.dumps(tool))

    def test_stream_projects_lifecycle_usage_and_bounded_metrics(self):
        wire = b"".join([
            event({"type": "response.created"}),
            event({"type": "response.output_item.added", "item": {"type": "message"}}),
            event({"type": "response.output_text.delta", "delta": "secret-text"}),
            event({"type": "response.output_item.done", "item": {"type": "message"}}),
            event({"type": "response.completed", "response": {"usage": {
                "input_tokens": 4, "output_tokens": 5,
                "total_tokens": 9,
                "input_tokens_details": {"cached_tokens": 1},
                "output_tokens_details": {"reasoning_tokens": 2},
            }}}),
            b"data: [DONE]\n\n",
        ])
        observed = MODULE.observe_stream(FakeResponse(wire), False, MODULE.time.monotonic())
        self.assertEqual(observed["projection"][-1], "response_end")
        self.assertTrue(observed["usage"]["nonnegative"])
        self.assertTrue(observed["usage"]["conserved"])
        self.assertEqual(observed["_output_tokens"], 5)
        self.assertNotIn("secret-text", json.dumps(MODULE.public_observation(observed)))

    def test_tool_stream_requires_complete_expected_structure(self):
        wire = b"".join([
            event({"type": "response.created"}),
            event({"type": "response.output_item.added", "item": {"type": "function_call"}}),
            event({"type": "response.function_call_arguments.delta", "delta": "{}"}),
            event({"type": "response.output_item.done", "item": {
                "type": "function_call", "name": "emit_probe", "call_id": "opaque", "arguments": "{\"value\":\"ready\"}"}}),
            event({"type": "response.completed", "response": {"usage": {
                "input_tokens": 4, "output_tokens": 5, "total_tokens": 9}}}),
        ])
        observed = MODULE.observe_stream(FakeResponse(wire), True, MODULE.time.monotonic())
        self.assertEqual(observed["terminal"], "tool_use")
        self.assertEqual(observed["projection"], ["response_start", "tool_start", "tool_delta", "tool_end", "response_end"])

    def test_wrong_mode_and_duplicate_tools_fail_closed(self):
        tool = {"type": "function_call", "name": "emit_probe", "call_id": "opaque", "arguments": "{\"value\":\"ready\"}"}
        with self.assertRaises(MODULE.ProbeFailure):
            MODULE.observe_json({"status": "completed", "output": [tool]}, False)
        with self.assertRaises(MODULE.ProbeFailure):
            MODULE.observe_json({"status": "completed", "output": [tool, tool]}, True)

    def test_network_and_file_boundaries_fail_closed(self):
        with self.assertRaises(MODULE.ProbeFailure):
            MODULE.endpoint("https://example.test/v1/responses")
        with self.assertRaises(MODULE.ProbeFailure):
            MODULE.observe_stream(FakeResponse(b"data: " + b"x" * (MODULE.MAX_EVENT + 1) + b"\n"), False, MODULE.time.monotonic())

    def test_usage_requires_nonnegative_conserved_totals(self):
        self.assertTrue(MODULE.usage_valid(MODULE.usage_shape({
            "input_tokens": 2, "output_tokens": 3, "total_tokens": 5,
        })))
        self.assertFalse(MODULE.usage_valid(MODULE.usage_shape({
            "input_tokens": 2, "output_tokens": 3, "total_tokens": 6,
        })))
        self.assertFalse(MODULE.usage_valid(MODULE.usage_shape({
            "input_tokens": 0, "output_tokens": 1, "total_tokens": True,
        })))

    def test_differential_allows_optional_usage_detail_presence_to_differ(self):
        usage = {
            "present": True, "nonnegative": True, "total_present": True, "conserved": True,
            "cache_read_present": False, "cache_creation_present": False, "reasoning_present": True,
        }
        arm = {
            "requested_samples": 10, "successful_samples": 10, "projection_consistent": True,
            "stream_projection": ["response_start", "message_start", "text_delta", "message_end", "response_end"],
            "stream_terminal": "end_turn", "stream_usage_invariants_valid": True,
            "stream_usage_shape": copy.deepcopy(usage),
            "nonstream": {"projection": ["response_start", "message_start", "text_delta", "message_end", "response_end"],
                          "terminal": "end_turn", "usage": copy.deepcopy(usage)},
            "tool_stream": {"projection": ["response_start", "tool_start", "tool_delta", "tool_end", "response_end"],
                            "terminal": "tool_use", "usage": copy.deepcopy(usage)},
        }
        incumbent = copy.deepcopy(arm)
        incumbent["stream_usage_shape"]["cache_read_present"] = True
        incumbent["nonstream"]["usage"]["cache_read_present"] = True
        incumbent["tool_stream"]["usage"]["cache_read_present"] = True
        self.assertTrue(all(MODULE.evaluate_differential(incumbent, arm).values()))

        incumbent["nonstream"]["usage"]["conserved"] = False
        result = MODULE.evaluate_differential(incumbent, arm)
        self.assertFalse(result["nonstream_usage_invariants_valid"])


if __name__ == "__main__":
    unittest.main()
