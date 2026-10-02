"""Re-seal only AC-01 in a new host-private copy using its authenticated exact parent."""
import datetime
import importlib.util
import json
import pathlib
import sqlite3
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

TASK = pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884')
exec((TASK / 'operator-tools/target-rehearsal.py').read_text().split('assert ROOT.lstat()')[0])
DIAG = TASK / 'archived-credential-diagnosis-20261002'
CAP = TASK / 'capture-2'
loc = json.loads((DIAG / 'target.private.json').read_text())
destination = DIAG / 'repaired-pristine'
assert not destination.exists(), 'never_overwrite_prior_repair'
destination.mkdir(mode=0o700)
state_copy(CAP / 'state', destination / 'state')
state_copy(CAP / 'credentials', destination / 'credentials')
key = bytearray((CAP / 'credentials/master-key').read_bytes())
spec = importlib.util.spec_from_file_location('crypto', TASK / 'operator-tools/private-crypto-audit.py')
crypto = importlib.util.module_from_spec(spec)
spec.loader.exec_module(crypto)

def aad(config, credential, upstream):
    data = bytearray(b'cpa-rust-gateway/control-plane/credential-aad/v1')
    for field in (config, credential, upstream):
        raw = field.encode()
        data.extend(len(raw).to_bytes(4, 'big'))
        data.extend(raw)
    return bytes(data)

def clear_open(envelope, context):
    nonce = envelope[1:25]
    return bytearray(ChaCha20Poly1305(crypto.hchacha(bytes(key), nonce[:16])).decrypt(b'\0' * 4 + nonce[16:], envelope[25:], aad(*context)))

def all_rows(db):
    return {name: sorted((tuple(row) for row in db.execute('SELECT * FROM ' + q(name))), key=lambda row: repr(row)) for (name,) in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")}

try:
    with sqlite3.connect(destination / 'state/control.sqlite3') as db:
        db.row_factory = sqlite3.Row
        db.execute('PRAGMA foreign_keys=ON')
        before = all_rows(db)
        version = db.execute('SELECT * FROM config_versions WHERE id=?', (loc['config_version_id'],)).fetchone()
        target = db.execute('SELECT * FROM upstream_credentials WHERE config_version_id=? AND id=? AND upstream_id=?', (loc['config_version_id'], loc['credential_id'], loc['upstream_id'])).fetchone()
        parent = db.execute('SELECT * FROM upstream_credentials WHERE config_version_id=? AND id=? AND upstream_id=?', (version['parent_id'], loc['credential_id'], loc['upstream_id'])).fetchone()
        assert version['status'] == 'archived' and version['revision'] == 0
        assert parent is not None and target['ciphertext'] == parent['ciphertext']
        assert target['key_version'] == parent['key_version'] == 1
        assert target['revision'] == parent['revision'] == 2 and target['status'] == parent['status'] == 'active'
        assert target['kind'] == parent['kind'] == 'oauth_json'
        assert hashlib.sha256(target['ciphertext']).hexdigest() == loc['ciphertext_sha256']
        plain = clear_open(parent['ciphertext'], (version['parent_id'], loc['credential_id'], loc['upstream_id']))
        try:
            nonce = os.urandom(24)
            replacement = b'\x01' + nonce + ChaCha20Poly1305(crypto.hchacha(bytes(key), nonce[:16])).encrypt(b'\0' * 4 + nonce[16:], bytes(plain), aad(loc['config_version_id'], loc['credential_id'], loc['upstream_id']))
            opened = clear_open(replacement, (loc['config_version_id'], loc['credential_id'], loc['upstream_id']))
            try:
                assert hmac.compare_digest(plain, opened)
            finally:
                opened[:] = b'\0' * len(opened)
            (DIAG / 'original-envelope.private.bin').write_bytes(target['ciphertext'])
            (DIAG / 'repaired-envelope.private.bin').write_bytes(replacement)
            cursor = db.execute("UPDATE upstream_credentials SET ciphertext=? WHERE config_version_id=? AND id=? AND upstream_id=? AND ciphertext=? AND key_version=1 AND revision=2 AND status='active'", (replacement, loc['config_version_id'], loc['credential_id'], loc['upstream_id'], target['ciphertext']))
            assert cursor.rowcount == 1
            db.commit()
        finally:
            plain[:] = b'\0' * len(plain)
        after = all_rows(db)
        changed = [name for name in before if before[name] != after[name]]
        assert changed == ['upstream_credentials']
        columns = [r['name'] for r in db.execute('PRAGMA table_info(upstream_credentials)')]
        cipher_index = columns.index('ciphertext')
        def except_cipher(rows):
            return sorted([r[:cipher_index] + r[cipher_index + 1:] for r in rows], key=repr)
        assert except_cipher(before['upstream_credentials']) == except_cipher(after['upstream_credentials'])
        assert sum(1 for r in after['upstream_credentials'] if r not in before['upstream_credentials']) == 1
        assert db.execute('PRAGMA integrity_check').fetchall()[0][0] == 'ok' and not db.execute('PRAGMA foreign_key_check').fetchall()
        schema = db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0]
    baseline = json.loads((CAP / 'logical-before.private.json').read_text())
    preservation = preserved(destination / 'state', baseline, (CAP / 'integrity-hmac-key').read_bytes())
    assert all(r['preserved'] for db_name, tables in preservation.items() for table, r in tables.items() if (db_name, table) != ('control.sqlite3', 'upstream_credentials'))
    receipt = {'status': 'PASS', 'record_alias': 'AC-01', 'method': 'authenticate byte-identical exact parent envelope under parent AAD; fresh-nonce re-seal under exact child AAD', 'trusted_source': {'same_credential_and_upstream': True, 'direct_parent': True, 'ciphertext_byte_identical': True, 'kind_status_revision_key_version_equal': True, 'source_aead_authenticated': True, 'plaintext_exactly_preserved': True}, 'changed_tables': changed, 'changed_rows': 1, 'changed_columns': ['ciphertext'], 'other_original_tables_preserved': sum(len(t) for t in preservation.values()) - 1, 'credential_and_config_status_and_revisions_preserved': True, 'all_six_keys_identical': all((destination / 'credentials' / name).read_bytes() == (CAP / 'credentials' / name).read_bytes() for name in KEY_NAMES), 'admin_auth_database_byte_identical': (destination / 'state/admin-auth.sqlite3').read_bytes() == (CAP / 'state/admin-auth.sqlite3').read_bytes(), 'schema_version': schema, 'integrity_check': 'ok', 'foreign_key_errors': 0, 'crypto_after': crypto.authenticate(destination / 'state/control.sqlite3', destination / 'credentials'), 'production_repaired': False, 'production_writes': 0, 'original_failure_and_pristine_copy_retained': True, 'recovered_plaintext_exported': False, 'completed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    assert receipt['crypto_after']['status'] == 'PASS'
    (DIAG / 'repair-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))
finally:
    key[:] = b'\0' * len(key)
