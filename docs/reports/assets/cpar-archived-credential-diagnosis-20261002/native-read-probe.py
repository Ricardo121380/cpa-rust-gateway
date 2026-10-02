"""Execute exact old/new ELF inside the retained namespace runner; emit value-free receipts."""
import datetime
import hashlib
import json
import os
import pathlib
import socket
import sqlite3
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

expected_source, expected_hash, expected_schema = sys.argv[1], sys.argv[2], int(sys.argv[3])
target = json.loads(pathlib.Path('/target.private.json').read_text())
result = {'status': 'FAIL', 'record_alias': 'AC-01', 'source_sha': expected_source, 'binary_sha256': hashlib.sha256(pathlib.Path('/gateway').read_bytes()).hexdigest(), 'real_provider_calls': 0, 'uid': os.getuid(), 'gid': os.getgid()}
assert result['binary_sha256'] == expected_hash and result['uid'] == 995 and result['gid'] == 986
assert not os.path.exists('/var/lib/cpa-rust-gateway') and not os.path.exists('/etc/cpa-rust-gateway/credentials')
assert not pathlib.Path('/proc/net/route').read_text().splitlines()[1:]
caps = dict(line.split(':', 1) for line in pathlib.Path('/proc/self/status').read_text().splitlines() if ':' in line)
assert int(caps['CapEff'], 16) == int(caps['CapBnd'], 16) == 0 and caps['NoNewPrivs'].strip() == '1'
with socket.socket() as s:
    try:
        s.settimeout(1)
        s.connect(('192.0.2.1', 443))
        raise AssertionError('external_route_available')
    except OSError as error:
        assert error.errno in (101, 113, 1, 13)
result['network_isolated'] = result['production_paths_hidden'] = result['capabilities_zero'] = True
with sqlite3.connect('file:/state/control.sqlite3?mode=ro', uri=True) as db:
    child = db.execute('SELECT id,parent_id,revision,status FROM config_versions WHERE id=?', (target['config_version_id'],)).fetchone()
    parent = db.execute('SELECT id,revision,status FROM config_versions WHERE id=?', (child[1],)).fetchone()
assert child[3] == 'archived' and parent[2] == 'archived'
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
headers = {'X-Management-Key': pathlib.Path('/credentials/management-key').read_text().strip(), 'Content-Type': 'application/json'}
base = 'http://127.0.0.1:28181'

def api(method, route, version=None, body=None, revision=None):
    request_headers = dict(headers)
    if version is not None:
        request_headers['X-Config-Version'] = version
    if revision is not None:
        request_headers['If-Match'] = 'rev-' + str(revision)
    req = urllib.request.Request(base + route, method=method, headers=request_headers, data=None if body is None else json.dumps(body).encode())
    try:
        response = opener.open(req, timeout=12)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        raw = response.read(4 * 1024 * 1024)
        value = json.loads(raw) if raw else None
        safe = {'http_status': response.status}
        if isinstance(value, dict) and isinstance(value.get('error'), dict):
            safe['error_code'] = value['error'].get('code')
        return safe, value

log = open('/work/gateway.private.log', 'wb')
process = subprocess.Popen(['/gateway', 'serve', '--state-dir', '/state', '--credential-dir', '/credentials', '--data-listen', '127.0.0.1:28180', '--management-listen', '127.0.0.1:28181'], stdout=log, stderr=log)
try:
    deadline = time.monotonic() + 30
    while True:
        try:
            with opener.open('http://127.0.0.1:28180/healthz', timeout=1) as reply:
                assert reply.status == 200
                break
        except (OSError, urllib.error.URLError):
            assert process.poll() is None and time.monotonic() < deadline, 'startup_failed'
            time.sleep(0.1)
    _, system = api('GET', '/admin/system')
    assert system['build']['build_revision'] == expected_source and system['build']['schema_version'] == expected_schema
    result['actual_build'] = system['build']
    cid = urllib.parse.quote(target['credential_id'], safe='')
    config = urllib.parse.quote(target['config_version_id'], safe='')
    reads = {}
    for label, method, route, version, body, revision in [
        ('version_detail', 'GET', '/admin/config-versions/' + config, None, None, None),
        ('credential_summary', 'GET', '/admin/credentials/' + cid, target['config_version_id'], None, None),
        ('credential_metadata', 'GET', '/admin/credentials/' + cid + '/metadata', target['config_version_id'], None, None),
        ('parent_metadata', 'GET', '/admin/credentials/' + cid + '/metadata', parent[0], None, None),
        ('target_export', 'POST', '/admin/credentials/' + cid + '/export', target['config_version_id'], {'format': 'cpa'}, None),
        ('parent_export', 'POST', '/admin/credentials/' + cid + '/export', parent[0], {'format': 'cpa'}, None),
        ('archived_validate', 'POST', '/admin/config-versions/' + config + '/validate', target['config_version_id'], None, None),
        ('archived_fork', 'POST', '/admin/config-versions/' + config + '/fork', target['config_version_id'], {'id': 'diagnosis-rejected-fork', 'description': 'isolated diagnosis'}, child[2]),
        ('archived_publish', 'POST', '/admin/config-versions/' + config + '/publish', target['config_version_id'], None, child[2]),
    ]:
        result['current_operation'] = label
        safe, value = api(method, route, version, body, revision)
        reads[label] = safe
        if label == 'target_export':
            target_export = value if safe['http_status'] == 200 else None
        if label == 'parent_export':
            result['target_and_parent_export_equal'] = target_export == value if target_export is not None and safe['http_status'] == 200 else None
        del value
    result['operations'] = reads
    result['status'] = 'PASS'
    result.pop('current_operation', None)
except Exception as error:
    result['failure_category'] = type(error).__name__
finally:
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
        result['forced_kill'] = True
        result['status'] = 'FAIL'
    log.close()
    result['isolated_process_stopped'] = process.poll() is not None
    result['observed_at_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    result['plaintext_or_export_body_retained'] = False
    pathlib.Path('/work/native-read-receipt.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
sys.exit(0 if result['status'] == 'PASS' else 1)
