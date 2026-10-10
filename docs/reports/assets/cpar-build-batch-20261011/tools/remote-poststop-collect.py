"""Read exact failed-sample attribution after stop; never attest a live runtime."""
import contextlib
import datetime
import hashlib
import importlib.util
import io
import json
import os
import pathlib
import subprocess

os.umask(0o077)
base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/six-combinations-20261011'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
assert subprocess.check_output(['systemctl', 'show', 'cpar-build-acceptance-20261010.service', '--value', '--property=MainPID'], text=True).strip() == '0'
tool = epoch / 'cpar_new_build_acceptance.py'
assert hashlib.sha256(tool.read_bytes()).hexdigest() == 'b33e551ef018df71a4bb0d3e301249b75eaf603d46590b20529e649ee1ff9d8a'
spec = importlib.util.spec_from_file_location('formal', tool)
formal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(formal)
bundle_path = epoch / 'responses-sse/wire-final.json'
bundle = json.loads(bundle_path.read_text())
sample = bundle['scenarios']['text_multi_turn'][0]
events = formal.sse_events(sample['wire'].encode())
terminal = [event['response'] for event in events if event.get('type') == 'response.completed']
assert len(terminal) == 1
value = terminal[0]
output = io.StringIO()
with contextlib.redirect_stdout(output):
    exec(formal.REMOTE_COLLECT.replace('RESPONSE_ID', repr(value['id'])), {
        'opts': {'--state-dir': str(epoch / 'state')}, 'pathlib': pathlib,
        'json': json, 'datetime': datetime, 'hashlib': hashlib,
        'opaque': formal.opaque, 'runtime': {},
    })
collected = formal.load_json(output.getvalue())
evidence = collected['evidence']
attribution = formal.verify_attempts(bundle['target'], evidence, value['id'])
usage = formal.native_usage('responses', value)
formal.verify_usage_lineage(usage, evidence)
collected.update(layer='POST_STOP_READ_ONLY_ATTRIBUTION', runtime_attestation='NOT_RUN; process already stopped')
with (epoch / 'post-stop-sse-attribution-private.json').open('x') as result:
    json.dump(collected, result, indent=2)
print(json.dumps({
    'status': 'PASS; post-stop attribution only', 'layer': collected['layer'],
    'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'original_wire_sha256': hashlib.sha256(bundle_path.read_bytes()).hexdigest(),
    'private_attribution_sha256': hashlib.sha256((epoch / 'post-stop-sse-attribution-private.json').read_bytes()).hexdigest(),
    'terminal_snapshot_status': value['status'], 'request_outcome': evidence['request_outcome'],
    'response_evidence_id': formal.opaque(value['id']), 'attribution': attribution,
    'native_usage_source': usage['source'], 'exact_usage_and_ledger_lineage': True,
    'original_execution_failure_preserved': bundle['execution_failure'],
    'new_provider_calls': 0, 'runtime_attestation': 'NOT_RUN; process already stopped',
}))
