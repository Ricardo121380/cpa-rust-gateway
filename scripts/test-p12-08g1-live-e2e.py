#!/usr/bin/env python3
"""No-network regression tests for the P12-08G1 live harness."""

import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest


PATH = Path(__file__).with_name("p12-08g1-live-e2e.py")
SPEC = importlib.util.spec_from_file_location("p12_08g1_live_e2e", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def events(*values):
    output = bytearray()
    for value in values:
        payload = value if isinstance(value, str) else json.dumps(value, separators=(",", ":"))
        output.extend(b"data: " + payload.encode() + b"\n\n")
    return io.BytesIO(bytes(output))


class LiveHarnessTest(unittest.TestCase):
    def test_only_exact_loopback_base_is_admitted(self):
        self.assertEqual(MODULE.loopback_endpoint("http://127.0.0.1:18180"), ("127.0.0.1", 18180))
        for value in ("https://127.0.0.1:18180", "http://localhost:18180", "http://127.0.0.1:18180/v1"):
            with self.assertRaises(MODULE.ProbeFailure):
                MODULE.loopback_endpoint(value)

    def test_nonstreaming_protocol_observers_keep_only_projection(self):
        responses = MODULE.observe_responses_json({
            "status": "completed",
            "output": [{"type": "message", "content": [{"type": "output_text", "text": "private"}]}],
            "usage": {"input_tokens": 2, "output_tokens": 3, "total_tokens": 5},
        }, False)
        chat = MODULE.observe_chat_json({
            "choices": [{"message": {"content": "private"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5},
        }, False)
        messages = MODULE.observe_messages_json({
            "type": "message", "content": [{"type": "text", "text": "private"}], "stop_reason": "end_turn",
            "usage": {"input_tokens": 2, "output_tokens": 3},
        }, False)
        encoded = json.dumps([responses, chat, messages])
        self.assertNotIn("private", encoded)
        self.assertTrue(all(item["usage_valid"] for item in (responses, chat, messages)))

    def test_tool_observers_require_exact_completed_arguments(self):
        responses = MODULE.observe_responses_json({
            "status": "completed",
            "output": [{"type": "function_call", "name": "emit_probe", "arguments": '{"value":"ready"}'}],
            "usage": {"input_tokens": 2, "output_tokens": 3, "total_tokens": 5},
        }, True)
        chat = MODULE.observe_chat_json({
            "choices": [{"message": {"tool_calls": [{"function": {"name": "emit_probe", "arguments": '{"value":"ready"}'}}]}, "finish_reason": "tool_calls"}],
            "usage": {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5},
        }, True)
        messages = MODULE.observe_messages_json({
            "type": "message", "content": [{"type": "tool_use", "name": "emit_probe", "input": {"value": "ready"}}],
            "stop_reason": "tool_use", "usage": {"input_tokens": 2, "output_tokens": 3},
        }, True)
        self.assertTrue(all(item["projection"] == "tool" for item in (responses, chat, messages)))
        with self.assertRaises(MODULE.ProbeFailure):
            MODULE.observe_messages_json({
                "type": "message", "content": [{"type": "tool_use", "name": "emit_probe", "input": {}}],
                "stop_reason": "tool_use", "usage": {"input_tokens": 2, "output_tokens": 3},
            }, True)

    def test_streaming_protocol_observers_close_usage_and_terminal(self):
        responses = MODULE.observe_responses_stream(events(
            {"type": "response.created"},
            {"type": "response.output_text.delta", "delta": "private"},
            {"type": "response.completed", "response": {"usage": {"input_tokens": 2, "output_tokens": 3, "total_tokens": 5}}},
            "[DONE]",
        ), False)
        chat = MODULE.observe_chat_stream(events(
            {"choices": [{"delta": {"content": "private"}, "finish_reason": None}]},
            {"choices": [{"delta": {}, "finish_reason": "stop"}]},
            {"choices": [], "usage": {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5}},
            "[DONE]",
        ), False)
        messages = MODULE.observe_messages_stream(events(
            {"type": "message_start", "message": {"usage": {"input_tokens": 2}}},
            {"type": "content_block_delta", "delta": {"type": "text_delta", "text": "private"}},
            {"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 3}},
            {"type": "message_stop"},
        ), False)
        self.assertTrue(all(item["usage_valid"] for item in (responses, chat, messages)))

    def test_fixed_request_shapes_cover_twelve_unique_tuples(self):
        shapes = []
        for protocol in ("chat", "responses", "messages"):
            for stream, tool in ((False, False), (True, False), (False, True), (True, True)):
                value = json.loads(MODULE.request_body(protocol, stream=stream, tool=tool))
                shapes.append((protocol, value["stream"], "tools" in value))
        self.assertEqual(len(shapes), 12)
        self.assertEqual(len(set(shapes)), 12)

    def test_resume_replaces_only_the_last_failed_tuple(self):
        tuples = MODULE.tuple_matrix()
        receipt = {
            "schema_version": 1,
            "value_free": True,
            "tuples": [
                {"protocol": "chat", "stream": False, "mode": "text", "result": "PASS"},
                {"protocol": "chat", "stream": True, "mode": "text", "result": "FAIL"},
            ],
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "receipt.json"
            path.write_text(json.dumps(receipt), encoding="utf-8")
            path.chmod(0o600)
            index, preserved = MODULE.resume_index(path, tuples)
        self.assertEqual(index, 1)
        self.assertEqual(len(preserved), 1)

    def test_resume_rejects_a_gap_or_reordered_tuple(self):
        receipt = {
            "schema_version": 1,
            "value_free": True,
            "tuples": [{"protocol": "responses", "stream": False, "mode": "text", "result": "FAIL"}],
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "receipt.json"
            path.write_text(json.dumps(receipt), encoding="utf-8")
            path.chmod(0o600)
            with self.assertRaises(MODULE.ProbeFailure):
                MODULE.resume_index(path, MODULE.tuple_matrix())


if __name__ == "__main__":
    unittest.main()
