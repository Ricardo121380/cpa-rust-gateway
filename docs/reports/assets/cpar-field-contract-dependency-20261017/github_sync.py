"""Authorized body-only suffix on three existing records; no mutation retry."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
PUBLIC = Path(__file__).resolve().parent
WORK = ROOT / 'output/cpar-field-contract-dependency-20261017'
REPO = 'repos/Ricardo121380/cpa-rust-gateway/'
COMMENTS = {47: 6081215242, 57: 6081234278, 9: 6081235490}
os.umask(0o077)

def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n')

def api(path, payload=None):
    cmd = ['gh', 'api', REPO+path]
    if payload:
        cmd += ['--method', 'PATCH', '--input', str(payload)]
    elif '/dependencies/' in path or '/comments?' in path:
        cmd += ['--paginate', '--slurp']
    result = subprocess.run(cmd, capture_output=True, timeout=45)
    if result.returncode:
        raise RuntimeError('GitHub operation failed; no retry; stderr_sha256='+hashlib.sha256(result.stderr).hexdigest())
    return json.loads(result.stdout)

def snapshot(number, folder, phase):
    data = {
        'issue': api(f'issues/{number}'),
        'comment': api(f'issues/comments/{COMMENTS[number]}'),
        'comments': api(f'issues/{number}/comments?per_page=100'),
        'deps': api(f'issues/{number}/dependencies/blocked_by?per_page=100'),
        'blocking': api(f'issues/{number}/dependencies/blocking?per_page=100'),
    }
    for key, value in data.items():
        save(folder/f'{number}-{phase}-{key}.json', value)
    assert any(x['id'] == COMMENTS[number] and x['body'] == data['comment']['body'] for page in data['comments'] for x in page)
    return data

def protected_issue(value):
    return {k: v for k, v in value.items() if k != 'updated_at'}

def main():
    assert len(sys.argv) == 2
    suffix = Path(sys.argv[1]).read_text()
    assert suffix.startswith('\n\n## 2026-10-17')
    folder = WORK/'github-sync'
    folder.mkdir(mode=0o700, exist_ok=False)
    before = {n: snapshot(n, folder, 'before') for n in COMMENTS}
    for number, data in before.items():
        baseline = {k: json.loads((WORK/f'github-baseline/{number}-{k}.json').read_text()) for k in data}
        assert protected_issue(data['issue']) == protected_issue(baseline['issue'])
        assert data['issue']['state'] == 'open' and 'pull_request' not in data['issue']
        assert data['comment']['body'] == baseline['comment']['body']
        assert data['deps'] == baseline['deps'] and data['blocking'] == baseline['blocking']
        assert len(data['comment']['body']+suffix) < 65536
    windows = {}
    for number, data in before.items():
        current = api(f'issues/comments/{COMMENTS[number]}')
        assert current['body'] == data['comment']['body']
        body = current['body']+suffix
        payload = folder/f'{number}-payload.json'
        save(payload, {'body': body})
        started = datetime.datetime.now(datetime.timezone.utc)
        patched = api(f'issues/comments/{COMMENTS[number]}', payload)
        finished = datetime.datetime.now(datetime.timezone.utc)
        windows[number] = (started, finished)
        save(folder/f'{number}-patch.json', patched)
        independent = api(f'issues/comments/{COMMENTS[number]}')
        save(folder/f'{number}-immediate-comment.json', independent)
        assert independent['body'] == body and independent['body'].startswith(current['body'])
        assert independent['id'] == current['id'] and independent['html_url'] == current['html_url']
        assert independent['user']['login'] == current['user']['login']
    after = {n: snapshot(n, folder, 'after') for n in COMMENTS}
    records = []
    for number, data in after.items():
        initial = before[number]
        assert data['comment']['body'] == initial['comment']['body']+suffix
        assert protected_issue(data['issue']) == protected_issue(initial['issue'])
        if data['issue']['updated_at'] != initial['issue']['updated_at']:
            observed = datetime.datetime.fromisoformat(data['issue']['updated_at'].replace('Z', '+00:00'))
            start, end = windows[number]
            assert start.replace(microsecond=0) <= observed <= end+datetime.timedelta(seconds=1)
            assert data['issue']['updated_at'] == data['comment']['updated_at']
        for edge in ['deps', 'blocking']:
            left = sum(initial[edge], []); right = sum(data[edge], [])
            assert len(left) == len(right)
            for a, b in zip(left, right):
                assert protected_issue(a) == protected_issue(b)
                if a['updated_at'] != b['updated_at']:
                    assert b['number'] in after and b['updated_at'] == after[b['number']]['issue']['updated_at']
        deps = sum(data['deps'], [])
        records.append({'issue': number, 'status': 'PASS', 'state': 'open', 'comment_id': COMMENTS[number],
            'url': data['comment']['html_url'], 'prefix_preserved': True, 'body_only_patch_count': 1,
            'issue_body_AC_metadata_preserved': True, 'both_dependency_directions_preserved': True,
            'dependency_pages': len(data['deps']), 'dependencies': len(deps),
            'open_dependencies': [x['number'] for x in deps if x['state'] == 'open'],
            'body_sha256': hashlib.sha256(data['comment']['body'].encode()).hexdigest(),
            'patch_started': windows[number][0].isoformat(), 'patch_finished': windows[number][1].isoformat()})
    bindings = {str(f.relative_to(ROOT)): {'bytes': f.stat().st_size, 'sha256': hashlib.sha256(f.read_bytes()).hexdigest()} for f in sorted(folder.glob('*.json'))}
    save(PUBLIC/'github-readback-bindings.json', {'files': bindings})
    summary = {'status': 'PASS', 'records': records, 'patches': 3, 'created_comments': 0, 'closed_issues': 0,
        'issue_body_AC_metadata_writes': 0, 'provider_calls': 0, 'push': False}
    save(PUBLIC/'github-sync-summary.json', summary)
    print(json.dumps(summary, indent=2))

if __name__ == '__main__':
    main()
