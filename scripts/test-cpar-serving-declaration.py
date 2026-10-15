"""Run the real inventory collector against controlled management HTTP pages."""
import contextlib
import copy
import datetime
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import pathlib
import re
import tempfile
import threading
import unittest
import urllib.error
import urllib.parse
import urllib.request

SPEC = importlib.util.spec_from_file_location(
    'formal', pathlib.Path(__file__).parent / 'acceptance/cpar_new_build_acceptance.py')
formal = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(formal)


def model():
    return {'id': 'grok-4.5', 'route_id': 'route', 'public_model_id': 'public',
            'sources': [{'candidate_id': 'candidate', 'endpoint_id': 'endpoint',
                         'catalog_admission': 'allowed_unlisted',
                         'capability_evidence': {'source': 'serving_snapshot',
                             'config_version_id': 'config', 'config_revision': 9,
                             'capabilities': {key: False for key in formal.EXTENSIONS}}}]}


class CollectorTests(unittest.TestCase):
    def collect(self, change=None):
        counts, calls = {}, []
        def response(path):
            parsed = urllib.parse.urlsplit(path)
            query = urllib.parse.parse_qs(parsed.query)
            count = counts.get(parsed.path, 0) + 1
            counts[parsed.path] = count
            if parsed.path == '/admin/config-versions':
                value = [{'id': 'config', 'status': 'active', 'revision': 'rev-9'}]
            elif parsed.path == '/admin/client-keys':
                value = {'items': [{'id': 'client', 'access_group_id': 'group',
                                   'status': 'active', 'expires_at_ms': None}],
                         'next_cursor': None}
            elif parsed.path == '/admin/models/effective':
                value = {'config_version': 'config', 'access_group_id': 'group',
                         'client_key_id': query['client_key_id'][0],
                         'projection_id': '1' * 64,
                         'observed_at_ms': int(datetime.datetime.now(datetime.timezone.utc).timestamp()*1000),
                         'items': [model()], 'next_cursor': None}
            else:
                value = []
            return change(parsed.path, query, count, value) if change else (200, value)

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                calls.append(('GET', self.path))
                status, value = response(self.path)
                self.send_response(status)
                self.send_header('Content-Type', 'application/json')
                self.send_header('ETag', '"rev-9"')
                self.end_headers()
                self.wfile.write(json.dumps(value).encode())
            def do_POST(self):
                calls.append(('POST', self.path))
                self.send_error(500)
            def log_message(self, *args):
                pass

        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as directory:
                pathlib.Path(directory, 'management-key').write_text('synthetic-management-key')
                context = dict(datetime=datetime, hashlib=hashlib, json=json, pathlib=pathlib,
                    urllib=__import__('urllib'), re=re, socket=__import__('socket'),
                    opts={'--credential-dir': directory,
                          '--management-listen': '127.0.0.1:' + str(server.server_port)},
                    result={}, runtime={}, opaque=formal.opaque)
                # Execute the exact production inventory block, replacing only process discovery.
                block = formal.REMOTE_READ.split('if INVENTORY:\n', 1)[1].split(
                    "runtime['stable_process']=runtime['stable_process']", 1)[0]
                exec(compile('if True:\n' + block, 'real-collector-inventory', 'exec'), context)
                result = context['result']
                result['runtime'] = context['runtime']
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)
        self.assertFalse([row for row in calls if row[0] == 'POST'])
        return result, calls

    def test_success_retains_page_context_and_complete_declaration(self):
        result, _ = self.collect()
        self.assertTrue(result['runtime']['capability_read_complete'])
        page = result['capability_reads']['effective_models'][0]['pages'][0]
        self.assertEqual(page['http_status'], 200)
        self.assertEqual(page['config_version'], 'config')
        self.assertEqual(page['item_count'], 1)
        self.assertEqual(page['models'][0]['source_count'], 1)
        self.assertEqual(page['models'][0]['sources'][0]['declaration_state'], 'object')
        self.assertEqual(result['runtime']['capability_observations'][0]['source'], 'serving_snapshot')

    def test_second_key_read_error_cannot_look_successful(self):
        def change(path, query, count, value):
            return (503, {}) if path == '/admin/client-keys' and count == 2 else (200, value)
        result, _ = self.collect(change)
        self.assertFalse(result['runtime']['capability_read_complete'])
        self.assertEqual(result['capability_reads']['client_keys']['pages'][0]['error'], 'http_503')
        self.assertTrue(result['capability_read_errors'])

    def test_target_key_on_second_page_is_enumerated(self):
        def change(path, query, count, value):
            if path == '/admin/client-keys' and not query.get('cursor'):
                value = {'items': [], 'next_cursor': 'second'}
            return 200, value
        result, _ = self.collect(change)
        keys = result['capability_reads']['client_keys']
        self.assertTrue(keys['complete'])
        self.assertEqual(len(keys['pages']), 2)
        self.assertEqual(keys['key_evidence_ids'], [formal.opaque('client')])
        self.assertEqual(len(result['runtime']['capability_observations']), 1)

    def test_key_cursor_cycle_is_incomplete(self):
        def change(path, query, count, value):
            if path == '/admin/client-keys':
                value = {'items': [], 'next_cursor': 'cycle'}
            return 200, value
        result, _ = self.collect(change)
        self.assertFalse(result['runtime']['capability_read_complete'])
        self.assertTrue(result['capability_read_errors'])

    def test_effective_failure_and_empty_stages_are_distinct(self):
        for kind in ['http', 'models', 'sources', 'missing', 'null']:
            with self.subTest(kind=kind):
                def change(path, query, count, value):
                    if path == '/admin/models/effective':
                        if kind == 'http': return 503, {}
                        if kind == 'models': value['items'] = []
                        if kind == 'sources': value['items'][0]['sources'] = []
                        if kind == 'missing': value['items'][0]['sources'][0].pop('capability_evidence')
                        if kind == 'null': value['items'][0]['sources'][0]['capability_evidence'] = None
                    return 200, value
                result, _ = self.collect(change)
                page = result['capability_reads']['effective_models'][0]['pages'][0]
                if kind == 'http':
                    self.assertEqual(page['error'], 'http_503')
                    self.assertFalse(result['runtime']['capability_read_complete'])
                elif kind == 'models': self.assertEqual(page['item_count'], 0)
                elif kind == 'sources': self.assertEqual(page['models'][0]['source_count'], 0)
                else:
                    self.assertEqual(page['models'][0]['sources'][0]['declaration_state'], kind)
                    self.assertIsNone(result['runtime']['capability_observations'][0]['capabilities'])

    def test_effective_page_context_and_projection_changes_fail_closed(self):
        for field in ['config_version', 'access_group_id', 'client_key_id', 'projection_id']:
            with self.subTest(field=field):
                def change(path, query, count, value):
                    if path == '/admin/models/effective':
                        if not query.get('cursor'): value['next_cursor'] = 'next'
                        elif field == 'projection_id': value[field] = '2' * 64
                        else: value[field] = 'foreign'
                    return 200, value
                result, _ = self.collect(change)
                self.assertFalse(result['runtime']['capability_read_complete'])
                self.assertTrue(result['capability_read_errors'])

    def test_final_configuration_read_covers_effective_enumeration(self):
        def change(path, query, count, value):
            if path == '/admin/config-versions' and counts.get('/admin/models/effective'):
                value[0]['revision'] = 'rev-10'
            if path == '/admin/models/effective': counts[path] = True
            return 200, value
        counts = {}
        result, _ = self.collect(change)
        self.assertFalse(result['configuration_stable'])

    def test_target_key_missing_is_distinct_from_read_failure(self):
        def change(path, query, count, value):
            if path == '/admin/client-keys': value['items'] = []
            return 200, value
        result, _ = self.collect(change)
        self.assertTrue(result['runtime']['capability_read_complete'])
        with self.assertRaisesRegex(formal.Blocked, 'target_client_key_not_enumerated'):
            formal.verify_capability_collection(result['runtime'], formal.opaque('client'))

    def test_declaration_identity_and_revision_are_still_strict(self):
        result, _ = self.collect()
        observed = result['runtime']
        frozen = observed['capability_observations'][0]
        target = {'declaration_evidence': copy.deepcopy(frozen)}
        formal.verify_current_declaration(target, observed)
        for field in ['config_version_id', 'config_revision', 'client_key_evidence_id',
                      'route_id', 'route_candidate_id', 'endpoint_id', 'upstream_model', 'capabilities']:
            with self.subTest(field=field):
                changed = copy.deepcopy(observed)
                changed['capability_observations'][0][field] = 10 if field == 'config_revision' else 'foreign'
                with self.assertRaises((formal.Blocked, ValueError)):
                    formal.verify_current_declaration(target, changed)
        changed = copy.deepcopy(observed)
        changed['capability_observations'].append(copy.deepcopy(frozen))
        with self.assertRaises(formal.Blocked):
            formal.verify_current_declaration(target, changed)


if __name__ == '__main__':
    unittest.main(verbosity=2)
