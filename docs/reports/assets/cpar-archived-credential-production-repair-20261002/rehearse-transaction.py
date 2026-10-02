"""Exercise the exact bounded production CAS on a new derived database, including failures."""
import pathlib
exec((pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884/archived-credential-production-repair-20261002') / 'repair-common.py').read_text())
assert json.loads((OPS / 'plan-pre-receipt.json').read_text())['status'] == 'PASS'
pre = OPS / 'capture-pre'
work = OPS / 'transaction-rehearsal'; work.mkdir(mode=0o700)
state_copy(pre / 'state', work / 'state')
state_copy(pre / 'credentials', work / 'credentials')
old = (OPS / 'original-envelope.private.bin').read_bytes()
new = (OPS / 'repaired-envelope.private.bin').read_bytes()
db_file = work / 'state/control.sqlite3'
forward = bounded_cas(db_file, pre / 'state/control.sqlite3', old, new, True)
healed = work / 'healed-reference.sqlite3'
shutil.copy2(db_file, healed)
reverse = bounded_cas(db_file, healed, new, old, False)
baseline = json.loads((pre / 'logical-before.private.json').read_text())
restored = preserved(work / 'state', baseline, (OPS / 'integrity-hmac-key').read_bytes())
assert all(item['preserved'] for tables in restored.values() for item in tables.values())
bounded_cas(db_file, pre / 'state/control.sqlite3', old, new, True)
stale_before = logical_state(work / 'state', (OPS / 'integrity-hmac-key').read_bytes())
try:
    bounded_cas(db_file, pre / 'state/control.sqlite3', old, new, True)
except AssertionError:
    stale_rejected = True
else:
    raise AssertionError('stale_cas_accepted')
assert logical_state(work / 'state', (OPS / 'integrity-hmac-key').read_bytes()) == stale_before
guard = OPS / 'parent-drift-rehearsal'; guard.mkdir(mode=0o700)
state_copy(pre / 'state', guard / 'state')
with sqlite3.connect(guard / 'state/control.sqlite3') as db:
    parent = db.execute('SELECT parent_id FROM config_versions WHERE id=?', (loc['config_version_id'],)).fetchone()[0]
    db.execute('UPDATE upstream_credentials SET revision=revision+1 WHERE config_version_id=? AND id=? AND upstream_id=?', (parent, loc['credential_id'], loc['upstream_id']))
before_guard = logical_state(guard / 'state', (OPS / 'integrity-hmac-key').read_bytes())
try:
    bounded_cas(guard / 'state/control.sqlite3', pre / 'state/control.sqlite3', old, new, True)
except AssertionError:
    parent_rejected = True
else:
    raise AssertionError('parent_drift_accepted')
assert logical_state(guard / 'state', (OPS / 'integrity-hmac-key').read_bytes()) == before_guard
lock = sqlite3.connect(guard / 'state/control.sqlite3', isolation_level=None)
try:
    lock.execute('BEGIN IMMEDIATE')
    tick = time.monotonic()
    try:
        bounded_cas(guard / 'state/control.sqlite3', pre / 'state/control.sqlite3', old, new, True)
    except sqlite3.OperationalError:
        lock_seconds = time.monotonic() - tick
    else:
        raise AssertionError('held_writer_lock_accepted')
    assert 4.8 <= lock_seconds < 5.5
finally:
    lock.execute('ROLLBACK'); lock.close()
assert logical_state(guard / 'state', (OPS / 'integrity-hmac-key').read_bytes()) == before_guard
deadline_work = OPS / 'deadline-rehearsal'; deadline_work.mkdir(mode=0o700)
state_copy(pre / 'state', deadline_work / 'state')
before_deadline = logical_state(deadline_work / 'state', (OPS / 'integrity-hmac-key').read_bytes())
real_row_data = row_data
def deliberately_delayed_rows(db):
    time.sleep(10.5)
    return real_row_data(db)
row_data = deliberately_delayed_rows
tick = time.monotonic()
try:
    try:
        bounded_cas(deadline_work / 'state/control.sqlite3', pre / 'state/control.sqlite3', old, new, True)
    except TransactionDeadline:
        deadline_seconds = time.monotonic() - tick
    else:
        raise AssertionError('transaction_deadline_not_enforced')
finally:
    row_data = real_row_data
assert 9.8 <= deadline_seconds < 10.5
assert logical_state(deadline_work / 'state', (OPS / 'integrity-hmac-key').read_bytes()) == before_deadline
auth = crypto.authenticate(db_file, work / 'credentials')
assert auth['status'] == 'PASS'
receipt('transaction-rehearsal.json', {'status': 'PASS', 'forward': forward, 'rollback': reverse, 'rollback_original_54_tables': 'PASS', 'stale_expected_envelope_rejected_without_delta': stale_rejected, 'changed_parent_revision_rejected_without_delta': parent_rejected, 'held_writer_lock_rejected_without_delta': True, 'observed_lock_timeout_seconds': round(lock_seconds, 6), 'transaction_deadline_rollback_without_delta': True, 'observed_transaction_deadline_seconds': round(deadline_seconds, 6), 'crypto_after_reapply': auth, 'production_writes': 0, 'real_provider_calls': 0, 'completed_at_utc': utc()})
