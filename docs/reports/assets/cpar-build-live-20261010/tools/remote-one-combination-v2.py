import copy
import hashlib
import importlib.util
import json
import os
import pathlib
import sqlite3
import subprocess
import sys
import time
import urllib.error
import urllib.request

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
unit = 'cpar-build-acceptance-20261010.service'
protocol, mode = sys.argv[1:]
assert protocol in ['responses', 'chat', 'messages'] and mode in ['json', 'sse']
name = protocol + '-' + mode
configuration = json.loads((base / 'private/repair-configure-receipt.json').read_text())
catalog = json.loads((base / 'private/catalog-receipt.json').read_text())
assert catalog['status'] == 'PASS' and catalog['selected_model'] == configuration['public_model']
artifact = json.loads((base / 'private/candidate-artifact-f346a03f.json').read_text())
tool = base / 'private/cpar_new_build_acceptance.py'
assert hashlib.sha256(tool.read_bytes()).hexdigest() == 'fbb5df7bc4fa105eef805b9c6c566349f2ac0425ae8cec23a21b57c4b42b5a0f'
assert hashlib.sha256(tool.with_name('cpar-batch-c-live.py').read_bytes()).hexdigest() == '7d4187759fb86210d5791c37a397f385e4c3254c69bf1f95319bdd55ee5f992f'
spec = importlib.util.spec_from_file_location('formal', tool)
formal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(formal)
manifest = formal.load_json((base / 'private/artifact-manifest-f346a03f.json').read_text())
budget_path = base / 'private/batch-budget-v2.json'
budget = json.loads(budget_path.read_text())
assert not budget.get('stopped') and name not in budget.get('started_combinations', [])
private = base / 'private/phase2' / name
private.mkdir(mode=0o700)

def save_budget():
    temporary = budget_path.with_suffix('.tmp')
    temporary.write_text(json.dumps(budget, indent=2) + '\n')
    temporary.replace(budget_path)

def local_inspect():
    script = formal.REMOTE_READ.replace('SERVICE', repr(unit)).replace('INVENTORY', 'True')
    result = subprocess.run([sys.executable, '-c', script], capture_output=True, text=True, check=True, timeout=90)
    observed = formal.load_json(result.stdout)
    formal.bind_manifest(observed['runtime'], manifest)
    formal.verify_runtime(observed['runtime'], artifact['product_source_sha'], 'LIVE_NEW_BUILD')
    pid = observed['runtime']['pid']
    assert pid == configuration['pid'] and artifact['guard_path'] in pathlib.Path('/proc', str(pid), 'maps').read_text()
    assert hashlib.sha256(pathlib.Path(artifact['guard_path']).read_bytes()).hexdigest() == artifact['guard_sha256']
    events = [json.loads(line)['event'] for line in (base / 'audit/candidate-egress.jsonl').read_text().splitlines()]
    assert not any(event in ['dns_oauth_denied', 'dns_other_denied', 'connect_other_denied'] for event in events)
    # The exact original observation stays private. Nested native inventory is not
    # part of the formal dispatch gate and is removed from public runtime copies.
    (private / 'inspection-private.json').write_text(json.dumps(observed, indent=2) + '\n')
    for inventory in [observed['inventory'], observed['runtime']['acceptance_inventory']['inventory']]:
        for row in inventory.get('credentials', {}).get('items', []):
            row.pop('native', None)
    return observed

observed = local_inspect()
runtime = observed['runtime']
with sqlite3.connect('file:' + str(base / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    accounts = db.execute("SELECT id,revision,enabled,auth_status FROM grok_accounts WHERE provider='build'").fetchall()
assert len(accounts) == 1 and formal.opaque(accounts[0][0]) == configuration['account_evidence_id']
assert accounts[0][1:] == (configuration['native_revision'], 1, 'active')
approval = {'status': 'PENDING_RUNTIME_BINDING', 'basis': 'user_explicit', 'binding_sha256': None, 'max_inference_requests': 9, 'max_attempts': 1, 'allow_fallback': False, 'currency': 'USD', 'budget_limit': None, 'cost_bound': {'status': 'NOT_PROVEN', 'evidence_sha256': None, 'per_request_upper': None, 'covers': []}, 'cost_policy': 'UNLIMITED_USER_EXPLICIT', 'cost_authorization_evidence_sha256': 'e5b414ef84fcd1916ac804f67859da024eb9cb18ecaade23c189cc8957e19a8e'}
limit = {'responses': 'max_output_tokens', 'chat': 'max_completion_tokens', 'messages': 'max_tokens'}[protocol]
plan = {'schema_version': 2, 'basic_scenario_version': 3, 'layer': 'LIVE_NEW_BUILD', 'source_sha': artifact['product_source_sha'], 'target': {'channel': 'grok.build', 'endpoint_id': configuration['endpoint'], 'route_id': configuration['route'], 'route_candidate_id': configuration['route_candidate'], 'upstream_model': catalog['selected_model'], 'public_model': catalog['selected_model'], 'response_model': catalog['selected_model'], 'credential_evidence_id': configuration['account_evidence_id'], 'credential_revision': None, 'config_version_id': None, 'config_revision': None, 'egress_evidence_id': None, 'egress_revision': None, 'protocol': protocol, 'mode': mode, 'parameters': {limit: 4096}, 'capabilities': {key: None for key in formal.EXTENSIONS}, 'declaration_observed_at_utc': None, 'declaration_evidence': None}, 'authorization': approval, 'collector': {'host': 'new-vps', 'service': unit, 'sudo': True}, 'boundaries': {}, 'extensions': {}, 'observation_dir': None}
budget.setdefault('started_combinations', []).append(name)
assert budget['pin_intents'] < 6 and budget['pin_intents'] + budget['basic_intents'] < 60
budget['pin_intents'] += 1
save_budget()
management = (base / 'credentials/management-key').read_text().strip()
client = urllib.request.build_opener(urllib.request.ProxyHandler({}), formal.NoRedirect())
body = {'provider_id': configuration['provider'], 'channel_id': configuration['endpoint'], 'route_id': configuration['route'], 'credential_id': accounts[0][0], 'credential_revision': accounts[0][1], 'requested_model': catalog['selected_model'], 'protocol': {'responses': 'openai_responses', 'chat': 'openai_chat_completions', 'messages': 'anthropic_messages'}[protocol], 'mode': mode, 'client_key_id': 'cpar-build-client'}
headers = {'X-Management-Key': management, 'X-Config-Version': configuration['configuration_id'], 'If-Match': configuration['configuration_etag'], 'Content-Type': 'application/json'}
request = urllib.request.Request('http://127.0.0.1:19181/admin/operations/channel-pin', method='POST', headers=headers, data=json.dumps(body).encode())
try:
    response = client.open(request, timeout=90)
except urllib.error.HTTPError as error:
    response = error
with response:
    pin = json.load(response)
    pin_status = response.status
(private / 'pin-private.json').write_text(json.dumps(pin, indent=2) + '\n')
summary = {'combination': name, 'observed_at_utc': formal.now(), 'pin_http_status': pin_status, 'pin_outcome': pin.get('outcome'), 'pin_attempt_count': pin.get('attempt_count'), 'pin_upstream_sent': pin.get('upstream_sent'), 'account_evidence_id': configuration['account_evidence_id'], 'source_sha': artifact['product_source_sha'], 'guard_sha256': artifact['guard_sha256'], 'historical_presend_rejections': budget['historical_management_presend_rejections'], 'basic_sent_requests': 0, 'status': 'BLOCKED'}
if pin_status != 200 or pin.get('outcome') != 'succeeded':
    summary['category'] = pin.get('failure_category') or pin.get('error', {}).get('code') or 'pin_not_successful'
    budget['stopped'] = True
    save_budget()
else:
    # Read after Pin so its fresh serving projection, not an earlier Catalog or
    # inventory revision, supplies the frozen declaration and exact lease.
    observed = local_inspect()
    runtime = observed['runtime']
    try:
        plan = formal.freeze_from_pin(plan, runtime, pin)
        plan['authorization']['status'] = 'APPROVED'
        plan['authorization']['binding_sha256'] = formal.execution_binding(plan, runtime, 'http://127.0.0.1:19180/v1')
        formal.verify_execution_policy(plan, runtime, 'http://127.0.0.1:19180/v1')
        (private / 'frozen-plan.json').write_text(json.dumps(plan, indent=2) + '\n')
        (private / 'runtime.json').write_text(json.dumps(observed, indent=2) + '\n')
        original_http = formal.http_call
        combination_sent = [0]

        def controlled_http(*args, **kwargs):
            assert combination_sent[0] < 9 and budget['pin_intents'] + budget['basic_intents'] < 60
            combination_sent[0] += 1
            budget['basic_intents'] += 1
            save_budget()
            return original_http(*args, **kwargs)

        def local_collect(host, service, response_id, sudo=False):
            assert host == 'new-vps' and service == unit and sudo is True
            formal.require(isinstance(response_id, str) and len(response_id) <= 256, 'invalid_response_identity')
            # One fixed read-only settling interval for the existing 1s billing
            # worker, not a Provider request retry or repeated collection loop.
            time.sleep(2)
            script = formal.REMOTE_READ.replace('SERVICE', repr(unit)).replace('INVENTORY', 'False').rsplit('print(json.dumps(result))', 1)[0] + formal.REMOTE_COLLECT.replace('RESPONSE_ID', repr(response_id))
            result = subprocess.run([sys.executable, '-c', script], capture_output=True, text=True, check=True, timeout=30)
            return formal.load_json(result.stdout)

        def progress(bundle):
            (private / 'wire-partial.json').write_text(json.dumps(bundle, indent=2) + '\n')

        formal.http_call = controlled_http
        formal.collect_attempt = local_collect
        key = (base / 'private/client-key').read_text().strip()
        bundle = formal.run_basic(plan, runtime, key, 'http://127.0.0.1:19180/v1', progress)
        (private / 'wire-final.json').write_text(json.dumps(bundle, indent=2) + '\n')
        audit = formal.audit_bundle(bundle)
        formal.write_receipt(private / 'basic-receipt.json', audit)
        summary.update(status=audit['basic_status'], basic_sent_requests=audit['sent_requests'], audit_status=audit['status'], execution_failure=bundle.get('execution_failure'), target=plan['target'], binding_sha256=plan['authorization']['binding_sha256'], collector='unchanged REMOTE_READ + REMOTE_COLLECT executed locally on authorized host; one 2s settling delay')
        if summary['status'] != 'PASS' or bundle.get('execution_failure'):
            budget['stopped'] = True
            save_budget()
    except (ValueError, KeyError, TypeError, IndexError, OSError, subprocess.SubprocessError) as error:
        summary.update(formal.verdict(lambda: (_ for _ in ()).throw(error)) if isinstance(error, (ValueError, KeyError, TypeError, IndexError)) else {'status': 'BLOCKED', 'category': 'environment_or_transport_unavailable'})
        budget['stopped'] = True
        save_budget()
(private / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary))
