"""Check archived bytes, execution ownership and successor audit invariants."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

assets = Path(__file__).resolve().parent
repo = assets.parents[3]


def read(name):
    return json.loads((assets / name).read_text())


def digest(data):
    return hashlib.sha256(data).hexdigest()


index = read('execution-index.json')
secret_pattern = r'''(?i)(authorization:[\s"']*bearer\s+[A-Za-z0-9._~+/=-]{16,}|(?:api[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|management[_-]?key)["']?\s*[:=]\s*["']?[A-Za-z0-9._~+/=-]{16,}|-----BEGIN[[:space:]][A-Z0-9[:space:]]*PRIVATE KEY-----|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{20,}|sk-[A-Za-z0-9_-]{16,}|ksk_[A-Za-z0-9_-]{16,})'''
for item in index['entries']:
    archived = assets / item['archived_raw_log_gzip']
    encoded = archived.read_bytes()
    raw = gzip.decompress(encoded)
    assert digest(encoded) == item['archive_sha256'], item['label']
    assert digest(raw) == item['raw_log_sha256'], item['label']
    assert len(raw) == item['raw_log_bytes'], item['label']
    receipt_bytes = (assets / item['archived_receipt']).read_bytes()
    assert digest(receipt_bytes) == item['receipt_sha256'], item['label']
    receipt = json.loads(receipt_bytes)
    for field in ('argv', 'cwd', 'utc_start', 'utc_end', 'exit_code', 'source_before', 'source_after'):
        assert receipt[field] == item[field], (item['label'], field)
    assert receipt['log_sha256'] == digest(raw) and receipt['bytes'] == len(raw)
    scan = subprocess.run(['rg', '--pcre2', '-a', secret_pattern], input=raw,
                          stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    assert scan.returncode == 1, ('archive secret scan failed without value output', item['label'])

matrix = read('issue57-successor.json')
rows = matrix['current_live_combinations']
assert len(rows) == len({row['id'] for row in rows}) == 198
assert sum(row['applicable'] for row in rows) == 186
assert sum(row['status'] == 'NOT_APPLICABLE_ACCEPTED' for row in rows) == 12
assert sum(bool(row['software_blockers']) for row in rows) == 24
assert all(not row['executed'] and row['execution_source_sha'] is None and row['real_provider_calls'] == 0 for row in rows)
extensions = matrix['current_extension_readiness']
assert len(extensions) == 49 and all(row['actual_declared'] is None for row in extensions)
historical_bytes = (repo / matrix['historical_matrix']['path']).read_bytes()
assert digest(historical_bytes) == matrix['historical_matrix']['sha256']
history = json.loads(historical_bytes)
for name, expected in (('stories', 88), ('baseline_findings', 8), ('acceptance_tests', 15), ('issue57_clauses', 10)):
    assert len(matrix[name]) == len(history[name]) == expected
    assert [row['id'] for row in matrix[name]] == [row['id'] for row in history[name]]
assert matrix['historical_kiro_cap_rows'] == history['historical_kiro_cap_rows']
assert matrix['historical_rich_metadata_rows'] == history['historical_rich_metadata_rows']
assert matrix['E05']['current'] == 'APPROVED_PASS_LOCAL_REUSED'
verification = read('verification-summary.json')
assert verification['full']['steps'] == 45 and verification['full']['rust'] == {'passed': 1500, 'failed': 0, 'ignored': 12}
assert verification['t12']['checks'] == 71 and verification['t12']['actual_loopback_receives'] == 5
assert verification['t12']['fallback_receives'] == 0
assert read('source-binding.json')['behavior_and_acceptance_source'] == verification['behavior_source'] == matrix['audit_source_sha']
assert read('preservation-check.json')['agents_sha256_unchanged']
assert not read('preservation-check.json')['changes']
print(json.dumps({'status': 'PASS', 'execution_entries': len(index['entries']), 'decompressed_archive_secret_scan': 'PASS',
                  'base_rows': len(rows), 'extension_rows': len(extensions), 'historical_bytes_unchanged': True,
                  'real_provider_calls': 0}))
