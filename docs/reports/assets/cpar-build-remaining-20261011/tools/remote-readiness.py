"""Value-free, read-only ownership/material check for the authorized batch."""
import ast
import datetime
import hashlib
import json
import os
import pathlib
import sqlite3
import stat
import subprocess
import urllib.request

base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
unit = 'cpar-build-acceptance-20261010.service'
opaque = lambda value: hashlib.sha256(('cpar-evidence:' + str(value)).encode()).hexdigest()
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
for path in [base, base / 'private', base / 'audit']:
    assert not path.is_symlink() and path.stat().st_uid == 0
    assert stat.S_IMODE(path.stat().st_mode) == 0o700
unit_state = dict(row.split('=', 1) for row in subprocess.check_output(
    ['systemctl', 'show', unit, '--property=MainPID,LoadState,FragmentPath,Transient,ActiveState,SubState'],
    text=True).splitlines())
assert unit_state['MainPID'] == '0' and unit_state['LoadState'] == 'not-found'
ports = []
for family in ['tcp', 'tcp6']:
    ports += [int(row.split()[1].rsplit(':', 1)[1], 16)
              for row in pathlib.Path('/proc/net', family).read_text().splitlines()[1:]
              if row.split()[3] == '0A' and int(row.split()[1].rsplit(':', 1)[1], 16) in [19180, 19181]]
assert not ports
competing = []
for proc in pathlib.Path('/proc').iterdir():
    if not proc.name.isdigit() or int(proc.name) == os.getpid():
        continue
    try:
        if str(base).encode() in (proc / 'cmdline').read_bytes():
            competing.append(int(proc.name))
        if any(str(fd.resolve()).startswith(str(base)) for fd in (proc / 'fd').iterdir()):
            competing.append(int(proc.name))
    except (PermissionError, FileNotFoundError, ProcessLookupError):
        pass
assert not competing
tree = ast.parse((base / 'private/bootstrap.py').read_text())
node = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'production')
exec(compile(ast.Module(body=[node], type_ignores=[]), 'production-proof', 'exec'))
production_state = production()
pid = int(production_state['MainPID'])
args = pathlib.Path('/proc', str(pid), 'cmdline').read_bytes().decode().split('\0')
assert args[args.index('--state-dir') + 1] == '/var/lib/cpa-rust-gateway'
assert args[args.index('--management-listen') + 1] == '127.0.0.1:18181'
credentials = pathlib.Path(args[args.index('--credential-dir') + 1])
headers = {'X-Management-Key': (credentials / 'management-key').read_text().strip()}
client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
def get(path):
    with client.open(urllib.request.Request('http://127.0.0.1:18181' + path, headers=headers), timeout=10) as response:
        assert response.status == 200
        return json.load(response)
with sqlite3.connect('file:' + str(base / 'private/six-combinations-20261011/state/control.sqlite3') + '?mode=ro', uri=True) as db:
    db.row_factory = sqlite3.Row
    rows = [dict(row) for row in db.execute('SELECT * FROM grok_accounts')]
    assert len(rows) == 1
    old = rows[0]
    assert opaque(old['id']) == '5beff1b67bd35a0547fe42e4b72970a80f0c2c28a77596c29703d83f397c3d54'
with sqlite3.connect('file:/var/lib/cpa-rust-gateway/control.sqlite3?mode=ro', uri=True) as db:
    db.row_factory = sqlite3.Row
    account = dict(db.execute('SELECT * FROM grok_accounts WHERE id=?', (old['id'],)).fetchone())
    source_schema = db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0]
pools = [r for r in get('/admin/operations/provider-account-pools?limit=100')['items']
         if r.get('account_id') == old['id'] and r.get('channel_id') == 'p12-12-production-grok-build-endpoint']
assert len(pools) == 1
pool = pools[0]
old_final = json.loads((base / 'private/six-combinations-20261011/final-receipt.json').read_text())
historical = old_final['budget']['historical_inference_requests'] + old_final['new_pin_actual_sends'] + old_final['new_basic_attempts']
assert historical == 21 and old_final['budget']['stopped'] is True
now = datetime.datetime.now(datetime.timezone.utc)
result = {
    'status': 'PASS; read-only observation', 'observed_at_utc': now.isoformat(),
    'unit': unit_state, 'ports_listening': ports, 'competing_processes': competing,
    'production': production_state, 'production_function_sha256': hashlib.sha256(ast.get_source_segment((base / 'private/bootstrap.py').read_text(), node).encode()).hexdigest(),
    'production_config': [{k:r[k] for k in ['id','revision','status']} for r in get('/admin/config-versions') if r['status'] == 'active'],
    'source_account': {k:account.get(k) for k in ['provider','enabled','auth_status','revision','worker_claim_kind','last_refresh_at_ms','continuation_floor_revision','continuation_current_revision']},
    'source_schema': source_schema, 'source_pool': {k:pool.get(k) for k in ['runtime_status','credential_revision','expires_at_ms','active_leases']},
    'account_evidence_id': opaque(account['id']), 'identity_digest_matches_authorized_source': account['identity_digest'] == old['identity_digest'],
    'source_changed_field_names': [k for k in account if account[k] != old.get(k)],
    'remaining_material_seconds': (pool['expires_at_ms'] - int(now.timestamp()*1000))/1000,
    'historical_inference_requests': historical, 'new_provider_calls': 0, 'oauth_refresh_reauthorization': 0,
}
print(json.dumps(result))
