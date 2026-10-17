"""Local allowlist-only inspection; never render raw native/request values.

Run after acquire.py. All private input stays in the protected local capture.
The public output contains fixed schema labels, types, counts, hashes and checks.
"""
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[5]
PUBLIC = Path(__file__).resolve().parent.parent
PRIVATE = ROOT / 'output/cpar-native-chat-offline-20261017/capture'
FIELDS = set('''id object created_at completed_at model status error incomplete_details
output parallel_tool_calls temperature tool_choice tools top_p max_output_tokens
previous_response_id reasoning store text truncation usage user metadata instructions
background service_tier prompt_cache_key safety_identifier type summary content role
annotations logprobs encrypted_content phase input_tokens output_tokens total_tokens
input_tokens_details output_tokens_details cached_tokens reasoning_tokens cache_read_tokens
cache_creation_tokens effort format num_sources_used num_server_side_tools_used
cost_in_usd_ticks max_tool_calls top_logprobs presence_penalty frequency_penalty'''.split())
ENUMS = set('''response completed in_progress incomplete failed message reasoning assistant
summary_text output_text reasoning_text'''.split())


def digest(value):
    return hashlib.sha256(value).hexdigest()


def load(name):
    path = PRIVATE / name
    assert not path.is_symlink() and path.stat().st_mode & 0o777 == 0o600
    return json.loads(path.read_bytes())


def structure(value):
    if isinstance(value, dict):
        return {'kind': 'object', 'fields': {
            (key if key in FIELDS else 'unknown-key-sha256:'+digest(key.encode())):
            structure(child) for key, child in value.items()}}
    if isinstance(value, list):
        return {'kind': 'array', 'length': len(value), 'items': [structure(v) for v in value]}
    if value is None:
        return {'kind': 'null'}
    if isinstance(value, str):
        return {'kind': 'string', 'bytes': len(value.encode()),
                **({'enum': value} if value in ENUMS else {})}
    return {'kind': type(value).__name__, 'is_zero': type(value) in (int, float) and value == 0}


def semantic_shape(value):
    if isinstance(value, dict):
        return {key: semantic_shape(child) for key, child in value.items()}
    if isinstance(value, list):
        return [semantic_shape(v) for v in value]
    if isinstance(value, str):
        return {'string': 'nonempty' if value else 'empty'}
    return {'type': type(value).__name__}


def main():
    os.umask(0o077)
    assert PRIVATE.stat().st_mode & 0o777 == 0o700
    raw = (PRIVATE/'native-capture/native-response.bin').read_bytes()
    request = (PRIVATE/'selected-request.json').read_bytes()
    assert len(raw) == 673 and digest(raw) == '7b5a3c2766766397af638665aaa7160f351780232a2c06f11d8f784ebbfdaa96'
    assert digest(request) == '245c50f95d487dbfa3333f0e33eb4978ea2cedd2c3d9bd1f1010aafbaaab6062'
    decompressed = gzip.decompress(raw)
    assert len(decompressed) <= 1048576
    native = json.loads(decompressed)
    selection = load('native-capture/selection.json')
    body = load('native-capture/body-receipt.json')
    transport = load('native-capture/transport.json')
    start = load('start-receipt.json')
    chat = load('chat-json/chat-summary.json')
    pin = load('chat-json/pin-private.json')
    lineage = load('lineage-private.json')
    assert body['sha256'] == digest(raw) and body['bytes'] == len(raw) and body['complete']
    assert selection['source_revision'] == start['source_sha'] == chat['source_sha'] == 'bf822b14a62c21cf582a7539e3635f69bd8b7677'
    assert selection['request_id'] == chat['request_id']
    assert selection['public_request_sha256'] == digest(request)
    assert selection['protocol'] == 'chat' and selection['mode'] == 'json'
    assert transport['content_encoding_class'] == 'Gzip' and transport['status'] == 200
    sidecars = lineage['sidecars']
    assert len(sidecars) == 1 and sidecars[0][0] == chat['request_id']
    assert digest(sidecars[0][2].encode()) == chat['validated_execution']['execution_evidence_sha256']
    evidence = json.loads(sidecars[0][2])
    assert evidence['credential_revision'] == pin['execution']['identity']['credential_revision'] == 153
    rows = [(kind, event_id, next(child for child in json.loads(payload).values() if isinstance(child, dict)), payload)
            for kind, event_id, payload in lineage['events']]
    assert [kind for kind, *_ in rows] == ['request', 'attempt', 'usage', 'request_finished']
    attempt = next(row for row in rows if row[0] == 'attempt')
    usage = next(row for row in rows if row[0] == 'usage')
    finished = next(row for row in rows if row[0] == 'request_finished')
    assert digest(attempt[3].encode()) == sidecars[0][1]
    assert attempt[1] == attempt[2]['attempt_id'] == chat['attempt_id'] == usage[2]['attempt_id']
    assert attempt[2]['outcome'] == 'succeeded' and finished[2]['outcome'] == 'failed'
    assert all(row[2]['request_id'] == chat['request_id'] for row in rows)
    assert len(lineage['ledger']) == 1 and lineage['ledger'][0] == [usage[1], chat['request_id'], None]
    expected_usage = {'input_tokens':228,'output_tokens':159,'reasoning_tokens':158,
                      'cached_tokens':128,'cache_read_tokens':None,'cache_creation_tokens':None,
                      'provenance':'measured','input_accounting':'inclusive'}
    assert usage[2]['usage'] == expected_usage
    reuse = json.loads((ROOT/'docs/reports/assets/cpar-serving-declaration-20261015/reuse.json').read_text())
    artifact = {}
    for name in ['binary', 'manifest', 'metadata']:
        facts = reuse[name]
        assert digest(Path(facts['path']).read_bytes()) == facts['sha256']
        artifact[name+'_sha256'] = facts['sha256']
    binary = Path(reuse['binary']['path']).read_bytes()
    embedded = re.findall(rb'gateway-release-revision=([0-9a-f]{40})\ngateway-release-rust-version=([^\n]+)\ngateway-release-target=([^\n]+)\n', binary)
    assert set(embedded) == {(b'bf822b14a62c21cf582a7539e3635f69bd8b7677', b'1.97.1', b'aarch64-unknown-linux-gnu')}
    assert start['binary_sha256'] == chat['binary_sha256'] == artifact['binary_sha256']
    assert start['manifest_sha256'] == artifact['manifest_sha256']
    original_request = json.loads(request)
    assert original_request['model'] == 'grok-4.5' and original_request['stream'] is False
    assert original_request['max_completion_tokens'] == 4096
    minimal = json.loads((PUBLIC/'minimal-native.json').read_text())
    assert semantic_shape(native['output']) == semantic_shape(minimal['output'])
    assert len({item['id'] for item in native['output']}) == len(native['output']) == 2
    assert native['usage']['total_tokens'] == native['usage']['input_tokens'] + native['usage']['output_tokens']
    unknown_usage = {key: value for key, value in native['usage'].items() if key not in FIELDS}
    assert len(unknown_usage) == 1
    extra = next(iter(unknown_usage.values()))
    assert isinstance(extra, dict) and set(extra) == {'input_tokens','output_tokens'}
    full = json.loads((PUBLIC/'minimal-native.json').read_text())
    full['usage'].update(num_sources_used=0,num_server_side_tools_used=0,
                         cost_in_usd_ticks=123000000,
                         synthetic_cost_details={'input_tokens':11,'output_tokens':17})
    assert all(type(extra[key]) is type(full['usage']['synthetic_cost_details'][key]) for key in extra)
    (PUBLIC/'usage-extensions-native.json').write_text(json.dumps(full,indent=2)+'\n')
    (PUBLIC/'structure.json').write_text(json.dumps({
        'decompressed_bytes':len(decompressed),'structure':structure(native)},indent=2)+'\n')
    receipt = {
        'status':'PASS', 'observed_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'epoch':'chat-json-serving-20261015','native_bytes':len(raw),'native_sha256':digest(raw),
        'decompressed_bytes':len(decompressed),'decompressed_sha256':digest(decompressed),
        'request_sha256':digest(request),'source_revision':selection['source_revision'],
        **artifact,'request_id':chat['request_id'],'attempt_id':chat['attempt_id'],
        'embedded_source_metadata_exact_match':True,
        'capture_request_attempt_sidecar_ledger_match':True,'actual_lease_revision':153,
        'native_usage':expected_usage,'ledger_rows':1,'ledger_cost':None,
        'native_usage_extra_root_fields':len(native['usage'])-5,
        'native_reported_cost_present':True,
        'native_reported_cost_is_zero':native['usage']['cost_in_usd_ticks']==0,
        'unknown_usage_object_key_sha256':digest(next(iter(unknown_usage)).encode()),
        'unknown_usage_object_shape':structure(extra),
        'native_model_equals_requested_public_model':native['model']==original_request['model'],
        'minimal_fixture_output_shape_matches':True,
        'unknown_native_semantics':'UNKNOWN; a field name or price-shaped integer is not an independent billing receipt',
        'external_provider_calls':0,'runtime_launches':0,
    }
    (PUBLIC/'identity-and-shape.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps(receipt,indent=2))


if __name__ == '__main__':
    main()
