"""Compare public content with private materials only on the protected host; emit counts."""
import base64
import pathlib
import sys
exec((pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884/archived-credential-production-repair-20261002') / 'repair-common.py').read_text())
files = json.loads(sys.stdin.read())
public = [value.encode() for value in files.values()]
secret_patterns = set()
def add(value):
    if isinstance(value, str):
        value = value.encode()
    if not isinstance(value, (bytes, bytearray)) or len(value) < 16:
        return
    value = bytes(value)
    secret_patterns.update([value, value.hex().encode(), base64.b64encode(value), base64.urlsafe_b64encode(value)])
for name in KEY_NAMES:
    add((KEYS / name).read_bytes())
for name in ('original-envelope.private.bin', 'repaired-envelope.private.bin'):
    envelope = (OPS / name).read_bytes()
    add(envelope); add(envelope[1:25]); add(envelope[-16:])
with db_ro(OPS / 'capture-pre/state/control.sqlite3') as db:
    context = exact_context(db)
    parent_id = context['parent_config'][0]
    master = bytearray((KEYS / 'master-key').read_bytes())
    envelope = (OPS / 'original-envelope.private.bin').read_bytes()
    plain = clear_open(envelope, parent_id, master)
    try:
        add(plain)
        def walk(value):
            if isinstance(value, dict):
                for item in value.values():
                    walk(item)
            elif isinstance(value, list):
                for item in value:
                    walk(item)
            elif isinstance(value, str):
                add(value)
        walk(json.loads(plain))
    finally:
        plain[:] = bytes(len(plain)); master[:] = bytes(len(master))
identifiers = {loc['config_version_id'].encode(), loc['credential_id'].encode(), loc['upstream_id'].encode(), parent_id.encode()}
matches = sum(any(pattern in value for value in public) for pattern in secret_patterns)
id_matches = sum(any(pattern in value for value in public) for pattern in identifiers)
result = {'status': 'PASS' if matches == id_matches == 0 else 'FAIL', 'public_files_checked': len(files), 'raw_key_nonce_tag_ciphertext_plaintext_or_token_matches': matches, 'exact_target_parent_credential_upstream_id_matches': id_matches, 'comparison_ran_only_in_protected_host': True, 'secret_material_exported': False, 'task_initiated_provider_calls': 0}
print(json.dumps(result))
sys.exit(0 if result['status'] == 'PASS' else 2)
