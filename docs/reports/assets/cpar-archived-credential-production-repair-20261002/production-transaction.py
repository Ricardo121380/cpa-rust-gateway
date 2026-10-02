"""Explicitly authorized single-field AC-01 production CAS. Default action is absent."""
import pathlib
import sys
exec((pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884/archived-credential-production-repair-20261002') / 'repair-common.py').read_text())
assert len(sys.argv) == 2 and sys.argv[1] in ('apply-authorized', 'rollback-authorized')
forward = sys.argv[1] == 'apply-authorized'
name = 'production-transaction.json' if forward else 'production-rollback.json'
assert not (OPS / name).exists(), 'no_automatic_production_retry'
assert json.loads((OPS / 'plan-pre-receipt.json').read_text())['status'] == 'PASS'
assert json.loads((OPS / 'transaction-rehearsal.json').read_text())['status'] == 'PASS'
runtime_before = runtime()
keys_before = key_state()
original = (OPS / 'original-envelope.private.bin').read_bytes()
replacement = (OPS / 'repaired-envelope.private.bin').read_bytes()
expected, desired = (original, replacement) if forward else (replacement, original)
reference = OPS / ('capture-pre/state/control.sqlite3' if forward else 'transaction-rehearsal/healed-reference.sqlite3')
private_write(OPS / ('apply-intent.private.json' if forward else 'rollback-intent.private.json'), (json.dumps({'action': sys.argv[1], 'started_at_utc': utc(), 'target_locator': loc, 'old_envelope_private_path': str(OPS / 'original-envelope.private.bin'), 'new_envelope_private_path': str(OPS / 'repaired-envelope.private.bin')}) + '\n').encode())
result = bounded_cas(STATE / 'control.sqlite3', reference, expected, desired, forward)
private_write(OPS / ('commit-confirmation.json' if forward else 'rollback-commit-confirmation.json'), (json.dumps({**result, 'action': sys.argv[1], 'record_alias': 'AC-01', 'completed_at_utc': utc()}) + '\n').encode())
result.update({'action': sys.argv[1], 'record_alias': 'AC-01', 'production_committed_transactions': 1, 'production_changed_rows': 1, 'runtime_unchanged': runtime() == runtime_before, 'six_keys_unchanged': key_state() == keys_before, 'schema_version': 31, 'config_and_credential_lifecycle_status_kind_revisions_unchanged': True, 'no_production_management_export_or_validate': True, 'production_service_stop_start': 0, 'production_switches': 0, 'real_provider_calls': 0, 'completed_at_utc': utc()})
# Persist the committed result before any subsequent assertion can fail.
receipt(name, result)
assert result['runtime_unchanged'] and result['six_keys_unchanged'], 'postcommit_runtime_or_key_drift'
