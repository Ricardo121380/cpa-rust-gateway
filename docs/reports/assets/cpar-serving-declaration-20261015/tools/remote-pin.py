import datetime
import fcntl
import hashlib
import importlib.util
import http.client
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
epoch = base / 'private/chat-json-serving-20261015'
unit = 'cpar-build-chat-json-serving-20261015.service'
assert sys.argv[1:] == []
protocol, mode = 'chat', 'json'
name = protocol + '-' + mode
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
lock = (epoch / 'execution.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
budget_path = epoch / 'budget.json'
budget = json.loads(budget_path.read_text())
assert not budget['stopped'] and budget['new_pin_intents'] == budget['new_basic_intents'] == 0
freeze = json.loads((epoch / 'freeze.json').read_text())
private = epoch / name
private.mkdir(mode=0o700)
start = json.loads((epoch / 'start-receipt.json').read_text())
tool = epoch / 'cpar_new_build_acceptance.py'
assert hashlib.sha256(tool.read_bytes()).hexdigest() == freeze['files']['cpar_new_build_acceptance.py']
spec = importlib.util.spec_from_file_location('formal', tool)
formal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(formal)
manifest = formal.load_json((epoch / 'artifact-manifest.json').read_text())
assert hashlib.sha256((epoch / 'user-authorization.json').read_bytes()).hexdigest() == freeze['files']['user-authorization.json']
sys.path.insert(0,str(epoch))
import cpar_build_json_diagnostic as diagnostic
diagnostic.validate_budget(budget)
approval_receipt = json.loads((epoch / 'user-authorization.json').read_text())
assert approval_receipt['max_new_inference_intents'] == 2 and approval_receipt['max_new_pin_intents'] == approval_receipt['max_new_chat_intents'] == 1
assert hashlib.sha256((epoch / 'historical-cost-authorization.json').read_bytes()).hexdigest() == '77980e82623fa9913bd0623803f1a0f6b33857335127ad96fde5064f0ebaaa0f'
summary = {'combination': name, 'status': 'BLOCKED', 'new_pin_intent': 0,
           'historical_inference_requests': 35, 'basic_sent_requests': 0,
           'source_sha': start['source_sha'], 'binary_sha256': start['binary_sha256']}

def save_budget():
    diagnostic.save_budget(budget_path,budget)

def inspect():
    for name, checksum in freeze['files'].items():
        assert hashlib.sha256((base / 'bin/chat-json-serving-20261015' / name).read_bytes()).hexdigest() == checksum
        if name not in ['gateway-aarch64-unknown-linux-gnu', 'artifact-manifest.json', 'gateway-build-metadata.json']:
            assert hashlib.sha256((epoch / name).read_bytes()).hexdigest() == checksum
    script = formal.REMOTE_READ.replace('SERVICE', repr(unit)).replace('INVENTORY', 'True')
    script = script.replace('    def get(path):', '    management_get_count = [0]\n    def get(path):\n        management_get_count[0] += 1')
    script = script.replace('print(json.dumps(result))', "result['management_get_count'] = management_get_count[0]\nprint(json.dumps(result))")
    result = subprocess.run([sys.executable, '-c', script], capture_output=True, text=True, check=True, timeout=90)
    observed = formal.load_json(result.stdout)
    formal.bind_manifest(observed['runtime'], manifest)
    formal.verify_runtime(observed['runtime'], start['source_sha'], 'LIVE_NEW_BUILD')
    runtime = observed['runtime']
    assert runtime['pid'] == start['pid'] and runtime['process_start'] == start['process_start']
    assert runtime['artifact_sha256'] == start['binary_sha256']
    proc = pathlib.Path('/proc', str(start['pid']))
    assert str(base / 'bin/egress-guard-tcp.so') in (proc / 'maps').read_text()
    args = (proc / 'cmdline').read_bytes().decode().rstrip('\0').split('\0')
    assert args[args.index('--background-provider-tasks') + 1] == 'disabled'
    for line in (base / 'audit/chat-json-serving-20261015.jsonl').read_bytes().splitlines() if (base / 'audit/chat-json-serving-20261015.jsonl').exists() else []:
        assert json.loads(line)['event'] in ['dns_build_allowed', 'connect_build_allowed']
    # Full inventory and native identities stay in the protected epoch.
    (private / ('inspection-' + str(summary.get('new_pin_intent', 0)) + '-private.json')).write_text(json.dumps(observed, indent=2) + '\n')
    summary['management_get_requests'] = summary.get('management_get_requests', 0) + observed['management_get_count']
    return observed

try:
    observed = inspect()
    runtime = observed['runtime']
    with sqlite3.connect('file:' + str(epoch / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
        account = db.execute("SELECT id,revision,enabled,auth_status,worker_claim_kind FROM grok_accounts").fetchall()
    assert len(account) == 1 and formal.opaque(account[0][0]) == start['account_evidence_id']
    assert account[0][1:] == (start['native_revision'], 1, 'active', None)
    pools = runtime['acceptance_inventory']['inventory']['native_pools']['items']
    assert len(pools) == 1 and pools[0]['active_leases'] == 0
    assert pools[0]['expires_at_ms'] > int(time.time() * 1000) + 600000
    assert pools[0]['credential_revision'] == start['runtime_projection_revision']
    formal.verify_capability_collection(runtime, formal.opaque('cpar-build-client'))
    declarations = runtime['capability_observations']
    assert len(declarations) == 1
    declaration = declarations[0]
    assert declaration['source'] == 'serving_snapshot'
    assert declaration['client_key_evidence_id'] == formal.opaque('cpar-build-client')
    assert declaration['config_version_id'] == 'cpar-build-config-20261010' and declaration['config_revision'] == 9
    assert declaration['route_id'] == 'cpar-build-route' and declaration['route_candidate_id'] == 'cpar-build-candidate'
    assert declaration['endpoint_id'] == 'cpar-build-endpoint' and declaration['upstream_model'] == 'grok-4.5'
    assert declaration['capabilities'] == {'reasoning': True, 'parallel': False, 'stored': True, 'continuation': True, 'compact': True, 'websocket': True}
    limit = {'responses': 'max_output_tokens', 'chat': 'max_completion_tokens', 'messages': 'max_tokens'}[protocol]
    target = {'channel': 'grok.build', 'endpoint_id': 'cpar-build-endpoint', 'route_id': 'cpar-build-route',
              'route_candidate_id': 'cpar-build-candidate', 'upstream_model': 'grok-4.5', 'public_model': 'grok-4.5',
              'response_model': 'grok-4.5', 'credential_evidence_id': start['account_evidence_id'],
              'credential_revision': pools[0]['credential_revision'], 'config_version_id': 'cpar-build-config-20261010', 'config_revision': 9,
              'egress_evidence_id': formal.opaque('direct'), 'egress_revision': {'source': 'not_applicable'},
              'protocol': protocol, 'mode': mode, 'parameters': {limit: 4096, **({'stream_options': {'include_usage': True}} if protocol == 'chat' and mode == 'sse' else {})},
              'capabilities': declaration['capabilities'], 'declaration_evidence': declaration,
              'declaration_observed_at_utc': declaration['observed_at_utc']}
    authorization = {'status': 'APPROVED', 'basis': 'user_explicit', 'max_inference_requests': 2,
                     'max_attempts': 1, 'allow_fallback': False, 'currency': 'USD', 'budget_limit': None,
                     'cost_policy': 'UNLIMITED_USER_EXPLICIT',
                     'cost_authorization_evidence_sha256': '77980e82623fa9913bd0623803f1a0f6b33857335127ad96fde5064f0ebaaa0f',
                     'cost_bound': {'status': 'NOT_PROVEN'}}
    plan = {'schema_version': 1, 'kind': 'chat_json_502_diagnostic', 'layer': 'LIVE_NEW_BUILD',
            'source_sha': start['source_sha'], 'target': target, 'epoch': str(epoch),
            'public_request_sha256': json.loads((epoch / 'prepare-receipt.json').read_text())['selected_request_sha256'], 'authorization': authorization,
            'collector': {'host': 'new-vps', 'service': unit, 'sudo': True}, 'boundaries': {}, 'extensions': {}}
    authorization['binding_sha256'] = diagnostic.binding(plan, runtime, 'http://127.0.0.1:19180/v1')
    formal.verify_target(target)
    formal.verify_current_declaration(target, runtime)
    diagnostic.verify_qualification(formal, plan, runtime, 'http://127.0.0.1:19180/v1')
    summary['presend_target'] = target
    summary['presend_runtime'] = {key: runtime[key] for key in ['pid', 'process_start', 'artifact_sha256', 'artifact_sha', 'manifest_sha256']}
    assert all(hashlib.sha256((base / path).read_bytes()).hexdigest() == value for path, value in start['historical_file_hashes'].items())
    # Reserve and latch before transport. A crash or unknown outcome cannot reopen the batch.
    diagnostic.reserve_pin(budget)
    save_budget()
    summary['new_pin_intent'] = 1
    body = {'provider_id': 'cpar-build-provider', 'channel_id': 'cpar-build-endpoint', 'route_id': 'cpar-build-route',
            'credential_id': account[0][0], 'credential_revision': start['native_revision'], 'requested_model': 'grok-4.5',
            'protocol': {'responses': 'openai_responses', 'chat': 'openai_chat_completions', 'messages': 'anthropic_messages'}[protocol],
            'mode': mode, 'client_key_id': 'cpar-build-client'}
    headers = {'X-Management-Key': (epoch / 'credentials/management-key').read_text().strip(),
               'X-Config-Version': 'cpar-build-config-20261010', 'If-Match': '"rev-9"', 'Content-Type': 'application/json'}
    client = urllib.request.build_opener(urllib.request.ProxyHandler({}), formal.NoRedirect())
    (private / 'pin-request-private.json').write_text(json.dumps(body, indent=2) + '\n')
    request = urllib.request.Request('http://127.0.0.1:19181/admin/operations/channel-pin', method='POST', headers=headers, data=json.dumps(body).encode())
    try:
        response = client.open(request, timeout=90)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        raw, deadline = bytearray(), time.monotonic() + 90
        wire_response = response.fp if isinstance(response, urllib.error.HTTPError) else response
        while len(raw) <= 1048576:
            if wire_response.fp is None:
                break
            assert time.monotonic() < deadline
            wire_response.fp.raw._sock.settimeout(min(15, deadline - time.monotonic()))
            try:
                part = wire_response.read1(min(65536, 1048577 - len(raw)))
            except http.client.IncompleteRead as error:
                raw.extend(error.partial)
                (private / 'pin-response.raw').write_bytes(raw)
                raise
            if not part:
                break
            raw.extend(part)
            (private / 'pin-response.raw').write_bytes(raw)
        raw = bytes(raw)
        (private / 'pin-response.raw').write_bytes(raw)
        assert len(raw) <= 1048576
        pin = formal.load_json(raw.decode())
        summary['management_pin_http_status'] = response.status
    (private / 'pin-private.json').write_text(json.dumps(pin, indent=2) + '\n')
    summary.update(pin_outcome=pin.get('outcome'), pin_attempt_count=pin.get('attempt_count'),
                   pin_upstream_sent=pin.get('upstream_sent'), pin_stage=pin.get('stage'),
                   pin_response_started=pin.get('response_started'), private_pin_sha256=hashlib.sha256(raw).hexdigest())
    assert summary['management_pin_http_status'] == 200 and pin.get('outcome') == 'succeeded'
    observed = inspect()
    frozen = formal.freeze_from_pin(plan, observed['runtime'], pin)
    # Freeze rechecks every identity and the scoped declaration. Its fresh observation
    # time is part of the binding, so bind this exact reviewed target before admission.
    frozen['authorization']['binding_sha256'] = diagnostic.binding(frozen, observed['runtime'], 'http://127.0.0.1:19180/v1')
    diagnostic.verify_qualification(formal, frozen, observed['runtime'], 'http://127.0.0.1:19180/v1')
    (private / 'frozen-plan.json').write_text(json.dumps(frozen, indent=2) + '\n')
    (private / 'runtime.json').write_text(json.dumps(observed, indent=2) + '\n')
    assert not any((epoch / 'native-capture').iterdir())
    diagnostic.finish_pin(budget,True)
    summary['status'] = 'PASS'
except BaseException as error:
    budget['stopped'] = True
    summary['category'] = str(error) if isinstance(error, (formal.Blocked, ValueError)) and str(error).replace('_', '').isalnum() else type(error).__name__
    if summary.get('pin_outcome') == 'failed':
        summary['status'] = 'FAIL'
    if budget['new_pin_intents'] == 1 and budget['pin_status'] == 'UNKNOWN':
        diagnostic.finish_pin(budget,False)
finally:
    save_budget()
    summary['observed_at_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    summary['new_basic_intents'] = budget['new_basic_intents']
    (private / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary))
