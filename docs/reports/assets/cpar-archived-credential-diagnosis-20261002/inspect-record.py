"""Bounded host-only diagnosis. Output contains no raw IDs, envelopes or key material."""
import datetime
import hashlib
import importlib.util
import json
import os
import pathlib
import sqlite3
import stat
from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

TASK = pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884')
DIAG = TASK / 'archived-credential-diagnosis-20261002'
os.umask(0o077)
assert os.geteuid() == 0 and not TASK.is_symlink()
assert TASK.stat().st_uid == 0 and stat.S_IMODE(TASK.stat().st_mode) == 0o700
DIAG.mkdir(mode=0o700, exist_ok=True)
assert not DIAG.is_symlink() and stat.S_IMODE(DIAG.stat().st_mode) == 0o700
CAP = TASK / 'capture-2'
locators = json.loads((TASK / 'credential-audit-failures.private.json').read_text())
assert len(locators) == 1
loc = locators[0]
spec = importlib.util.spec_from_file_location('crypto', TASK / 'operator-tools/private-crypto-audit.py')
crypto = importlib.util.module_from_spec(spec)
spec.loader.exec_module(crypto)
key = bytearray((CAP / 'credentials/master-key').read_bytes())
assert len(key) == 32

def ro(p):
    connection = sqlite3.connect(f'file:{p}?mode=ro&immutable=1', uri=True)
    connection.row_factory = sqlite3.Row
    return connection

def utc(ms):
    return datetime.datetime.fromtimestamp(ms / 1000, datetime.timezone.utc).isoformat()

def aad(config, credential, upstream):
    encoded = bytearray(b'cpa-rust-gateway/control-plane/credential-aad/v1')
    for field in (config, credential, upstream):
        raw = field.encode('utf-8')
        encoded.extend(len(raw).to_bytes(4, 'big'))
        encoded.extend(raw)
    return bytes(encoded)

def open_record(row, context=None):
    envelope = row['ciphertext']
    if row['key_version'] != 1:
        return {'status': 'FAIL', 'category': 'KEY_VERSION_UNAVAILABLE'}
    if len(envelope) < 41 or envelope[0] != 1:
        return {'status': 'FAIL', 'category': 'ENVELOPE_STRUCTURE'}
    nonce = envelope[1:25]
    context = context or (row['config_version_id'], row['id'], row['upstream_id'])
    try:
        plain = bytearray(ChaCha20Poly1305(crypto.hchacha(bytes(key), nonce[:16])).decrypt(b'\0' * 4 + nonce[16:], envelope[25:], aad(*context)))
        try:
            return {'status': 'PASS', 'nonempty': bool(plain)}
        finally:
            plain[:] = b'\0' * len(plain)
    except InvalidTag:
        return {'status': 'FAIL', 'category': 'AEAD_INVALID_TAG'}

try:
    with ro(CAP / 'state/control.sqlite3') as db:
        row = db.execute('SELECT * FROM upstream_credentials WHERE config_version_id=? AND id=? AND upstream_id=?', (loc['config_version_id'], loc['credential_id'], loc['upstream_id'])).fetchone()
        assert row is not None and hashlib.sha256(row['ciphertext']).hexdigest() == loc['ciphertext_sha256']
        version = db.execute('SELECT * FROM config_versions WHERE id=?', (loc['config_version_id'],)).fetchone()
        related = db.execute('SELECT c.*,v.status lifecycle,v.created_at_ms,v.parent_id FROM upstream_credentials c JOIN config_versions v ON v.id=c.config_version_id WHERE c.id=? ORDER BY v.created_at_ms,c.config_version_id', (loc['credential_id'],)).fetchall()
        aliases = {r['config_version_id']: ('AC-01' if r['config_version_id'] == loc['config_version_id'] else 'related-' + str(i + 1)) for i, r in enumerate(related)}
        candidates = []
        for item in related:
            candidates.append({'alias': aliases[item['config_version_id']], 'configuration_lifecycle': item['lifecycle'], 'credential_status': item['status'], 'credential_kind': item['kind'], 'credential_revision': item['revision'], 'configuration_created_at_utc': utc(item['created_at_ms']), 'same_upstream': item['upstream_id'] == row['upstream_id'], 'same_ciphertext': item['ciphertext'] == row['ciphertext'], 'parent_alias': aliases.get(item['parent_id']), 'exact_record_context_probe': open_record(item), 'failed_envelope_with_related_context': open_record(row, (item['config_version_id'], item['id'], item['upstream_id']))})
        bindings = db.execute('SELECT b.enabled binding_enabled,e.enabled endpoint_enabled,u.enabled upstream_enabled FROM endpoint_credential_bindings b JOIN upstream_endpoints e ON e.config_version_id=b.config_version_id AND e.id=b.endpoint_id JOIN upstreams u ON u.config_version_id=b.config_version_id AND u.id=b.upstream_id WHERE b.config_version_id=? AND b.credential_id=?', (loc['config_version_id'], loc['credential_id'])).fetchall()
        audit = [{'action': r['action'], 'at_utc': utc(r['occurred_at_ms'])} for r in db.execute('SELECT action,occurred_at_ms FROM management_audit_events WHERE config_version_id=? OR replaced_config_version_id=? ORDER BY id', (loc['config_version_id'], loc['config_version_id']))]
        resource = [{'action': r['action'], 'resource_kind': r['resource_kind'], 'at_utc': utc(r['occurred_at_ms'])} for r in db.execute('SELECT action,resource_kind,occurred_at_ms FROM management_resource_audit_events WHERE config_version_id=? AND (resource_id=? OR resource_kind=\'config_version\') ORDER BY id', (loc['config_version_id'], loc['credential_id']))]
        active = db.execute("SELECT id FROM config_versions WHERE status='active'").fetchall()
        assert len(active) == 1
        latest = db.execute("SELECT config_version_id,replaced_config_version_id FROM management_audit_events WHERE action IN ('config_published','config_rolled_back') ORDER BY id DESC LIMIT 1").fetchone()
        origin = db.execute('SELECT source_version_id FROM configuration_edit_origins WHERE config_version_id=?', (loc['config_version_id'],)).fetchone()
        output = {'status': 'OBSERVED', 'record_alias': 'AC-01', 'captured_at_utc': '2026-10-02T02:47:53.274953+00:00', 'configuration_lifecycle': version['status'], 'credential_status': row['status'], 'credential_kind': row['kind'], 'credential_revision': row['revision'], 'configuration_revision': version['revision'], 'configuration_created_at_utc': utc(version['created_at_ms']), 'envelope': {'format_version': row['ciphertext'][0], 'bytes': len(row['ciphertext']), 'nonce_bytes': 24, 'tag_bytes': 16, 'key_version': row['key_version']}, 'exact_context_probe': open_record(row), 'related_same_credential_id': candidates, 'binding_count': len(bindings), 'enabled_bindings': sum(bool(r['binding_enabled']) for r in bindings), 'fully_enabled_bindings': sum(all(r) for r in bindings), 'active_configuration_is_target': active[0]['id'] == loc['config_version_id'], 'active_same_credential_id_count': db.execute('SELECT count(*) FROM upstream_credentials WHERE config_version_id=? AND id=?', (active[0]['id'], loc['credential_id'])).fetchone()[0], 'latest_rollback_predecessor_is_target': latest is not None and latest['replaced_config_version_id'] == loc['config_version_id'], 'edit_origin_present': origin is not None, 'edit_origin_alias': aliases.get(origin['source_version_id']) if origin else None, 'lifecycle_audit': audit, 'targeted_resource_audit': resource, 'refresh_state_count': db.execute('SELECT count(*) FROM credential_refresh_state WHERE config_version_id=? AND credential_id=?', (loc['config_version_id'], loc['credential_id'])).fetchone()[0], 'source_db_sha256': hashlib.sha256((CAP / 'state/control.sqlite3').read_bytes()).hexdigest(), 'plaintext_exported': False, 'raw_locator_exported': False, 'production_writes': 0}
        (DIAG / 'target.private.json').write_text(json.dumps(loc, indent=2) + '\n')
        (DIAG / 'related-contexts.private.json').write_text(json.dumps([{'config_version_id': r['config_version_id'], 'credential_id': r['id'], 'upstream_id': r['upstream_id'], 'alias': aliases[r['config_version_id']]} for r in related], indent=2) + '\n')
        (DIAG / 'inspection.json').write_text(json.dumps(output, indent=2) + '\n')
        print(json.dumps(output))
finally:
    key[:] = b'\0' * len(key)
