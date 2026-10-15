"""Prepare derived catalog state in a newly copied, operator-owned diagnostic DB.

This never reads credentials or changes model grants. The caller owns the new DB,
has bound the one approved account and target, and must preserve the source DB.
"""
import hashlib
import json
import os
from pathlib import Path
import sqlite3


def detach_expired_catalog(database, target, now_ms, archive):
    """Archive an expired inherited target before removing it from the fresh copy."""
    scope = (target['config_version_id'], target['endpoint_id'], target['credential_id'])
    candidate = database.execute(
        'SELECT route_id,endpoint_id,upstream_model,enabled,capability_override_json '
        'FROM route_candidates WHERE config_version_id=? AND id=?',
        (scope[0], target['route_candidate_id'])).fetchone()
    assert candidate is not None
    assert tuple(candidate[:4]) == (target['route_id'], scope[1], target['upstream_model'], 1)
    assert json.loads(candidate[4]).get('allow_unlisted_model') is True
    assert database.execute('SELECT count(*) FROM route_candidates WHERE config_version_id=? AND enabled=1',
                            (scope[0],)).fetchone()[0] == 1
    key = database.execute('SELECT access_group_id,status FROM client_keys WHERE config_version_id=? AND id=?',
                           (scope[0], target['client_key_id'])).fetchone()
    assert key is not None and key[1] == 'active'
    assert database.execute('SELECT enabled FROM access_group_routes WHERE config_version_id=? AND access_group_id=? AND route_id=?',
                            (scope[0], key[0], target['route_id'])).fetchone() == (1,)
    rows = {}
    for table in ('model_catalog_targets', 'model_catalog_models', 'model_catalog_failures'):
        cursor = database.execute('SELECT * FROM ' + table +
            ' WHERE config_version_id=? AND endpoint_id=? AND credential_id=?', scope)
        fields = [column[0] for column in cursor.description]
        rows[table] = [dict(zip(fields, row)) for row in cursor]
    targets = rows['model_catalog_targets']
    scoped_targets = database.execute(
        'SELECT count(*) FROM model_catalog_targets WHERE config_version_id=?', (scope[0],)).fetchone()[0]
    if not targets:
        assert scoped_targets == 0
        return {'status': 'NO_INHERITED_CATALOG', 'removed_rows': 0}
    assert len(targets) == 1 and targets[0]['expires_at_ms'] <= now_ms
    assert any(row['model'] == target['upstream_model'] and row['present_in_last_success'] == 1
               for row in rows['model_catalog_models'])
    assert scoped_targets == 1
    encoded = (json.dumps(rows, sort_keys=True, indent=2) + '\n').encode()
    archive = Path(archive)
    with archive.open('xb') as output:
        os.chmod(archive, 0o600)
        output.write(encoded)
        output.flush()
        os.fsync(output.fileno())
    descriptor = os.open(archive.parent, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    removed = 0
    for table in ('model_catalog_models', 'model_catalog_failures', 'model_catalog_targets'):
        count = database.execute('DELETE FROM ' + table +
            ' WHERE config_version_id=? AND endpoint_id=? AND credential_id=?', scope).rowcount
        assert count == len(rows[table])
        removed += count
    return {'status': 'EXPIRED_INHERITED_CATALOG_DETACHED', 'removed_rows': removed,
            'catalog_expires_at_ms': targets[0]['expires_at_ms'],
            'archive_sha256': hashlib.sha256(encoded).hexdigest()}


if __name__ == '__main__':
    import sys
    assert len(sys.argv) == 5
    with sqlite3.connect(sys.argv[1]) as connection:
        receipt = detach_expired_catalog(connection, json.loads(sys.argv[2]), int(sys.argv[3]), sys.argv[4])
        connection.commit()
    print(json.dumps(receipt))
