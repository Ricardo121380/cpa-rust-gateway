"""One body-only append per existing progress record, followed by independent GETs.

Requires the authorized, reviewed suffix file as its sole argument. Raw GitHub
snapshots stay local; no issue body, state, labels, milestone or dependency write.
Any failure stops without retrying a potentially applied PATCH.
"""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[5]
PUBLIC = Path(__file__).resolve().parent.parent
PRIVATE = ROOT / 'output/cpar-native-chat-offline-20261017'
REPO = 'repos/Ricardo121380/cpa-rust-gateway/'
COMMENTS = {47:6081215242,57:6081234278,9:6081235490}
os.umask(0o077)


def save(path, value):
    path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')


def api(path, payload=None):
    command = ['gh','api',REPO+path]
    if payload is not None:
        command += ['--method','PATCH','--input',str(payload)]
    elif '/dependencies/' in path:
        command += ['--paginate','--slurp']
    result = subprocess.run(command,capture_output=True,timeout=45)
    if result.returncode:
        raise RuntimeError('GitHub API failed; no retry; stderr_sha256='+hashlib.sha256(result.stderr).hexdigest())
    return json.loads(result.stdout)


def snapshot(number, folder, phase):
    value = {
        'issue':api(f'issues/{number}'),
        'comment':api(f'issues/comments/{COMMENTS[number]}'),
        'deps':api(f'issues/{number}/dependencies/blocked_by?per_page=100'),
    }
    for key, data in value.items():
        save(folder/f'{number}-{phase}-{key}.json',data)
    return value


def protected_issue(value):
    return {key:child for key,child in value.items() if key != 'updated_at'}


def main():
    assert len(sys.argv)==2
    suffix=Path(sys.argv[1]).read_text()
    assert suffix.startswith('\n\n## 2026-10-17')
    folder=PRIVATE/'github-sync'
    folder.mkdir(mode=0o700,exist_ok=False)
    before={number:snapshot(number,folder,'before') for number in COMMENTS}
    for number,row in before.items():
        baseline=json.loads((PRIVATE/f'github-baseline/{number}-comment.json').read_text())
        issue=json.loads((PRIVATE/f'github-baseline/{number}-issue.json').read_text())
        deps=json.loads((PRIVATE/f'github-baseline/{number}-deps.json').read_text())
        assert row['comment']['body']==baseline['body']
        assert protected_issue(row['issue'])==protected_issue(issue)
        assert row['deps']==deps and row['issue']['state']=='open'
        assert 'pull_request' not in row['issue']
        assert row['comment']['id']==COMMENTS[number]
        assert len(row['comment']['body']+suffix)<65536
    expected={}
    records=[]
    for number,row in before.items():
        # Recheck the exact record immediately before its one authorized PATCH.
        current=api(f'issues/comments/{COMMENTS[number]}')
        assert current['body']==row['comment']['body']
        body=current['body']+suffix
        expected[number]=body
        payload=folder/f'{number}-payload.json'
        save(payload,{'body':body})
        response=api(f'issues/comments/{COMMENTS[number]}',payload)
        save(folder/f'{number}-patch.json',response)
        independent=api(f'issues/comments/{COMMENTS[number]}')
        save(folder/f'{number}-immediate-comment.json',independent)
        assert independent['body']==body and independent['body'].startswith(current['body'])
    after={number:snapshot(number,folder,'after') for number in COMMENTS}
    for number,row in after.items():
        assert row['comment']['body']==expected[number]
        assert protected_issue(row['issue'])==protected_issue(before[number]['issue'])
        assert row['issue']['state']=='open'
        old=sum(before[number]['deps'],[])
        new=sum(row['deps'],[])
        assert len(old)==len(new)
        for initial,final in zip(old,new):
            assert protected_issue(initial)==protected_issue(final)
            if initial['updated_at']!=final['updated_at']:
                assert final['number'] in after
                assert final['updated_at']==after[final['number']]['issue']['updated_at']
        records.append({
            'issue':number,'comment_id':COMMENTS[number],
            'url':row['comment']['html_url'],'status':'PASS',
            'body_prefix_preserved':True,'body_only_patch_count':1,
            'body_sha256':hashlib.sha256(expected[number].encode()).hexdigest(),
            'issue_body_and_metadata_preserved':True,'state':'open',
            'dependency_pages':len(row['deps']),'dependency_count':len(new),
            'open_dependencies':[item['number'] for item in new if item['state']=='open'],
            'dependency_metadata_and_states_preserved':True,
        })
    summary={'status':'PASS','observed_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'records':records,'patch_count':3,'created_comments':0,'closed_issues':0,
        'issue_body_AC_metadata_writes':0,'provider_calls':0,'push':False}
    save(PUBLIC/'github-sync-summary.json',summary)
    print(json.dumps(summary,indent=2))


if __name__=='__main__':
    main()
