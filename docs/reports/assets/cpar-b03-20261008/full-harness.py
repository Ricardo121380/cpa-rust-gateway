import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
assets = Path(sys.argv[2]).resolve()
source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
before = subprocess.check_output(['git', 'status', '--porcelain=v1', '--untracked-files=all'], cwd=root, text=True)
assert not before, before
paths = subprocess.check_output(['git', 'diff', '--name-only', '9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2', 'HEAD'], cwd=root, text=True).splitlines()
hashes = {p: hashlib.sha256((root/p).read_bytes()).hexdigest() for p in paths}
env = os.environ.copy()
env['CARGO_NET_OFFLINE'] = 'true'
env['CARGO_TARGET_DIR'] = str(assets.parents[3] / 'target')
env['CHECK_REPORT_PATH'] = str(assets/'full-check.md')
started = datetime.datetime.now(datetime.timezone.utc)
print('Full source=' + source + '; checkout clean', flush=True)
result = subprocess.run(['bash','scripts/check.sh','full'],cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
finished = datetime.datetime.now(datetime.timezone.utc)
after = subprocess.check_output(['git','status','--porcelain=v1','--untracked-files=all'],cwd=root,text=True)
unchanged = all(hashlib.sha256((root/p).read_bytes()).hexdigest()==sha for p,sha in hashes.items())
output = {'source_commit':source, 'command':['bash','scripts/check.sh','full'], 'output':result.stdout}
(assets/'full-output.json').write_text(json.dumps(output,indent=2)+'\n')
report = (assets/'full-check.md').read_text() if (assets/'full-check.md').exists() else ''
counts = re.findall(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',result.stdout)
receipt = {'source_commit':source,'command':['bash','scripts/check.sh','full'],'checkout':str(root),'status_before':before,'status_after':after,'source_files_sha256':hashes,'source_hashes_unchanged':unchanged,'environment':{k:env[k] for k in ['CARGO_NET_OFFLINE','CARGO_TARGET_DIR','CHECK_REPORT_PATH']},'started_utc':started.isoformat(),'finished_utc':finished.isoformat(),'elapsed_seconds':(finished-started).total_seconds(),'exit_code':result.returncode,'steps':{'passed':len(re.findall(r'^\|.*\| PASS \|$',report,re.M)),'failed':len(re.findall(r'^\|.*\| FAIL',report,re.M))},'rust_tests':{k:sum(int(c[i]) for c in counts) for i,k in enumerate(['passed','failed','ignored'])},'output_sha256':hashlib.sha256(result.stdout.encode()).hexdigest(),'report_sha256':hashlib.sha256(report.encode()).hexdigest(),'boundary':'Local exact-source clean checkout. No real Provider, remote CI, artifact signing, deployment or production acceptance.'}
(assets/'full-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(result.stdout[-8000:],flush=True)
print(json.dumps({k:receipt[k] for k in ['exit_code','steps','rust_tests','status_after','source_hashes_unchanged']}),flush=True)
sys.exit(result.returncode or (1 if after or not unchanged else 0))
