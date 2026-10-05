"""Future approved-window recipe: capture, verify, preserve, full restore.

NOT EXECUTED during the 2026-10-05 review. Never stops/starts a service itself.
Run only on new-vps, as root, after explicit maintenance authorization and drain.
Sensitive bytes and logs remain in the root-only window directory.
"""
import argparse
import datetime
import hashlib
import json
import os
import pathlib
import shutil
import sqlite3
import stat
import subprocess

TASK = pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884')
SERVICE = 'cpa-rust-gateway.service'
STATE = pathlib.Path('/var/lib/cpa-rust-gateway')
KEYS = pathlib.Path('/etc/cpa-rust-gateway/credentials')
OLD_RELEASE = pathlib.Path('/opt/cpa-rust-gateway/releases/b0cf36e387b774439acc23830221389eb3ac74d0')
OLD_HASH = '0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b'
NEW_HASH = '542a3c30aae7f09cac80b7cce33f3a153a87a93fe1e842cd6d8c8e355a930a87'
PREFIX = pathlib.Path('/var/backups/cpa-rust-gateway/80f5838902eb56be6dccc3ce047eb227d6c18e6c-542a3c30aae7')
KEY_NAMES = ['management-key', 'management-csrf', 'master-key', 'backup-key', 'client-key-pepper', 'grok-build-cache-key']

def properties():
    return dict(s.split('=', 1) for s in subprocess.check_output(['systemctl', 'show', SERVICE, '-p', 'MainPID', '-p', 'ActiveState', '-p', 'SubState'], text=True).splitlines())

def stopped():
    p = properties()
    assert p['MainPID'] == '0' and p['ActiveState'] in ['inactive', 'failed'], 'service_not_stopped'
    # Refuse all other open holders of the state/key trees; never kill other processes.
    holders = []
    for proc in pathlib.Path('/proc').iterdir():
        if not proc.name.isdigit() or int(proc.name) == os.getpid():
            continue
        try:
            for fd in (proc / 'fd').iterdir():
                try:
                    dest = os.readlink(fd).removesuffix(' (deleted)')
                    if any(dest == str(root) or dest.startswith(str(root) + '/') for root in [STATE, KEYS]):
                        holders.append(int(proc.name)); break
                except FileNotFoundError:
                    pass
        except FileNotFoundError:
            pass
    assert not holders, 'other_state_or_key_holders_remain'

def cp(source, dest):
    assert not dest.exists() and not dest.is_symlink(), 'destination_already_exists'
    subprocess.run(['cp', '--archive', '--no-dereference', '--', str(source), str(dest)], check=True)

def inventory(root):
    result = {}
    for p in [root] + sorted(root.rglob('*')):
        s = p.lstat()
        r = {'uid': s.st_uid, 'gid': s.st_gid, 'mode': stat.S_IMODE(s.st_mode), 'mtime_ns': s.st_mtime_ns, 'xattr_sha256': {name: hashlib.sha256(os.getxattr(p, name, follow_symlinks=False)).hexdigest() for name in os.listxattr(p, follow_symlinks=False)}}
        if stat.S_ISREG(s.st_mode):
            r.update(type='file', size=s.st_size, sha256=hashlib.sha256(p.read_bytes()).hexdigest())
        elif stat.S_ISDIR(s.st_mode):
            r['type'] = 'directory'
        elif stat.S_ISLNK(s.st_mode):
            r.update(type='symlink', target=os.readlink(p))
        else:
            raise AssertionError('unsupported_special_file')
        result[str(p.relative_to(root))] = r
    return result

def save(path, value):
    with path.open('x') as f:
        os.fchmod(f.fileno(), 0o600)
        json.dump(value, f, indent=2); f.write('\n'); f.flush(); os.fsync(f.fileno())
    fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try: os.fsync(fd)
    finally: os.close(fd)

def durable_tree(root):
    for p in sorted(root.rglob('*')):
        if not p.is_symlink() and p.is_file():
            with p.open('rb') as f: os.fsync(f.fileno())
    for p in sorted([root] + [p for p in root.rglob('*') if p.is_dir() and not p.is_symlink()], key=lambda p: len(p.parts), reverse=True):
        fd = os.open(p, os.O_RDONLY | os.O_DIRECTORY)
        try: os.fsync(fd)
        finally: os.close(fd)

def capture(window, name, old=False):
    stopped()
    if old:
        assert pathlib.Path('/opt/cpa-rust-gateway/current').resolve() == OLD_RELEASE, 'old_current_drift'
        assert hashlib.sha256((OLD_RELEASE / 'gateway').read_bytes()).hexdigest() == OLD_HASH, 'old_binary_drift'
        assert subprocess.check_output(['systemctl', 'cat', SERVICE]) == (TASK / 'capture-2/unit.private').read_bytes(), 'unit_drift'
    dest = window / name; dest.mkdir(mode=0o700)
    cp(STATE, dest / 'state'); cp(KEYS, dest / 'credentials')
    assert inventory(STATE) == inventory(dest / 'state'), 'state_copy_not_complete'
    assert inventory(KEYS) == inventory(dest / 'credentials'), 'key_copy_not_complete'
    assert all((dest / 'credentials' / k).is_file() and not (dest / 'credentials' / k).is_symlink() for k in KEY_NAMES)
    if old:
        cp(OLD_RELEASE, dest / 'old-release')
        assert inventory(OLD_RELEASE) == inventory(dest / 'old-release')
    unit = pathlib.Path('/etc/systemd/system/cpa-rust-gateway.service')
    cp(unit, dest / 'unit-file')
    dropins = unit.with_name(unit.name + '.d')
    if dropins.exists(): cp(dropins, dest / 'drop-ins')
    (dest / 'unit.private').write_bytes(subprocess.check_output(['systemctl', 'cat', SERVICE]))
    (dest / 'current.private').write_text(str(pathlib.Path('/opt/cpa-rust-gateway/current').resolve()) + '\n')
    names = ['state', 'credentials', 'unit-file'] + (['drop-ins'] if (dest / 'drop-ins').exists() else []) + (['old-release'] if old else [])
    save(dest / 'file-manifest.private.json', {n: inventory(dest / n) for n in names})
    durable_tree(dest)
    stopped()
    save(dest / 'capture.json', {'status': 'PASS', 'captured_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'service_stopped_for_entire_copy': True, 'two_databases_WAL_SHM_journal_backups_keys_deployment_metadata': 'whole state/key directories; complete old release for pre-upgrade', 'cross_store_consistency': 'all inspected holders absent and service stopped before and after copy'})
    return dest

def helpers(window):
    namespace = {}
    source = TASK / 'operator-tools/target-rehearsal.py'
    expected = json.loads((window / 'recovery-tool-hashes.json').read_text())
    for n, digest in expected.items():
        assert hashlib.sha256((TASK / 'operator-tools' / n).read_bytes()).hexdigest() == digest, 'recovery_tool_drift'
    # Definitions only; do not execute the historical capture or regression body.
    exec(source.read_text().split('assert ROOT.lstat()')[0], namespace)
    work = window / 'verification'; work.mkdir(mode=0o700)
    namespace['ROOT'] = work
    return namespace

def verify(window):
    stopped(); pre = window / 'pre-upgrade'
    assert json.loads((pre / 'capture.json').read_text())['status'] == 'PASS'
    n = helpers(window); work = n['ROOT']
    pristine = work / 'restored-pristine-state'
    cp(pre / 'state', pristine)
    assert inventory(pristine) == json.loads((pre / 'file-manifest.private.json').read_text())['state'], 'restore_pristine_copy_mismatch'
    # Even SQLite mode=ro can touch WAL shared-memory bookkeeping. Never open the
    # immutable pre-upgrade archive itself; all database reads use this derived copy.
    integrity_key = os.urandom(32)
    (work / 'integrity-hmac-key.private').write_bytes(integrity_key)
    baseline = {}
    for dbname in n['DBS']:
        with n['ro'](pristine / dbname) as db:
            db.execute('BEGIN')
            assert db.execute('PRAGMA integrity_check').fetchall() == [('ok',)] and not db.execute('PRAGMA foreign_key_check').fetchall()
            if dbname == 'control.sqlite3': assert db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0] == 31
            baseline[dbname] = {}
            for (table,) in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"):
                cols = [r[1] for r in db.execute('PRAGMA table_info(' + n['q'](table) + ')')]
                baseline[dbname][table] = {**n['digest_rows'](db, table, cols, integrity_key), 'columns': cols}
    save(work / 'baseline.private.json', baseline)
    old = n['startup'](pristine, pre / 'credentials', pre / 'old-release/gateway', n['OLD_SHA'], 31, 'old-restore-probe', baseline, integrity_key)
    assert old['status'] == 'PASS' and old['isolated_process_stopped'], 'fresh_backup_old_restore_failed'
    upgraded = work / 'candidate-pristine'; upgraded.mkdir(mode=0o700)
    n['state_copy'](pristine, upgraded / 'state')
    binary = TASK / 'artifact-542a3c30aae7/gateway-aarch64-unknown-linux-gnu'
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == NEW_HASH
    migrated = n['isolated_run'](n['sandbox'](binary, state=upgraded / 'state', service_identity=True, command=['--', '/gateway', 'admin', 'audit', '--db', '/state/control.sqlite3']), capture_output=True, timeout=30)
    (upgraded / 'audit.private.log').write_bytes(migrated.stdout + migrated.stderr)
    with n['ro'](upgraded / 'state/control.sqlite3') as db:
        assert db.execute('SELECT max(version) FROM schema_migrations').fetchone()[0] == 34
    # The known CLI follow-up capability compilation failure is never declared audit PASS.
    assert migrated.returncode == 1 and b'missing_endpoint_capability_profile' in migrated.stderr + migrated.stdout, 'unexpected_migration_cli_result'
    tables = n['preserved'](upgraded / 'state', baseline, integrity_key, True)
    assert all(r['preserved'] for rows in tables.values() for r in rows.values()), 'migration_original_columns_changed'
    spec = n['importlib'].util.spec_from_file_location('private_crypto', TASK / 'operator-tools/private-crypto-audit.py')
    crypto = n['importlib'].util.module_from_spec(spec); spec.loader.exec_module(crypto)
    before = crypto.authenticate(pristine / 'control.sqlite3', pre / 'credentials')
    after = crypto.authenticate(upgraded / 'state/control.sqlite3', pre / 'credentials')
    assert before['status'] == after['status'] == 'PASS' and before == after, 'credential_compatibility_failed'
    new = n['startup'](upgraded / 'state', pre / 'credentials', binary, n['SHA'], 34, 'candidate-startup-probe', baseline, integrity_key)
    assert new['status'] == 'PASS' and new['isolated_process_stopped'], 'candidate_private_startup_failed'
    # Pristine capture bytes/metadata must still match the immutable capture manifest.
    original = json.loads((pre / 'file-manifest.private.json').read_text())
    assert all(inventory(pre / name) == rows for name, rows in original.items()), 'pre_upgrade_capture_modified'
    stopped()
    save(window / 'fresh-backup-verification.json', {'status': 'PASS', 'old_restore': 'PASS', 'candidate_31_to_34_pristine_original_columns': 'PASS', 'old_and_candidate_network_isolated_startup': 'PASS', 'crypto': before, 'cli_audit': 'exit1_known_followup_limitation_not_PASS', 'original_table_count': sum(map(len, baseline.values())), 'completed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()})

def restore(window, decision):
    stopped(); pre = window / 'pre-upgrade'; preserved = window / 'post-failure'
    assert preserved.is_dir() and json.loads((preserved / 'capture.json').read_text())['status'] == 'PASS', 'preserve_candidate_first'
    assert json.loads((window / 'fresh-backup-verification.json').read_text())['status'] == 'PASS'
    assert decision is not None and stat.S_IMODE(decision.stat().st_mode) == 0o600
    d = json.loads(decision.read_text())
    assert d['approved_restore_pre_upgrade'] is True and d['request_attempt_usage_ledger_and_auth_changes_preserved'] is True and d['approval_reference'] and d['new_write_disposition'] in ['none_observed', 'retained_separately_pending_reconciliation'], 'explicit_data_restore_decision_missing'
    original = json.loads((pre / 'file-manifest.private.json').read_text())
    assert all(inventory(pre / name) == rows for name, rows in original.items()), 'backup_changed'
    assert subprocess.check_output(['systemctl', 'cat', SERVICE]) == (pre / 'unit.private').read_bytes(), 'unit_drift_requires_separate_review'
    unit = pathlib.Path('/etc/systemd/system/cpa-rust-gateway.service')
    assert inventory(unit) == original['unit-file'], 'unit_metadata_drift'
    if 'drop-ins' in original: assert inventory(unit.with_name(unit.name + '.d')) == original['drop-ins'], 'dropin_metadata_drift'
    assert not (window / 'displaced-live-state').exists() and not (window / 'displaced-live-keys').exists()
    assert STATE.stat().st_dev == KEYS.stat().st_dev == window.stat().st_dev, 'cross_filesystem_quarantine_requires_revised_plan'
    os.rename(STATE, window / 'displaced-live-state'); os.rename(KEYS, window / 'displaced-live-keys')
    cp(pre / 'state', STATE); cp(pre / 'credentials', KEYS)
    assert inventory(STATE) == original['state'] and inventory(KEYS) == original['credentials'], 'full_restore_copy_failed_keep_service_stopped'
    durable_tree(STATE); durable_tree(KEYS)
    if not OLD_RELEASE.exists(): cp(pre / 'old-release', OLD_RELEASE)
    assert inventory(OLD_RELEASE) == original['old-release'], 'old_release_drift'
    durable_tree(OLD_RELEASE)
    current = pathlib.Path('/opt/cpa-rust-gateway/current')
    temporary = current.with_name('current.cpar-rollback-' + window.name)
    assert not temporary.exists() and not temporary.is_symlink()
    temporary.symlink_to(OLD_RELEASE); os.replace(temporary, current)
    fd = os.open(current.parent, os.O_RDONLY | os.O_DIRECTORY)
    try: os.fsync(fd)
    finally: os.close(fd)
    stopped()
    save(window / 'full-restore.json', {'status': 'PASS_RESTORED_NOT_STARTED', 'state_auth_keys_old_release_current_and_metadata': 'byte_and_metadata_equal', 'failed_state_and_new_records_retained': True, 'unit_bytes_unchanged': True, 'service_start': 'separate_manual_step_after_integrity_checks'})

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=['capture', 'verify', 'preserve', 'restore'])
    parser.add_argument('--window', type=pathlib.Path, required=True)
    parser.add_argument('--authorized-maintenance', action='store_true', required=True)
    parser.add_argument('--data-decision', type=pathlib.Path)
    args = parser.parse_args()
    assert os.geteuid() == 0 and args.authorized_maintenance
    os.umask(0o077)
    window = args.window
    assert window.is_dir() and window.resolve().parent == PREFIX and not window.is_symlink() and window.stat().st_uid == 0 and stat.S_IMODE(window.stat().st_mode) == 0o700, 'wrong_window_directory'
    if args.phase == 'capture': capture(window, 'pre-upgrade', old=True)
    elif args.phase == 'verify': verify(window)
    elif args.phase == 'preserve': capture(window, 'post-failure')
    else: restore(window, args.data_decision)
    print(json.dumps({'phase': args.phase, 'status': 'PASS', 'service_operations_in_this_helper': 0, 'real_provider_calls': 0, 'sensitive_files_remain_on_host': True}))

if __name__ == '__main__':
    main()
