"""LOCAL_SIMULATED: complete synthetic exit entry, slot latch and tail injection."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

from system_exit_adapter import adapt_exit, AuditTailClock, save_budget
TOOLS = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('full_flow', TOOLS / 'full-flow-tests.py')
flow = importlib.util.module_from_spec(spec); spec.loader.exec_module(flow)


class Design(unittest.TestCase):
    def fixture(self):
        directory = tempfile.TemporaryDirectory(prefix='design-', dir=TOOLS.parent)
        self.addCleanup(directory.cleanup)
        return flow.Fixture(directory.name)

    def adapt_started(self, f):
        self.assertEqual(f.through('start')[0], 0)
        start_path = f.epoch / 'start-receipt.json'
        start = json.loads(start_path.read_text()); start['production_active_configuration'] = []; start['historical_file_hashes'] = {}
        flow.write_json(start_path, start)
        budget = json.loads((f.epoch / 'budget.json').read_text())
        budget.update(provider_intent_limit=0, provider_intents=0, new_inference_limit=0, cumulative_inference_limit=35)
        flow.write_json(f.epoch / 'budget.json', budget)
        raw = (TOOLS / 'remote-exit.py').read_text()
        adapted = adapt_exit(raw, flow.EPOCH, f.unit)
        original = f.run
        def execute_exit(command, input=None, text=False, check=False, **kwargs):
            if command[0] == 'ssh' and f.action == 'exit':
                module = types.SimpleNamespace(production_identity=lambda: f.production, save_budget=save_budget)
                with patch.dict(sys.modules, {'system_exit_adapter': module}):
                    code, out, err = f.execute(adapted)
                return subprocess.CompletedProcess(command, code, out.encode(), err.encode())
            return original(command, input=input, text=text, check=check, **kwargs)
        f.run = execute_exit
        return raw, adapted

    def test_complete_exit_entry_with_zero_budget_and_frozen_binding(self):
        f = self.fixture(); raw, adapted = self.adapt_started(f)
        before = (f.epoch / 'startup-observations.json').read_bytes()
        self.assertEqual(f.invoke('exit')[0], 0)
        final = json.loads((f.epoch / 'final-receipt.json').read_text())
        self.assertEqual(final['resource_exit_status'], 'PASS')
        self.assertEqual(final['count_reconciliation_status'], 'PASS')
        self.assertEqual(final['management_get_intents_in_exit'], 0)
        self.assertTrue(json.loads((f.epoch / 'budget.json').read_text())['stopped'])
        self.assertFalse(f.started)
        self.assertEqual((f.epoch / 'startup-observations.json').read_bytes(), before)
        self.assertTrue((f.epoch / 'startup-stop-intent.json').exists())
        begin, end = 'try:\n    identity = probe.identity()', "pid = identity['pid']"
        self.assertEqual(raw[raw.index(begin):raw.index(end)], adapted[adapted.index(begin):adapted.index(end)])

    def test_adapted_exit_unknown_identity_latches_without_physical_stop(self):
        f = self.fixture(); self.adapt_started(f)
        (f.proc / '202/environ').write_bytes((f.proc / '202/environ').read_bytes() + b'LD_PRELOAD=duplicate\0')
        self.assertNotEqual(f.invoke('exit')[0], 0)
        self.assertTrue(f.started)
        self.assertTrue(json.loads((f.epoch / 'budget.json').read_text())['stopped'])
        self.assertFalse((f.epoch / 'startup-stop-intent.json').exists())

    def test_adapter_has_no_production_credential_read(self):
        source = adapt_exit((TOOLS / 'remote-exit.py').read_text(), 'synthetic-A', 'cpar-synthetic-A.service')
        self.assertNotIn('management-key', source)
        self.assertNotIn('private/bootstrap.py', source)
        self.assertIn("start['assembled_launch_sha256'] == freeze['assembled_launch_sha256']", source)
        self.assertIn("start['argv_sha256']", source)
        self.assertIn('probe.stop(identity, persist_stop)', source)

    def test_tail_clock_appends_marker_only_after_full_real_clock_interval(self):
        directory = tempfile.TemporaryDirectory(); self.addCleanup(directory.cleanup)
        root = Path(directory.name); audit = root / 'audit.jsonl'
        fake = types.SimpleNamespace(now=100.0)
        fake.monotonic = lambda: fake.now
        fake.sleep = lambda seconds: setattr(fake, 'now', fake.now + seconds)
        probe = types.SimpleNamespace(epoch=root, trace={'observations': []})
        clock = AuditTailClock(fake, probe, audit, 'synthetic-marker')
        begin = clock.monotonic()
        probe.trace['observations'].append({'phase': 'quiet_audit_started', 'monotonic_seconds': begin + .003})
        clock.sleep(64.5); self.assertFalse(audit.exists())
        clock.sleep(.5); raw = audit.read_bytes()
        self.assertEqual(clock.injection['elapsed_seconds'], 65)
        self.assertEqual(json.loads(raw)['marker'], 'synthetic-marker')
        clock.sleep(1); self.assertEqual(audit.read_bytes(), raw)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), clock.injection['sha256'])

    def test_tail_injection_never_truncates_existing_audit(self):
        directory = tempfile.TemporaryDirectory(); self.addCleanup(directory.cleanup)
        root = Path(directory.name); audit = root / 'audit.jsonl'; audit.write_bytes(b'previous-event\n')
        fake = types.SimpleNamespace(now=0.0)
        fake.monotonic = lambda: fake.now
        fake.sleep = lambda seconds: setattr(fake, 'now', fake.now + seconds)
        probe = types.SimpleNamespace(epoch=root, trace={'observations': []})
        clock = AuditTailClock(fake, probe, audit, 'synthetic-marker'); clock.monotonic()
        probe.trace['observations'].append({'phase': 'quiet_audit_started', 'monotonic_seconds': 0})
        with self.assertRaises(AssertionError): clock.sleep(65)
        self.assertEqual(audit.read_bytes(), b'previous-event\n')

    def test_failed_system_slot_is_durable_and_never_dispatches_next(self):
        f = self.fixture()
        names = ['startup_probe.py', 'launch_recipe.py', 'linux-system-validation.py', 'remote-exit.py', 'system_exit_adapter.py']
        for name in [*names, 'system-control.py']:
            (f.public / 'tools' / name).write_bytes((TOOLS / name).read_bytes())
        (f.private / 'recipe-private.json').write_text('{}')
        files = {name: hashlib.sha256((f.public / 'tools' / name).read_bytes()).hexdigest() for name in names}
        files['recipe.json'] = hashlib.sha256(b'{}').hexdigest()
        for name in ['artifact-manifest.json', 'gateway-build-metadata.json']:
            files[name] = hashlib.sha256((f.private / 'payload' / name).read_bytes()).hexdigest()
        flow.write_json(f.public / 'system-freeze.json', {'files': files,
            'controller_sha256': hashlib.sha256((f.public / 'tools/system-control.py').read_bytes()).hexdigest()})
        flow.write_json(f.public / 'review-system-source.json', {'test_authority': 'LOCAL_SIMULATED',
            'axes': {'Standards': {'status': 'PASS'}, 'Spec': {'status': 'PASS'}},
            'system_freeze_sha256': hashlib.sha256((f.public / 'system-freeze.json').read_bytes()).hexdigest()})
        calls = []
        def dispatch(command, **kwargs):
            state = json.loads((f.private / 'system-stage.json').read_text())
            self.assertEqual(state['runs'][0]['status'], 'UNKNOWN')
            calls.append(command)
            return subprocess.CompletedProcess(command, 1, b'{"status":"FAIL","resource_exit_status":"UNKNOWN"}', b'')
        for letter in ['A', 'B']:
            with flow.process_file_lifetime(), patch.object(subprocess, 'run', dispatch), patch.object(sys, 'argv', ['controller', letter]):
                with self.assertRaises(AssertionError): runpy.run_path(str(f.public / 'tools/system-control.py'))
        state = json.loads((f.private / 'system-stage.json').read_text())
        self.assertTrue(state['stopped']); self.assertEqual(len(state['runs']), 1); self.assertEqual(len(calls), 1)


if __name__ == '__main__':
    unittest.main(verbosity=2)
