import ast
import datetime
import hashlib
import json
import os
import pathlib
import sqlite3
import stat
import subprocess
import time
import urllib.request

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
unit = 'cpar-build-acceptance-20261010.service'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert not (base / 'private/repair-configure-receipt.json').exists()
initial = json.loads((base / 'private/configure-receipt.json').read_text())
old_artifact = json.loads((base / 'private/candidate-artifact.json').read_text())
artifact = json.loads((base / 'private/candidate-artifact-f346a03f.json').read_text())
binary = base / 'bin' / artifact['binary_file']
guard = pathlib.Path(artifact['guard_path'])
assert hashlib.sha256(binary.read_bytes()).hexdigest() == artifact['binary_sha256']
assert hashlib.sha256(guard.read_bytes()).hexdigest() == artifact['guard_sha256']
assert artifact['product_source_sha'] == 'f346a03f912b150006a288c38087550bb3d04cb0'
source = ast.parse((base / 'private/bootstrap.py').read_text())
functions = ast.Module(body=[node for node in source.body if isinstance(node, ast.FunctionDef) and node.name == 'production'], type_ignores=[])
exec(compile(functions, 'production-proof', 'exec'))
before = production()
assert before == json.loads((base / 'private/bootstrap-receipt.json').read_text())['production_after']
pid = int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
assert pid == initial['pid']
assert hashlib.sha256(pathlib.Path('/proc', str(pid), 'exe').read_bytes()).hexdigest() == old_artifact['binary_sha256']
old_budget_file = base / 'private/batch-budget.json'
old_budget_bytes = old_budget_file.read_bytes()
old_budget = json.loads(old_budget_bytes)
assert old_budget['stopped'] is True and old_budget['catalog_intents'] == 1 and old_budget['pin_intents'] == 1 and old_budget['basic_intents'] == 0
pin_error = json.loads((base / 'private/responses-json/pin-private.json').read_text())
assert pin_error['error']['code'] == 'invalid_management_request'
with sqlite3.connect('file:' + str(base / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    event_count = db.execute('SELECT count(*) FROM gateway_event_log').fetchone()[0]
    action_counts = dict(db.execute("SELECT action,count(*) FROM management_resource_audit_events WHERE action LIKE 'channel_pin%' GROUP BY action"))
    accounts = db.execute("SELECT id,revision,enabled,auth_status,worker_claim_kind FROM grok_accounts WHERE provider='build'").fetchall()
assert event_count == 0 and action_counts == {'channel_pin_requested': 1, 'channel_pin_started': 1}
assert len(accounts) == 1 and accounts[0][1:] == (initial['native_revision'], 1, 'active', None)
assert hashlib.sha256(('cpar-evidence:' + accounts[0][0]).encode()).hexdigest() == initial['account_evidence_id']
audit = base / 'audit/candidate-egress.jsonl'
audit_before = audit.read_bytes()
events = [json.loads(line)['event'] for line in audit_before.splitlines()]
assert sorted(events) == ['connect_build_allowed', 'dns_build_allowed']
resolution = {'status': 'PROVEN_ZERO_INFERENCE', 'historical_pin_http_status': 400,
              'historical_error_code': 'invalid_management_request', 'historical_management_admission_intents': 1,
              'historical_provider_inference_requests': 0, 'gateway_event_count': event_count,
              'channel_pin_action_counts': action_counts, 'initial_source_sha': old_artifact['product_source_sha'],
              'source_proof': 'InvalidTarget/invalid_input HTTP400 paths precede transport; post-execution receipt validation maps to Unavailable HTTP503, not HTTP400',
              'local_reproduction': 'old artifact rejects Pin before any loopback receive while public control sends; fixed scheduler-snapshot source succeeds',
              'old_budget_sha256': hashlib.sha256(old_budget_bytes).hexdigest(), 'old_pin_response_sha256': hashlib.sha256((base / 'private/responses-json/pin-private.json').read_bytes()).hexdigest(),
              'egress_counts_support_only': {'dns_build_allowed': 1, 'connect_build_allowed': 1},
              'server_http_receive_count_claim': False}
(base / 'private/pin400-zero-resolution.json').write_text(json.dumps(resolution, indent=2) + '\n')
subprocess.run(['systemctl', 'stop', unit], check=True)
for _ in range(100):
    if subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=LoadState'], text=True).strip() == 'not-found':
        break
    time.sleep(.1)
else:
    raise AssertionError('owned transient unit did not unload')
launch = json.loads((base / 'private/recovered-launch.json').read_text())['command']
old_binary = str(base / 'bin' / old_artifact['binary_file'])
assert launch.count(old_binary) == 1
launch = [str(binary) if value == old_binary else value for value in launch]
launch += json.loads((base / 'private/copy-runtime-options.json').read_text())
subprocess.run(launch, check=True, capture_output=True)
client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
headers = {'X-Management-Key': (base / 'credentials/management-key').read_text().strip(), 'X-Config-Version': initial['configuration_id']}
def get(path):
    with client.open(urllib.request.Request('http://127.0.0.1:19181' + path, headers=headers), timeout=3) as response:
        assert response.status == 200
        return json.load(response)
deadline = time.monotonic() + 30
while True:
    try:
        versions = get('/admin/config-versions')
        break
    except urllib.error.URLError:
        assert time.monotonic() < deadline
        time.sleep(.2)
active = [row for row in versions if row['status'] == 'active']
assert len(active) == 1 and active[0]['id'] == initial['configuration_id'] and active[0]['revision'].strip('"') == initial['configuration_etag'].strip('"')
pid = int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
assert pid > 0 and pid != initial['pid']
proc = pathlib.Path('/proc', str(pid))
actual = proc.joinpath('cmdline').read_bytes().decode().rstrip('\0').split('\0')
assert actual[0] == str(binary) and actual[actual.index('--background-provider-tasks') + 1] == 'disabled'
assert actual[actual.index('--data-listen') + 1] == '127.0.0.1:19180' and actual[actual.index('--management-listen') + 1] == '127.0.0.1:19181'
assert hashlib.sha256(proc.joinpath('exe').read_bytes()).hexdigest() == artifact['binary_sha256']
assert str(guard) in proc.joinpath('maps').read_text()
environment = dict(row.split('=', 1) for row in proc.joinpath('environ').read_bytes().decode().split('\0') if '=' in row)
assert environment['LD_PRELOAD'] == str(guard) and environment['CPAR_BUILD_EGRESS_AUDIT'] == str(audit)
assert not any(environment.get(key) for key in ['HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'http_proxy', 'https_proxy', 'all_proxy'])
status = dict(line.split(':', 1) for line in proc.joinpath('status').read_text().splitlines() if ':' in line)
assert int(status['CapEff'].strip(), 16) == 4
started = time.monotonic()
while time.monotonic() - started < 65:
    assert pid == int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
    assert audit.read_bytes() == audit_before
    time.sleep(.5)
pools = get('/admin/operations/provider-account-pools?limit=100')['items']
assert len(pools) == 1 and pools[0]['runtime_status'] == 'available' and pools[0]['active_leases'] == 0
assert pools[0]['expires_at_ms'] > int(time.time() * 1000)
with sqlite3.connect('file:' + str(base / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    assert db.execute('SELECT count(*) FROM gateway_event_log').fetchone()[0] == 0
    assert db.execute('SELECT revision,enabled,auth_status,worker_claim_kind FROM grok_accounts').fetchall() == [(initial['native_revision'], 1, 'active', None)]
assert old_budget_file.read_bytes() == old_budget_bytes
assert before == production()
configuration = {**initial, 'pid': pid, 'actual_serve_arguments': actual,
                 'binary_sha256': artifact['binary_sha256'], 'product_source_sha': artifact['product_source_sha'],
                 'observed_runtime_credential_revision': pools[0]['credential_revision'],
                 'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                 'observed_worker_interval_seconds': time.monotonic() - started,
                 'automatic_provider_requests_since_restart': 0, 'initial_configuration_receipt_preserved': True}
(base / 'private/repair-configure-receipt.json').write_text(json.dumps(configuration, indent=2) + '\n')
budget = {**old_budget, 'schema_version': 2, 'pin_intents': 0, 'basic_intents': 0, 'stopped': False,
          'started_combinations': [], 'started_at_utc': configuration['observed_at_utc'],
          'historical_management_presend_rejections': 1, 'historical_provider_inference_requests': 0,
          'presend_resolution': resolution, 'provider_inference_limit': 60,
          'pin_intents_definition': 'reserved one-shot Provider Pin admissions in repaired epoch',
          'initial_budget_preserved_sha256': resolution['old_budget_sha256']}
with (base / 'private/batch-budget-v2.json').open('x') as output:
    json.dump(budget, output, indent=2)
receipt = {'status': 'PASS', 'configuration': configuration, 'presend_resolution': resolution,
           'production_before': before, 'production_after': production(), 'guard_mapped': True,
           'guard_environment_exact': True, 'effective_capabilities_hex': status['CapEff'].strip(),
           'retained_catalog_requests': 1, 'new_catalog_requests': 0, 'new_inference_requests': 0,
           'new_oauth_requests': 0, 'historical_budget_preserved': True}
(base / 'private/repair-restart-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt))
