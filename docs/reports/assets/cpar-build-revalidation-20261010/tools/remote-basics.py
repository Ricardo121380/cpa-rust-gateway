import fcntl
import hashlib
import importlib.util
import json
import os
import pathlib
import sqlite3
import subprocess
import sys
import time

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/revalidation-20261010'
unit = 'cpar-build-acceptance-20261010.service'
protocol, mode = sys.argv[1:]
assert protocol in ['responses', 'chat', 'messages'] and mode in ['json', 'sse']
name = protocol + '-' + mode
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
lock = (epoch / 'execution.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
budget_path = epoch / 'budget.json'
budget = json.loads(budget_path.read_text())
assert not budget['stopped'] and budget['initial_responses_json_pin_passed']
assert budget['combinations'][name] == {'pin_status': 'PASS', 'basic_status': 'NOT_RUN'}
assert name == 'responses-json' or budget['combinations']['responses-json']['basic_status'] == 'PASS'
private = epoch / name
assert not (private / 'basic-summary.json').exists()
tool = epoch / 'cpar_new_build_acceptance.py'
assert hashlib.sha256(tool.read_bytes()).hexdigest() == 'a46a2880e9614048411a25f594b0532c582e0a2ba29ab32d090068d269b53683'
spec = importlib.util.spec_from_file_location('formal', tool)
formal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(formal)
plan = formal.load_json((private / 'frozen-plan.json').read_text())
runtime = formal.load_json((private / 'runtime.json').read_text())['runtime']
start = json.loads((epoch / 'start-receipt.json').read_text())
summary = {'combination': name, 'status': 'BLOCKED', 'basic_intents': 0,
           'source_sha': start['source_sha'], 'binary_sha256': start['binary_sha256'],
           'formal_tool_sha256': hashlib.sha256(tool.read_bytes()).hexdigest()}

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
    with sqlite3.connect('file:' + str(base / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
        account = db.execute('SELECT id,revision,enabled,auth_status,worker_claim_kind FROM grok_accounts').fetchall()
    assert len(account) == 1 and formal.opaque(account[0][0]) == start['account_evidence_id']
    assert account[0][1:] == (133, 1, 'active', None)
    assert start['expires_at_ms'] > int(time.time() * 1000) + 60000
    for line in (base / 'audit/revalidation-20261010.jsonl').read_bytes().splitlines():
        assert json.loads(line)['event'] in ['dns_build_allowed', 'connect_build_allowed']

try:
    check_identity()
    original_http = formal.http_call
    budget['stopped'] = True
    budget['combinations'][name]['basic_status'] = 'UNKNOWN'
    save_budget()

    def controlled_http(*args, **kwargs):
        check_identity()
        assert summary['basic_intents'] < 9
        assert budget['new_pin_intents'] + budget['new_basic_intents'] < 60
        summary['basic_intents'] += 1
        budget['new_basic_intents'] += 1
        save_budget()
        return original_http(*args, **kwargs)

    def collect(host, service, response_id, sudo=False):
        assert host == 'new-vps' and service == unit and sudo is True
        formal.require(isinstance(response_id, str) and len(response_id) <= 256, 'invalid_response_identity')
        time.sleep(2)
        script = formal.REMOTE_READ.replace('SERVICE', repr(unit)).replace('INVENTORY', 'False').rsplit('print(json.dumps(result))', 1)[0]
        script += formal.REMOTE_COLLECT.replace('RESPONSE_ID', repr(response_id))
        result = subprocess.run([sys.executable, '-c', script], capture_output=True, text=True, check=True, timeout=30)
        return formal.load_json(result.stdout)

    def progress(bundle):
        (private / 'wire-partial.json').write_text(json.dumps(bundle, indent=2) + '\n')

    formal.http_call = controlled_http
    formal.collect_attempt = collect
    key = (base / 'private/client-key').read_text().strip()
    bundle = formal.run_basic(plan, runtime, key, 'http://127.0.0.1:19180/v1', progress)
    (private / 'wire-final.json').write_text(json.dumps(bundle, indent=2) + '\n')
    audit = formal.audit_bundle(bundle)
    formal.write_receipt(private / 'basic-receipt.json', audit)
    summary.update(status=audit['basic_status'], formal_audit_status=audit['status'],
                   observed_response_count=audit['sent_requests'], checks=audit['checks'],
                   execution_failure=bundle.get('execution_failure'), target=plan['target'],
                   private_wire_sha256=hashlib.sha256((private / 'wire-final.json').read_bytes()).hexdigest(),
                   collector='unchanged formal REMOTE_READ/REMOTE_COLLECT; local authorized-host subprocess; one 2s billing settling delay')
    budget['combinations'][name]['basic_status'] = summary['status']
    if summary['status'] == 'PASS' and not bundle.get('execution_failure') and summary['basic_intents'] == 9:
        budget['stopped'] = False
except BaseException as error:
    budget['stopped'] = True
    summary['category'] = str(error) if isinstance(error, ValueError) and str(error).replace('_', '').isalnum() else type(error).__name__
    budget['combinations'][name]['basic_status'] = 'BLOCKED'
finally:
    save_budget()
    summary['observed_at_utc'] = formal.now()
    summary['new_pin_intents_total'] = budget['new_pin_intents']
    summary['new_basic_intents_total'] = budget['new_basic_intents']
    (private / 'basic-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary))
