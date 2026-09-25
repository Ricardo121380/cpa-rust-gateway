#!/usr/bin/env python3
"""Bounded Agent compatibility check against prism-local-gateway's owned TLS mock."""
import json
import pathlib
import sys
import time
import urllib.error
import urllib.request

out = pathlib.Path(sys.argv[1]).resolve()
info = json.loads((out / 'local-preview.json').read_text())
root = pathlib.Path(info['root'])
assert info['synthetic'] and root.name.startswith('prism-complete-local-')
http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
data = f"http://127.0.0.1:{info['data_port']}"
admin = f"http://127.0.0.1:{info['admin_port']}"
client_headers = {'Authorization': 'Bearer ' + (root / 'client-key').read_text(),
                  'Content-Type': 'application/json'}
admin_headers = {'X-Management-Key': (root / 'credentials/management-key').read_text()}


def request(path, body=None, management=False):
    req = urllib.request.Request(
        (admin if management else data) + path,
        headers=admin_headers if management else client_headers,
        data=None if body is None else json.dumps(body).encode())
    try:
        response = http.open(req, timeout=20)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        return response.status, response.headers, response.read(1_000_000).decode()


start = int(time.time() * 1000)
checks = []
for stream, stored in ((False, False), (True, False), (False, True), (True, True)):
    history = [{'role': 'user', 'content': 'agent-roundtrip-acceptance synthetic task'}]
    previous = None
    for turn in range(3):
        body = {'model': 'Exact/Model-v2', 'stream': stream, 'store': stored, 'input': history,
                'reasoning': {'effort': 'low', 'summary': 'auto'},
                'include': ['reasoning.encrypted_content'],
                'tools': [{'type': 'function', 'name': 'read', 'parameters': {
                    'type': 'object', 'properties': {'path': {'type': 'string'}}}}]}
        if previous:
            body['previous_response_id'] = previous
        status, _, raw = request('/v1/responses', body)
        if status != 200:
            try:
                error = json.loads(raw).get('error', {})
                code = error.get('code', error.get('type')) if isinstance(error, dict) else None
            except ValueError:
                code = None
            _, _, availability = request('/admin/runtime/availability', management=True)
            diagnostic = {'stream': stream, 'stored': stored, 'turn': turn + 1,
                          'status': status, 'code': code,
                          'availability': json.loads(availability)}
            (out / 'agent-failure.json').write_text(json.dumps(diagnostic, indent=2) + '\n')
            print(json.dumps({'agent_failure': diagnostic}), flush=True)
            raise AssertionError((stream, stored, turn, status, code))
        if stream:
            frames = [json.loads(line[6:]) for line in raw.splitlines() if line.startswith('data: ')]
            assert not any(frame['type'] == 'response.failed' for frame in frames)
            completed = [frame for frame in frames if frame['type'] == 'response.completed']
            assert len(completed) == 1
            response = completed[0]['response']
        else:
            response = json.loads(raw)
        assert response['status'] == 'completed'
        assert response['usage']['input_tokens'] == 10 + turn
        items = response['output']
        assert any(item['type'] == 'reasoning' for item in items)
        calls = [item for item in items if item['type'] == 'function_call']
        if turn < 2:
            assert len(calls) == 1 and calls[0]['call_id'] == f'call_{turn}'
            # Replay precisely what CPAR returned, including IDs/status. Never delete reasoning.
            if stored:
                previous = response['id']
                history = []
            else:
                history.extend(items)
            history.append({'type': 'function_call_output', 'call_id': calls[0]['call_id'],
                            'output': 'synthetic tool result; no filesystem operation'})
        else:
            assert not calls
            assert any(part.get('text') == 'agent roundtrip complete'
                       for item in items for part in item.get('content', []))
        checks.append({'stream': stream, 'stored': stored, 'turn': turn + 1, 'completed': True})

for path, body in [
    ('/v1/responses', {'model': 'Exact/Model-v2', 'input': [{'type': 'reasoning', 'id': 'bad',
                                                        'encrypted_content': 'synthetic-unowned'}]}),
    ('/v1/chat/completions', {'model': 'Exact/Model-v2', 'messages': 42}),
    ('/v1/messages', {'model': 'Exact/Model-v2', 'messages': 42}),
]:
    status, headers, _ = request(path, body)
    assert status == 400 and headers.get('X-Request-Id'), (path, status)

deadline = time.monotonic() + 15
while True:
    status, _, raw = request('/admin/requests/summary?from_ms=' + str(start) + '&limit=100', management=True)
    assert status == 200
    page = json.loads(raw)
    summary = page['summary']
    if summary['requests'] == 15 and sum(row['ledger_records'] for row in page['items']) == 12:
        break
    assert time.monotonic() < deadline, summary
    time.sleep(.1)
assert (summary['succeeded'], summary['failed'], summary['attempts']) == (12, 3, 12), summary
assert all(row['duration_ms'] is not None for row in page['items'])
assert all(row['ledger_records'] == 0 for row in page['items'] if row['outcome'] == 'failed')
provider = [json.loads(line) for line in (root / 'agent-roundtrip-calls.jsonl').read_text().splitlines()]
assert len(provider) == 12
assert [row['turn'] for row in provider] == [0, 1, 2] * 4
report = {'passed': True, 'real_gateway': True, 'provider': 'owned loopback TLS mock',
          'real_provider_calls': 0, 'checks': checks, 'summary': summary,
          'replayed_gateway_output': True, 'decode_failures_observed': 3,
          'successful_requests_materialized': 12}
(out / 'agent-roundtrip.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report))
