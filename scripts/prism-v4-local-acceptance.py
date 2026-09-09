#!/usr/bin/env python3
"""Prism acceptance against the real local serve binary; synthetic state only."""
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
    data_port, management_port = free_port(), free_port()
    base = f'http://127.0.0.1:{management_port}'
    checks = []
    log = (root / 'gateway.log').open('wb')
    process = subprocess.Popen([
        str(ROOT / 'target/debug/gateway'), 'serve', '--state-dir', str(state),
        '--credential-dir', str(credentials), '--data-listen', f'127.0.0.1:{data_port}',
        '--management-listen', f'127.0.0.1:{management_port}',
    ], stdout=log, stderr=log)

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


if __name__ == '__main__':
    main()
