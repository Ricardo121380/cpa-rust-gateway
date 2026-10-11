#!/usr/bin/env python3
"""No Provider calls: diagnostic limits, latch, crash/replay and exact synthetic semantics."""
import copy
import ast
import importlib.util
import json
import hashlib
import io
from pathlib import Path
import subprocess
import sqlite3
import shlex
import sys
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("diagnostic",Path(__file__).parent/"acceptance/cpar_build_json_diagnostic.py")
diagnostic = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostic)


class DiagnosticTests(unittest.TestCase):
    tools = Path(__file__).parent.parent/'docs/reports/assets/cpar-build-json-502-20261011/tools'

    def test_actual_startup_injects_environment_without_changing_guard_or_audit(self):
        source=ast.parse((self.tools/'remote-start.py').read_text())
        injection=next(node for node in source.body if isinstance(node,ast.Assign) and
                       (any(isinstance(target,ast.Subscript) and isinstance(target.value,ast.Name)
                            and target.value.id=='launch' for target in node.targets) or
                        isinstance(node.value,ast.Call) and isinstance(node.value.func,ast.Attribute)
                        and node.value.func.attr=='with_diagnostic_environment'))
        epoch=Path('/protected/epoch');binary=Path('/protected/bin/gateway');guard=Path('/protected/bin/guard.so')
        audit=Path('/protected/audit/epoch.jsonl')
        environment='--property=Environment=LD_PRELOAD='+str(guard)+' CPAR_BUILD_EGRESS_AUDIT='+str(audit)
        original=['systemd-run','--unit=owned.service','--property=NoNewPrivileges=yes',environment,
                  str(binary),'serve','--state-dir',str(epoch/'state'),'--background-provider-tasks','disabled']
        namespace={'launch':list(original),'binary':binary,'guard':guard,'audit':audit,'epoch':epoch,
                   'prepare':{'selected_request_sha256':'a'*64},'source_sha':'b'*40,'diagnostic':diagnostic}
        try:
            exec(compile(ast.fix_missing_locations(ast.Module(body=[injection],type_ignores=[])),'actual-startup-env','exec'),namespace)
        except ValueError as error:
            self.fail('approved Environment property rejected: '+type(error).__name__)
        result=namespace['launch']
        self.assertEqual(result[:3],original[:3])
        self.assertEqual(result[4:],original[4:])
        entries=shlex.split(result[3].removeprefix('--property=Environment='))
        self.assertEqual(entries[:2],['LD_PRELOAD='+str(guard),'CPAR_BUILD_EGRESS_AUDIT='+str(audit)])
        self.assertEqual(entries[2:],['CPAR_BUILD_DIAGNOSTIC_DIR='+str(epoch/'native-capture'),
                                      'CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256='+'a'*64,
                                      'CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION='+'b'*40])

    def test_launch_injection_rejects_missing_duplicate_or_changed_environment(self):
        valid=['systemd-run','--property=Environment=LD_PRELOAD=/guard CPAR_BUILD_EGRESS_AUDIT=/audit',
               '/gateway','serve']
        for launch in ([valid[0],*valid[2:]], [*valid[:2],valid[1],*valid[2:]],
                       [valid[0],valid[1].replace('/guard','/other'),*valid[2:]],
                       [valid[0],valid[1]+' CPAR_BUILD_DIAGNOSTIC_DIR=/old',*valid[2:]]):
            before=list(launch)
            with self.assertRaises(ValueError):
                diagnostic.with_diagnostic_environment(launch,'/gateway','/guard','/audit','/capture','a'*64,'b'*40)
            self.assertEqual(launch,before)
        result=diagnostic.with_diagnostic_environment(valid,'/gateway','/guard','/audit','/capture','a'*64,'b'*40)
        self.assertEqual(valid[1],'--property=Environment=LD_PRELOAD=/guard CPAR_BUILD_EGRESS_AUDIT=/audit')
        self.assertEqual(len(result),len(valid))

    def test_actual_startup_rejects_terminated_epoch_before_launch(self):
        source=ast.parse((self.tools/'remote-start.py').read_text())
        guards=[node for node in source.body if isinstance(node,ast.Assert) and
                any(isinstance(value,ast.Subscript) and isinstance(value.value,ast.Name)
                    and value.value.id=='budget' and isinstance(value.slice,ast.Constant)
                    and value.slice.value=='stopped' for value in ast.walk(node))]
        self.assertEqual(len(guards),1,'missing pre-launch stopped-epoch guard')
        launch=next(node for node in source.body if isinstance(node,ast.Try))
        self.assertLess(guards[0].lineno,launch.lineno)
        check=compile(ast.fix_missing_locations(ast.Module(body=guards,type_ignores=[])),'actual-startup-budget','exec')
        budget=diagnostic.new_budget()
        exec(check,{'budget':budget})
        budget['stopped']=True
        with self.assertRaises(AssertionError):
            exec(check,{'budget':budget})

    def startup_failure(self, started, changed_identity=False):
        source=ast.parse((self.tools/'remote-start.py').read_text())
        handler=next(node for node in source.body if isinstance(node,ast.Try)).handlers[0]
        calls=[]
        identity={'pid':11,'process_start':'100','binary_sha256':'a'*64,'state_dir':'owned'}
        state={'identity':dict(identity,pid=22) if changed_identity else (identity if started else None)}
        class Commands:
            @staticmethod
            def run(args,**options):
                calls.append(args)
                state['identity']=None
        namespace={'subprocess':Commands,'json':json,'unit':'owned.service','started':started,'launch_requested':started,
                   'startup_identity':identity if started else None,
                   'read_owned_identity':lambda:state['identity'],
                   'temporary_ports':lambda:[],'process_exists':lambda pid:False}
        with tempfile.TemporaryDirectory() as root:
            namespace['epoch']=Path(root)
            functions=[node for node in source.body if isinstance(node,ast.FunctionDef) and node.name=='shutdown_owned_start']
            exec(compile(ast.fix_missing_locations(ast.Module(body=functions,type_ignores=[])),'startup-cleanup','exec'),namespace)
            failure=ast.Try(body=[ast.Raise(exc=ast.Call(func=ast.Name(id='RuntimeError',ctx=ast.Load()),args=[],keywords=[]))],
                            handlers=[handler],orelse=[],finalbody=[])
            with self.assertRaises(RuntimeError):
                exec(compile(ast.fix_missing_locations(ast.Module(body=[failure],type_ignores=[])),'startup-failure','exec'),namespace)
            receipt=json.loads((Path(root)/'start-failure-v2.json').read_text())
        return calls,receipt

    def test_startup_cleanup_never_stops_changed_identity(self):
        calls,receipt=self.startup_failure(True,changed_identity=True)
        self.assertEqual(calls,[])
        self.assertIsNot(receipt['temporary_service_stopped'],True)

    def test_startup_without_owned_process_never_claims_a_verified_stop(self):
        calls,receipt=self.startup_failure(False)
        self.assertEqual(calls,[])
        self.assertIsNot(receipt['temporary_service_stopped'],True)

    def test_startup_cleanup_stops_only_the_bound_process_and_verifies_exit(self):
        calls,receipt=self.startup_failure(True)
        self.assertEqual(calls,[['systemctl','stop','owned.service']])
        self.assertIs(receipt['temporary_service_stopped'],True)
        self.assertEqual(receipt['owned_process_stop_status'],'STOPPED_VERIFIED')

    def collect_usage(self, *, persisted_input=1, source_event='usage-1', client_key='client',
                      candidate='candidate', sidecar=True, execution_change=None, payload_hash=None):
        source=ast.parse((self.tools/'remote-chat.py').read_text())
        attempt=next(node for node in source.body if isinstance(node,ast.Try))
        collection=next(node for node in attempt.body if isinstance(node,ast.With))
        with sqlite3.connect(':memory:') as db:
            db.executescript('CREATE TABLE gateway_event_log(event_ordinal INTEGER,event_type TEXT,event_id TEXT,request_id TEXT,payload_json TEXT);'
                             'CREATE TABLE gateway_attempt_execution(attempt_id TEXT,request_id TEXT,attempt_payload_sha256 TEXT,evidence_json TEXT);'
                             'CREATE TABLE billing_ledger_entries(source_event_id TEXT,request_id TEXT,response_id TEXT,cost_microunits INTEGER,'
                             'input_tokens INTEGER,output_tokens INTEGER,reasoning_tokens INTEGER,cache_read_tokens INTEGER,cache_creation_tokens INTEGER,cached_tokens INTEGER,'
                             'usage_evidence_json TEXT,account_id TEXT,model TEXT);')
            usage={'provenance':'measured','input_accounting':'inclusive','input_tokens':persisted_input,'output_tokens':None,
                   'reasoning_tokens':None,'cache_read_tokens':None,'cache_creation_tokens':None,'cached_tokens':None}
            capabilities={'reasoning':True,'parallel':False,'stored':True,'continuation':True,'compact':True,'websocket':True}
            target={'channel':'grok.build','endpoint_id':'endpoint','route_id':'route','route_candidate_id':'candidate',
                    'upstream_model':'grok-4.5','credential_evidence_id':'account','credential_revision':138,
                    'config_version_id':'config','config_revision':9,'egress_evidence_id':'direct',
                    'egress_revision':{'source':'not_applicable'},'capabilities':capabilities,
                    'declaration_evidence':{'client_key_evidence_id':'client'}}
            execution={'channel':'grok.build','credential_revision':138,'config_version_id':'config','config_revision':9,
                       'egress':{'kind':'direct'},'capabilities':capabilities}
            execution.update(execution_change or {})
            rows=[('request',{'request_id':'r','client_key_id':client_key,'protocol':'openai_chat_completions',
                              'streaming':False,'requested_model':'grok-4.5','public_model':'grok-4.5'}),
                  ('attempt',{'request_id':'r','attempt_id':'a','attempt_number':1,'endpoint_id':'endpoint','route_id':'route',
                              'route_candidate_id':candidate,'upstream_model':'grok-4.5','credential_id':'account',
                              'outcome':'succeeded','retry_decision':'completed'}),
                  ('request_finished',{'request_id':'r','outcome':'failed'}),
                  ('usage',{'request_id':'r','attempt_id':'a','response_id':'response','usage':usage})]
            for ordinal,(kind,data) in enumerate(rows,1):
                payload=json.dumps({kind:data})
                db.execute('INSERT INTO gateway_event_log VALUES(?,?,?,?,?)',
                           (ordinal,kind,'usage-1' if kind=='usage' else 'a' if kind=='attempt' else kind,'r',payload))
                if kind=='attempt' and sidecar:
                    db.execute('INSERT INTO gateway_attempt_execution VALUES(?,?,?,?)',
                               ('a','r',payload_hash or hashlib.sha256(payload.encode()).hexdigest(),
                                json.dumps(execution,separators=(',',':'))))
            db.execute('INSERT INTO billing_ledger_entries VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)',
                       (source_event,'r','response',None,persisted_input,None,None,None,None,None,json.dumps({'provenance':'measured','input_accounting':'inclusive'}),'account','grok-4.5'))
            db.commit()
            class Formal:
                opaque=staticmethod(lambda value:value)
            namespace={'db':db,'sqlite3':sqlite3,'json':json,'result':{'request_id':'r'},'start':{'account_evidence_id':'account'},
                       'native':{'id':'response','usage':{'input_tokens':1}},'formal':Formal,'diagnostic':diagnostic,
                       'plan':{'target':target},'summary':{}}
            exec(compile(ast.fix_missing_locations(ast.Module(body=collection.body,type_ignores=[])),'chat-lineage','exec'),namespace)
            return namespace['summary']

    def test_real_collection_rejects_changed_native_usage_and_wrong_ledger_source(self):
        for options in ({'persisted_input':999},{'source_event':'unrelated'}):
            with self.assertRaises((ValueError,AssertionError)):
                self.collect_usage(**options)

    def test_real_collection_accepts_exact_nullable_counters_and_event_source(self):
        summary=self.collect_usage()
        self.assertEqual(summary['validated_native_usage']['input_tokens'],1)
        self.assertIsNone(summary['validated_native_usage']['output_tokens'])
        self.assertIsNone(summary['validated_native_usage']['cache_creation_tokens'])
        self.assertTrue(summary['ledger_source_event_exact_match'])

    def test_real_collection_rejects_wrong_client_candidate_or_missing_execution(self):
        for options in ({'client_key':'wrong'},{'candidate':'wrong'},{'sidecar':False}):
            with self.subTest(options=options), self.assertRaises((ValueError,AssertionError)):
                self.collect_usage(**options)

    def test_real_collection_rejects_wrong_actual_lease_configuration_egress_or_binding(self):
        for options in ({'execution_change':{'credential_revision':137}},
                        {'execution_change':{'config_version_id':'wrong'}},
                        {'execution_change':{'config_revision':8}},
                        {'execution_change':{'egress':{'kind':'unknown'}}},
                        {'execution_change':{'capabilities':{'reasoning':False}}},
                        {'payload_hash':'0'*64}):
            with self.subTest(options=options), self.assertRaises((ValueError,AssertionError)):
                self.collect_usage(**options)

    def test_absent_native_usage_stays_unknown_and_invalid_counters_are_rejected(self):
        for value in ({}, {'usage':None}, {'usage':{}}, {'usage':{'total_tokens':999}}):
            self.assertIsNone(diagnostic.native_usage_summary(value))
        for value in (True,-1,1.5,'1',18446744073709551616):
            with self.assertRaises(ValueError):
                diagnostic.native_usage_summary({'usage':{'input_tokens':value}})

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
