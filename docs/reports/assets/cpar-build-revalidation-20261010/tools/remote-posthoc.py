import base64
import copy
import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/revalidation-20261010'
unit = 'cpar-build-acceptance-20261010.service'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID'])) == 0
assert json.loads((epoch / 'budget.json').read_text())['stopped'] is True
source = base64.b64decode(FORMAL_SOURCE_B64, validate=True)
assert hashlib.sha256(source).hexdigest() == '2107227d545acb898ea6b036bdcef6ec5fc030e266b880d63f876b62746a9f48'
tool = epoch / 'posthoc-oracle-2107227d.py'
if tool.exists():
    assert tool.read_bytes() == source
else:
    with tool.open('xb') as output:
        output.write(source)
spec = importlib.util.spec_from_file_location('posthoc_formal', tool)
formal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(formal)

def forbidden(*args, **kwargs):
    raise RuntimeError('posthoc_network_forbidden')

formal.http_call = forbidden
formal.collect_attempt = forbidden
private = epoch / 'responses-json'
bundle = formal.load_json((private / 'wire-final.json').read_text())
working = copy.deepcopy(bundle)
sample = working['scenarios']['text_multi_turn'][0]
collected = formal.load_json((private / 'posthoc-collector-private.json').read_text())
sample.update(evidence=collected['evidence'], runtime=collected['runtime'])
answer, calls, native = formal.basic_answer(working['target'], sample, 'text_multi_turn', 0, 3, set())
attribution = formal.verify_attempts(working['target'], sample['evidence'], answer['id'])
formal.verify_usage_lineage(native, sample['evidence'])
runtime_check = formal.verdict(lambda: formal.verify_turn_runtime(sample['runtime'], working['runtime'], working['source_sha'], working['layer']))
if runtime_check['status'] == 'PASS':
    audit = formal.audit_bundle(working)
    assert audit['status'] == audit['basic_status'] == 'FAIL'
else:
    assert runtime_check == {'status': 'BLOCKED', 'category': 'runtime_observation_stale'}
    audit = {'status': 'BLOCKED', 'basic_status': 'BLOCKED', 'checks': []}
assert working['execution_failure'] == bundle['execution_failure']
assert sum(len(rows) for rows in working['scenarios'].values()) == 1
receipt = {
    'status': 'PARTIAL; received first-turn boundary PASS; full current-runtime audit BLOCKED', 'observed_at_utc': formal.now(),
    'networked_inference_requests': 0, 'management_requests': 0,
    'source_sha': working['source_sha'], 'runtime_binary_sha256': working['runtime']['artifact_sha256'],
    'original_executed_oracle_sha256': 'a46a2880e9614048411a25f594b0532c582e0a2ba29ab32d090068d269b53683',
    'posthoc_oracle_sha256': hashlib.sha256(source).hexdigest(),
    'first_turn_response_and_request_controls': 'PASS',
    'exact_execution_and_usage_lineage': 'PASS', 'attribution': attribution,
    'native_usage': native, 'tool_call_count': len(calls),
    'original_execution_failure': working['execution_failure'],
    'original_batch_status_preserved': 'FAIL',
    'offline_runtime_check': runtime_check, 'offline_full_audit_status': audit['status'],
    'completed_basic_scenarios': 0, 'observed_basic_responses': 1,
    'audit_checks': audit['checks'],
    'private_wire_sha256': hashlib.sha256((private / 'wire-final.json').read_bytes()).hexdigest(),
    'original_evidence_modified': False, 'temporary_main_pid': 0,
    'scope': 'received first turn only; no multi-turn history/tool/extension completion inferred'}
with (private / 'posthoc-receipt.json').open('x') as output:
    json.dump(receipt, output, indent=2)
print(json.dumps(receipt))
