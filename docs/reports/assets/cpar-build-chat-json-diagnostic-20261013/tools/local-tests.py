"""Network-free checks of this batch's actual recipe binding and waiting policy."""
import ast
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import shlex
import sys
import unittest

sys.dont_write_bytecode = True
TOOLS = Path(__file__).resolve().parent
ROOT = TOOLS.parents[4]
PRIVATE = ROOT / 'output/cpar-build-chat-json-diagnostic-20261013'
sys.path.insert(0, str(TOOLS))
import launch_recipe
import qualification
spec = importlib.util.spec_from_file_location('diagnostic', ROOT / 'scripts/acceptance/cpar_build_json_diagnostic.py')
diagnostic = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostic)


def observation():
    return {'status': 'PASS; read-only observation', 'authorization_continuity_status': 'PASS',
            'identity_digest_matches_authorized_source': True,
            'account_evidence_id': '5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54',
            'source_account': {'provider': 'build', 'enabled': 1, 'auth_status': 'active',
                               'worker_claim_kind': None, 'revision': 140, 'last_refresh_at_ms': 500,
                               'continuation_floor_revision': 77, 'continuation_current_revision': 141},
            'authorized_native_revision': 136, 'authorized_last_refresh_at_ms': 100,
            'authorized_continuation_floor_revision': 77, 'authorized_continuation_current_revision': 137,
            'source_changed_field_names': ['revision', 'credential_ciphertext', 'continuation_current_revision'],
            'source_schema': 31, 'observed_now_ms': 100000,
            'source_pool': {'runtime_status': 'available', 'active_leases': 0,
                            'credential_revision': 141, 'expires_at_ms': 3700001},
            'historical_budget_intents': 35, 'historical_gateway_pin_confirmed_sends': 34,
            'ports_listening': [], 'competing_processes': [],
            'unit': {'MainPID': '0', 'LoadState': 'not-found'}}


class BoundPreparation(unittest.TestCase):
    def test_strict_hour_boundary_and_only_short_positive_material_waits(self):
        row = observation()
        self.assertEqual(qualification.classify(row), 'PREPARE_WINDOW_AVAILABLE')
        row['source_pool']['expires_at_ms'] -= 1
        self.assertEqual(qualification.classify(row), 'WAITING_FOR_QUALIFICATION')
        row['source_pool']['expires_at_ms'] = row['observed_now_ms']
        with self.assertRaises(AssertionError):
            qualification.classify(row)

    def test_auth_identity_and_continuity_abnormalities_never_wait(self):
        changes = [('auth_status', 'reauthorization_required'), ('enabled', 0), ('provider', 'web'),
                   ('continuation_floor_revision', 78), ('continuation_current_revision', 142),
                   ('last_refresh_at_ms', 99), ('worker_claim_kind', 'refresh')]
        for field, value in changes:
            row = observation(); row['source_account'][field] = value
            with self.subTest(field=field), self.assertRaises(AssertionError):
                qualification.classify(row)
        row = observation(); row['identity_digest_matches_authorized_source'] = False
        with self.assertRaises(AssertionError):
            qualification.classify(row)

    def test_pool_drift_and_nonrefresh_change_fail_even_with_larger_revision(self):
        for field, value in [('runtime_status', 'blocked'), ('active_leases', 1), ('credential_revision', 142)]:
            row = observation(); row['source_pool'][field] = value
            with self.subTest(field=field), self.assertRaises(AssertionError):
                qualification.classify(row)
        row = observation(); row['source_changed_field_names'].append('identity_digest')
        with self.assertRaises(AssertionError):
            qualification.classify(row)

    def test_observation_spacing_count_and_total_window(self):
        qualification.may_observe([], 0)
        previous = [{'local_started_at_ms': 0, 'classification': 'WAITING_FOR_QUALIFICATION'}]
        with self.assertRaises(AssertionError):
            qualification.may_observe(previous, 599999)
        qualification.may_observe(previous, 600000)
        with self.assertRaises(AssertionError):
            qualification.may_observe(previous, 3600001)
        with self.assertRaises(AssertionError):
            qualification.may_observe(previous * 7, 600000)
        previous[0]['classification'] = 'UNKNOWN'
        with self.assertRaises(AssertionError):
            qualification.may_observe(previous, 600000)

    def test_complete_actual_recipe_assembly_preserves_every_unbound_argument(self):
        recipe = json.loads((PRIVATE / 'recipe-private.json').read_text())
        base = Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
        epoch = base / 'private/chat-json-diagnostic-20261013'
        source = 'bf822b14a62c21cf582a7539e3635f69bd8b7677'
        selected = '245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062'
        approved = recipe['approved_launch']; before = copy.deepcopy(approved)
        launch = launch_recipe.assemble(approved, recipe['runtime_options'], base, epoch,
                                       'cpar-build-chat-json-diagnostic-20261013.service', source, selected, diagnostic)
        self.assertEqual(approved, before)
        self.assertEqual(len(launch), 36)
        self.assertEqual(launch[3:18], approved[3:18])
        self.assertEqual(launch[19], approved[19])
        self.assertEqual(launch[23], approved[23])
        env = [value for value in launch if value.startswith('--property=Environment=')]
        self.assertEqual(len(env), 1)
        self.assertEqual(shlex.split(env[0].split('Environment=', 1)[1]),
                         ['LD_PRELOAD=' + str(base / 'bin/egress-guard-tcp.so'),
                          'CPAR_BUILD_EGRESS_AUDIT=' + str(base / 'audit/chat-json-diagnostic-20261013.jsonl'),
                          'CPAR_BUILD_DIAGNOSTIC_DIR=' + str(epoch / 'native-capture'),
                          'CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256=' + selected,
                          'CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION=' + source])
        self.assertEqual(launch[34:], ['--background-provider-tasks', 'disabled'])
        with self.assertRaises(AssertionError):
            launch_recipe.assemble(approved, ['--background-provider-tasks', 'enabled'], base, epoch,
                                   'cpar-build-chat-json-diagnostic-20261013.service', source, selected, diagnostic)

    def test_actual_bound_start_rejects_stopped_before_systemd_operation(self):
        source = ast.parse((TOOLS / 'remote-start.py').read_text())
        guard = next(node for node in source.body if isinstance(node, ast.Assert)
                     and 'budget' in ast.unparse(node) and 'stopped' in ast.unparse(node))
        operation = next(node for node in ast.walk(source) if isinstance(node, ast.Call)
                         and ast.unparse(node.func) == 'subprocess.run' and ast.unparse(node.args[0]) == 'launch')
        self.assertLess(guard.lineno, operation.lineno)
        program = compile(ast.Module(body=[guard], type_ignores=[]), 'bound-stop-check', 'exec')
        budget = diagnostic.new_budget(); exec(program, {'budget': budget})
        budget['stopped'] = True
        with self.assertRaises(AssertionError):
            exec(program, {'budget': budget})

    def test_actual_start_requires_whole_process_argv_and_private_selection(self):
        source = (TOOLS / 'remote-start.py').read_text()
        self.assertIn('assert args == launch[launch.index(str(binary)):]', source)
        self.assertIn("assert hashlib.sha256(json.dumps(launch,separators=(',',':')).encode()).hexdigest() == freeze['assembled_launch_sha256']", source)
        for key in ['LD_PRELOAD', 'CPAR_BUILD_EGRESS_AUDIT', 'CPAR_BUILD_DIAGNOSTIC_DIR',
                    'CPAR_BUILD_DIAGNOSTIC_REQUEST_SHA256', 'CPAR_BUILD_DIAGNOSTIC_SOURCE_REVISION']:
            self.assertIn("assert env['" + key + "']", source)

    def test_new_payload_and_request_binding_use_fixed_candidate(self):
        payload = PRIVATE / 'payload'
        freeze = json.loads((payload / 'freeze.json').read_text())
        self.assertEqual(freeze['epoch'], 'chat-json-diagnostic-20261013')
        self.assertEqual(freeze['fixed_driver_source_sha'], 'c23d2c356ec0253d93e41ca94aac821ad167fcd6')
        self.assertEqual(freeze['source_sha'], 'bf822b14a62c21cf582a7539e3635f69bd8b7677')
        self.assertEqual(len(freeze['files']), 16)
        for name, expected in freeze['files'].items():
            self.assertEqual(hashlib.sha256((payload / name).read_bytes()).hexdigest(), expected)
        self.assertEqual(freeze['selected_request_sha256'], '245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062')


if __name__ == '__main__':
    unittest.main()
