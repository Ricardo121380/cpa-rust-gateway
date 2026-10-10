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
closeout = read('closeout-execution-index.json')
assert closeout['initial_execution_index_sha256'] == digest((assets / 'execution-index.json').read_bytes())
execution_entries = index['entries'] + closeout['entries']
secret_pattern = r'''(?i)(authorization:[\s"']*bearer\s+[A-Za-z0-9._~+/=-]{16,}|(?:api[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|management[_-]?key)["']?\s*[:=]\s*["']?[A-Za-z0-9._~+/=-]{16,}|-----BEGIN[[:space:]][A-Z0-9[:space:]]*PRIVATE KEY-----|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{20,}|sk-[A-Za-z0-9_-]{16,}|ksk_[A-Za-z0-9_-]{16,})'''
for item in execution_entries:
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
assert next(row for row in matrix['issue57_clauses'] if row['id'] == 'AC02')['status'] == 'PASS_LOCAL_BOUNDED'
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
review = read('review-final.json')
assert review['review_head'] == '85f43c2f28ee0a9bbaea3916abaec302b2cedcac'
assert review['sha256'] == digest((assets / review['ref']).read_bytes())
assert review['standards']['status'] == 'PASS' and review['standards']['hard_count'] == 0
assert review['spec']['current_slice'] == 'PASS' and review['spec']['hard_count'] == 0
assert review['spec']['historical_complete_coverage'] == 'PARTIAL'
github = read('github-sync-summary.json')
assert github['status'] == 'PASS' and github['implementation_ticket_counts'] == {'OPEN': 13, 'CLOSED': 35}
assert len(github['targets']) == 5 and github['new_comments'] == github['state_changes'] == github['dependency_changes'] == 0
for target in github['targets']:
    before, after = read(target['before_ref']), read(target['after_ref'])
    assert after['issue']['state'] == 'open' and target['state'] == 'OPEN'
    assert digest(after['issue']['body'].encode()) == target['body_sha256']
    assert sorted((dep['id'], dep['number']) for dep in before['blocked_by']) == sorted((dep['id'], dep['number']) for dep in after['blocked_by'])
    assert {row['id'] for row in before['comments']} == {row['id'] for row in after['comments']}
    comment = next(row for row in after['comments'] if row['id'] == target['comment_id'])
    old_comment = next(row for row in before['comments'] if row['id'] == target['comment_id'])
    assert digest(comment['body'].encode()) == target['comment_sha256']
    assert comment['body'].startswith(old_comment['body'])
    before_lines, after_lines = before['issue']['body'].splitlines(), after['issue']['body'].splitlines()
    assert len(before_lines) == len(after_lines)
    changes = [(old, new) for old, new in zip(before_lines, after_lines) if old != new]
    assert len(changes) == (1 if target['issue'] == 57 else 0)
    if changes:
        assert changes[0][0].startswith('- [ ] 受影响既有账号、停用意图、权限')
        assert changes[0][1] == changes[0][0].replace('[ ]', '[x]', 1)
print(json.dumps({'status': 'PASS', 'execution_entries': len(execution_entries), 'decompressed_archive_secret_scan': 'PASS',
                  'base_rows': len(rows), 'extension_rows': len(extensions), 'historical_bytes_unchanged': True,
                  'github_targets': len(github['targets']), 'real_provider_calls': 0}))
