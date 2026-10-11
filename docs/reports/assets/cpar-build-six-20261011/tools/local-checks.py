"""Exercise the actual owned driver gates and transport boundary without Provider I/O."""
import ast
import base64
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import types

HERE = Path(__file__).resolve().parent
checks = []

def module(nodes):
    return compile(ast.fix_missing_locations(ast.Module(body=nodes, type_ignores=[])), '<owned-driver>', 'exec')

def gate(path, first, last):
    source = (HERE / path).read_text()
    return compile(source[source.index(first):source.index(last, source.index(first))], str(path), 'exec')

def expect(name, operation, rejected=False):
    try:
        operation()
    except (AssertionError, RuntimeError):
        assert rejected, name
    else:
        assert not rejected, name
    checks.append({'case': name, 'status': 'PASS'})

prepare_tree = ast.parse((HERE / 'remote-prepare.py').read_text())
budget_node = next(n for n in prepare_tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'budget' for t in n.targets))
initial = ast.literal_eval(budget_node.value)
order = ['chat-json', 'chat-sse', 'messages-json', 'messages-sse', 'responses-json', 'responses-sse']
pin_gate = gate('remote-pin.py', "assert budget['historical_provider_pin_requests']", 'private = epoch / name')
basic_gate = gate('remote-basics.py', "assert not budget['stopped']", 'private = epoch / name')

def admitted(code, budget, name):
    exec(code, {'budget': budget, 'name': name, 'order': order})

budget = copy.deepcopy(initial)
for name in order:
    expect('ordered_pin_' + name, lambda: admitted(pin_gate, budget, name))
    budget['new_pin_intents'] += 1
    budget['combinations'][name] = {'pin_status': 'PASS', 'basic_status': 'NOT_RUN'}
    expect('ordered_basic_' + name, lambda: admitted(basic_gate, budget, name))
    budget['new_basic_intents'] += 9
    budget['combinations'][name]['basic_status'] = 'PASS'
assert budget['new_pin_intents'] == 6 and budget['new_basic_intents'] == 54
assert 33 + budget['new_pin_intents'] + budget['new_basic_intents'] == 93
expect('skip_first_combination_rejected', lambda: admitted(pin_gate, initial, order[1]), True)
expect('repeat_pin_rejected', lambda: admitted(pin_gate, budget, order[0]), True)
expect('repeat_basic_rejected', lambda: admitted(basic_gate, budget, order[-1]), True)
for result in ['FAIL', 'UNKNOWN', 'BLOCKED']:
    failed = copy.deepcopy(initial)
    failed['stopped'] = True
    failed['combinations'][order[0]] = {'pin_status': 'PASS', 'basic_status': result}
    expect('global_stop_' + result, lambda: admitted(pin_gate, failed, order[1]), True)
for field, value in [('historical_inference_requests', 32), ('historical_gateway_pin_confirmed_sends', 33), ('new_inference_limit', 61), ('cumulative_inference_limit', 94)]:
    wrong = copy.deepcopy(initial); wrong[field] = value
    expect('binding_rejected_' + field, lambda: admitted(pin_gate, wrong, order[0]), True)

basic_tree = ast.parse((HERE / 'remote-basics.py').read_text())
controlled = next(n for n in ast.walk(basic_tree) if isinstance(n, ast.FunctionDef) and n.name == 'controlled_http')
save = next(n for n in basic_tree.body if isinstance(n, ast.FunctionDef) and n.name == 'save_budget')
with tempfile.TemporaryDirectory(prefix='cpar-six-driver-') as tmp:
    private = Path(tmp)
    pin_tree = ast.parse((HERE / 'remote-pin.py').read_text())
    pin_try = next(n for n in pin_tree.body if isinstance(n, ast.Try))
    reserve_index = next(i for i, n in enumerate(pin_try.body) if isinstance(n, ast.AugAssign))
    pin_save = next(n for n in pin_tree.body if isinstance(n, ast.FunctionDef) and n.name == 'save_budget')
    env = {'budget': copy.deepcopy(initial), 'budget_path': private / 'pin-budget.json', 'name': order[0], 'summary': {}, 'json': json}
    exec(module([pin_save] + pin_try.body[reserve_index:reserve_index + 5]), env)
    saved = json.loads(env['budget_path'].read_text())
    assert saved['new_pin_intents'] == 1 and saved['stopped'] is True
    assert saved['combinations'][order[0]]['pin_status'] == 'UNKNOWN'
    checks.append({'case': 'pin_reserved_and_latched_before_transport', 'status': 'PASS'})
    for case, before_pin, before_basic, status in [('last_request', 6, 53, 200), ('limit_rejection', 6, 54, 200), ('http_local_rejection', 1, 0, 400), ('unknown_transport', 1, 0, None)]:
        folder = private / case; folder.mkdir()
        b = copy.deepcopy(initial); b.update(stopped=True, new_pin_intents=before_pin, new_basic_intents=before_basic)
        summary = {'basic_intents': 0}; transport = []
        budget_path = folder / 'budget.json'
        def exchange(base_url, key, path, body, timeout, on_progress):
            saved = json.loads(budget_path.read_text())
            assert saved['stopped'] is True and saved['new_basic_intents'] == before_basic + 1
            assert len(list(folder.glob('http-*-private.json'))) == 1
            transport.append(1)
            on_progress({'http_status': status}, b'partial-or-response')
            if status is None:
                raise RuntimeError('unknown_transport')
            return {'status': status}
        formal = types.SimpleNamespace(now=lambda: '2026-10-11T00:00:00Z', PROTOCOLS={'chat': '/chat/completions'}, http_exchange=exchange)
        env = dict(base64=base64, copy=copy, json=json, summary=summary, budget=b, budget_path=budget_path, private=folder, formal=formal, protocol='chat', check_identity=lambda: None)
        exec(module([save, controlled]), env)
        expect(case, lambda: env['controlled_http']('http://127.0.0.1:19180/v1', 'synthetic', 'chat', {'model': 'synthetic'}), case in ['limit_rejection', 'unknown_transport'])
        expected = 0 if case == 'limit_rejection' else 1
        assert len(transport) == expected and b['new_basic_intents'] == before_basic + expected and b['stopped'] is True
        if expected:
            capture = json.loads((folder / 'http-01-private.json').read_text())
            assert base64.b64decode(capture['wire_bytes_b64']) == b'partial-or-response'

exit_tree = ast.parse((HERE / 'remote-exit.py').read_text())
account_names = {'pin_send_accounting_complete', 'pin_known_sends', 'pin_actual_sends'}
account_nodes = [n for n in exit_tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id in account_names for t in n.targets)]
for case, pins, count, expected in [('known_pin', [{'attempt_count': 1, 'upstream_sent': True}], 1, 1), ('known_local_pin_rejection', [{'attempt_count': 0, 'upstream_sent': False}], 1, 0), ('missing_pin_unknown', [], 1, None), ('ambiguous_pin_unknown', [{'attempt_count': None, 'upstream_sent': None}], 1, None)]:
    env = {'pins': pins, 'budget': {'new_pin_intents': count}}
    exec(module(account_nodes), env)
    assert env['pin_actual_sends'] == expected
    checks.append({'case': case, 'status': 'PASS'})
receipt_node = next(n for n in exit_tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'receipt' for t in n.targets))
projection = dict(zip([ast.literal_eval(k) for k in receipt_node.value.keys], receipt_node.value.values))
assert ast.literal_eval(projection['resource_exit_status']) == 'PASS'
assert isinstance(projection['count_reconciliation_status'], ast.IfExp)
checks.append({'case': 'resource_exit_independent_of_send_count', 'status': 'PASS'})
basic_count_names = {'basic_confirmed_sends', 'basic_send_accounting_complete', 'send_accounting_complete'}
basic_count_nodes = [n for n in exit_tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id in basic_count_names for t in n.targets)]
for case, outcomes, intents, expected in [('complete_basics', ['succeeded', 'succeeded'], 1, True), ('no_attempt_unknown', ['succeeded'], 1, False), ('failed_attempt_unknown', ['succeeded', 'failed'], 1, False)]:
    env = {'attempts': [{'outcome': value} for value in outcomes], 'prepare': {'inherited_event_counts': {'attempt': 1}}, 'delta': {'request': intents, 'request_finished': intents, 'attempt': len(outcomes) - 1}, 'budget': {'new_basic_intents': intents}, 'pin_send_accounting_complete': True}
    exec(module(basic_count_nodes), env)
    assert env['send_accounting_complete'] is expected
    count_status = eval(compile(ast.Expression(projection['count_reconciliation_status']), '<count-status>', 'eval'), env)
    assert count_status == ('PASS' if expected else 'UNKNOWN')
    checks.append({'case': case, 'status': 'PASS'})
print(json.dumps({'status': 'PASS', 'layer': 'LOCAL_SIMULATED', 'real_provider_requests': 0, 'checks': checks, 'total': len(checks), 'driver_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(HERE.glob('remote-*.py'))}}, indent=2))
