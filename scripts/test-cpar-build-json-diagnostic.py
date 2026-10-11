#!/usr/bin/env python3
"""No Provider calls: diagnostic limits, latch, crash/replay and exact synthetic semantics."""
import copy
import importlib.util
import json
import hashlib
import io
from pathlib import Path
import subprocess
import sys
import tarfile
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

    def upload(self, root, *, unexpected=False, bad_hash=False):
        base=Path(root)/'protected';base.mkdir(mode=0o700)
        (base/'bin').mkdir(mode=0o700)
        (base/'owner.json').write_text(json.dumps({'task':'cpar-build-20261010'}))
        names=['gateway-aarch64-unknown-linux-gnu','artifact-manifest.json','gateway-build-metadata.json',
               'cpar-batch-c-live.py','cpar_new_build_acceptance.py','cpar_build_json_diagnostic.py',
               'remote-readiness.py','remote-prepare.py','remote-start.py','remote-pin.py','remote-chat.py','remote-exit.py',
               'user-authorization.json','historical-cost-authorization.json']
        payloads={name:('synthetic '+name).encode() for name in names}
        freeze={'kind':'chat_json_502_diagnostic','epoch':'chat-json-502-20261011','source_sha':'a'*40,
                'files':{name:hashlib.sha256(data).hexdigest() for name,data in payloads.items()}}
        raw=json.dumps(freeze).encode()
        stream=io.BytesIO()
        with tarfile.open(fileobj=stream,mode='w') as archive:
            for name,data in [('freeze.json',raw),*payloads.items()]:
                if bad_hash and name==names[0]: data=b'changed'
                member=tarfile.TarInfo(name);member.size=len(data);archive.addfile(member,io.BytesIO(data))
            if unexpected:
                member=tarfile.TarInfo('../outside');member.size=1;archive.addfile(member,io.BytesIO(b'x'))
        tool=Path(__file__).parent.parent/'docs/reports/assets/cpar-build-json-502-20261011/tools/remote-upload.py'
        source=tool.read_text().replace('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010',str(base))
        # Only replace the remote owner-uid assertion; the actual stream/path/hash/mode code runs.
        source=source.replace('base.stat().st_uid==0','base.stat().st_uid==os.getuid()')
        result=subprocess.run([sys.executable,'-c',source,hashlib.sha256(raw).hexdigest()],input=stream.getvalue(),capture_output=True)
        return result,base

    def test_exact_upload_reads_each_member_once_and_keeps_modes(self):
        with tempfile.TemporaryDirectory() as root:
            result,base=self.upload(root)
            self.assertEqual(result.returncode,0,result.stderr.decode())
            files=list((base/'bin/chat-json-502-20261011').iterdir())
            self.assertEqual(len(files),15)
            self.assertTrue(all(path.stat().st_mode & 0o777==(0o700 if path.name.startswith('gateway-aarch64') else 0o600) for path in files))

    def test_upload_rejects_extra_path_and_hash_mismatch(self):
        for options in ({'unexpected':True},{'bad_hash':True}):
            with tempfile.TemporaryDirectory() as root:
                result,base=self.upload(root,**options)
                self.assertNotEqual(result.returncode,0)
                self.assertFalse((base/'bin/outside').exists())


if __name__=="__main__":
    unittest.main()
