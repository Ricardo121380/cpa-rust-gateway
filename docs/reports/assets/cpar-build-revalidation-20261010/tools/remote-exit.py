import ast
import datetime
import fcntl
import hashlib
import importlib.util
import json
import os
import pathlib
import sqlite3
import stat
import subprocess
import sys
import time
import urllib.request

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/revalidation-20261010'
unit = 'cpar-build-acceptance-20261010.service'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert stat.S_IMODE(base.stat().st_mode) == 0o700
assert not (epoch / 'final-receipt.json').exists()
lock = (epoch / 'execution.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
start = json.loads((epoch / 'start-receipt.json').read_text())
pid = int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
proc = pathlib.Path('/proc', str(pid))
assert pid == start['pid']
assert (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19] == start['process_start']
assert digest(proc / 'exe') == start['binary_sha256']
budget = json.loads((epoch / 'budget.json').read_text())
assert budget['stopped'] and budget['new_pin_intents'] == 1 and budget['new_basic_intents'] == 1
tool = epoch / 'cpar_new_build_acceptance.py'
assert digest(tool) == 'a46a2880e9614048411a25f594b0532c582e0a2ba29ab32d090068d269b53683'
spec = importlib.util.spec_from_file_location('formal', tool)
formal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(formal)
receipt = {'observed_at_utc': formal.now(), 'source_sha': start['source_sha'],
           'binary_sha256': start['binary_sha256'], 'process_pid_before_stop': pid,
           'new_pin_intents': 1, 'new_basic_intents': 1, 'new_catalog_intents': 0,
           'networked_inference_in_this_exit': 0, 'management_get_requests': 0}
client = urllib.request.build_opener(urllib.request.ProxyHandler({}))

def get(port, credentials, path):
    receipt['management_get_requests'] += 1
    headers = {'X-Management-Key': (credentials / 'management-key').read_text().strip()}
    with client.open(urllib.request.Request('http://127.0.0.1:' + str(port) + path, headers=headers), timeout=3) as response:
        assert response.status == 200
        return json.load(response)

try:
    private = epoch / 'responses-json'
    bundle = formal.load_json((private / 'wire-final.json').read_text())
    samples = bundle['scenarios']['text_multi_turn']
    assert len(samples) == 1 and samples[0]['http_status'] == 200
    value = formal.load_json(samples[0]['wire'])
    response_id = value['id']
    script = formal.REMOTE_READ.replace('SERVICE', repr(unit)).replace('INVENTORY', 'False').rsplit('print(json.dumps(result))', 1)[0]
    script += formal.REMOTE_COLLECT.replace('RESPONSE_ID', repr(response_id))
    result = subprocess.run([sys.executable, '-c', script], capture_output=True, text=True, check=True, timeout=30)
    with (private / 'posthoc-collector-private.json').open('x') as output:
        output.write(result.stdout)
    collected = formal.load_json(result.stdout)
    receipt['posthoc_collector_sha256'] = digest(private / 'posthoc-collector-private.json')
    receipt['first_response'] = {
        'http_status': samples[0]['http_status'], 'status': value.get('status'),
        'error_field_present': 'error' in value, 'error_field_is_null': value.get('error') is None,
        'incomplete_details_is_null': value.get('incomplete_details') is None,
        'response_evidence_id': formal.opaque(response_id),
        'output_types': [item.get('type') for item in value.get('output', [])]}
    formal.verify_turn_runtime(collected['runtime'], bundle['runtime'], bundle['source_sha'], bundle['layer'])
    attribution = formal.verify_attempts(bundle['target'], collected['evidence'], response_id)
    native = formal.native_usage('responses', value)
    formal.verify_usage_lineage(native, collected['evidence'])
    receipt['first_response'].update(attribution=attribution, native_usage=native,
                                      exact_execution_and_usage_lineage='PASS',
                                      protocol='openai_responses', stream=False,
                                      target={key: formal.public_identity(key, val) for key, val in bundle['target'].items() if key not in ['declaration_evidence', 'declaration_observed_at_utc']})
    pools = get(19181, base / 'credentials', '/admin/operations/provider-account-pools?limit=100')['items']
    assert len(pools) == 1 and pools[0]['active_leases'] == 0
    receipt['active_leases_before_stop'] = 0
except BaseException as error:
    receipt['posthoc_status'] = 'BLOCKED'
    receipt['posthoc_category'] = str(error) if isinstance(error, ValueError) and str(error).replace('_', '').isalnum() else type(error).__name__
finally:
    subprocess.run(['systemctl', 'stop', unit], check=True, capture_output=True, timeout=30)

receipt['main_pid_after_stop'] = int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
assert receipt['main_pid_after_stop'] == 0 and not proc.exists()
listeners = []
for family in ['tcp', 'tcp6']:
    for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:]:
        parts = row.split()
        if parts[3] == '0A' and int(parts[1].rsplit(':', 1)[1], 16) in [19180, 19181]:
            listeners.append(parts[1])
assert not listeners
receipt['temporary_ports_listening'] = listeners
audit = base / 'audit/revalidation-20261010.jsonl'
audit_hash = digest(audit)
time.sleep(5)
assert digest(audit) == audit_hash
receipt['network_audit_sha256'] = audit_hash
receipt['network_audit_events'] = [json.loads(line)['event'] for line in audit.read_bytes().splitlines()]
assert all(event in ['dns_build_allowed', 'connect_build_allowed'] for event in receipt['network_audit_events'])
receipt['post_stop_flat_seconds'] = 5
receipt['temporary_instance_stopped'] = True
receipt['historical_file_hashes'] = {path: digest(base / path) for path in start['historical_file_hashes']}
assert receipt['historical_file_hashes'] == start['historical_file_hashes']
source = ast.parse((base / 'private/bootstrap.py').read_text())
functions = ast.Module(body=[node for node in source.body if isinstance(node, ast.FunctionDef) and node.name == 'production'], type_ignores=[])
exec(compile(functions, 'production-proof', 'exec'))
receipt['production_after'] = production()
assert receipt['production_after'] == start['production_before']
source_proc = pathlib.Path('/proc', receipt['production_after']['MainPID'])
source_args = (source_proc / 'cmdline').read_bytes().decode().split('\0')
source_credentials = pathlib.Path(source_args[source_args.index('--credential-dir') + 1])
active = [row for row in get(18181, source_credentials, '/admin/config-versions') if row['status'] == 'active']
receipt['production_active_configuration'] = [{key: row[key] for key in ['id', 'revision', 'status']} for row in active]
assert receipt['production_active_configuration'] == start['production_active_configuration']
with sqlite3.connect('file:' + str(base / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    db.row_factory = sqlite3.Row
    receipt['durable_event_counts'] = dict(db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type'))
    attempts = [json.loads(row[0])['attempt'] for row in db.execute("SELECT payload_json FROM gateway_event_log WHERE event_type='attempt'")]
    receipt['basic_attempt_count'] = len(attempts)
    receipt['basic_outcomes'] = [row.get('outcome') for row in attempts]
    receipt['basic_retry_decisions'] = [row.get('retry_decision') for row in attempts]
    receipt['billing'] = [dict(row) for row in db.execute('SELECT cost_microunits,cost_confidence,billing_status FROM billing_ledger_entries')]
    account = dict(db.execute('SELECT * FROM grok_accounts').fetchone())
    receipt['account_native_revision_after'] = account['revision']
    receipt['worker_claim_after'] = account['worker_claim_kind']
    assert account['revision'] == 133 and account['worker_claim_kind'] is None
    receipt['management_resource_audit_count'] = db.execute('SELECT count(*) FROM management_resource_audit_events').fetchone()[0]
with sqlite3.connect('file:/var/lib/cpa-rust-gateway/control.sqlite3?mode=ro', uri=True) as db:
    db.row_factory = sqlite3.Row
    original = dict(db.execute('SELECT * FROM grok_accounts WHERE id=?', (account['id'],)).fetchone())
    assert all(account.get(name) == val for name, val in original.items() if name != 'import_batch_id')
receipt['source_account_still_matches'] = True
receipt['new_pin_actual_sends'] = 1
receipt['new_basic_actual_sends'] = receipt['basic_attempt_count']
receipt['provider_http_receive_count'] = 'UNKNOWN; no independent upstream receive counter'
receipt['status'] = 'PASS' if receipt.get('posthoc_status') != 'BLOCKED' else 'PARTIAL'
receipt['private_evidence_files'] = {str(path.relative_to(base)): {'sha256': digest(path), 'mode': oct(stat.S_IMODE(path.stat().st_mode)), 'bytes': path.stat().st_size} for path in epoch.rglob('*') if path.is_file() and path.name != 'execution.lock'}
assert all(row['mode'] == '0o600' for row in receipt['private_evidence_files'].values())
with (epoch / 'final-receipt.json').open('x') as output:
    json.dump(receipt, output, indent=2)
print(json.dumps(receipt))
