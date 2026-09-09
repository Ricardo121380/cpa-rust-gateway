#!/usr/bin/env python3
"""Prism acceptance against the real local serve binary; synthetic state only."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import ssl
import threading
import json
import os
from pathlib import Path
import secrets
import socket
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

    class MockProvider(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def do_POST(self):
            self.rfile.read(int(self.headers.get('Content-Length', '0')))
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
    tls.load_cert_chain(cert, private_key)
    provider.socket = tls.wrap_socket(provider.socket, server_side=True)
    threading.Thread(target=provider.serve_forever, daemon=True).start()
    provider_port = provider.server_port
    data_port, management_port = free_port(), free_port()
    base = f'http://127.0.0.1:{management_port}'
    checks = []
    log = (root / 'gateway.log').open('wb')
    process = subprocess.Popen([
        str(ROOT / 'target/debug/gateway'), 'serve', '--state-dir', str(state),
        '--credential-dir', str(credentials), '--data-listen', f'127.0.0.1:{data_port}',
        '--management-listen', f'127.0.0.1:{management_port}',
    ], stdout=log, stderr=log, env={**os.environ, 'SSL_CERT_FILE': str(cert)})

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
        validation = mutate(f'/admin/config-versions/{scope}/validate', {})
        (root / 'validation.json').write_text(json.dumps(validation, indent=2))
        assert validation['valid'], 'configuration invalid; inspect validation.json'
        mutate(f'/admin/config-versions/{scope}/publish', {})
        checks.append('real configuration validation and publication')

        status, _, processing = request('/admin/operations/billing-processing')
        assert status == 200
        checks.append('production billing processing endpoint')
        with urllib.request.urlopen(base + '/admin-ui/', timeout=10) as response:
            html = response.read().decode()
            assert 'assets/main.js' in html and response.headers.get('Content-Security-Policy')
        checks.append('embedded production SPA and CSP')
        report = {'checks': checks, 'state_directory': str(state), 'management_url': base,
                  'processing': processing}
        (root / 'evidence.json').write_text(json.dumps(report, indent=2))
        print(json.dumps({'passed': checks, 'evidence': str(root / 'evidence.json')}, indent=2))
    finally:
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
