import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
receipt = Path(sys.argv[2]).resolve()
command = sys.argv[3:]
tracked = subprocess.check_output(['git', 'diff', '--name-only', '9265e6cea83b6172742dc2e4e60b5e5d94c1a1c2'], cwd=root, text=True).splitlines()
tracked += ['apps/gateway/src/runtime/batch_c/http/responses_done/responses_order.rs', 'crates/protocol-openai-responses/src/upstream_response.rs']
sources = {p: hashlib.sha256((root / p).read_bytes()).hexdigest() for p in sorted(set(tracked)) if (root / p).is_file()}
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
env = os.environ.copy()
env['CARGO_NET_OFFLINE'] = 'true'
print('Running ' + ' '.join(command), flush=True)
result = subprocess.run(command, cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
data = {'command':command, 'cwd':str(root), 'started_utc':started, 'finished_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(), 'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(), 'status':subprocess.check_output(['git','status','--porcelain=v1'],cwd=root,text=True), 'environment':{'CARGO_NET_OFFLINE':'true'}, 'source_files_sha256':sources, 'exit_code':result.returncode, 'counts':[{'passed':int(p),'failed':int(f),'ignored':int(i)} for p,f,i in re.findall(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',result.stdout)], 'output_sha256':hashlib.sha256(result.stdout.encode()).hexdigest(), 'output':result.stdout}
receipt.parent.mkdir(parents=True, exist_ok=True)
receipt.write_text(json.dumps(data,indent=2)+'\n')
print(result.stdout[-18000:], flush=True)
print('Captured actual exit code: ' + str(result.returncode), flush=True)
sys.exit(result.returncode)
