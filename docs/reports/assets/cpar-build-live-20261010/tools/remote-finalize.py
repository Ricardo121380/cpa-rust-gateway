import ast
import datetime
import hashlib
import json
import os
import pathlib
import re
import sqlite3
import stat
import subprocess
import time
import urllib.request

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
unit = 'cpar-build-acceptance-20261010.service'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert stat.S_IMODE(base.stat().st_mode) == 0o700
assert not (base / 'private/final-receipt.json').exists()
configuration = json.loads((base / 'private/repair-configure-receipt.json').read_text())
artifact = json.loads((base / 'private/candidate-artifact-f346a03f.json').read_text())
budget_bytes = (base / 'private/batch-budget-v2.json').read_bytes()
budget = json.loads(budget_bytes)
old_budget_bytes = (base / 'private/batch-budget.json').read_bytes()
assert hashlib.sha256(old_budget_bytes).hexdigest() == budget['initial_budget_preserved_sha256']
assert budget['stopped'] is True
assert budget['catalog_intents'] == 1 and budget['pin_intents'] == 1 and budget['basic_intents'] == 0
assert budget['started_combinations'] == ['responses-json']
pin_bytes = (base / 'private/phase2/responses-json/pin-private.json').read_bytes()
pin = json.loads(pin_bytes)
assert pin['outcome'] == 'failed' and pin['attempt_count'] == 1 and pin['upstream_sent'] is True
assert pin['response_started'] is False and pin['stage'] == 'http_transport'
opaque = lambda value: hashlib.sha256(('cpar-evidence:' + str(value)).encode()).hexdigest()
assert opaque(pin['credential_id']) == configuration['account_evidence_id']
assert pin['runtime_build'].split(':')[0] == artifact['product_source_sha']
execution = pin['execution']
assert execution['route_candidate_id'] == configuration['route_candidate']
assert execution['upstream_model'] == configuration['public_model']
identity = execution['identity']
assert identity['channel'] == 'grok.build' and identity['config_version_id'] == configuration['configuration_id'] and identity['config_revision'] == 9
assert identity['credential_revision'] == pin['runtime_credential_revision'] == 134
assert pin['credential_revision'] == configuration['native_revision'] == 133
assert identity['egress'] == {'kind': 'direct'}

source = ast.parse((base / 'private/bootstrap.py').read_text())
functions = ast.Module(body=[node for node in source.body if isinstance(node, ast.FunctionDef) and node.name == 'production'], type_ignores=[])
exec(compile(functions, 'production-proof', 'exec'))
baseline = json.loads((base / 'private/bootstrap-receipt.json').read_text())['production_after']
before = production()
assert before == baseline

client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
def active_configuration(service):
    pid = int(subprocess.check_output(['systemctl', 'show', service, '--value', '--property=MainPID']))
    args = pathlib.Path('/proc', str(pid), 'cmdline').read_bytes().decode().rstrip('\0').split('\0')
    key = (pathlib.Path(args[args.index('--credential-dir') + 1]) / 'management-key').read_text().strip()
    listener = args[args.index('--management-listen') + 1]
    assert listener.startswith('127.0.0.1:')
    with client.open(urllib.request.Request('http://' + listener + '/admin/config-versions', headers={'X-Management-Key': key}), timeout=10) as response:
        assert response.status == 200
        rows = [v for v in json.load(response) if v['status'] == 'active']
    assert len(rows) == 1
    return {k: rows[0][k] for k in ['id', 'revision', 'status']}

candidate_config = active_configuration(unit)
production_config = active_configuration('cpa-rust-gateway.service')
assert candidate_config == {'id': configuration['configuration_id'], 'revision': 'rev-9', 'status': 'active'}
assert production_config == {'id': 'kimi-binding-a65aff19-302e-471c-846c-2ed3a6a4af75', 'revision': 'rev-1', 'status': 'active'}
pid = int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
assert pid == configuration['pid']
assert hashlib.sha256(pathlib.Path('/proc', str(pid), 'exe').read_bytes()).hexdigest() == artifact['binary_sha256']
assert artifact['guard_path'] in pathlib.Path('/proc', str(pid), 'maps').read_text()

log_bytes = (base / 'private/gateway.log').read_bytes()
log = log_bytes.decode(errors='replace')
rejections = []
for line in log.splitlines():
    if 'grok_build_http_rejection' not in line:
        continue
    try:
        row = json.loads(line)
    except ValueError:
        line = re.sub(r'\x1b\[[0-9;]*m', '', line)
        fields = {'request_id': pin['request_id'] if pin['request_id'] in line else None}
        match = re.search(r'upstream_status\s*=\s*(\d{3})', line)
        if match:
            fields['upstream_status'] = int(match.group(1))
        for name in ['body_kind', 'error_code', 'error_type', 'error_param', 'signal']:
            match = re.search(name + r'\s*=\s*"?([A-Za-z0-9_.-]{1,80})', line)
            if match:
                fields[name] = match.group(1)
    else:
        fields = row.get('fields', row)
    if fields.get('request_id') != pin['request_id']:
        continue
    item = {'event': 'grok_build_http_rejection', 'request_evidence_id': opaque(pin['request_id'])}
    for name in ['upstream_status', 'body_kind', 'error_code', 'error_type', 'error_param', 'signal']:
        value = fields.get(name)
        if name == 'upstream_status' and type(value) is int and 100 <= value <= 599:
            item[name] = value
        elif isinstance(value, str) and re.fullmatch(r'[A-Za-z0-9_.-]{1,80}', value):
            item[name] = value
    rejections.append(item)

with sqlite3.connect('file:' + str(base / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    event_counts = dict(db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type'))
    action_counts = dict(db.execute("SELECT action,count(*) FROM management_resource_audit_events WHERE action LIKE 'channel_pin%' GROUP BY action"))
    account = db.execute("SELECT revision,enabled,auth_status,worker_claim_kind FROM grok_accounts WHERE provider='build'").fetchall()
assert not event_counts and action_counts == {'channel_pin_requested': 2, 'channel_pin_started': 2}
assert account == [(133, 1, 'active', None)]
audit_path = base / 'audit/candidate-egress.jsonl'
audit_bytes = audit_path.read_bytes()
audit_counts = {}
for line in audit_bytes.splitlines():
    event = json.loads(line)['event']
    audit_counts[event] = audit_counts.get(event, 0) + 1
assert audit_counts == {'dns_build_allowed': 2, 'connect_build_allowed': 2}

subprocess.run(['systemctl', 'stop', unit], check=True)
for _ in range(100):
    if int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID'])) == 0:
        break
    time.sleep(.1)
else:
    raise AssertionError('owned candidate did not stop')
assert not pathlib.Path('/proc', str(pid)).exists()
listeners = []
for family in ['tcp', 'tcp6']:
    for line in pathlib.Path('/proc/net', family).read_text().splitlines()[1:]:
        values = line.split()
        if values[3] == '0A' and int(values[1].rsplit(':', 1)[1], 16) in [19180, 19181]:
            listeners.append(family)
assert not listeners
time.sleep(1)
assert audit_path.read_bytes() == audit_bytes
assert (base / 'private/batch-budget-v2.json').read_bytes() == budget_bytes
assert (base / 'private/batch-budget.json').read_bytes() == old_budget_bytes
after = production()
assert after == before == baseline
assert active_configuration('cpa-rust-gateway.service') == production_config

protected_files = ['private/phase2/responses-json/pin-private.json', 'private/batch-budget.json', 'private/batch-budget-v2.json', 'private/gateway.log', 'credentials/management-key', 'private/client-key']
assert all(stat.S_IMODE((base / name).stat().st_mode) == 0o600 for name in protected_files)
receipt = {
    'cleanup_status': 'PASS', 'real_acceptance_status': 'BLOCKED', 'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'candidate_service': unit, 'candidate_pid_before_stop': pid, 'candidate_pid_after_stop': 0, 'candidate_ports_absent': True,
    'product_source_sha': artifact['product_source_sha'], 'binary_sha256': artifact['binary_sha256'], 'guard_sha256': artifact['guard_sha256'],
    'production_before': before, 'production_after': after, 'production_active_configuration': production_config, 'candidate_active_configuration': candidate_config,
    'pin': {**{k: pin[k] for k in ['outcome', 'attempt_count', 'upstream_sent', 'response_started', 'stage', 'observed_at_ms', 'runtime_build', 'credential_revision', 'runtime_credential_revision', 'requested_model', 'protocol', 'mode']},
            'request_evidence_id': opaque(pin['request_id']), 'credential_evidence_id': opaque(pin['credential_id']),
            'client_key_evidence_id': opaque(pin['client_key_id']), 'execution': {'attempt_evidence_id': opaque(execution['attempt_id']), 'route_candidate_id': execution['route_candidate_id'], 'upstream_model': execution['upstream_model'], 'identity': identity}},
    'closed_provider_rejections': rejections, 'private_pin_sha256': hashlib.sha256(pin_bytes).hexdigest(), 'private_log_sha256': hashlib.sha256(log_bytes).hexdigest(),
    'catalog_get_requests': 1, 'provider_pin_admission_intents': 1, 'protected_pin_attempts': 1, 'basic_inference_requests': 0,
    'historical_management_presend_rejections': 1, 'historical_provider_inference_requests': 0, 'remaining_combinations_not_run': ['responses-sse', 'chat-json', 'chat-sse', 'messages-json', 'messages-sse'],
    'budget_stopped': True, 'old_budget_preserved_sha256': hashlib.sha256(old_budget_bytes).hexdigest(), 'repaired_budget_sha256': hashlib.sha256(budget_bytes).hexdigest(),
    'egress_audit_counts': audit_counts, 'egress_audit_sha256': hashlib.sha256(audit_bytes).hexdigest(), 'server_http_receive_count_claim': False,
    'retries': 0, 'fallbacks': 0, 'extensions': 0, 'reruns': 0, 'oauth': 0, 'automatic_catalog': 0, 'quota_queries': 0,
    'gateway_event_counts': event_counts, 'channel_pin_resource_actions': action_counts, 'basic_usage_and_ledger_qualification': 'NOT_RUN',
    'native_account_state_unchanged': True, 'private_material_retained': True, 'private_material_mode_check': 'PASS', 'production_writes_by_task': 0,
    'billing_amount': {'status': 'NOT_PROVEN', 'cost_policy': 'UNLIMITED_USER_EXPLICIT', 'amount_claimed': None},
}
(base / 'private/final-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt))
