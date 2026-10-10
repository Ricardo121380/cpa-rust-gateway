import datetime
import hashlib
import json
import pathlib
import sqlite3
import stat
import subprocess
import urllib.request

base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
old = json.loads((base / 'private/final-receipt.json').read_text())
checks = []
for path, key in [
    ('private/phase2/responses-json/pin-private.json', 'private_pin_sha256'),
    ('private/batch-budget-v2.json', 'repaired_budget_sha256'),
    ('private/batch-budget.json', 'old_budget_preserved_sha256'),
    ('private/gateway.log', 'private_log_sha256'),
    ('audit/candidate-egress.jsonl', 'egress_audit_sha256'),
]:
    raw = (base / path).read_bytes()
    row = {'path': path, 'sha256': hashlib.sha256(raw).hexdigest(),
           'previous_sha256': old[key], 'match': hashlib.sha256(raw).hexdigest() == old[key], 'bytes': len(raw)}
    if not row['match'] and path == 'private/gateway.log':
        digest = hashlib.sha256()
        offset = 0
        for line in raw.splitlines(keepends=True):
            digest.update(line)
            offset += len(line)
            if digest.hexdigest() == old[key]:
                row.update(previous_exact_prefix_preserved=True, previous_bytes=offset,
                           appended_bytes=len(raw) - offset,
                           appended_lines=len(raw[offset:].splitlines()))
                break
    checks.append(row)
opaque = lambda value: hashlib.sha256(('cpar-evidence:' + str(value)).encode()).hexdigest()
with sqlite3.connect('file:' + str(base / 'state/control.sqlite3') + '?mode=ro', uri=True) as db:
    db.row_factory = sqlite3.Row
    local = [dict(row) for row in db.execute("SELECT id,enabled,auth_status,revision,worker_claim_kind FROM grok_accounts WHERE provider='build'")]
    assert len(local) == 1 and opaque(local[0]['id']) == old['pin']['credential_evidence_id']
    account_id = local[0].pop('id')
    local[0]['account_evidence_id'] = opaque(account_id)
pid = int(subprocess.check_output(['systemctl', 'show', 'cpa-rust-gateway.service', '--value', '--property=MainPID']))
args = pathlib.Path('/proc', str(pid), 'cmdline').read_bytes().decode().split('\0')
state = args[args.index('--state-dir') + 1]
credential_dir = args[args.index('--credential-dir') + 1]
listener = args[args.index('--management-listen') + 1]
assert listener == '127.0.0.1:18181'
key = (pathlib.Path(credential_dir) / 'management-key').read_text().strip()
client = urllib.request.build_opener(urllib.request.ProxyHandler({}))
with client.open(urllib.request.Request('http://' + listener + '/admin/operations/provider-account-pools?limit=100', headers={'X-Management-Key': key}), timeout=15) as response:
    pool = json.load(response)['items']
rows = [row for row in pool if row.get('account_id') == account_id and row.get('channel_id') == 'p12-12-production-grok-build-endpoint']
assert len(rows) == 1
with sqlite3.connect('file:' + state + '/control.sqlite3?mode=ro', uri=True) as db:
    db.row_factory = sqlite3.Row
    columns = {row[1] for row in db.execute('PRAGMA table_info(grok_accounts)')}
    selected = [name for name in ['enabled', 'auth_status', 'revision', 'worker_claim_kind', 'worker_claim_expires_at_ms', 'external_account_id', 'expires_at_ms'] if name in columns]
    source = dict(db.execute('SELECT ' + ','.join(selected) + ' FROM grok_accounts WHERE id=?', (account_id,)).fetchone())
    external = source.pop('external_account_id', None)
    source['external_account_evidence_id'] = opaque(external) if external else None
source['account_evidence_id'] = opaque(account_id)
runtime = {key: rows[0].get(key) for key in ['runtime_status', 'credential_revision', 'expires_at_ms', 'active_leases', 'availability']}
receipt = {
    'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'old_private_evidence_hashes': checks, 'candidate_copy': local[0],
    'source_account': source, 'source_pool': runtime,
    'now_ms': int(datetime.datetime.now(datetime.timezone.utc).timestamp() * 1000),
    'candidate_unit_exists': pathlib.Path('/etc/systemd/system/cpar-build-acceptance-20261010.service').exists(),
    'candidate_transient_unit_exists': pathlib.Path('/run/systemd/transient/cpar-build-acceptance-20261010.service').exists(),
    'provider_requests_by_this_read': 0,
}
print(json.dumps(receipt))
