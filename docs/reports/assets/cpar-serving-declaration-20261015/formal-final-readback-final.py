"""Independent, read-only reconciliation after the stopped formal diagnosis."""
import ast
import gzip
import hashlib
import json
import os
import pathlib
import sqlite3
import subprocess
import sys

sys.dont_write_bytecode = True
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/chat-json-serving-20261015'
unit = 'cpar-build-chat-json-serving-20261015.service'
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
opaque = lambda value: hashlib.sha256(('cpar-evidence:' + str(value)).encode()).hexdigest()
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
state = dict(row.split('=', 1) for row in subprocess.check_output(
    ['systemctl', 'show', unit, '--property=MainPID,LoadState,ActiveState,SubState'], text=True).splitlines())
assert state['MainPID'] == '0' and state['LoadState'] == 'not-found'
assert not (pathlib.Path('/sys/fs/cgroup/system.slice') / unit).exists()
start = json.loads((epoch / 'start-receipt.json').read_text())
prepare = json.loads((epoch / 'prepare-receipt.json').read_text())
assert not pathlib.Path('/proc', str(start['pid'])).exists()
stop = json.loads((epoch / 'startup-stop-intent.json').read_text())
assert stop['stop_intent'] and (stop['pid'], stop['process_start']) == (start['pid'], start['process_start'])
assert sha(epoch / 'startup-observations.json') == start['startup_observations_sha256']
final = json.loads((epoch / 'final-receipt.json').read_text())
inventory = final.pop('private_files')
assert all(name.startswith(str(epoch.relative_to(base)) + '/') for name in inventory)
# The native exit inventory is {base-relative path: {sha256, bytes, mode}}.
# SQLite checkpoint/WAL lifecycle is verified by durable SQL below, not by a
# pre-Python-exit file digest. Every immutable captured artifact is rehashed.
immutable_inventory = {name: facts for name, facts in inventory.items()
    if not any(name.endswith(suffix) for suffix in ('.sqlite3', '.sqlite3-wal', '.sqlite3-shm'))}
assert all(sha(base / name) == facts['sha256'] and (base / name).stat().st_size == facts['bytes']
           for name, facts in immutable_inventory.items())
final.update(private_file_count=len(inventory), private_inventory_sha256=hashlib.sha256(json.dumps(inventory, sort_keys=True).encode()).hexdigest())
assert final == expected_exit
assert sha(epoch / 'startup-stop-intent.json') == final['stop_intent_sha256']
assert sha(epoch / 'exit-observations.json') == final['exit_observations_sha256']
budget = json.loads((epoch / 'budget.json').read_text())
assert budget == final['budget'] and budget['stopped'] is True
assert (budget['new_pin_intents'], budget['new_basic_intents']) == (1, 1)
assert (budget['pin_status'], budget['chat_status']) == ('PASS', 'REPRODUCED')
assert budget['retries'] == budget['fallbacks'] == budget['refresh_reauthorization'] == 0
assert budget['provider_receive_count'] is budget['actual_cost'] is None
frozen = json.loads((epoch / 'freeze.json').read_text())
uploaded = base / 'bin/chat-json-serving-20261015'
assert sha(uploaded / 'freeze.json') == '13c1404942aa62215ebc3f89be090b2c27ea473f6f5a4d1001ef56e80cfcba72'
assert frozen == json.loads((uploaded / 'freeze.json').read_text())
assert all(sha(uploaded / name) == digest for name, digest in frozen['files'].items())
assert sha(epoch / 'selected-request.json') == '245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062'
assert sha(epoch / 'inherited-expired-catalog-private.json') == prepare['isolated_catalog_preparation']['archive_sha256']
pin = json.loads((epoch / 'chat-json/pin-private.json').read_text())
chat = json.loads((epoch / 'chat-json/chat-summary.json').read_text())
assert pin['outcome'] == 'succeeded' and pin['attempt_count'] == 1 and pin['upstream_sent'] is True
lease_revision = pin['execution']['identity']['credential_revision']
assert lease_revision == chat['validated_execution']['actual_lease_credential_revision']
assert chat['http_status'] == 502 and chat['rejection'] == 'unknown_extensions'
assert chat['request_outcome'] == 'failed' and chat['attempt_outcome'] == 'succeeded'
assert chat['usage_rows'] == chat['ledger_rows'] == 1 and chat['ledger_source_event_exact_match'] is True
with sqlite3.connect('file:' + str(epoch / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    counts = dict(db.execute('SELECT event_type,count(*) FROM gateway_event_log GROUP BY event_type'))
    assert counts == {key: value + final['new_event_counts'].get(key, 0) for key, value in prepare['inherited_event_counts'].items()}
    assert db.execute('SELECT count(*) FROM billing_ledger_entries').fetchone()[0] == prepare['inherited_ledger_count'] + 1
    assert db.execute('SELECT count(*) FROM model_catalog_targets').fetchone()[0] == 0
    assert db.execute('SELECT revision,worker_claim_kind FROM grok_accounts').fetchall() == [(start['native_revision'], None)]
declaration_reads = []
for name in ['inspection-0-private.json', 'inspection-1-private.json', 'chat-presend-runtime-private.json']:
    observed = json.loads((epoch / 'chat-json' / name).read_text())
    runtime = observed['runtime']
    assert runtime['capability_read_complete'] and observed['configuration_stable']
    assert observed['capability_read_errors'] == []
    keys = runtime['capability_reads']['client_keys']
    assert keys['complete'] and keys['key_evidence_ids'] == [opaque('cpar-build-client')]
    declaration = runtime['capability_observations']
    assert len(declaration) == 1 and declaration[0]['source'] == 'serving_snapshot'
    pages = runtime['capability_reads']['effective_models'][0]['pages']
    assert len(pages) == 1 and pages[0]['http_status'] == 200 and pages[0]['item_count'] == 1
    assert pages[0]['models'][0]['source_count'] == 1
    assert pages[0]['models'][0]['sources'][0]['declaration_state'] == 'object'
    declaration_reads.append({'name': name, 'sha256': sha(epoch / 'chat-json' / name),
        'key_pages': len(keys['pages']), 'effective_pages': len(pages), 'models': 1, 'sources': 1,
        'declaration': declaration[0]})
for family in ['tcp', 'tcp6']:
    assert not any(row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181]
                   for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:])
assert all(not path.is_symlink() and (path.stat().st_mode & 0o777) == (0o700 if path.is_dir() else 0o600)
           for path in epoch.rglob('*'))
assert all(sha(base / name) == digest for name, digest in start['historical_file_hashes'].items())
source = (base / 'private/bootstrap.py').read_text()
node = next(node for node in ast.parse(source).body if isinstance(node, ast.FunctionDef) and node.name == 'production')
exec(compile(ast.Module(body=[node], type_ignores=[]), 'production-metadata-proof', 'exec'))
assert production() == final['production_after']
print(json.dumps({'status': 'PASS', 'kind': 'READ_ONLY_FINAL_FORMAL_RESOURCE_AND_COUNT_READBACK',
    'unit': state, 'resource_exit_status': 'PASS', 'budget_stopped': True, 'unused_intents': 0,
    'new_pin_intents': 1, 'new_chat_intents': 1, 'new_gateway_pin_confirmed_sends': 2,
    'new_event_counts': final['new_event_counts'], 'new_ledger_rows': 1,
    'actual_fresh_pin_lease_revision': lease_revision, 'chat_status': chat['status'],
    'http_status': chat['http_status'], 'rejection': chat['rejection'], 'declaration_reads': declaration_reads,
    'startup_observations_preserved': True, 'raw_exit_receipt_matches_public_projection': True, 'immutable_inventory_verified_count': len(immutable_inventory), 'sqlite_verification': 'durable event/ledger/catalog/account SQL; checkpoint lifecycle distinct from capture-time inventory',
    'production_identity_preserved': True, 'historical_hashes_preserved': True,
    'provider_independent_receive_count': 'UNKNOWN', 'provider_cost': 'UNKNOWN',
    'new_provider_calls_in_readback': 0, 'new_launches_in_readback': 0}))
