"""LOCAL_SIMULATED: execute the frozen controller and complete stage programs.

Only filesystem/UID/checksum fixtures, clock, systemd, and HTTP transports are
substituted. The uploaded programs, collector, Pin freezer, policy, SQL reads,
capture decoder, and Request/Attempt/Usage/ledger verification run unchanged.
There is no SSH, systemd, Linux process, or Provider evidence in this suite.
"""
import base64
import contextlib
import copy
import email.message
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import sqlite3
import subprocess
import sys
import tarfile
import tempfile
import time
import types
import unittest
import urllib.parse
import urllib.request
from unittest.mock import patch

sys.dont_write_bytecode = True
os.umask(0o077)
TOOLS = Path(__file__).resolve().parent
REPO = TOOLS.parents[4]
EPOCH = 'chat-json-rehearsal-20261013'
SOURCE = 'bf822b14a62c21cf582a7539e3635f69bd8b7677'
BASE = '/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010'
ACCOUNT_EVIDENCE = '5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54'
CONTRACTS = json.loads((TOOLS.parent / 'contract-fixtures.json').read_text())
sha = lambda value: hashlib.sha256(value).hexdigest()
opaque = lambda value: sha(('cpar-evidence:' + str(value)).encode())


def assert_dto(value, schema, schemas):
    """Check only the frozen OpenAPI schemas used by this transport fixture."""
    if '$ref' in schema:
        return assert_dto(value, schemas[schema['$ref'].rsplit('/', 1)[1]], schemas)
    for keyword in ['anyOf', 'oneOf']:
        if keyword in schema:
            matches = 0
            for branch in schema[keyword]:
                try:
                    assert_dto(value, branch, schemas)
                    matches += 1
                except AssertionError:
                    pass
            assert matches == 1 if keyword == 'oneOf' else matches >= 1
            return
    kinds = schema.get('type', [])
    kinds = [kinds] if isinstance(kinds, str) else kinds
    actual = {dict: 'object', list: 'array', str: 'string', int: 'integer', bool: 'boolean', type(None): 'null'}[type(value)]
    assert not kinds or actual in kinds, (actual, kinds)
    if 'enum' in schema: assert value in schema['enum'], (value, schema['enum'])
    if 'const' in schema: assert value == schema['const']
    if isinstance(value, dict):
        assert set(schema.get('required', [])).issubset(value), ('required', schema.get('required'), value.keys())
        properties = schema.get('properties', {})
        if schema.get('additionalProperties') is False: assert set(value).issubset(properties)
        for key in value.keys() & properties.keys(): assert_dto(value[key], properties[key], schemas)
    if type(value) is int:
        assert value >= schema.get('minimum', value) and value <= schema.get('maximum', value)
    if isinstance(value, str):
        assert schema.get('minLength', 0) <= len(value) <= schema.get('maxLength', len(value))
        if 'pattern' in schema: assert re.fullmatch(schema['pattern'], value)


@contextlib.contextmanager
def process_file_lifetime():
    """In-process programs need the descriptor cleanup that process exit provides."""
    opened = []
    original = Path.open
    def tracked(path, *args, **kwargs):
        result = original(path, *args, **kwargs)
        if path.name.endswith('.lock'): opened.append(result)
        return result
    try:
        with patch.object(Path, 'open', tracked): yield
    finally:
        for descriptor in opened: descriptor.close()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_text(json.dumps(value, separators=(',', ':')))
    path.chmod(0o600)


class Response:
    def __init__(self, value, status=200, request_id=None):
        self.raw = json.dumps(value, separators=(',', ':')).encode()
        self.position = 0
        self.status = status
        self.headers = email.message.Message()
        self.headers['Content-Type'] = 'application/json'
        self.headers['ETag'] = '"rev-9"'
        if request_id:
            self.headers['x-request-id'] = request_id
        self.fp = types.SimpleNamespace(raw=types.SimpleNamespace(
            _sock=types.SimpleNamespace(settimeout=lambda seconds: None)))

    def read(self, count=-1):
        end = len(self.raw) if count < 0 else self.position + count
        result = self.raw[self.position:end]
        self.position += len(result)
        if self.position == len(self.raw):
            self.fp = None
        return result

    read1 = read

    def __enter__(self): return self
    def __exit__(self, *args): pass


class Fixture:
    def __init__(self, root, source_revision='missing'):
        self.root = Path(root)
        self.public = self.root / 'docs/reports/assets' / ('cpar-build-' + EPOCH)
        self.private = self.root / 'output' / ('cpar-build-' + EPOCH)
        self.base = self.root / 'linux/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010'
        self.production_root = self.base.parents[1]
        self.proc = self.root / 'linux/proc'
        self.epoch = self.base / 'private' / EPOCH
        self.unit = 'cpar-build-' + EPOCH + '.service'
        self.source_revision = source_revision
        self.candidate_revision = 145
        self.lease_revision = 145
        self.account_id = 'LOCAL_SIMULATED-account'
        self.started = False
        self.clock = 0.0
        self.action = None
        self.failure = None
        self.calls = []
        self.http_calls = []
        self.public_error = 'missing'
        self.http_status = 200
        self.attempt_outcome = 'succeeded'
        self.mutate_lineage = None
        self.production = {'MainPID': '101', 'process_start': '701', 'current_target': 'b0cf36e387b774439acc23830221389eb3ac74d0'}
        for p in [self.public / 'tools', self.private / 'payload', self.base / 'private',
                  self.base / 'audit', self.base / 'bin', self.base / 'credentials', self.proc / 'net']:
            p.mkdir(parents=True, exist_ok=True, mode=0o700)
        write_json(self.base / 'owner.json', {'task': 'cpar-build-20261010'})
        self.binary_bytes = ('LOCAL_SIMULATED\ngateway-release-revision=' + SOURCE + '\n').encode()
        self.guard_bytes = b'LOCAL_SIMULATED guard fixture'
        (self.base / 'bin/egress-guard-tcp.so').write_bytes(self.guard_bytes)
        for name in ['master-key', 'management-key', 'management-csrf', 'backup-key',
                     'client-key-pepper', 'grok-build-cache-key']:
            (self.base / 'credentials' / name).write_bytes(b'LOCAL_SIMULATED-key')
        (self.base / 'private/client-key').write_text('LOCAL_SIMULATED-client-key')
        self.source_credentials = self.production_root / 'credentials'
        self.source_credentials.mkdir(mode=0o700)
        for name in ['master-key', 'management-key']:
            (self.source_credentials / name).write_bytes(b'LOCAL_SIMULATED-key')
        self.account = dict(CONTRACTS['source_observation']['source_account'],
                            id=self.account_id, identity_digest='LOCAL_SIMULATED-identity',
                            credential_key_version=1, credential_ciphertext='LOCAL_SIMULATED')
        self.initialize_db(self.production_root / 'control.sqlite3', 31, self.account)
        old = dict(self.account, revision=133, continuation_current_revision=134)
        authorized = dict(self.account, revision=136, continuation_current_revision=137)
        self.initialize_db(self.base / 'state/control.sqlite3', 34, old, inherited=True)
        self.initialize_db(self.base / 'private/abd45431-six-20261011/state/control.sqlite3', 31, authorized)
        write_json(self.base / 'private/abd45431-six-20261011/final-receipt.json', {
            'budget': {'stopped': True, 'historical_inference_requests': 35,
                       'historical_gateway_pin_confirmed_sends': 34, 'new_pin_intents': 0, 'new_basic_intents': 0},
            'new_pin_actual_sends': 0, 'new_basic_attempts': 0})
        write_json(self.base / 'private/final-receipt.json', {'pin': {'credential_evidence_id': opaque(self.account_id)}})
        write_json(self.base / 'private/chat-json-502-20261011/prepare-receipt.json', {'historical_file_hashes': {
            'private/final-receipt.json': sha((self.base / 'private/final-receipt.json').read_bytes())}})
        write_json(self.base / 'private/chat-json-502-20261011/budget.json', {'stopped': True})
        write_json(self.base / 'bin/chat-json-502-20261011/freeze.json', {'files': {}})
        self.payload = json.dumps({'model': 'grok-4.5', 'messages': [
            {'role': 'user', 'content': 'Remember ACCEPT:' + 'a' * 24 + '. Reply READY.'}],
            'stream': False, 'max_completion_tokens': 4096}, separators=(',', ':')).encode()
        write_json(self.base / 'private/abd45431-six-20261011/chat-json/http-01-private.json',
                   {'request_wire_b64': base64.b64encode(self.payload).decode()})
        (self.base / 'private/bootstrap.py').write_text('def production():\n    return ' + repr(self.production) + '\n')
        self.make_process(101, ['fixture-production', 'serve', '--state-dir', str(self.production_root),
                              '--credential-dir', str(self.source_credentials), '--management-listen', '127.0.0.1:18181'])
        self.write_ports(False)
        assembled = json.loads((REPO / 'docs/reports/assets/cpar-build-chat-json-diagnostic-20261013/assembled-launch.json').read_text())['command']
        approved = []
        for value in assembled:
            value = value.replace('chat-json-diagnostic-20261013', EPOCH).replace(BASE, str(self.base))
            value = value.replace('--unit=' + self.unit, '--unit=cpar-build-acceptance-20261010.service')
            value = value.replace(str(self.epoch / 'state'), str(self.base / 'state'))
            value = value.replace(str(self.epoch / 'credentials'), str(self.base / 'credentials'))
            value = value.replace(str(self.epoch / 'gateway-v2.log'), str(self.base / 'private/gateway.log'))
            value = value.replace(str(self.base / 'audit' / (EPOCH + '.jsonl')), str(self.base / 'audit/candidate-egress.jsonl'))
            value = value.replace(str(self.base / 'bin' / EPOCH / 'gateway-aarch64-unknown-linux-gnu'),
                                  str(self.base / 'bin/gateway-aarch64-unknown-linux-gnu'))
            if value.startswith('--property=Environment='):
                value = value.split(' CPAR_BUILD_DIAGNOSTIC_DIR=')[0]
            approved.append(value)
        write_json(self.base / 'private/recovered-launch.json', {'command': approved})
        write_json(self.base / 'private/copy-runtime-options.json', [])
        legacy = ['private/recovered-launch.json', 'private/guard-preflight-corrected.json', 'private/bootstrap-receipt.json']
        for name in legacy:
            path = self.base / name
            if not path.exists(): write_json(path, {'layer': 'LOCAL_SIMULATED'})
            path.chmod(0o644)
        receipt = self.base / 'private/chat-json-502-20261011/prepare-receipt.json'
        previous = json.loads(receipt.read_text())
        previous['historical_file_hashes'].update({name: sha((self.base / name).read_bytes()) for name in legacy})
        write_json(receipt, previous)
        self.cost = b'LOCAL_SIMULATED historical cost authority'
        self.substitutions = {
            BASE: str(self.base), '/var/lib/cpa-rust-gateway': str(self.production_root),
            "'/proc'": repr(str(self.proc)), "'/proc/net'": repr(str(self.proc / 'net')),
            ACCOUNT_EVIDENCE: opaque(self.account_id),
            '91c943c76884a2b674a0b24d5d4876ff40cf61b53c379ffe2d04a062a0dddf06': sha(self.guard_bytes),
            'af73dd3177479147adca982bfcc95de7264442845f7b5a31e134643bc998bff0': sha((self.base / 'private/recovered-launch.json').read_bytes()),
            '37517e5f3dc66819f61f5a7bb8ace1921282415f10551d2defa5c3eb0985b570': sha((self.base / 'private/copy-runtime-options.json').read_bytes()),
            '77980e82623fa9913bd0623803f1a0f6b33857335127ad96fde5064f0ebaaa0f': sha(self.cost),
            '.st_uid == 0': '.st_uid == ' + str(os.getuid()),
            '.st_uid==0': '.st_uid==' + str(os.getuid())}
        self.freeze_payload(approved)

    def adapt(self, source):
        if str(self.root) in source:
            return source  # Uploaded fixture bytes have already crossed this boundary.
        pattern = '|'.join(re.escape(key) for key in sorted(self.substitutions, key=len, reverse=True))
        return re.sub(pattern, lambda match: self.substitutions[match.group()], source)

    def initialize_db(self, path, schema, account, inherited=False):
        path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        with sqlite3.connect(path) as db:
            columns = ','.join(k + (' INTEGER' if type(v) is int else ' TEXT') for k, v in account.items())
            db.execute('CREATE TABLE grok_accounts (' + columns + ')')
            db.execute('INSERT INTO grok_accounts VALUES (' + ','.join('?' for _ in account) + ')', tuple(account.values()))
            db.executescript('CREATE TABLE schema_migrations(version INTEGER);CREATE TABLE upstream_credentials(id TEXT);'
                'CREATE TABLE management_resource_audit_events(id TEXT);'
                'CREATE TABLE gateway_event_log(event_type TEXT,event_id TEXT,request_id TEXT,event_ordinal INTEGER,payload_json TEXT);'
                'CREATE TABLE gateway_attempt_execution(attempt_id TEXT,request_id TEXT,attempt_payload_sha256 TEXT,evidence_json TEXT);'
                'CREATE TABLE billing_ledger_entries(source_event_id TEXT,request_id TEXT,response_id TEXT,cost_microunits INTEGER,'
                'account_id TEXT,model TEXT,input_tokens INTEGER,output_tokens INTEGER,reasoning_tokens INTEGER,'
                'cache_read_tokens INTEGER,cache_creation_tokens INTEGER,cached_tokens INTEGER,usage_evidence_json TEXT,'
                'cost_confidence TEXT,billing_status TEXT);')
            db.execute('INSERT INTO schema_migrations VALUES (?)', (schema,))
            if inherited:
                for i, kind in enumerate(['request', 'attempt', 'request_finished', 'usage']):
                    data = {'request_id': 'inherited', 'outcome': 'succeeded', 'retry_decision': 'completed'}
                    db.execute('INSERT INTO gateway_event_log VALUES (?,?,?,?,?)',
                               (kind, 'old-' + kind, 'inherited', i, json.dumps({kind: data})))
                db.execute('INSERT INTO billing_ledger_entries(source_event_id,cost_confidence,billing_status) VALUES (?,?,?)',
                           ('old-usage', 'unknown', 'unpriced'))
        path.chmod(0o600)

    def freeze_payload(self, approved):
        payload = self.private / 'payload'
        for name in ['local-control.py', 'remote-upload.py', 'remote-no-launch-exit.py']:
            shutil.copyfile(TOOLS / name, self.public / 'tools' / name)
        for p in TOOLS.glob('*.py'):
            if p.name.startswith('remote-') and p.name not in ['remote-upload.py', 'remote-no-launch-exit.py'] or p.name in ['qualification.py', 'launch_recipe.py']:
                (payload / p.name).write_text(self.adapt(p.read_text()))
        for name in ['cpar_new_build_acceptance.py', 'cpar_build_json_diagnostic.py']:
            (payload / name).write_text(self.adapt((REPO / 'scripts/acceptance' / name).read_text()))
        (payload / 'cpar-batch-c-live.py').write_text('# LOCAL_SIMULATED unused historical collector\n')
        (payload / 'gateway-aarch64-unknown-linux-gnu').write_bytes(self.binary_bytes)
        (payload / 'historical-cost-authorization.json').write_bytes(self.cost)
        write_json(payload / 'user-authorization.json', {'max_new_inference_intents': 2, 'max_new_pin_intents': 1, 'max_new_chat_intents': 1})
        manifest = {'schema_version': 'cpa-rust-gateway-artifact-manifest-v1', 'revision': SOURCE,
                    'target': 'aarch64-unknown-linux-gnu', 'files': [{'name': 'gateway-aarch64-unknown-linux-gnu',
                    'bytes': len(self.binary_bytes), 'sha256': sha(self.binary_bytes)}]}
        write_json(payload / 'artifact-manifest.json', manifest)
        write_json(payload / 'gateway-build-metadata.json', {'revision': SOURCE})
        namespace = {}
        exec((payload / 'cpar_build_json_diagnostic.py').read_text(), namespace)
        recipe = {}
        exec((payload / 'launch_recipe.py').read_text(), recipe)
        launch = recipe['assemble'](approved, [], self.base, self.epoch, self.unit, SOURCE, sha(self.payload),
                                    types.SimpleNamespace(**namespace))
        frozen = {'kind': 'chat_json_502_diagnostic', 'epoch': EPOCH, 'source_sha': SOURCE,
                  'files': {p.name: sha(p.read_bytes()) for p in payload.iterdir()},
                  'assembled_launch_sha256': sha(json.dumps(launch, separators=(',', ':')).encode()),
                  'local_controller_sha256': sha((self.public / 'tools/local-control.py').read_bytes()),
                  'no_launch_exit_tool_sha256': sha((self.public / 'tools/remote-no-launch-exit.py').read_bytes()),
                  'upload_tool_sha256': sha((self.public / 'tools/remote-upload.py').read_bytes())}
        write_json(payload / 'freeze.json', frozen)
        with tarfile.open(self.private / 'upload.tar', 'w') as archive:
            for name in ['freeze.json', *sorted(frozen['files'])]:
                archive.add(payload / name, arcname=name)
        # Test authority, explicitly scoped to LOCAL_SIMULATED fixtures.
        write_json(self.public / 'review-pre-execution.json', {
            'test_authority': 'LOCAL_SIMULATED; not independent review',
            'axes': {'Standards': {'status': 'PASS'}, 'Spec': {'status': 'PASS'}},
            'payload_freeze_sha256': sha((payload / 'freeze.json').read_bytes())})

    def make_process(self, pid, argv):
        p = self.proc / str(pid)
        (p / 'fd').mkdir(parents=True, exist_ok=True)
        (p / 'cmdline').write_bytes(('\0'.join(argv) + '\0').encode())
        (p / 'stat').write_text(str(pid) + ' (fixture) ' + ' '.join(['S'] + ['0'] * 18 + ['701']))
        if pid == 202:
            (p / 'exe').symlink_to(argv[0])
        else:
            (p / 'exe').write_bytes(self.binary_bytes)
        (p / 'status').write_text('CapEff:\t0000000000000004\n')
        (p / 'maps').write_text(str(self.base / 'bin/egress-guard-tcp.so'))

    def write_ports(self, active):
        (self.proc / 'net/tcp6').write_text('header\n')
        rows = ''.join('0: 0100007F:%04X 00000000:0000 0A 0:0 00:0 0 0 0 %s\n' % (p, i)
                       for p, i in [(19180, 201), (19181, 202)]) if active else ''
        (self.proc / 'net/tcp').write_text('header\n' + rows)

    def pool(self, source=False):
        value = {'provider_id': 'cpar-build-provider', 'channel_id': 'p12-12-production-grok-build-endpoint' if source else 'cpar-build-endpoint',
                 'account_id': self.account_id, 'account_kind': 'native', 'active_leases': 0,
                 'enabled': True, 'auth_status': 'active', 'runtime_status': 'available', 'max_concurrency': 1,
                 'priority': 0, 'weight': 1, 'refresh_due_at_ms': None, 'quota_sync_due_at_ms': None,
                 'entitlement': None, 'presentation': None,
                 'expires_at_ms': int(time.time() * 1000) + 7200000}
        # One frozen expiry across source, candidate, Pin and Chat reads.
        value['expires_at_ms'] = self.expiry
        revision = self.source_revision if source else self.candidate_revision
        if revision != 'missing': value['credential_revision'] = revision
        if not source: value['config_version_id'] = 'cpar-build-config-20261010'
        schema = CONTRACTS['source_api' if source else 'candidate_api']['schemas']['ProviderAccountPoolItem']
        # The explicitly authorized source-summary extension is separate from
        # the old DTO. All actual old fields and every candidate field validate.
        checked = {k: v for k, v in value.items() if not (source and k == 'credential_revision')}
        assert_dto(checked, schema, CONTRACTS['source_api' if source else 'candidate_api']['schemas'])
        return value

    def capabilities(self):
        return {'reasoning': True, 'parallel': False, 'stored': True, 'continuation': True, 'compact': True, 'websocket': True}

    def management(self, path, source):
        if self.failure == 'prepare-timeout' and self.action == 'prepare':
            raise TimeoutError('LOCAL_SIMULATED')
        if self.failure == 'start-auth' and self.action == 'start' and not source:
            import urllib.error
            raise urllib.error.HTTPError('http://fixture', 401, 'LOCAL_SIMULATED', {}, None)
        if path == '/admin/config-versions':
            return [{'id': 'fixture-production' if source else 'cpar-build-config-20261010', 'revision': 'rev-1' if source else 'rev-9', 'status': 'active'}]
        if path.startswith('/admin/operations/provider-account-pools'):
            return {'items': [self.pool(source)]}
        values = {
            '/admin/routes': [{'id': 'cpar-build-route', 'max_attempts': 1}],
            '/admin/route-candidates': [{'id': 'cpar-build-candidate', 'route_id': 'cpar-build-route',
                'endpoint_id': 'cpar-build-endpoint', 'upstream_model': 'grok-4.5', 'enabled': True, 'credential_scope': 'all_active'}],
            '/admin/client-keys': [{'id': 'cpar-build-client'}],
            '/admin/models/effective': {'items': [{'id': 'grok-4.5', 'route_id': 'cpar-build-route', 'sources': [{
                'candidate_id': 'cpar-build-candidate', 'endpoint_id': 'cpar-build-endpoint',
                'capability_evidence': {'source': 'serving_snapshot', 'config_version_id': 'cpar-build-config-20261010',
                    'config_revision': 9, 'capabilities': self.capabilities()}}]}],
                'projection_id': 'fixture-projection', 'observed_at_ms': int(time.time() * 1000)},
        }
        for prefix, value in values.items():
            if path.split('?')[0] == prefix: return value
        return []

    def assert_reserved(self, kind):
        stage = json.loads((self.private / 'stage.json').read_text())
        assert stage['phase'] == 'EXECUTING_' + kind.upper() and kind in stage['formal_actions']
        budget = json.loads((self.epoch / 'budget.json').read_text())
        assert budget['stopped'] is True and budget['new_pin_intents'] == 1
        assert budget['new_basic_intents'] == int(kind == 'chat')
        assert budget['pin_status' if kind == 'pin' else 'chat_status'] == 'UNKNOWN'

    def open(self, request, timeout=0):
        url = urllib.parse.urlsplit(request.full_url)
        self.http_calls.append((self.action, request.get_method(), url.port, url.path))
        if request.get_method() == 'GET':
            return Response(self.management(url.path + ('?' + url.query if url.query else ''), url.port == 18181))
        if url.path.endswith('/channel-pin'):
            self.assert_reserved('pin')
            body = json.loads(request.data)
            schemas = CONTRACTS['candidate_api']['schemas']
            assert_dto(body, schemas['ChannelPinInput'], schemas)
            assert body['credential_revision'] == 144 and body['requested_model'] == 'grok-4.5'
            if self.failure == 'pin-timeout': raise TimeoutError('LOCAL_SIMULATED')
            (self.base / 'audit' / (EPOCH + '.jsonl')).write_text('{"event":"connect_build_allowed"}\n')
            pin = dict(body, outcome='succeeded', attempt_count=1, upstream_sent=True, response_started=True,
                       request_id='fixture-pin-request', config_version_id='cpar-build-config-20261010', config_revision=9,
                       runtime_credential_revision=145, server_instance='fixture-server',
                       connection_revision=9, permission_basis='management_key_projection',
                       runtime_build=SOURCE + ':LOCAL_SIMULATED', observed_at_ms=int(time.time() * 1000),
                       execution={'attempt_id': 'fixture-pin-attempt', 'route_candidate_id': 'cpar-build-candidate', 'upstream_model': 'grok-4.5',
                           'identity': {'channel': 'grok.build', 'credential_revision': self.lease_revision,
                               'config_version_id': 'cpar-build-config-20261010', 'config_revision': 9,
                               'egress': {'kind': 'direct'}, 'capabilities': self.capabilities()}})
            assert_dto(pin, schemas['ChannelPin'], schemas)
            return Response(pin)
        assert url.path == '/v1/chat/completions' and url.port == 19180
        self.assert_reserved('chat')
        assert request.data == self.payload
        if self.failure == 'chat-timeout': raise TimeoutError('LOCAL_SIMULATED')
        self.materialize_chat()
        value = {'id': 'fixture-response', 'model': 'grok-4.5', 'choices': [
            {'index': 0, 'message': {'role': 'assistant', 'content': 'READY'}, 'finish_reason': 'stop'}]}
        if self.public_error == 'null': value['error'] = None
        if self.http_status == 502:
            value = {'error': {'code': 'UpstreamProtocolError', 'message': 'response projection cannot represent the reasoning metadata'}}
        return Response(value, self.http_status, 'fixture-request')

    def materialize_chat(self):
        native = {'id': 'fixture-native-response', 'output': [
            {'id': 'fixture-reasoning', 'type': 'reasoning', 'encrypted_content': 'LOCAL_SIMULATED', 'summary': []},
            {'id': 'fixture-message', 'type': 'message', 'role': 'assistant', 'phase': 'final_answer',
             'content': [{'type': 'output_text', 'text': 'READY', 'annotations': []}]}],
            'usage': {'input_tokens': 10, 'output_tokens': 5, 'output_tokens_details': {'reasoning_tokens': 3},
                      'input_tokens_details': {'cached_tokens': 4}}}
        raw = json.dumps(native, separators=(',', ':')).encode()
        directory = self.epoch / 'native-capture'
        write_json(directory / 'selection.json', {'request_id': 'fixture-request', 'public_request_sha256': sha(self.payload)})
        write_json(directory / 'body-receipt.json', {'complete': True, 'bytes': len(raw), 'sha256': sha(raw)})
        write_json(directory / 'transport.json', {'content_encoding_class': 'Identity'})
        (directory / 'native-response.bin').write_bytes(raw)
        request = {'request_id': 'fixture-request', 'client_key_id': 'cpar-build-client',
                   'protocol': 'openai_chat_completions', 'streaming': False, 'requested_model': 'grok-4.5', 'public_model': 'grok-4.5'}
        attempt = {'request_id': 'fixture-request', 'attempt_id': 'fixture-attempt', 'attempt_number': 1,
                   'route_id': 'cpar-build-route', 'route_candidate_id': 'cpar-build-candidate',
                   'endpoint_id': 'cpar-build-endpoint', 'upstream_model': 'grok-4.5', 'credential_id': self.account_id,
                   'outcome': self.attempt_outcome, 'retry_decision': 'completed' if self.attempt_outcome == 'succeeded' else 'exhausted'}
        usage = {'provenance': 'measured', 'input_accounting': 'inclusive', 'input_tokens': 10, 'output_tokens': 5,
                 'reasoning_tokens': 3, 'cached_tokens': 4, 'cache_read_tokens': None, 'cache_creation_tokens': None}
        data = [('request', 'fixture-request-event', request), ('attempt', 'fixture-attempt', attempt),
                ('request_finished', 'fixture-finished', {'request_id': 'fixture-request',
                    'outcome': 'succeeded' if self.http_status == 200 and self.attempt_outcome == 'succeeded' else 'failed'})]
        if self.attempt_outcome == 'succeeded':
            data.append(('usage', 'fixture-usage', {'request_id': 'fixture-request', 'attempt_id': 'fixture-attempt',
                        'response_id': native['id'], 'usage': usage}))
        identity = {'channel': 'grok.build', 'credential_revision': self.lease_revision,
                    'config_version_id': 'cpar-build-config-20261010', 'config_revision': 9,
                    'egress': {'kind': 'direct'}, 'capabilities': self.capabilities()}
        payload = json.dumps({'attempt': attempt}, separators=(',', ':'))
        with sqlite3.connect(self.epoch / 'state/control.sqlite3') as db:
            for ordinal, (kind, event_id, value) in enumerate(data, 10):
                db.execute('INSERT INTO gateway_event_log VALUES (?,?,?,?,?)',
                           (kind, event_id, 'fixture-request', ordinal, json.dumps({kind: value}, separators=(',', ':'))))
            if self.mutate_lineage != 'missing-sidecar':
                db.execute('INSERT INTO gateway_attempt_execution VALUES (?,?,?,?)', (
                    'fixture-attempt', 'wrong-request' if self.mutate_lineage == 'sidecar-request' else 'fixture-request',
                    '0' * 64 if self.mutate_lineage == 'attempt-bytes' else sha(payload.encode()),
                    json.dumps(identity, separators=(',', ':'))))
            if self.attempt_outcome == 'succeeded':
                db.execute('INSERT INTO billing_ledger_entries VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)', (
                    'wrong-usage' if self.mutate_lineage == 'ledger-source' else 'fixture-usage',
                    'fixture-request', native['id'], None, self.account_id, 'grok-4.5', 10, 5, 3, None, None, 4,
                    json.dumps({'provenance': 'measured', 'input_accounting': 'inclusive'}), 'unknown', 'unpriced'))
        if self.mutate_lineage == 'lease-after-pin':
            with sqlite3.connect(self.epoch / 'state/control.sqlite3') as db:
                identity['credential_revision'] += 1
                db.execute('UPDATE gateway_attempt_execution SET evidence_json=?', (json.dumps(identity, separators=(',', ':')),))

    def check_output(self, command, text=False):
        assert command[:2] == ['systemctl', 'show']
        pid = 202 if command[2] == self.unit and self.started else 0
        if '--property=MainPID' in command: value = str(pid) + '\n'
        elif '--property=LoadState' in command: value = 'loaded\n' if pid else 'not-found\n'
        else: value = 'MainPID=0\nLoadState=not-found\nActiveState=inactive\nSubState=dead\n'
        return value if text else value.encode()

    def execute(self, script, arguments=(), stream=b''):
        output = io.StringIO()
        old_modules = {name: sys.modules.get(name) for name in ['qualification', 'launch_recipe', 'cpar_build_json_diagnostic', 'cpar_new_build_acceptance']}
        old_path = sys.path[:]
        for name in old_modules: sys.modules.pop(name, None)
        sys.path.insert(0, str(self.epoch if self.epoch.exists() else self.private / 'payload'))
        stdin = types.SimpleNamespace(buffer=io.BytesIO(stream))
        try:
            with process_file_lifetime(), patch.object(sys, 'argv', ['fixture', *arguments]), patch.object(sys, 'stdin', stdin), contextlib.redirect_stdout(output):
                exec(compile(self.adapt(script), '<LOCAL_SIMULATED-stage>', 'exec'), {'__name__': '__fixture__'})
            return 0, output.getvalue(), ''
        except SystemExit as error:
            return error.code or 0, output.getvalue(), ''
        except BaseException as error:
            return 1, output.getvalue(), type(error).__name__ + ':' + str(error)
        finally:
            sys.path[:] = old_path
            for name, previous in old_modules.items():
                if previous is None: sys.modules.pop(name, None)
                else: sys.modules[name] = previous

    def run(self, command, input=None, text=False, check=False, **kwargs):
        if command[0] == 'systemd-run':
            assert self.action == 'start'
            if self.failure == 'start-command-timeout':
                raise TimeoutError('LOCAL_SIMULATED')
            self.started = True
            binary = str(self.base / 'bin' / EPOCH / 'gateway-aarch64-unknown-linux-gnu')
            self.make_process(202, command[command.index(binary):])
            p = self.proc / '202'
            import shlex
            env = next(v.split('Environment=', 1)[1] for v in command if v.startswith('--property=Environment='))
            (p / 'environ').write_bytes(('\0'.join(shlex.split(env)) + '\0').encode())
            (p / 'fd/3').symlink_to('socket:[201]'); (p / 'fd/4').symlink_to('socket:[202]')
            self.write_ports(True)
            if self.failure == 'start-audit':
                (self.base / 'audit' / (EPOCH + '.jsonl')).write_text('{"event":"connect_build_allowed"}\n')
            return subprocess.CompletedProcess(command, 0, b'', b'')
        if command[:2] == ['systemctl', 'stop']:
            assert command[2] == self.unit and self.started
            self.started = False
            shutil.rmtree(self.proc / '202')
            self.write_ports(False)
            return subprocess.CompletedProcess(command, 0)
        if command[0] == 'ssh':
            stage = json.loads((self.private / 'stage.json').read_text())
            if self.action == 'observe': assert stage['observations'][-1]['classification'] == 'UNKNOWN'
            elif 'stop latched' not in (input or b'').decode(errors='replace'):
                assert stage['phase'] == 'EXECUTING_' + self.action.upper() and self.action in stage['formal_actions']
            self.calls.append(self.action)
            if input == (self.private / 'upload.tar').read_bytes():
                script = (self.public / 'tools/remote-upload.py').read_text()
                expected = sha((self.private / 'payload/freeze.json').read_bytes())
                if self.failure == 'upload-before-directory': expected = '0' * 64
                if self.failure == 'upload-partial':
                    partial = io.BytesIO()
                    with tarfile.open(fileobj=partial, mode='w') as archive:
                        for name in ['freeze.json', 'artifact-manifest.json', 'gateway-build-metadata.json']:
                            archive.add(self.private / 'payload' / name, arcname=name)
                    input = partial.getvalue()
                code, out, err = self.execute(script, [expected], input)
                if self.failure == 'upload-complete-unknown' and not code:
                    code, err = 1, 'LOCAL_SIMULATED lost transport result after upload'
            else:
                code, out, err = self.execute(input.decode())
        elif len(command) >= 3 and command[1] == '-c':
            # Execute the actual inventory collector, using raw management DTOs.
            code, out, err = self.execute(command[2])
        else: raise AssertionError('unsubstituted external operation')
        if check and code: raise subprocess.CalledProcessError(code, command)
        return subprocess.CompletedProcess(command, code, out if text else out.encode(), err if text else err.encode())

    def sleep(self, seconds): self.clock += seconds

    def invoke(self, action):
        self.action = action
        if not hasattr(self, 'expiry'): self.expiry = int(time.time() * 1000) + 7200000
        original_read_bytes = Path.read_bytes
        def read_process_bytes(path):
            if self.failure == 'start-identity-not-ready' and action == 'start' and path == self.proc / '202/cmdline':
                return b'LOCAL_SIMULATED-exec-pending\0'
            return original_read_bytes(path)
        with process_file_lifetime(), patch.object(subprocess, 'run', self.run), patch.object(subprocess, 'check_output', self.check_output), \
             patch.object(Path, 'read_bytes', read_process_bytes), \
             patch.object(urllib.request, 'build_opener', lambda *args: types.SimpleNamespace(open=self.open)), \
             patch.object(time, 'monotonic', lambda: self.clock), patch.object(time, 'sleep', self.sleep), \
             patch.object(sys, 'argv', ['local-control.py', action]), contextlib.redirect_stdout(io.StringIO()):
            try:
                runpy.run_path(str(self.public / 'tools/local-control.py'), run_name='__main__')
                code = 0
            except SystemExit as error: code = error.code
        return code, json.loads((self.private / 'stage.json').read_text())

    def through(self, end):
        for action in ['observe', 'upload', 'prepare', 'start', 'pin', 'chat', 'exit']:
            code, state = self.invoke(action)
            if code or action == end: return code, state
        raise AssertionError('unknown final action')


class FullFlow(unittest.TestCase):
    def fixture(self, **kwargs):
        directory = tempfile.TemporaryDirectory(prefix='cpar-flow-', dir=TOOLS.parent)
        self.addCleanup(directory.cleanup)
        return Fixture(directory.name, **kwargs)

    def test_complete_missing_and_null_source_candidate_and_lease_present(self):
        for source in ['missing', None]:
            with self.subTest(source=source):
                f = self.fixture(source_revision=source)
                code, state = f.through('exit')
                self.assertEqual(code, 0, [(p.name, p.read_text()) for p in f.public.glob('*.json') if p.name != 'review-pre-execution.json'])
                self.assertEqual(state['formal_actions'], ['upload', 'prepare', 'start', 'pin', 'chat', 'exit'])
                pin = json.loads((f.epoch / 'chat-json/frozen-plan.json').read_text())
                chat = json.loads((f.epoch / 'chat-json/chat-summary.json').read_text())
                final = json.loads((f.epoch / 'final-receipt.json').read_text())
                self.assertEqual(chat['status'], 'NOT_REPRODUCED')
                self.assertEqual(pin['target']['credential_revision'], 145)
                self.assertEqual(chat['validated_execution']['actual_lease_credential_revision'], 145)
                self.assertEqual((chat['usage_rows'], chat['ledger_rows']), (1, 1))
                self.assertEqual(chat['validated_native_usage']['reasoning_tokens'], 3)
                self.assertEqual(final['new_gateway_pin_actual_sends'], 2)
                self.assertTrue(state['stopped'])

    def test_source_revision_mismatch_rejects_in_observation_and_prepare(self):
        f = self.fixture(source_revision=146)
        self.assertNotEqual(f.through('observe')[0], 0)
        self.assertFalse((f.base / 'bin' / EPOCH).exists())
        f = self.fixture()
        self.assertEqual(f.through('upload')[0], 0)
        f.source_revision = 146
        self.assertNotEqual(f.invoke('prepare')[0], 0)
        self.assertFalse(f.started)

    def test_rejected_observation_and_before_upload_have_no_creation_exit_proof(self):
        for revision in [146, 'missing']:
            f = self.fixture(source_revision=revision)
            code, state = f.through('observe')
            self.assertEqual(code == 0, revision == 'missing')
            self.assertEqual(f.invoke('exit')[0], 0)
            receipt = json.loads((f.public / 'exit.json').read_text())['result']
            self.assertEqual(receipt['remote_epoch_status'], 'NOT_CREATED')
            self.assertEqual(receipt['owned_process_stop_status'], 'NOT_STARTED')
            self.assertFalse((f.base / 'bin' / EPOCH).exists())
            self.assertFalse([r for r in f.http_calls if r[1] == 'POST'])
            self.assertNotEqual(f.invoke('exit')[0], 0)

    def test_pin_wire_response_matches_frozen_candidate_dto(self):
        f = self.fixture()
        self.assertEqual(f.through('pin')[0], 0)
        schemas = CONTRACTS['candidate_api']['schemas']
        raw = (f.epoch / 'chat-json/pin-private.json').read_text()
        pin = json.loads(raw)
        assert_dto(pin, schemas['ChannelPin'], schemas)

    def test_no_observation_exit_uses_fresh_baseline_and_tool_hash_is_binding(self):
        f = self.fixture()
        self.assertEqual(f.invoke('exit')[0], 0)
        receipt = json.loads((f.public / 'exit.json').read_text())['result']
        self.assertEqual(receipt['production_baseline_comparison'], 'NOT_OBSERVED; fresh read-only baseline stable')
        self.assertFalse([r for r in f.http_calls if r[1] == 'POST'])
        f = self.fixture()
        (f.public / 'tools/remote-no-launch-exit.py').write_text('# changed frozen tool\n')
        self.assertNotEqual(f.invoke('observe')[0], 0)
        self.assertEqual(f.calls, [])

    def test_upload_absent_partial_and_complete_unknown_have_independent_exit(self):
        for failure in ['upload-before-directory', 'upload-partial', 'upload-complete-unknown']:
            f = self.fixture(); f.failure = failure
            self.assertNotEqual(f.through('upload')[0], 0)
            before = len(f.http_calls)
            self.assertNotEqual(f.invoke('upload')[0], 0)
            self.assertNotEqual(f.invoke('prepare')[0], 0)
            self.assertEqual(len(f.http_calls), before)
            self.assertEqual(f.invoke('exit')[0], 0)
            receipt = json.loads((f.public / 'exit.json').read_text())['result']
            self.assertEqual(receipt['resource_exit_status'], 'PASS')
            self.assertEqual(receipt['remote_epoch_status'], 'NOT_CREATED')
            self.assertEqual(receipt['owned_process_stop_status'], 'NOT_STARTED')
            self.assertEqual(receipt['payload_integrity_status'], {
                'upload-before-directory': 'NOT_UPLOADED', 'upload-partial': 'PARTIAL',
                'upload-complete-unknown': 'PASS'}[failure])
            self.assertTrue(json.loads((f.private / 'stage.json').read_text())['stopped'])
            self.assertFalse([r for r in f.http_calls if r[1] == 'POST'])

    def test_candidate_revision_missing_null_or_changed_rejects_before_pin(self):
        for revision in ['missing', None, 146]:
            f = self.fixture(); f.candidate_revision = revision
            code, state = f.through('start')
            self.assertNotEqual(code, 0)
            self.assertTrue(state['stopped']); self.assertFalse(f.started)
            self.assertEqual([r for r in f.http_calls if r[1] == 'POST'], [])

    def test_actual_pin_lease_cannot_use_source_compatibility(self):
        for revision in [None, 144, 146]:
            f = self.fixture(); f.lease_revision = revision
            code, state = f.through('pin')
            self.assertNotEqual(code, 0)
            self.assertTrue(state['stopped'])
            self.assertNotEqual(f.invoke('chat')[0], 0)
            self.assertEqual(len([r for r in f.http_calls if r[1] == 'POST']), 1)

    def test_start_auth_failure_does_not_retry_and_stops_owned_process(self):
        f = self.fixture(); f.failure = 'start-auth'
        self.assertNotEqual(f.through('start')[0], 0)
        self.assertFalse(f.started)
        self.assertEqual(len([r for r in f.http_calls if r[0] == 'start' and r[2] == 19181]), 1)

    def test_pin_and_chat_unknown_latch_and_prohibit_reentry(self):
        for failure, end in [('pin-timeout', 'pin'), ('chat-timeout', 'chat')]:
            f = self.fixture(); f.failure = failure
            code, state = f.through(end)
            self.assertTrue(state['stopped'])
            before = len(f.http_calls)
            self.assertNotEqual(f.invoke(end)[0], 0)
            self.assertNotEqual(f.invoke('observe')[0], 0)
            self.assertEqual(len(f.http_calls), before)
            self.assertEqual(f.invoke('exit')[0], 0)
            final = json.loads((f.epoch / 'final-receipt.json').read_text())
            self.assertEqual(final['resource_exit_status'], 'PASS')
            self.assertEqual(final['count_reconciliation_status'], 'UNKNOWN')
            self.assertIsNone(final['new_gateway_pin_actual_sends'])

    def test_null_error_does_not_interrupt_postsend_evidence(self):
        f = self.fixture(); f.public_error = 'null'
        self.assertEqual(f.through('chat')[0], 0)
        self.assertEqual(json.loads((f.epoch / 'chat-json/chat-summary.json').read_text())['status'], 'NOT_REPRODUCED')

    def test_exact_attempt_usage_and_ledger_binding_rejects_mutations(self):
        for mutation in ['missing-sidecar', 'sidecar-request', 'attempt-bytes', 'ledger-source', 'lease-after-pin']:
            f = self.fixture(); f.mutate_lineage = mutation
            self.assertEqual(f.through('chat')[0], 0)
            value = json.loads((f.epoch / 'chat-json/chat-summary.json').read_text())
            self.assertEqual(value['status'], 'UNKNOWN')
            self.assertTrue(json.loads((f.epoch / 'budget.json').read_text())['stopped'])
            self.assertNotEqual(f.invoke('pin')[0], 0)

    def test_strict_rejection_keeps_native_usage_and_unknown_cost(self):
        f = self.fixture(); f.http_status = 502
        self.assertEqual(f.through('exit')[0], 0)
        value = json.loads((f.epoch / 'chat-json/chat-summary.json').read_text())
        self.assertEqual(value['status'], 'REPRODUCED')
        self.assertEqual(value['rejection'], 'reasoning_unsupported')
        self.assertEqual(value['ledger_cost_observations'], [None])
        self.assertEqual(value['validated_native_usage']['cached_tokens'], 4)

    def test_prepare_failure_unknown_and_exact_hour_margin_stop_before_start(self):
        for failure in ['prepare-timeout', 'hour-margin']:
            f = self.fixture()
            self.assertEqual(f.through('upload')[0], 0)
            if failure == 'hour-margin': f.expiry = int(time.time() * 1000) + 3600000
            else: f.failure = failure
            code, state = f.invoke('prepare')
            self.assertNotEqual(code, 0); self.assertTrue(state['stopped'])
            before = len(f.http_calls)
            self.assertNotEqual(f.invoke('start')[0], 0)
            self.assertEqual(len(f.http_calls), before)
            self.assertFalse(f.started)

    def test_start_unknown_and_audit_activity_stop_without_inference(self):
        for failure in ['start-command-timeout', 'start-audit']:
            f = self.fixture(); f.failure = failure
            code, state = f.through('start')
            self.assertNotEqual(code, 0); self.assertTrue(state['stopped']); self.assertFalse(f.started)
            self.assertNotEqual(f.invoke('pin')[0], 0)
            self.assertFalse([r for r in f.http_calls if r[1] == 'POST'])

    def test_stopped_success_and_changed_frozen_bytes_never_send_again(self):
        f = self.fixture(); self.assertEqual(f.through('chat')[0], 0)
        before = len(f.http_calls)
        for action in ['upload', 'prepare', 'start', 'pin', 'chat', 'observe']:
            self.assertNotEqual(f.invoke(action)[0], 0)
        self.assertEqual(len(f.http_calls), before)
        f = self.fixture(); self.assertEqual(f.through('start')[0], 0)
        (f.private / 'payload/remote-pin.py').write_text('# changed frozen bytes\n')
        self.assertNotEqual(f.invoke('pin')[0], 0)
        self.assertFalse([r for r in f.http_calls if r[1] == 'POST'])

    def test_failed_attempt_exit_keeps_send_unknown_after_successful_cleanup(self):
        f = self.fixture(); f.attempt_outcome = 'failed'; f.http_status = 502
        self.assertEqual(f.through('exit')[0], 0)
        final = json.loads((f.epoch / 'final-receipt.json').read_text())
        self.assertEqual(final['resource_exit_status'], 'PASS')
        self.assertEqual(final['count_reconciliation_status'], 'UNKNOWN')
        self.assertEqual(final['new_gateway_pin_confirmed_sends_lower_bound'], 1)
        self.assertIsNone(final['new_gateway_pin_actual_sends'])

    def test_prepare_and_start_failure_without_start_receipt_have_verified_exit(self):
        for failure, end in [('prepare-timeout', 'prepare'), ('start-auth', 'start')]:
            f = self.fixture(); f.failure = failure
            self.assertNotEqual(f.through(end)[0], 0)
            self.assertFalse(f.started)
            self.assertEqual(f.invoke('exit')[0], 0)
            receipt = json.loads((f.public / 'exit.json').read_text())['result']
            self.assertEqual(receipt['resource_exit_status'], 'PASS')
            self.assertEqual(receipt['owned_process_stop_status'], 'NOT_STARTED' if end == 'prepare' else 'STOPPED_VERIFIED')

    def test_start_identity_failure_later_stable_owned_process_has_resource_exit(self):
        f = self.fixture(); f.failure = 'start-identity-not-ready'
        self.assertNotEqual(f.through('start')[0], 0)
        self.assertTrue(f.started)
        self.assertFalse((f.epoch / 'start-receipt.json').exists())
        self.assertEqual(json.loads((f.epoch / 'start-failure-v2.json').read_text())['owned_process_stop_status'],
                         'IDENTITY_OR_EXIT_UNKNOWN')
        self.assertNotEqual(f.invoke('pin')[0], 0)
        original_run = f.run
        def check_stop_persistence(command, **kwargs):
            if command[:2] == ['systemctl', 'stop']:
                self.assertTrue((f.epoch / 'exit-process-binding.json').exists())
                self.assertTrue(json.loads((f.epoch / 'budget.json').read_text())['stopped'])
            return original_run(command, **kwargs)
        f.run = check_stop_persistence
        self.assertEqual(f.invoke('exit')[0], 0)
        self.assertFalse(f.started)
        receipt = json.loads((f.public / 'exit.json').read_text())['result']
        self.assertEqual(receipt['resource_exit_status'], 'PASS')
        self.assertEqual(receipt['owned_process_stop_status'], 'STOPPED_VERIFIED')
        self.assertEqual(receipt['startup_validation_status'], 'NOT_PASSED')
        intent = json.loads((f.epoch / 'exit-process-binding.json').read_text())
        self.assertEqual(intent['pid'], 202)
        self.assertEqual(intent['binary_sha256'], sha(f.binary_bytes))
        self.assertFalse([r for r in f.http_calls if r[1] == 'POST'])
        self.assertTrue(json.loads((f.epoch / 'budget.json').read_text())['stopped'])

    def test_failed_start_exit_never_stops_mismatched_process_identity(self):
        for mismatch in ['exe-path', 'argv-state', 'guard-map', 'duplicate-environment', 'process-start']:
            with self.subTest(mismatch=mismatch):
                f = self.fixture(); f.failure = 'start-identity-not-ready'
                self.assertNotEqual(f.through('start')[0], 0)
                proc = f.proc / '202'
                if mismatch == 'exe-path':
                    other = f.root / 'same-bytes-other-binary'; other.write_bytes(f.binary_bytes)
                    (proc / 'exe').unlink(); (proc / 'exe').symlink_to(other)
                elif mismatch == 'argv-state':
                    (proc / 'cmdline').write_bytes((proc / 'cmdline').read_bytes().replace(
                        str(f.epoch / 'state').encode(), str(f.root / 'unowned-state').encode()))
                elif mismatch == 'guard-map':
                    (proc / 'maps').write_text('LOCAL_SIMULATED-unowned-guard')
                elif mismatch == 'duplicate-environment':
                    with (proc / 'environ').open('ab') as output:
                        output.write(('CPAR_BUILD_DIAGNOSTIC_DIR=' + str(f.epoch / 'native-capture') + '\0').encode())
                original_text = Path.read_text
                reads = []
                def read_process_start(path, *args, **kwargs):
                    value = original_text(path, *args, **kwargs)
                    if mismatch == 'process-start' and path == proc / 'stat':
                        reads.append(value)
                        if len(reads) > 1: return value.rsplit('701', 1)[0] + '702'
                    return value
                with patch.object(Path, 'read_text', read_process_start):
                    self.assertNotEqual(f.invoke('exit')[0], 0)
                self.assertTrue(f.started)
                self.assertTrue(json.loads((f.epoch / 'budget.json').read_text())['stopped'])
                self.assertFalse([r for r in f.http_calls if r[1] == 'POST'])


if __name__ == '__main__':
    unittest.main(verbosity=2)
