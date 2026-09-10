#!/usr/bin/env python3
"""Real local gateway auth acceptance. Synthetic credentials, no Provider requests."""
import argparse
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


def available_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--report', type=Path)
    parser.add_argument('--browser-output', type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    checks = []
    with tempfile.TemporaryDirectory(prefix='prism-login-acceptance-') as temp:
        root = Path(temp).resolve()
        state, credentials = root / 'state', root / 'credentials'
        state.mkdir(mode=0o700)
        credentials.mkdir(mode=0o700)
        for name in ['master-key', 'backup-key', 'client-key-pepper', 'grok-build-cache-key']:
            (credentials / name).write_bytes(secrets.token_bytes(32))
        legacy = 'mgmt_' + secrets.token_hex(32)
        (credentials / 'management-key').write_text(legacy)
        (credentials / 'management-csrf').write_text('csrf_' + secrets.token_hex(32))
        for path in credentials.iterdir():
            path.chmod(0o600)
        password_file = root / 'initial-password'
        init = [str(binary), 'admin-login', 'init', '--state-dir', str(state), '--password-file', str(password_file)]
        subprocess.run(init, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        initial = password_file.read_text().strip()
        assert len(initial) == 64 and password_file.stat().st_mode & 0o777 == 0o600
        assert (state / 'admin-auth.sqlite3').stat().st_mode & 0o777 == 0o600
        assert subprocess.run(init, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode != 0
        assert password_file.read_text().strip() == initial
        checks.append('one-time owner-only bootstrap, existing credentials preserved')
        data_port, admin_port = available_port(), available_port()
        while data_port == admin_port:
            admin_port = available_port()
        base = f'http://127.0.0.1:{admin_port}'
        command = [str(binary), 'serve', '--data-listen', f'127.0.0.1:{data_port}', '--management-listen', f'127.0.0.1:{admin_port}', '--state-dir', str(state), '--credential-dir', str(credentials)]
        # Explicit no-proxy opener: loopback acceptance never depends on ambient proxy settings.
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        def api(path, body=None, grant=None, origin=base, override=None):
            headers = {'Origin': origin} if origin else {}
            if grant:
                headers.update({'X-Management-Key': grant['session_token'], 'X-Management-CSRF-Token': grant['csrf_token']})
            if override:
                headers.update(override)
            payload = None if body is None else json.dumps(body).encode()
            if payload is not None:
                headers['Content-Type'] = 'application/json'
            request = urllib.request.Request(base + path, data=payload, headers=headers)
            try:
                response = opener.open(request, timeout=8)
            except urllib.error.HTTPError as error:
                response = error
            with response:
                raw = response.read()
                return response.status, (json.loads(raw) if raw and response.headers.get_content_type() == 'application/json' else raw)
        def start():
            process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            for _ in range(100):
                if process.poll() is not None:
                    raise RuntimeError('gateway exited before readiness')
                try:
                    if api('/admin-ui/')[0] == 200:
                        return process
                except OSError:
                    pass
                time.sleep(0.1)
            stop(process)
            raise RuntimeError('gateway did not become ready')
        def stop(process):
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=35)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                raise RuntimeError('gateway shutdown exceeded bound')
        process = start()
        try:
            login = lambda password: api('/admin/auth/login', {'username': 'admin', 'password': password})
            assert api('/admin/auth/login', {'username': 'admin', 'password': initial}, origin='https://wrong.example')[0] == 404
            assert login('wrong-password')[0] == 401
            status, grant = login(initial)
            assert status == 200 and grant['password_change_required']
            assert api('/admin/config-versions', grant=grant)[0] == 404
            assert api('/admin/config-versions', origin=None, override={'X-Management-Key': legacy})[0] == 200
            checks.append('exact origin, wrong password, first-session restriction, legacy CLI')
            new_password = secrets.token_urlsafe(24)
            change = {'current_password': initial, 'new_password': new_password}
            assert api('/admin/auth/password', change, grant, override={'X-Management-CSRF-Token': 'wrong'})[0] == 404
            if args.browser_output:
                browser_script = Path(__file__).resolve().parents[1] / 'web/prism/e2e/real-admin-login.mjs'
                subprocess.run(['node', str(browser_script)], input=json.dumps({'base': base, 'initial': initial, 'newPassword': new_password, 'output': str(args.browser_output.resolve())}), text=True, check=True)
                checks.append('real embedded browser first-login/password-change/refresh flow and responsive visual acceptance')
            else:
                assert api('/admin/auth/password', change, grant)[0] == 204
            assert api('/admin/config-versions', grant=grant)[0] == 404
            assert login(initial)[0] == 401
            status, grant = login(new_password)
            assert status == 200 and not grant['password_change_required']
            assert api('/admin/config-versions', grant=grant)[0] == 200
            status, created = api('/admin/config-versions', {'id': 'login-acceptance-draft', 'description': 'Synthetic login acceptance'}, grant)
            assert status == 201 and created['id'] == 'login-acceptance-draft'
            assert any(row['id'] == 'login-acceptance-draft' for row in api('/admin/config-versions', grant=grant)[1])
            checks.append('first password change, automatic CSRF, revoked initial password/session, real draft write/read')
            assert api('/admin/auth/logout', {}, grant)[0] == 204
            assert api('/admin/config-versions', grant=grant)[0] == 404
            checks.append('server logout revocation')
            stop(process)
            process = start()
            assert api('/admin/config-versions', grant=grant)[0] == 404
            status, grant = login(new_password)
            assert status == 200 and not grant['password_change_required']
            checks.append('restart persists changed password and discards sessions')
        finally:
            if process.poll() is None:
                stop(process)
    report = {'result': 'passed', 'real_gateway': True, 'provider_requests': 0, 'checks': checks}
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
