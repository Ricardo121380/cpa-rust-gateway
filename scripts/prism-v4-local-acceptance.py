#!/usr/bin/env python3
"""Prism acceptance against the real local serve binary; synthetic state only."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import ssl
import threading
import argparse
import json
import os
from pathlib import Path
import secrets
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--priced', action='store_true')
    parser.add_argument('--large', action='store_true')
    parser.add_argument('--browser', action='store_true')
    parser.add_argument('--catalog-expiry', action='store_true')
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='prism-v4-acceptance-'))
    os.chmod(root, 0o700)
    state, credentials = root / 'state', root / 'credentials'
    state.mkdir()
    credentials.mkdir()
    key, csrf = 'mgmt_' + secrets.token_hex(24), 'csrf_' + secrets.token_hex(24)
    for name, value in [('management-key', key.encode()), ('management-csrf', csrf.encode())] + [
        (name, secrets.token_bytes(32)) for name in
        ['master-key', 'backup-key', 'client-key-pepper', 'grok-build-cache-key']
    ]:
        (credentials / name).write_bytes(value)
        os.chmod(credentials / name, 0o600)
    cert_config = root / 'cert.cnf'
    cert_config.write_text("[req]\ndistinguished_name=dn\nx509_extensions=ext\nprompt=no\n[dn]\nCN=localhost\n[ext]\nsubjectAltName=IP:127.0.0.1,DNS:localhost\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,digitalSignature,keyEncipherment\n")
    cert, private_key = root / 'local-ca.pem', root / 'local-key.pem'
    subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes',
                    '-keyout', str(private_key), '-out', str(cert), '-days', '1',
                    '-config', str(cert_config)], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    leaf_key, leaf_csr, leaf_cert = root / 'server-key.pem', root / 'server.csr', root / 'server.pem'
    leaf_ext = root / 'server.ext'
    leaf_ext.write_text('subjectAltName=IP:127.0.0.1,DNS:localhost\nbasicConstraints=critical,CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n')
    for command in [
        ['openssl', 'req', '-new', '-newkey', 'rsa:2048', '-nodes', '-subj', '/CN=localhost', '-keyout', str(leaf_key), '-out', str(leaf_csr)],
        ['openssl', 'x509', '-req', '-in', str(leaf_csr), '-CA', str(cert), '-CAkey', str(private_key), '-CAcreateserial', '-out', str(leaf_cert), '-days', '1', '-extfile', str(leaf_ext)],
    ]:
        subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    held = threading.Event()
    release = threading.Event()
    provider_calls = []

    class MockProvider(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def do_POST(self):
            submitted = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))))
            provider_calls.append(self.path)
            if 'hold-through-expiry' in json.dumps(submitted):
                held.set()
                if not release.wait(15):
                    raise RuntimeError('acceptance did not release held response')
            if 'controlled-failure' in json.dumps(submitted):
                body = b'{"error":{"message":"synthetic failure","type":"server_error"}}'
                self.send_response(503)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            body = json.dumps({'id': 'resp_local_mock', 'object': 'response',
                'created_at': int(time.time()), 'status': 'completed', 'model': 'local-exact-model',
                'output': [{'id': 'msg_local', 'type': 'message', 'role': 'assistant', 'status': 'completed',
                            'content': [{'type': 'output_text', 'text': 'local acceptance', 'annotations': []}]}],
                'usage': {'input_tokens': 10, 'output_tokens': 3, 'total_tokens': 13}}).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    provider = ThreadingHTTPServer(('127.0.0.1', 0), MockProvider)
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls.load_cert_chain(leaf_cert, leaf_key)
    provider.socket = tls.wrap_socket(provider.socket, server_side=True)
    threading.Thread(target=provider.serve_forever, daemon=True).start()
    provider_port = provider.server_port
    data_port, management_port = free_port(), free_port()
    base = f'http://127.0.0.1:{management_port}'
    checks = []
    log = (root / 'gateway.log').open('wb')
    def start_gateway():
        return subprocess.Popen([
        str(ROOT / 'target/debug/gateway'), 'serve', '--state-dir', str(state),
        '--credential-dir', str(credentials), '--data-listen', f'127.0.0.1:{data_port}',
        '--management-listen', f'127.0.0.1:{management_port}',
    ], stdout=log, stderr=log, env={**os.environ, 'SSL_CERT_FILE': str(cert)})

    process = start_gateway()

    def request(path, method='GET', body=None, revision=None, scope=None):
        headers = {'x-management-key': key, 'Origin': base,
                   'x-management-csrf-token': csrf, 'Content-Type': 'application/json'}
        if revision is not None:
            headers['If-Match'] = revision
        if scope is not None:
            headers['X-Config-Version'] = scope
        req = urllib.request.Request(base + path, method=method, headers=headers,
                                     data=None if body is None else json.dumps(body).encode())
        try:
            with urllib.request.urlopen(req, timeout=10) as response:
                raw = response.read()
                return response.status, response.headers, json.loads(raw) if raw else None
        except urllib.error.HTTPError as error:
            # Never include request bodies or headers in errors.
            raise RuntimeError(f'{method} {path}: HTTP {error.code}: {error.read().decode()}') from None

    try:
        deadline = time.monotonic() + 15
        while True:
            if process.poll() is not None:
                raise RuntimeError(f'gateway exited; inspect {root / "gateway.log"}')
            try:
                status, _, versions = request('/admin/config-versions')
                break
            except urllib.error.URLError:
                if time.monotonic() > deadline:
                    raise
                time.sleep(.05)
        assert status == 200 and versions == []
        checks.append('empty real management listener')
        status, _, version = request('/admin/config-versions', 'POST',
                                     {'id': 'prism-local-v4', 'description': 'Synthetic Prism V4 acceptance'})
        assert status == 201
        checks.append('create draft through authenticated same-origin management API')
        status, _, versions = request('/admin/config-versions')
        assert status == 200 and any(v['id'] == 'prism-local-v4' for v in versions)
        checks.append('reread persisted draft')
        scope = 'prism-local-v4'
        version = next(v for v in versions if v['id'] == scope)
        revision = version['revision']
        status, headers, _ = request('/admin/public-models', 'POST', {
            'id': 'local-model', 'model_name': 'local-exact-model', 'status': 'active',
            'display_name': 'Local acceptance', 'capabilities': {'streaming': True},
        }, revision=revision, scope=scope)
        assert status == 201
        revision = headers['ETag']
        status, headers, _ = request('/admin/public-models/local-model/routes', 'POST', {
            'id': 'local-route', 'policy': 'smooth_weighted_round_robin',
            'max_attempts': 3, 'bootstrap_timeout_ms': 30000,
        }, revision=revision, scope=scope)
        assert status == 201
        revision = headers['ETag']
        status, _, routes = request('/admin/routes?limit=1', scope=scope)
        assert status == 200 and routes['items'][0]['id'] == 'local-route'
        assert routes['next_cursor'] is None
        checks.append('real complete route enumeration includes unbound draft')
        status, _, validation = request('/admin/routes/local-route/validate', 'POST', {},
                                        revision=revision, scope=scope)
        assert status == 200 and not validation['valid']
        checks.append('candidate-less route fails real topology validation')
        def mutate(path, body, method='POST'):
            nonlocal revision
            status, headers, result = request(path, method, body, revision=revision, scope=scope)
            assert status in (200, 201, 204)
            revision = headers.get('ETag', revision)
            return result

        mutate('/admin/egress-policies', {'id': 'local-egress', 'name': 'Loopback only',
            'allowed_schemes': ['https'], 'allowed_hosts': ['127.0.0.1'],
            'allowed_ports': [provider_port], 'allowed_cidrs': ['127.0.0.1/32'],
            'redirect_mode': 'deny', 'max_redirects': 0})
        mutate('/admin/upstreams', {'id': 'local-upstream', 'name': 'Loopback mock',
            'kind': 'openai-compatible', 'enabled': True, 'tags': [], 'egress_policy_id': 'local-egress'})
        mutate('/admin/upstreams/local-upstream/endpoints', {'id': 'local-endpoint',
            'adapter_id': 'openai-compatible.responses', 'api_format': 'openai/responses',
            'base_url': f'https://127.0.0.1:{provider_port}/v1', 'inference_path': '/responses',
            'models_path': None, 'transport': 'https', 'enabled': True})
        mutate('/admin/upstreams/local-upstream/credentials', {'id': 'local-credential',
            'kind': 'bearer', 'secret': secrets.token_hex(24), 'status': 'active'})
        mutate('/admin/endpoints/local-endpoint/credential-bindings', {'credential_id': 'local-credential',
            'enabled': True, 'priority': 0, 'weight': 1, 'concurrency': 4})
        candidate = {'id': 'local-candidate', 'endpoint_id': 'local-endpoint',
            'upstream_model': 'local-exact-model', 'credential_scope': 'all_active',
            'transform_mode': 'passthrough', 'enabled': True, 'priority': 0, 'weight': 1,
            'capability_override': {'allow_unlisted_model': True}}
        mutate('/admin/routes/local-route/candidates', candidate)
        mutate('/admin/routes/local-route/candidates/local-candidate', {**candidate, 'weight': 2}, 'PATCH')
        mutate('/admin/access-groups', {'id': 'local-group', 'name': 'Local', 'status': 'active', 'limits': {}})
        mutate('/admin/access-groups/local-group/routes', {'route_id': 'local-route', 'enabled': True})
        issued = mutate('/admin/client-keys', {'id': 'local-client', 'access_group_id': 'local-group', 'status': 'active'})
        checks.append('real upstream, binding, candidate edit and authorization management writes')
        if args.priced:
            mutate('/admin/billing/catalogs', {'catalog_version_id': 'local-prices',
                'effective_at_ms': 0, 'source': 'operator', 'entries': [{
                    'provider_id': 'local-upstream', 'channel_id': 'local-endpoint', 'model': 'local-exact-model',
                    'input_microunits_per_million': 1_000_000, 'output_microunits_per_million': 2_000_000,
                    'reasoning_microunits_per_million': 0, 'cache_read_microunits_per_million': 0,
                    'cache_creation_microunits_per_million': 0, 'cached_microunits_per_million': 0}]})
            checks.append('import price catalog through real draft management API')

        validation = mutate(f'/admin/config-versions/{scope}/validate', {})
        (root / 'validation.json').write_text(json.dumps(validation, indent=2))
        assert validation['valid'], 'configuration invalid; inspect validation.json'
        mutate(f'/admin/config-versions/{scope}/publish', {})
        checks.append('real configuration validation and publication')
        process.terminate()
        assert process.wait(timeout=40) == 0
        if args.catalog_expiry:
            now = int(time.time() * 1000)
            expiry = now + 6000
            with sqlite3.connect(state / 'control.sqlite3') as db:
                db.execute('INSERT INTO model_catalog_targets VALUES (?, ?, ?, 1, ?, ?, ?, ?)',
                           (scope, 'local-endpoint', 'local-credential', expiry - 72 * 3600_000, expiry - 66 * 3600_000, expiry - 48 * 3600_000, expiry))
                db.execute('INSERT INTO model_catalog_models VALUES (?, ?, ?, ?, 1, 0, NULL, NULL)',
                           (scope, 'local-endpoint', 'local-credential', 'local-exact-model'))
                db.execute('INSERT INTO model_catalog_failures VALUES (?, ?, ?, ?, ?)',
                           (scope, 'local-endpoint', 'local-credential', now, 'transport'))
        process = start_gateway()
        deadline = time.monotonic() + 15
        while True:
            assert process.poll() is None, f'gateway restart failed; inspect {root / "gateway.log"}'
            try:
                status, _, effective = request('/admin/models/effective?access_group_id=local-group', scope=scope)
                break
            except urllib.error.URLError:
                if time.monotonic() > deadline:
                    raise
                time.sleep(.05)
        assert status == 200 and any(model['id'] == 'local-exact-model' for model in effective['items'])
        checks.append('restart serves published effective model under existing group ID')
        if args.catalog_expiry:
            evidence = effective['items'][0]['sources'][0]['catalog_evidence']
            assert evidence and evidence[0]['expires_at_ms'] == expiry
            held_result = []
            def held_request():
                try:
                    req = urllib.request.Request(f'http://127.0.0.1:{data_port}/v1/responses',
                        headers={'Authorization': 'Bearer ' + issued['key'], 'Content-Type': 'application/json'},
                        data=json.dumps({'model': 'local-exact-model', 'input': 'hold-through-expiry', 'stream': False}).encode())
                    with urllib.request.urlopen(req, timeout=15) as response:
                        held_result.append((response.status, json.loads(response.read())))
                except Exception as error:
                    held_result.append(type(error).__name__)
            thread = threading.Thread(target=held_request)
            thread.start()
            assert held.wait(3), 'request did not acquire a lease before expiry'
            while int(time.time() * 1000) <= expiry + 50:
                time.sleep(.05)
            _, _, expired = request('/admin/models/effective?access_group_id=local-group', scope=scope)
            assert expired['items'] == [], 'expired catalog still offers new model selection'
            calls_before = len(provider_calls)
            req = urllib.request.Request(f'http://127.0.0.1:{data_port}/v1/responses',
                headers={'Authorization': 'Bearer ' + issued['key'], 'Content-Type': 'application/json'},
                data=json.dumps({'model': 'local-exact-model', 'input': 'after-expiry', 'stream': False}).encode())
            try:
                urllib.request.urlopen(req, timeout=10).close()
                raise AssertionError('expired catalog admitted a new request')
            except urllib.error.HTTPError as error:
                assert error.code in (400, 403, 404, 503)
            assert len(provider_calls) == calls_before
            release.set()
            thread.join(timeout=10)
            assert held_result and held_result[0][0] == 200, 'in-flight snapshot did not finish successfully'
            checks.append('hard expiry removes effective selection and blocks new Provider lease despite failed discovery evidence')
            checks.append('request leased before hard expiry completes under its pinned snapshot')
            report = {'checks': checks, 'expires_at_ms': expiry, 'provider_calls': len(provider_calls)}
            (root / 'evidence.json').write_text(json.dumps(report, indent=2))
            print(json.dumps({'passed': checks, 'evidence': str(root / 'evidence.json')}, indent=2))
            return

        data_request = urllib.request.Request(f'http://127.0.0.1:{data_port}/v1/responses',
            headers={'Authorization': 'Bearer ' + issued['key'], 'Content-Type': 'application/json'},
            data=json.dumps({'model': 'local-exact-model', 'input': 'Synthetic local acceptance', 'stream': False}).encode())
        try:
            with urllib.request.urlopen(data_request, timeout=15) as response:
                result = json.loads(response.read())
                assert response.status == 200
        except urllib.error.HTTPError as error:
            raise RuntimeError(f'local data request: HTTP {error.code}: {error.read().decode()}') from None
        checks.append('real data listener routes a controlled request to TLS loopback Provider')
        deadline = time.monotonic() + 10
        while True:
            status, _, ledger = request('/admin/operations/billing?limit=10')
            if ledger['items']:
                break
            if time.monotonic() > deadline:
                raise RuntimeError('billing worker did not materialize the controlled request')
            time.sleep(.1)
        assert len(ledger['items']) == 1
        row = ledger['items'][0]
        assert row['model'] == 'local-exact-model'
        assert (row['input_tokens'], row['output_tokens']) == (10, 3)
        assert all(row[name] is None for name in ['reasoning_tokens', 'cache_read_tokens', 'cache_creation_tokens', 'cached_tokens'])
        if args.priced:
            assert row['cost_confidence'] == 'partial' and row['cost_microunits'] == 16
            assert row['catalog_version_id'] == 'local-prices'
        else:
            assert row['cost_confidence'] == 'unpriced' and row['cost_microunits'] is None
        (root / 'ledger.json').write_text(json.dumps(ledger, indent=2))
        checks.append('controlled request materializes into persistent billing ledger')
        failed_request = urllib.request.Request(f'http://127.0.0.1:{data_port}/v1/responses',
            headers={'Authorization': 'Bearer ' + issued['key'], 'Content-Type': 'application/json'},
            data=json.dumps({'model': 'local-exact-model', 'input': 'controlled-failure', 'stream': False}).encode())
        try:
            urllib.request.urlopen(failed_request, timeout=15).close()
            raise AssertionError('controlled Provider failure unexpectedly succeeded')
        except urllib.error.HTTPError as error:
            assert error.code in (502, 503)
        checks.append('controlled Provider failure reaches real request error path')

        process.terminate()
        assert process.wait(timeout=40) == 0
        with sqlite3.connect(state / 'control.sqlite3') as db:
            for table, id_column in [('stored_responses', 'response_id'), ('stored_response_compactions', 'compact_id')]:
                for identity, expiry in [('ttl-expired', 1), ('ttl-live', 9223372036854775807)]:
                    db.execute(f'INSERT INTO {table} (client_key_id, {id_column}, created_at_ms, expires_at_ms, payload_version, key_version, ciphertext) VALUES (?, ?, 0, ?, 1, 1, zeroblob(41))', ('local-client', identity, expiry))
        process = start_gateway()
        deadline = time.monotonic() + 15
        while True:
            assert process.poll() is None, 'gateway failed second restart'
            try:
                _, _, progress = request('/admin/operations/billing-processing')
                if progress['state'] == 'current':
                    break
            except urllib.error.URLError:
                pass
            if time.monotonic() > deadline:
                raise RuntimeError('billing checkpoint did not resume after restart')
            time.sleep(.05)
        _, _, replayed = request('/admin/operations/billing?limit=10')
        assert replayed == ledger, 'ledger changed after checkpoint replay'
        checks.append('restart resumes checkpoint without duplicate ledger or cost')
        deadline = time.monotonic() + 5
        while True:
            with sqlite3.connect(state / 'control.sqlite3') as db:
                counts = [db.execute(f'SELECT count(*) FROM {table} WHERE expires_at_ms=1').fetchone()[0]
                          for table in ['stored_responses', 'stored_response_compactions']]
                live = [db.execute(f'SELECT count(*) FROM {table} WHERE expires_at_ms=9223372036854775807').fetchone()[0]
                        for table in ['stored_responses', 'stored_response_compactions']]
            if counts == [0, 0]:
                break
            if time.monotonic() > deadline:
                raise RuntimeError('serve TTL worker did not remove expired synthetic records')
            time.sleep(.05)
        assert live == [1, 1]
        checks.append('serve TTL worker deletes expired responses and compactions and preserves live rows')
        if args.large:
            narrow = f"?from_ms={row['occurred_at_ms']}&to_ms={row['occurred_at_ms'] + 1}&limit=10"
            _, _, baseline_usage = request('/admin/operations/usage' + narrow)
            assert baseline_usage['items']
            failure_path = '/admin/operations/provider-account-pools/failures?account_id=local-credential&limit=10'
            _, _, baseline_failures = request(failure_path, scope=scope)
            assert baseline_failures['items']

            samples = []
            for total in [99_999, 100_000, 100_001, 100_005]:
                with sqlite3.connect(state / 'control.sqlite3') as db:
                    db.row_factory = sqlite3.Row
                    template = dict(db.execute('SELECT * FROM billing_ledger_entries LIMIT 1').fetchone())
                    columns = [column for column in template if column != 'ledger_id']
                    count = db.execute('SELECT count(*) FROM billing_ledger_entries').fetchone()[0]
                    def ledger_rows():
                        for index in range(count, total):
                            copy = {**template, 'source_event_id': f'bulk-ledger-{index}',
                                    'request_id': f'bulk-request-{index}', 'occurred_at_ms': 0}
                            yield tuple(copy[column] for column in columns)
                    db.executemany(f"INSERT INTO billing_ledger_entries ({','.join(columns)}) VALUES ({','.join('?' for _ in columns)})", ledger_rows())
                    payload = json.loads(db.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='request' LIMIT 1").fetchone()[0])
                    event_count = db.execute('SELECT count(*) FROM gateway_event_log').fetchone()[0]
                    def event_rows():
                        for index in range(event_count, total):
                            identity = f'bulk-event-{index}'
                            payload['request']['request_id'] = identity
                            yield (identity, identity, json.dumps(payload))
                    db.executemany("INSERT INTO gateway_event_log (event_type, event_id, request_id, occurred_at_ms, payload_json) VALUES ('request', ?, ?, 0, ?)", event_rows())
                _, _, narrow_ledger = request('/admin/operations/billing' + narrow)
                assert narrow_ledger['items'] == ledger['items'] and narrow_ledger['summary'] == ledger['summary']
                _, _, usage = request('/admin/operations/usage' + narrow)
                assert usage['items'] == baseline_usage['items']
                _, _, failures = request(failure_path, scope=scope)
                assert failures['items'] == baseline_failures['items']

                _, _, wide = request('/admin/operations/billing?limit=1')
                assert wide['summary']['records'] == total
                samples.append({'ledger_rows': total, 'event_rows': total, 'narrow_records': 1})
            (root / 'large-sample.json').write_text(json.dumps(samples, indent=2))
            checks.append('99999/100000/100001/100005 histories preserve filtered billing, usage, failures and complete ledger summary')




        status, _, processing = request('/admin/operations/billing-processing')
        assert status == 200
        checks.append('production billing processing endpoint')
        with urllib.request.urlopen(base + '/admin-ui/', timeout=10) as response:
            html = response.read().decode()
            assert 'assets/main.js' in html and response.headers.get('Content-Security-Policy')
        checks.append('embedded production SPA and CSP')
        if args.browser:
            subprocess.run(['node', str(ROOT / 'web/prism/e2e/real-gateway-audit.mjs')],
                           input=json.dumps({'base': base, 'key': key, 'csrf': csrf, 'output': str(root / 'browser')}),
                           text=True, check=True)
            checks.append('production browser pages captured at three sizes in light and dark')

        report = {'checks': checks, 'state_directory': str(state), 'management_url': base,
                  'processing': processing}
        (root / 'evidence.json').write_text(json.dumps(report, indent=2))
        print(json.dumps({'passed': checks, 'evidence': str(root / 'evidence.json')}, indent=2))
    finally:
        release.set()
        process.terminate()
        try:
            process.wait(timeout=40)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            raise RuntimeError('gateway did not stop within acceptance timeout') from None
        log.close()
        provider.shutdown()
        provider.server_close()


if __name__ == '__main__':
    main()
