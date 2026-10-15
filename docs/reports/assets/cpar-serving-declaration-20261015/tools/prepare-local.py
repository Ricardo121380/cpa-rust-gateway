"""Freeze local successor bytes; never performs SSH, credential reads or calls."""
import difflib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import tarfile

os.umask(0o077)
public = Path(__file__).resolve().parent.parent
root = public.parents[3]
private = root / 'output/cpar-serving-declaration-20261015'
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
assert sys.argv[1] in ('system', 'diagnostic')
from startup_probe import save
if sys.argv[1] == 'system':
    assert not (private / 'system-stage.json').exists()
    files = {name: sha(public / 'tools' / name) for name in ['startup_probe.py', 'launch_recipe.py',
        'linux-system-validation.py', 'remote-exit.py', 'system_exit_adapter.py']}
    files['recipe.json'] = sha(private / 'recipe-private.json')
    for name in ['artifact-manifest.json', 'gateway-build-metadata.json']:
        files[name] = sha(private / 'payload' / name)
    freeze = {'status': 'FROZEN', 'base': '0e0c3e7f035cc108294a4250d402c3e78d6a0c36', 'files': files,
        'controller_sha256': sha(public / 'tools/system-control.py'), 'max_runs': 4, 'scenarios': ['A', 'B', 'C', 'D'],
        'provider_intent_limit': 0, 'binary_sha256': 'cdadcb2cd679fe02e291f7a02fd6714aa92df816a91a5624858112ad19818b1e',
        'guard_sha256': '91c943c76884a2b674a0b24d5d4876ff40cf61b53c379ffe2d04a062a0dddf06'}
    from system_exit_adapter import adapt_exit
    original = (public / 'tools/remote-exit.py').read_text()
    adapted = adapt_exit(original, 'linux-final-synthetic-20261013-A', 'cpar-linux-final-synthetic-20261013-A.service')
    (public / 'system-exit-A.diff').write_text(''.join(difflib.unified_diff(original.splitlines(True), adapted.splitlines(True), fromfile='formal-remote-exit.py', tofile='synthetic-executed-remote-exit-A.py', n=0)))
    freeze['adapted_exit_A_sha256'] = hashlib.sha256(adapted.encode()).hexdigest()
    save(public / 'system-freeze.json', freeze)
else:
    assert not (private / 'stage.json').exists()
    payload = private / 'payload'
    for name in ['remote-readiness.py', 'remote-prepare.py', 'remote-start.py', 'remote-pin.py', 'remote-chat.py',
                 'remote-exit.py', 'launch_recipe.py', 'qualification.py', 'startup_probe.py']:
        shutil.copyfile(public / 'tools' / name, payload / name)
    shutil.copyfile(public / 'user-authorization.json', payload / 'user-authorization.json')
    spec = importlib.util.spec_from_file_location('frozen_diagnostic', payload / 'cpar_build_json_diagnostic.py')
    diagnostic = importlib.util.module_from_spec(spec); spec.loader.exec_module(diagnostic)
    import launch_recipe
    base = Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
    epoch = base / 'private/chat-json-serving-20261015'
    recipe = json.loads((private / 'recipe-private.json').read_text())
    launch = launch_recipe.assemble(recipe['approved_launch'], recipe['runtime_options'], base, epoch,
        'cpar-build-chat-json-serving-20261015.service', 'bf822b14a62c21cf582a7539e3635f69bd8b7677',
        '245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062', diagnostic)
    gate_path = public / 'system-gate.json'
    if not gate_path.exists():
        save(public / 'system-gate-pre-run.json', {'status': 'NOT_RUN', 'layer': 'REAL_LINUX_SYSTEM_SYNTHETIC', 'runs': [], 'formal_execution_allowed': False})
        gate_path = public / 'system-gate-pre-run.json'
    gate = json.loads(gate_path.read_text())
    frozen = {'kind': 'chat_json_502_diagnostic', 'epoch': epoch.name,
        'source_sha': 'bf822b14a62c21cf582a7539e3635f69bd8b7677',
        'driver_baseline_source_sha': '0e0c3e7f035cc108294a4250d402c3e78d6a0c36',
        'unchanged_diagnostic_protocol_source_sha': 'c23d2c356ec0253d93e41ca94aac821ad167fcd6',
        'batch_base': '0e0c3e7f035cc108294a4250d402c3e78d6a0c36',
        'files': {p.name: sha(p) for p in payload.iterdir() if p.name != 'freeze.json'},
        'assembled_launch_sha256': hashlib.sha256(json.dumps(launch, separators=(',', ':')).encode()).hexdigest(),
        'local_controller_sha256': sha(public / 'tools/local-control.py'), 'upload_tool_sha256': sha(public / 'tools/remote-upload.py'),
        'no_launch_exit_tool_sha256': sha(public / 'tools/remote-no-launch-exit.py'), 'system_gate_sha256': sha(gate_path),
        'selected_request_sha256': '245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062',
        'actual_account_material': None, 'actual_pin_lease': None, 'actual_execution_identity': None, 'reused_prior_freeze': False}
    assert len(frozen['files']) == 18
    save(payload / 'freeze.json', frozen)
    with tarfile.open(private / 'upload.tar', 'w') as archive:
        for name in ['freeze.json', *sorted(frozen['files'])]: archive.add(payload / name, arcname=name)
    save(public / ('prepared-freeze.json' if gate['status'] == 'PASS' else 'prepared-freeze-pre-system.json'),
        {'status': 'PREPARED_ONLY', 'freeze': frozen, 'payload_freeze_sha256': sha(payload / 'freeze.json'),
            'system_gate_status': gate['status'], 'launch_command': launch, 'upload_tar_sha256': sha(private / 'upload.tar')})
print(json.dumps({'status': 'FROZEN_LOCAL', 'scope': sys.argv[1], 'ssh': 0, 'provider_calls': 0}))
