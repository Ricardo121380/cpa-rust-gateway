"""Actual controller boundary regressions in private temporary roots; no network."""
import fcntl
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
tests_spec = importlib.util.spec_from_file_location('bound_tests', TOOLS / 'local-tests.py')
bound_tests = importlib.util.module_from_spec(tests_spec)
tests_spec.loader.exec_module(bound_tests)


class ControllerBoundary(unittest.TestCase):
    def prepare(self, root):
        root = Path(root)
        public = root / 'docs/reports/assets/cpar-build-chat-json-rehearsal-20261013'
        private = root / 'output/cpar-build-chat-json-rehearsal-20261013'
        (public / 'tools').mkdir(parents=True)
        (private / 'payload').mkdir(parents=True, mode=0o700)
        for name in ['local-control.py', 'qualification.py', 'remote-upload.py']:
            shutil.copyfile(TOOLS / name, public / 'tools' / name)
        (private / 'payload/qualification.py').write_bytes((TOOLS / 'qualification.py').read_bytes())
        (private / 'payload/remote-readiness.py').write_text('# FROZEN_OBSERVATION\n')
        (public / 'tools/remote-readiness.py').write_text('# UNREVIEWED_OBSERVATION\n')
        sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
        freeze = {'local_controller_sha256': sha(public / 'tools/local-control.py'),
                  'upload_tool_sha256': sha(public / 'tools/remote-upload.py'),
                  'files': {name: sha(private / 'payload' / name)
                            for name in ['qualification.py', 'remote-readiness.py']}}
        (private / 'payload/freeze.json').write_text(json.dumps(freeze))
        (public / 'review-pre-execution.json').write_text(json.dumps({
            'axes': {'Standards': {'status': 'PASS'}, 'Spec': {'status': 'PASS'}},
            'payload_freeze_sha256': sha(private / 'payload/freeze.json')}))
        return public, private

    def test_mutated_public_observation_never_reaches_transport(self):
        with tempfile.TemporaryDirectory() as root:
            public, private = self.prepare(root)
            path = public / 'tools/local-control.py'
            spec = importlib.util.spec_from_file_location('isolated_control', path)
            control = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(control)
            calls = []
            def fake_remote(script, *args, **kwargs):
                calls.append(script)
                return bound_tests.observation() | {'remaining_material_seconds': 3600.001,
                    'production': {'MainPID': '123'}, 'production_config': []}
            control.remote = fake_remote
            control.main('observe')
            self.assertEqual(len(calls), 1)
            self.assertNotIn('UNREVIEWED_OBSERVATION', calls[0])
            self.assertIn('FROZEN_OBSERVATION', calls[0])

    def test_concurrent_nonowner_cannot_rewrite_stopped_state(self):
        with tempfile.TemporaryDirectory() as root:
            public, private = self.prepare(root)
            state = {'phase': 'PREPARED_LOCAL', 'stopped': True, 'observations': [], 'formal_actions': []}
            raw = json.dumps(state).encode()
            (private / 'stage.json').write_bytes(raw)
            with (private / 'local-execution.lock').open('a') as owner:
                fcntl.flock(owner, fcntl.LOCK_EX | fcntl.LOCK_NB)
                result = subprocess.run([sys.executable, '-B', str(public / 'tools/local-control.py'), 'observe'],
                                        capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 2)
            self.assertEqual((private / 'stage.json').read_bytes(), raw)
            self.assertFalse((private / 'stop-latch.stdout').exists())


if __name__ == '__main__':
    unittest.main()
