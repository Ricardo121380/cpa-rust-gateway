import collections, datetime, hashlib, json, pathlib, sys
root = pathlib.Path(sys.argv[1]).resolve()
def read(name):
    return json.loads((root / name).read_text())
files = ['final-initialize-green.json', 'final-api-5x3-green.json', 'final-device-main.json', 'final-device-boundaries.json', 'final-desktop-pointer.json']
scenes = {name: read(name)['results'] for name in files}
assert [len(scenes[name]) for name in files] == [2, 15, 2, 12, 2]
assert all(row['result'] == 'PASS' for rows in scenes.values() for row in rows)
expected = []
failed = []
for row in scenes['final-api-5x3-green.json']:
    before = {a['id'] for a in row['before']['accounts']}
    after = {a['id'] for a in row['after']['accounts']}
    added = after - before
    assert len(added) == 1 and before <= after
    assert row['permissionsChanged'] is False and row['grantsCreated'] == 0
    expected.extend(added)
for row in scenes['final-device-main.json']:
    account = row['committed']['credential_id']
    assert row['databaseBeforeLoss'] == row['databaseAfterRecovery']
    assert len(row['databaseBeforeLoss']) == 1 and row['databaseBeforeLoss'][0][0] == account
    expected.append(account)
for row in scenes['final-device-boundaries.json']:
    account = row['start']['body']['id']
    success = row['session']['mode'] == 'pending_then_success'
    assert len(row['database']) == int(success)
    (expected if success else failed).append(account)
for row in scenes['final-desktop-pointer.json']:
    imports = [r for r in row['applied']['records'] if r['method'] == 'POST' and r['status'] == 201 and r['path'].endswith('/account-import')]
    assert len(imports) == 1
    expected.append(imports[0]['response']['id'])
assert len(expected) == len(set(expected)) == 21
facts = read('accepted-stock/stored-facts.json')
active = [r[0] for r in facts['versions'] if r[1] == 'active']
assert len(active) == 1
accounts = [r[1] for r in facts['accounts'] if r[0] == active[0]]
bindings = [r[2] for r in facts['bindings'] if r[0] == active[0]]
assert collections.Counter(accounts) == collections.Counter(expected)
assert collections.Counter(bindings) == collections.Counter(expected)
assert not set(failed) & {r[1] for r in facts['accounts']}
devices = read('accepted-stock/device-receives.json')
assert len(devices['sessions']) == 14
assert sum(r['token_requests'] for r in devices['sessions']) == 14
for row in devices['sessions']:
    assert row['token_requests'] == (0 if row['mode'] == 'ttl' else 2 if row['mode'] == 'pending_then_success' else 1)
assert read('accepted-stock/provider-receives.json') == []
descriptor = read('accepted-stock/descriptor.json')
assert descriptor['source_sha'] == '6a37f74d63258d283da5f55e5e5ef31b9b9ed0c4' and descriptor['source_dirty'] == []
assert descriptor['real_provider_calls'] == 0 and descriptor['outside_loopback_denied_by_os_sandbox']
assert descriptor['device_transport']['stock_production_workflows'] and descriptor['device_transport']['network_interposition']
for name in files[1:]:
    assert read(name)['sourceSha'] == descriptor['source_sha']
    assert read(name)['binarySha256'] == descriptor['binary_sha256']
previous = read('final-persistence-oracle.json')
assert previous['expected_account_ids'] == expected
assert previous['total_scenes'] == 33 and previous['active_account_count'] == previous['active_binding_count'] == 21
names = files + ['accepted-stock/stored-facts.json', 'accepted-stock/device-receives.json', 'accepted-stock/provider-receives.json', 'accepted-stock/descriptor.json', 'final-persistence-oracle.json']
result = {'result': 'PASS', 'verified_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'operation_source': descriptor['source_sha'], 'active_config': active[0], 'actual_page_scenes': {name: len(rows) for name, rows in scenes.items()}, 'total_scenes': 33, 'active_account_count': len(accounts), 'active_binding_count': len(bindings), 'unique_expected_accounts': 21, 'failed_accounts_absent': len(failed), 'device_sessions': 14, 'device_token_requests': 14, 'provider_inference_requests': 0, 'input_sha256': {name: hashlib.sha256((root/name).read_bytes()).hexdigest() for name in names}}
(root / 'final-persistence-verification.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({k: v for k, v in result.items() if k != 'input_sha256'}))
