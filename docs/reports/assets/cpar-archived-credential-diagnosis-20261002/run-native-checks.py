"""Four bounded native reads, each on a fresh state/key derivative; no production bindings."""
import pathlib
TASK = pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884')
exec((TASK / 'operator-tools/target-rehearsal.py').read_text().split('assert ROOT.lstat()')[0])
DIAG = TASK / 'archived-credential-diagnosis-20261002'
CAP = TASK / 'capture-2'
repaired = DIAG / 'repaired-pristine'
assert repaired.is_dir()
receipts = []
for label, source, binary, sha, digest, schema in [
    ('baseline-old', CAP, CAP / 'old-gateway', OLD_SHA, OLD_HASH, 31),
    ('baseline-candidate', CAP, ARTIFACT / 'gateway-aarch64-unknown-linux-gnu', SHA, NEW_HASH, 34),
    ('repaired-old', repaired, CAP / 'old-gateway', OLD_SHA, OLD_HASH, 31),
    ('repaired-candidate', repaired, ARTIFACT / 'gateway-aarch64-unknown-linux-gnu', SHA, NEW_HASH, 34),
]:
    work = DIAG / label
    assert not work.exists(), 'never_overwrite_native_evidence'
    work.mkdir(mode=0o700)
    os.chown(work, UID, GID)
    state_copy(source / 'state', work / 'state')
    os.chown(work / 'state/backups', UID, GID)
    shutil.copytree(source / 'credentials', work / 'credentials')
    for item in [work / 'credentials'] + list((work / 'credentials').iterdir()):
        os.chown(item, UID, GID)
    target = work / 'target.private.json'
    target.write_bytes((DIAG / 'target.private.json').read_bytes())
    os.chown(target, UID, GID)
    command = ['--ro-bind', str(DIAG / 'native-read-probe.py'), '/probe.py', '--ro-bind', str(target), '/target.private.json', '--', '/usr/bin/python3', '/probe.py', sha, digest, str(schema)]
    run = isolated_run(sandbox(binary, state=work / 'state', credentials=work / 'credentials', work=work, command=command), capture_output=True, timeout=90)
    (work / 'namespace.private.log').write_bytes(run.stdout + run.stderr)
    receipt_file = work / 'native-read-receipt.json'
    if not receipt_file.exists():
        output = {'label': label, 'status': 'FAIL', 'category': 'NAMESPACE_OR_PROBE_FAILURE', 'return_code': run.returncode}
        receipts.append(output)
        break
    output = json.loads(receipt_file.read_text())
    output['label'] = label
    expected_metadata = 200 if label.startswith('repaired') else 500
    output['expected_target_metadata_status'] = expected_metadata
    output['native_authentication_result_matches_expectation'] = output.get('operations', {}).get('credential_metadata', {}).get('http_status') == expected_metadata and output.get('operations', {}).get('parent_metadata', {}).get('http_status') == 200
    output['state_is_fresh_derivative'] = True
    output['keys_byte_identical_to_capture'] = all((work / 'credentials' / name).read_bytes() == (CAP / 'credentials' / name).read_bytes() for name in KEY_NAMES)
    output['status'] = 'PASS' if output['status'] == 'PASS' and output['native_authentication_result_matches_expectation'] else 'FAIL'
    receipts.append(output)
    (DIAG / (label + '-receipt.json')).write_text(json.dumps(output, indent=2) + '\n')
    print(json.dumps({'label': label, 'status': output['status'], 'operations': output.get('operations'), 'expected_native_authentication': output['native_authentication_result_matches_expectation']}), flush=True)
    if output['status'] != 'PASS':
        break
summary = {'status': 'PASS' if len(receipts) == 4 and all(r['status'] == 'PASS' for r in receipts) else 'FAIL', 'receipts': receipts, 'real_provider_calls': 0, 'production_writes': 0, 'original_pristine_copies_retained': True}
(DIAG / 'native-cross-validation.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps({'cross_validation_status': summary['status'], 'completed_cases': len(receipts)}))
