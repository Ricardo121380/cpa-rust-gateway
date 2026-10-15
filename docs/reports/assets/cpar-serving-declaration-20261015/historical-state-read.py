"""Read only allowlisted metadata from the stopped epoch; no HTTP or credentials."""
import datetime
import hashlib
import json
import pathlib
import sqlite3
import subprocess

base = pathlib.Path('/var/lib/cpa-rust-gateway/acceptance/cpar-build-20261010')
epoch = base / 'private/chat-json-linux-final-20261013'
assert json.loads((base / 'owner.json').read_text())['task'] == 'cpar-build-20261010'
budget = json.loads((epoch / 'budget.json').read_text())
assert budget['stopped'] is True
assert budget['new_pin_intents'] == budget['new_basic_intents'] == 0
unit = 'cpar-build-chat-json-linux-final-20261013.service'
state = dict(line.split('=', 1) for line in subprocess.check_output(
    ['systemctl', 'show', unit, '--property=MainPID,LoadState'], text=True).splitlines())
assert state == {'MainPID': '0', 'LoadState': 'not-found'}
opaque = lambda value: hashlib.sha256(('cpar-evidence:' + str(value)).encode()).hexdigest()
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
database = epoch / 'state/control.sqlite3'
before = digest(database)
inspection_path = epoch / 'chat-json/inspection-0-private.json'
inspection = json.loads(inspection_path.read_text())
result = {'kind': 'STOPPED_ARTIFACT_READ', 'unit': state,
          'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'inspection_sha256': digest(inspection_path), 'database_sha256': before,
          'capability_observation_count': len(inspection['runtime']['capability_observations']),
          'capability_read_errors': inspection.get('capability_read_errors'),
          'provider_intents': 0, 'management_http_requests': 0, 'new_runtime_launches': 0}
queries = {
    'configuration': 'SELECT id,revision,status FROM config_versions',
    'models': 'SELECT config_version_id,id,model_name,status,capabilities_json FROM public_models',
    'routes': 'SELECT config_version_id,id,public_model_id,max_attempts FROM model_routes',
    'candidates': 'SELECT config_version_id,id,route_id,endpoint_id,upstream_model,enabled,credential_scope,capability_override_json FROM route_candidates',
    'keys': 'SELECT config_version_id,id,access_group_id,status,expires_at_ms FROM client_keys',
    'groups': 'SELECT config_version_id,id,status FROM access_groups',
    'grants': 'SELECT config_version_id,access_group_id,route_id,enabled FROM access_group_routes',
    'catalog_targets': 'SELECT config_version_id,endpoint_id,credential_id,snapshot_version,observed_at_ms,stale_at_ms,expires_at_ms FROM model_catalog_targets',
    'catalog_models': 'SELECT config_version_id,endpoint_id,credential_id,model,present_in_last_success FROM model_catalog_models',
}
with sqlite3.connect('file:' + str(database) + '?mode=ro', uri=True) as db:
    db.row_factory = sqlite3.Row
    db.execute('BEGIN')
    for name, query in queries.items():
        rows = [dict(row) for row in db.execute(query)]
        for row in rows:
            for field in ('credential_id', 'access_group_id'):
                if field in row:
                    row[field + '_evidence_id'] = opaque(row.pop(field))
            if name in ('keys', 'groups'):
                row['id_evidence'] = opaque(row.pop('id'))
        result[name] = rows
    result['native_accounts'] = [dict(row) for row in db.execute(
        'SELECT provider,enabled,auth_status,revision,worker_claim_kind FROM grok_accounts')]
    result['schema_version'] = db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0]
    db.rollback()
assert digest(database) == before
print(json.dumps(result))
