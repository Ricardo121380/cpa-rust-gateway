"""Collect the already received fifth response; never send another inference."""
import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess
import sys

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/six-combinations-20261010'
private = epoch / 'responses-json'
assert json.loads((epoch / 'budget.json').read_text())['stopped'] is True
tool = epoch / 'cpar_new_build_acceptance.py'
assert hashlib.sha256(tool.read_bytes()).hexdigest() == '2107227d545acb898ea6b036bdcef6ec5fc030e266b880d63f876b62746a9f48'
spec = importlib.util.spec_from_file_location('formal', tool)
formal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(formal)
bundle = formal.load_json((private / 'wire-final.json').read_text())
samples = bundle['scenarios']['single_tool']
assert len(samples) == 2 and bundle['execution_failure']['category'] == 'history_or_result_not_used'
last = samples[-1]
value = formal.load_json(last['wire'])
script = formal.REMOTE_READ.replace('SERVICE', repr('cpar-build-acceptance-20261010.service')).replace('INVENTORY', 'False').rsplit('print(json.dumps(result))', 1)[0]
script += formal.REMOTE_COLLECT.replace('RESPONSE_ID', repr(value['id']))
collected = subprocess.run([sys.executable, '-c', script], capture_output=True, text=True, check=True, timeout=30)
observed = formal.load_json(collected.stdout)
(private / 'fifth-collector-private.json').write_text(json.dumps(observed, indent=2) + '\n')
formal.verify_turn_runtime(observed['runtime'], bundle['runtime'], bundle['source_sha'], bundle['layer'])
attribution = formal.verify_attempts(bundle['target'], observed['evidence'], value['id'])
usage = formal.native_usage('responses', value)
formal.verify_usage_lineage(usage, observed['evidence'])
text, calls = formal.observe('responses', value)
previous = formal.load_json(samples[0]['wire'])
_, previous_calls = formal.observe('responses', previous)
expected = last['expected_text']
outputs = [item for item in last['request']['input'] if item.get('type') == 'function_call_output']
formal.check_history('responses', samples[0]['request'], last['request'], previous, previous_calls, samples[0]['next_token'])
assert len(previous_calls) == len(outputs) == 1
receipt = {'status': 'PASS; read-only fifth-response attribution and diagnostic',
           'observed_at_utc': formal.now(), 'original_execution_failure': bundle['execution_failure'],
           'http_status': last['http_status'], 'response_status': value.get('status'),
           'error_is_null': value.get('error') is None, 'incomplete_is_null': value.get('incomplete_details') is None,
           'actual_target': bundle['target'], 'attribution': attribution, 'native_usage': usage,
           'posthoc_runtime': observed['runtime'], 'completed_text_bytes': len(text.encode()),
           'completed_text_sha256': hashlib.sha256(text.encode()).hexdigest(),
           'expected_token_present': expected in text, 'final_tool_calls': len(calls),
           'first_tool_name': previous_calls[0][1], 'first_tool_arguments': previous_calls[0][2],
           'native_assistant_items_retained': all(item in last['request']['input'] for item in previous['output']),
           'call_id_matched': outputs[0]['call_id'] == previous_calls[0][0],
           'result_equals_expected_token': outputs[0]['output'] == expected,
           'result_equals_documented_lookup_value': outputs[0]['output'] == str(previous_calls[0][2]['value']),
           'initial_prompt_requests_exact_result_replay': 'exactly' in samples[0]['request']['input'].lower(),
           'followup_has_user_replay_instruction': any(item.get('role') == 'user' and 'exactly' in str(item.get('content', '')).lower() for item in last['request']['input']),
           'history_shape_check': 'PASS', 'provider_inference_in_this_script': 0,
           'private_wire_sha256': hashlib.sha256((private / 'wire-final.json').read_bytes()).hexdigest(),
           'private_fifth_collection_sha256': hashlib.sha256(collected.stdout.encode()).hexdigest()}
with (epoch / 'posthoc-receipt.json').open('x') as output:
    json.dump(receipt, output, indent=2)
print(json.dumps(receipt))
