import datetime, hashlib, json, os, pathlib, re, subprocess, sys, time

name, cwd, *command = sys.argv[1:]
root = pathlib.Path('/Volumes/Asgard/Agentprojects/CPA-Rust/repo/cpa-rust-gateway')
assets = root / 'docs/reports/assets/cpar-b04-20261008'
cwd = pathlib.Path(cwd).resolve()
def git(*args):
    return subprocess.check_output(['git', *args], cwd=cwd, text=True).strip()
paths = set(git('diff', 'HEAD', '--name-only').splitlines())
paths.update(git('ls-files', '--others', '--exclude-standard', 'apps', 'crates').splitlines())
paths = sorted(p for p in paths if p and (p.endswith('.rs') or p.startswith('docs/contracts/') or p.startswith('docs/adr/')))
hashes = {p: hashlib.sha256((cwd/p).read_bytes()).hexdigest() for p in paths if (cwd/p).is_file()}
start = datetime.datetime.now(datetime.timezone.utc).isoformat()
before = git('status', '--short')
head = git('rev-parse', 'HEAD')
tick = time.monotonic()
environment = os.environ.copy()
environment['CARGO_NET_OFFLINE'] = 'true'
environment['CARGO_TARGET_DIR'] = str(root/'target')
environment['CHECK_REPORT_PATH'] = str(assets/(name+'-check.md'))
completed = subprocess.run(command, cwd=cwd, env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
output = completed.stdout
raw = assets/(name+'-output.json')
raw.write_text(json.dumps({'output':output},ensure_ascii=False,indent=2)+'\n')
totals = [(int(p),int(f),int(i)) for p,f,i in re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;',output)]
receipt = {'name':name,'cwd':str(cwd),'execution_head':head,'head_after':git('rev-parse','HEAD'),'command':command,'started_utc':start,'ended_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'duration_seconds':round(time.monotonic()-tick,3),'exit_code':completed.returncode,'workspace_before':before,'workspace_after':git('status','--short'),'input_hashes':hashes,'output_sha256':hashlib.sha256(raw.read_bytes()).hexdigest(),'test_totals':{'passed':sum(p for p,_,_ in totals),'failed':sum(f for _,f,_ in totals),'ignored':sum(i for _,_,i in totals)},'test_summaries':totals}
(assets/(name+'-receipt.json')).write_text(json.dumps(receipt,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({k:receipt[k] for k in ['name','execution_head','exit_code','duration_seconds','test_totals']},ensure_ascii=False))
print(output[-12000:])
sys.exit(completed.returncode)
