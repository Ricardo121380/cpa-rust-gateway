"""Formal entry/collector/audit with an owned no-replay budget and raw capture."""
import base64
import contextlib
import copy
import fcntl
import hashlib
import importlib.util
import io
import json
import os
import pathlib
import runpy
import sqlite3
import subprocess
import sys
import time

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/abd45431-six-20261011'
unit = 'cpar-build-acceptance-20261010.service'
protocol, mode = sys.argv[1:]
name = protocol + '-' + mode
order = ['chat-json', 'chat-sse', 'messages-json', 'messages-sse', 'responses-json', 'responses-sse']
assert name in order
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
lock = (epoch / 'execution.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
budget_path = epoch / 'budget.json'
budget = json.loads(budget_path.read_text())
assert not budget['stopped'] and budget['historical_responses_json_basic_passed']
assert budget['historical_inference_requests'] == 33 and budget['historical_gateway_pin_confirmed_sends'] == 32 and budget['cumulative_inference_limit'] == 93
assert budget['new_inference_limit'] == 60 and budget['new_pin_limit'] == 6 and budget['new_basic_limit'] == 54
assert budget['combinations'][name] == {'pin_status': 'PASS', 'basic_status': 'NOT_RUN'}
assert len(budget['combinations']) == order.index(name) + 1
assert all(budget['combinations'][previous] == {'pin_status': 'PASS', 'basic_status': 'PASS'} for previous in order[:order.index(name)])
private = epoch / name
assert not (private / 'basic-summary.json').exists()
tool = epoch / 'cpar_new_build_acceptance.py'
entry = epoch / 'cpar-batch-c-live.py'
assert hashlib.sha256(tool.read_bytes()).hexdigest() == '4ca87111f10858118e4a721699ad44e9f5abce2775f6d9423bd9a1427858f971'
assert hashlib.sha256(entry.read_bytes()).hexdigest() == '7d4187759fb86210d5791c37a397f385e4c3254c69bf1f95319bdd55ee5f992f'
spec = importlib.util.spec_from_file_location('cpar_new_build_acceptance', tool)
formal = importlib.util.module_from_spec(spec)
sys.modules['cpar_new_build_acceptance'] = formal
spec.loader.exec_module(formal)
plan = formal.load_json((private / 'frozen-plan.json').read_text())
runtime = formal.load_json((private / 'runtime.json').read_text())['runtime']
start = json.loads((epoch / 'start-receipt.json').read_text())
summary = {'combination': name, 'status': 'BLOCKED', 'basic_intents': 0,
           'source_sha': start['source_sha'], 'binary_sha256': start['binary_sha256'],
           'formal_tool_sha256': hashlib.sha256(tool.read_bytes()).hexdigest(),
           'formal_entry_sha256': hashlib.sha256(entry.read_bytes()).hexdigest()}

def save_budget():
    temporary = budget_path.with_suffix('.tmp')
    temporary.write_text(json.dumps(budget, indent=2) + '\n')
    temporary.replace(budget_path)

def check_identity():
    pid = int(subprocess.check_output(['systemctl', 'show', unit, '--value', '--property=MainPID']))
    assert pid == runtime['pid'] == start['pid']
    proc = pathlib.Path('/proc', str(pid))
    assert (proc / 'stat').read_text().rsplit(')', 1)[1].split()[19] == start['process_start']
    assert hashlib.sha256((proc / 'exe').read_bytes()).hexdigest() == start['binary_sha256']
    args = (proc / 'cmdline').read_bytes().decode().rstrip('\0').split('\0')
    assert args[args.index('--background-provider-tasks') + 1] == 'disabled'
    assert args[args.index('--state-dir') + 1] == str(epoch / 'state')
    with sqlite3.connect('file:' + str(epoch / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
        account = db.execute('SELECT id,revision,enabled,auth_status,worker_claim_kind FROM grok_accounts').fetchall()
    assert len(account) == 1 and formal.opaque(account[0][0]) == start['account_evidence_id']
    assert account[0][1:] == (136, 1, 'active', None)
    assert start['expires_at_ms'] > int(time.time() * 1000) + 60000
    for line in (base / 'audit/abd45431-six-20261011.jsonl').read_bytes().splitlines():
        assert json.loads(line)['event'] in ['dns_build_allowed', 'connect_build_allowed']

captured_bundle = None
try:
    check_identity()
    budget['stopped'] = True
    budget['combinations'][name]['basic_status'] = 'UNKNOWN'
    save_budget()

    def controlled_http(base_url, key, actual_protocol, body, timeout=90):
        check_identity()
        assert base_url == 'http://127.0.0.1:19180/v1' and actual_protocol == protocol
        assert summary['basic_intents'] < 9 and budget['new_basic_intents'] < 54
        assert budget['new_pin_intents'] <= 6 and budget['new_pin_intents'] + budget['new_basic_intents'] < 60
        assert 33 + budget['new_pin_intents'] + budget['new_basic_intents'] < 93
        row = {'request': copy.deepcopy(body), 'request_wire_b64': base64.b64encode(json.dumps(body, separators=(',', ':')).encode()).decode(),
               'wire_bytes_b64': '', 'protocol': protocol, 'sent_at_utc': formal.now()}
        path = private / ('http-' + str(summary['basic_intents'] + 1).zfill(2) + '-private.json')
        assert not path.exists()
        path.write_text(json.dumps(row, indent=2) + '\n')
        summary['basic_intents'] += 1
        budget['new_basic_intents'] += 1
        save_budget()
        def raw_progress(metadata, raw):
            row.update(metadata, wire_bytes_b64=base64.b64encode(raw).decode())
            path.write_text(json.dumps(row, indent=2) + '\n')
        result = formal.http_exchange(base_url, key, formal.PROTOCOLS[protocol], body, timeout=timeout, on_progress=raw_progress)
        result.update(request_wire_b64=row['request_wire_b64'], wire_bytes_b64=row['wire_bytes_b64'])
        check_identity()
        return result

    def collect(host, service, response_id, sudo=False):
        assert host == 'new-vps' and service == unit and sudo is True
        formal.require(isinstance(response_id, str) and len(response_id) <= 256, 'invalid_response_identity')
        time.sleep(2)
        script = formal.REMOTE_READ.replace('SERVICE', repr(unit)).replace('INVENTORY', 'False').rsplit('print(json.dumps(result))', 1)[0]
        script += formal.REMOTE_COLLECT.replace('RESPONSE_ID', repr(response_id))
        result = subprocess.run([sys.executable, '-c', script], capture_output=True, text=True, check=True, timeout=30)
        return formal.load_json(result.stdout)

    original_run = formal.run_basic
    def capture_run(plan, runtime, key, base_url, progress=None):
        global captured_bundle
        def retained_progress(bundle):
            (private / 'wire-partial.json').write_text(json.dumps(bundle, indent=2) + '\n')
            if progress:
                progress(bundle)
        captured_bundle = original_run(plan, runtime, key, base_url, retained_progress)
        (private / 'wire-final.json').write_text(json.dumps(captured_bundle, indent=2) + '\n')
        return captured_bundle

    formal.http_call = controlled_http
    formal.collect_attempt = collect
    formal.run_basic = capture_run
    os.environ['CPAR_ACCEPTANCE_CLIENT_KEY'] = (base / 'private/client-key').read_text().strip()
    sys.argv = [str(entry), '--new-build', 'run', '--plan', str(private / 'frozen-plan.json'), '--runtime', str(private / 'runtime.json'),
                '--base-url', 'http://127.0.0.1:19180/v1', '--execute', '--out', str(private / 'basic-receipt.json')]
    with contextlib.redirect_stdout(io.StringIO()):
        try:
            runpy.run_path(str(entry), run_name='__main__')
        except SystemExit as result:
            summary['formal_entry_exit'] = result.code
    audit = formal.load_json((private / 'basic-receipt.json').read_text())
    summary.update(status=audit['basic_status'], formal_audit_status=audit['status'], observed_response_count=audit['sent_requests'],
                   checks=audit['checks'], execution_failure=captured_bundle.get('execution_failure') if captured_bundle else None, target=plan['target'],
                   private_wire_sha256=hashlib.sha256((private / 'wire-final.json').read_bytes()).hexdigest(),
                   collector='unchanged formal REMOTE_READ/REMOTE_COLLECT; local authorized-host subprocess; one 2s billing settling delay')
    budget['combinations'][name]['basic_status'] = summary['status']
    if summary['status'] == 'PASS' and captured_bundle and not captured_bundle.get('execution_failure') and summary['basic_intents'] == 9:
        budget['stopped'] = False
except BaseException as error:
    budget['stopped'] = True
    summary['category'] = str(error) if isinstance(error, ValueError) and str(error).replace('_', '').isalnum() else type(error).__name__
    budget['combinations'][name]['basic_status'] = summary['status']
finally:
    os.environ.pop('CPAR_ACCEPTANCE_CLIENT_KEY', None)
    save_budget()
    summary.update(observed_at_utc=formal.now(), new_pin_intents_total=budget['new_pin_intents'], new_basic_intents_total=budget['new_basic_intents'])
    (private / 'basic-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary))
