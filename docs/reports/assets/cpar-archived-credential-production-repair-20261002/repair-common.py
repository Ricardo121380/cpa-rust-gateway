"""Host-only AC-01 helpers. No identifiers, keys or material are emitted."""
import ctypes
import pathlib
import signal
import time
from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

TASK = pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884')
exec((TASK / 'operator-tools/target-rehearsal.py').read_text().split('assert ROOT.lstat()')[0])
OPS = TASK / 'archived-credential-production-repair-20261002'
CAP = TASK / 'capture-2'
DIAG = TASK / 'archived-credential-diagnosis-20261002'
STATE = pathlib.Path('/var/lib/cpa-rust-gateway')
KEYS = pathlib.Path('/etc/cpa-rust-gateway/credentials')
SERVICE = 'cpa-rust-gateway.service'
loc = json.loads((DIAG / 'target.private.json').read_text())
spec = importlib.util.spec_from_file_location('crypto', TASK / 'operator-tools/private-crypto-audit.py')
crypto = importlib.util.module_from_spec(spec)
spec.loader.exec_module(crypto)

def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def private_write(destination, value):
    assert not destination.exists(), 'immutable_evidence_already_exists'
    with destination.open('xb') as output:
        output.write(value)
        output.flush()
        os.fsync(output.fileno())
    os.chmod(destination, 0o600)

def receipt(name, value):
    private_write(OPS / name, (json.dumps(value, indent=2) + '\n').encode())
    print(json.dumps(value), flush=True)

def direct(item):
    info = item.lstat()
    assert stat.S_ISREG(info.st_mode), 'non_direct_file'
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns,
            stat.S_IMODE(info.st_mode), info.st_uid, info.st_gid)

def key_state():
    return {name: (direct(KEYS / name), hashlib.sha256((KEYS / name).read_bytes()).digest()) for name in KEY_NAMES}

def runtime():
    properties = subprocess.check_output(['systemctl', 'show', SERVICE, '-p', 'MainPID', '-p', 'ActiveState', '-p', 'SubState', '-p', 'ExecMainStartTimestamp'], text=True, timeout=5)
    unit = dict(line.split('=', 1) for line in properties.splitlines())
    pid = int(unit['MainPID'])
    assert unit['ActiveState'] == 'active' and unit['SubState'] == 'running' and pid == 499070, 'runtime_identity_drift'
    assert hashlib.sha256(pathlib.Path(f'/proc/{pid}/exe').read_bytes()).hexdigest() == OLD_HASH, 'binary_drift'
    assert subprocess.check_output(['systemctl', 'cat', SERVICE], timeout=5) == (CAP / 'unit.private').read_bytes(), 'unit_drift'
    assert str(pathlib.Path('/opt/cpa-rust-gateway/current').resolve()) == (CAP / 'old-current.private').read_text().strip(), 'current_drift'
    loaded = pathlib.Path(f'/proc/{pid}/root/run/credentials/{SERVICE}')
    assert all(hmac.compare_digest((KEYS / name).read_bytes(), (loaded / name).read_bytes()) and hmac.compare_digest((KEYS / name).read_bytes(), (CAP / 'credentials' / name).read_bytes()) for name in KEY_NAMES), 'key_drift'
    listeners = subprocess.check_output(['ss', '-lntp'], text=True, timeout=5)
    ports = sorted({int(line.split()[3].rsplit(':', 1)[1]) for line in listeners.splitlines()[1:] if f'pid={pid},' in line})
    assert ports == [18180, 18181], 'listener_drift'
    return {'unit': unit, 'ports': ports, 'old_binary_unit_current_and_six_keys_match': True}

def db_ro(item):
    db = sqlite3.connect(f'file:{item}?mode=ro', uri=True, timeout=1)
    db.row_factory = sqlite3.Row
    db.execute('PRAGMA query_only=ON')
    return db

def exact_context(db):
    assert db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0] == 31, 'schema_drift'
    child = db.execute('SELECT * FROM config_versions WHERE id=?', (loc['config_version_id'],)).fetchone()
    assert child is not None and child['status'] == 'archived' and child['revision'] == 0, 'child_lifecycle_drift'
    parent = db.execute('SELECT * FROM config_versions WHERE id=?', (child['parent_id'],)).fetchone()
    assert parent is not None and parent['status'] == 'archived', 'parent_lifecycle_drift'
    def credential(config):
        row = db.execute('SELECT * FROM upstream_credentials WHERE config_version_id=? AND id=? AND upstream_id=?', (config, loc['credential_id'], loc['upstream_id'])).fetchone()
        assert row is not None and row['kind'] == 'oauth_json' and row['status'] == 'active' and row['revision'] == 2 and row['key_version'] == 1, 'credential_identity_drift'
        return tuple(row)
    active = db.execute("SELECT * FROM config_versions WHERE status='active'").fetchall()
    assert len(active) == 1 and active[0]['id'] != child['id'], 'active_reference'
    assert db.execute('SELECT count(*) FROM upstream_credentials WHERE config_version_id=? AND id=?', (active[0]['id'], loc['credential_id'])).fetchone()[0] == 0, 'active_credential_reference'
    transition = db.execute("SELECT * FROM management_audit_events WHERE action IN ('config_published','config_rolled_back') ORDER BY id DESC LIMIT 1").fetchone()
    assert transition is not None and transition['replaced_config_version_id'] != child['id'], 'rollback_reference'
    return {'child_config': tuple(child), 'parent_config': tuple(parent), 'target': credential(child['id']), 'parent': credential(parent['id']), 'active_config': tuple(active[0]), 'last_transition': tuple(transition)}

def aad(config):
    data = bytearray(b'cpa-rust-gateway/control-plane/credential-aad/v1')
    for value in (config, loc['credential_id'], loc['upstream_id']):
        raw = value.encode()
        data.extend(len(raw).to_bytes(4, 'big'))
        data.extend(raw)
    return bytes(data)

def clear_open(envelope, config, key):
    assert len(key) == 32 and len(envelope) == 4166 and envelope[0] == 1, 'envelope_shape_drift'
    nonce = envelope[1:25]
    return bytearray(ChaCha20Poly1305(crypto.hchacha(bytes(key), nonce[:16])).decrypt(bytes(4) + nonce[16:], envelope[25:], aad(config)))

def row_data(db):
    return {table: sorted([tuple(row) for row in db.execute('SELECT * FROM ' + q(table))], key=repr) for (table,) in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")}

def logical_state(state, key):
    result = {}
    for name in DBS:
        with db_ro(state / name) as db:
            db.execute('BEGIN')
            assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok' and not db.execute('PRAGMA foreign_key_check').fetchall(), 'database_integrity'
            result[name] = {table: {**digest_rows(db, table, [r[1] for r in db.execute('PRAGMA table_info(' + q(table) + ')')], key), 'columns': [r[1] for r in db.execute('PRAGMA table_info(' + q(table) + ')')]} for (table,) in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")}
    return result

def only_cipher_delta(db, before, after, expected, desired):
    assert [table for table in before if before[table] != after[table]] == ['upstream_credentials'], 'unrelated_table_delta'
    columns = [r[1] for r in db.execute('PRAGMA table_info(upstream_credentials)')]
    position = columns.index('ciphertext')
    original = [row for row in before['upstream_credentials'] if row[columns.index('config_version_id')] == loc['config_version_id'] and row[columns.index('id')] == loc['credential_id'] and row[columns.index('upstream_id')] == loc['upstream_id']]
    assert len(original) == 1 and original[0][position] == expected, 'expected_target_drift'
    replacement = list(original[0]); replacement[position] = desired
    wanted = list(before['upstream_credentials']); wanted.remove(original[0]); wanted.append(tuple(replacement))
    assert sorted(wanted, key=repr) == after['upstream_credentials'], 'not_exactly_one_field'

SQL = """UPDATE upstream_credentials SET ciphertext=?
WHERE config_version_id=? AND id=? AND upstream_id=? AND ciphertext=?
AND key_version=1 AND revision=2 AND kind='oauth_json' AND status='active'
AND EXISTS (SELECT 1 FROM config_versions v WHERE v.id=upstream_credentials.config_version_id
AND v.status='archived' AND v.revision=0 AND v.parent_id=?)"""

class TransactionDeadline(Exception):
    pass

def bounded_cas(db_file, reference_file, expected, desired, forward):
    """Recheck all identity guards inside a single writer transaction; no retries."""
    key = bytearray((OPS / 'capture-pre/credentials/master-key').read_bytes())
    db = sqlite3.connect(db_file, timeout=5, isolation_level=None)
    db.row_factory = sqlite3.Row
    db.execute('PRAGMA foreign_keys=ON')
    db.execute('PRAGMA busy_timeout=5000')
    acquired = None
    def alarm(*args):
        raise TransactionDeadline('transaction_deadline')
    prior_handler = signal.signal(signal.SIGALRM, alarm)
    lock_started = time.monotonic()
    committed = False
    try:
        db.execute('BEGIN IMMEDIATE')
        acquired = time.monotonic()
        signal.setitimer(signal.ITIMER_REAL, 10)
        db.set_progress_handler(lambda: int(time.monotonic() - acquired >= 10), 1000)
        with db_ro(reference_file) as reference:
            context = exact_context(reference)
        actual = exact_context(db)
        assert actual == context, 'transaction_context_drift'
        columns = [r[1] for r in db.execute('PRAGMA table_info(upstream_credentials)')]
        assert actual['target'][columns.index('ciphertext')] == expected, 'expected_ciphertext_drift'
        parent_id = actual['parent_config'][0]
        plain = clear_open(actual['parent'][columns.index('ciphertext')], parent_id, key)
        try:
            verified = clear_open(desired if forward else expected, loc['config_version_id'], key)
            try:
                assert hmac.compare_digest(plain, verified), 'plaintext_changed'
            finally:
                verified[:] = bytes(len(verified))
            original = expected if forward else desired
            assert hmac.compare_digest(original, actual['parent'][columns.index('ciphertext')]), 'original_parent_drift'
            try:
                bad = clear_open(original, loc['config_version_id'], key)
            except InvalidTag:
                pass
            else:
                bad[:] = bytes(len(bad))
                raise AssertionError('original_no_longer_bad')
        finally:
            plain[:] = bytes(len(plain))
        before = row_data(db)
        changed = db.execute(SQL, (desired, loc['config_version_id'], loc['credential_id'], loc['upstream_id'], expected, parent_id)).rowcount
        assert changed == 1, 'cas_not_one_row'
        after = row_data(db)
        only_cipher_delta(db, before, after, expected, desired)
        assert db.execute('PRAGMA integrity_check').fetchone()[0] == 'ok' and not db.execute('PRAGMA foreign_key_check').fetchall(), 'precommit_integrity'
        assert time.monotonic() - acquired < 10, 'transaction_deadline'
        db.execute('COMMIT')
        committed = True
        return {'status': 'PASS', 'changed_rows': changed, 'changed_columns': ['ciphertext'], 'other_tables_unchanged_in_writer_transaction': len(before) - 1, 'all_other_rows_and_columns_unchanged': True, 'source_plaintext_exactly_preserved': True, 'lock_wait_seconds': round(acquired - lock_started, 6), 'transaction_seconds': round(time.monotonic() - acquired, 6), 'lock_timeout_seconds': 5, 'transaction_deadline_seconds': 10, 'commit_completed': True}
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        db.set_progress_handler(None, 0)
        signal.signal(signal.SIGALRM, prior_handler)
        if not committed and db.in_transaction:
            db.execute('ROLLBACK')
        db.close()
        key[:] = bytes(len(key))
