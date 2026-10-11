#!/usr/bin/env python3
"""No Provider calls: diagnostic limits, latch, crash/replay and exact synthetic semantics."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("diagnostic",Path(__file__).parent/"acceptance/cpar_build_json_diagnostic.py")
diagnostic = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostic)


class DiagnosticTests(unittest.TestCase):
    def test_one_pin_one_chat_and_permanent_stop(self):
        budget=diagnostic.new_budget()
        diagnostic.reserve_pin(budget)
        self.assertTrue(budget["stopped"])
        with self.assertRaises(ValueError): diagnostic.reserve_chat(budget)
        with self.assertRaises(ValueError): diagnostic.reserve_pin(budget)
        diagnostic.finish_pin(budget,True)
        diagnostic.reserve_chat(budget)
        for result in ("REPRODUCED","NOT_REPRODUCED","UNKNOWN"):
            finished=copy.deepcopy(budget)
            diagnostic.finish_chat(finished,result)
            self.assertTrue(finished["stopped"])
            self.assertEqual(finished["new_pin_intents"]+finished["new_basic_intents"],2)
            with self.assertRaises(ValueError): diagnostic.reserve_chat(finished)
            with self.assertRaises(ValueError): diagnostic.reserve_pin(finished)

    def test_failed_pin_never_opens_chat(self):
        budget=diagnostic.new_budget()
        diagnostic.reserve_pin(budget)
        diagnostic.finish_pin(budget,False)
        with self.assertRaises(ValueError): diagnostic.reserve_chat(budget)
        with self.assertRaises(ValueError): diagnostic.finish_pin(budget,True)

    def test_saved_unknown_cannot_be_replayed_after_crash(self):
        for stage in ("pin","chat"):
            budget=diagnostic.new_budget()
            diagnostic.reserve_pin(budget)
            if stage=="chat":
                diagnostic.finish_pin(budget,True)
                diagnostic.reserve_chat(budget)
            with tempfile.TemporaryDirectory() as root:
                path=Path(root)/"budget.json"
                diagnostic.save_budget(path,budget)
                restored=json.loads(path.read_text())
                self.assertEqual(path.stat().st_mode & 0o777,0o600)
                with self.assertRaises(ValueError): diagnostic.reserve_pin(restored)
                with self.assertRaises(ValueError): diagnostic.reserve_chat(restored)

    def test_budget_cannot_reuse_old_58_or_expand(self):
        for key,value in (("new_inference_limit",60),("new_pin_limit",6),("new_basic_limit",54),
                          ("historical_inference_requests",33),("retries",1),("provider_receive_count",0),("actual_cost",0)):
            budget=diagnostic.new_budget();budget[key]=value
            with self.assertRaises(ValueError): diagnostic.validate_budget(budget)

    def test_only_original_request_semantics_are_selected(self):
        body={"model":"grok-4.5","stream":False,"max_completion_tokens":4096,
              "messages":[{"role":"user","content":"Remember ACCEPT:"+"a"*24+". Reply READY."}]}
        diagnostic.validate_request(body)
        for key,value in (("model","other"),("stream",True),("max_completion_tokens",2048),("tools",[]),
                          ("messages",[{"role":"user","content":"arbitrary private input"}])):
            changed=copy.deepcopy(body);changed[key]=value
            with self.assertRaises(ValueError): diagnostic.validate_request(changed)


if __name__=="__main__":
    unittest.main()
