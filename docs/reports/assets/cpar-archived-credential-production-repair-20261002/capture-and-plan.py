"""One fresh guarded online capture per phase; pre phase builds fresh private material."""
import pathlib
import sys
exec((pathlib.Path('/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884/archived-credential-production-repair-20261002') / 'repair-common.py').read_text())
phase = sys.argv[1]
assert phase in ('pre', 'post')
capture = OPS / ('capture-' + phase)
assert not capture.exists(), 'never_reuse_fresh_capture'
if phase == 'post':
    assert json.loads((OPS / 'production-transaction.json').read_text())['commit_completed']
capture.mkdir(mode=0o700)
state = capture / 'state'; state.mkdir(mode=0o700)
credentials = capture / 'credentials'; credentials.mkdir(mode=0o700)
info = STATE.stat(); os.chown(state, info.st_uid, info.st_gid); os.chmod(state, stat.S_IMODE(info.st_mode))
libc = ctypes.CDLL(None, use_errno=True)
libc.inotify_init1.argtypes = [ctypes.c_int]; libc.inotify_init1.restype = ctypes.c_int
libc.inotify_add_watch.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_uint32]; libc.inotify_add_watch.restype = ctypes.c_int
watcher = libc.inotify_init1(os.O_NONBLOCK | os.O_CLOEXEC)
assert watcher >= 0
observers = {}
started = utc(); tick = time.monotonic()
try:
    for item in [KEYS, STATE / 'backups'] + [p for p in (STATE / 'backups').rglob('*') if p.is_dir() and not p.is_symlink()]:
        assert not item.is_symlink()
        assert libc.inotify_add_watch(watcher, os.fsencode(item), 0x2 | 0x4 | 0x8 | 0x40 | 0x80 | 0x100 | 0x200 | 0x400 | 0x800) >= 0
    before_runtime = runtime()
    before_keys = key_state()
    observers = {name: db_ro(STATE / name) for name in DBS}
    versions_before = {name: db.execute('PRAGMA data_version').fetchone()[0] for name, db in observers.items()}
    for name in DBS:
        direct(STATE / name)
        with db_ro(STATE / name) as source:
            source.execute('BEGIN')
            source.execute('SELECT count(*) FROM sqlite_master').fetchone()
            deadline = time.monotonic() + 10
            def progress(*args):
                assert time.monotonic() < deadline, 'online_backup_deadline'
            with sqlite3.connect(state / name) as destination:
                source.backup(destination, pages=64, sleep=0.01, progress=progress)
        info = (STATE / name).stat(); os.chown(state / name, info.st_uid, info.st_gid); os.chmod(state / name, stat.S_IMODE(info.st_mode))
    for name in KEY_NAMES:
        direct(KEYS / name)
        shutil.copy2(KEYS / name, credentials / name)
        info = (KEYS / name).stat(); os.chown(credentials / name, info.st_uid, info.st_gid)
    shutil.copytree(STATE / 'backups', state / 'backups', symlinks=True)
    for item in [state / 'backups'] + list((state / 'backups').rglob('*')):
        source = STATE / item.relative_to(state)
        info = source.lstat()
        os.chown(item, info.st_uid, info.st_gid, follow_symlinks=False)
        if not item.is_symlink():
            assert stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode), 'archive_special_file'
            os.chmod(item, stat.S_IMODE(info.st_mode))
    versions_after = {name: db.execute('PRAGMA data_version').fetchone()[0] for name, db in observers.items()}
    after_keys = key_state()
    after_runtime = runtime()
    try:
        events = os.read(watcher, 1024 * 1024)
    except BlockingIOError:
        events = b''
    stable = versions_before == versions_after and before_keys == after_keys and not events and before_runtime == after_runtime
    observation = {'status': 'PASS' if stable else 'BLOCKED', 'phase': phase, 'started_at_utc': started, 'completed_at_utc': utc(), 'duration_seconds': round(time.monotonic() - tick, 6), 'database_data_versions_before': versions_before, 'database_data_versions_after': versions_after, 'cross_database_keys_archive_runtime_stable_window': stable, 'six_persistent_loaded_and_original_keys_match': True, 'key_and_archive_inotify_quiet': not events, 'runtime': after_runtime, 'method': 'SQLite Online Backup of both databases with same-connection data_version and key/archive/runtime guards; committed WAL included', 'retry_count': 0, 'service_stop_start': 0, 'snapshot_exported': False}
    receipt('capture-' + phase + '-receipt.json', observation)
    assert stable, 'fresh_capture_not_consistent_stop'
finally:
    for db in observers.values():
        db.close()
    os.close(watcher)

integrity_file = OPS / 'integrity-hmac-key'
if phase == 'pre':
    private_write(integrity_file, os.urandom(32))
integrity_key = integrity_file.read_bytes()
baseline = logical_state(state, integrity_key)
private_write(capture / 'logical-before.private.json', (json.dumps(baseline, indent=2) + '\n').encode())
private_write(capture / 'integrity-hmac-key', integrity_key)
private_write(capture / 'old-gateway', pathlib.Path('/proc/499070/exe').read_bytes()); os.chmod(capture / 'old-gateway', 0o755)
private_write(capture / 'unit.private', subprocess.check_output(['systemctl', 'cat', SERVICE], timeout=5))
private_write(capture / 'old-current.private', (str(pathlib.Path('/opt/cpa-rust-gateway/current').resolve()) + '\n').encode())
metadata = {str(item.relative_to(capture)): {'uid': item.lstat().st_uid, 'gid': item.lstat().st_gid, 'mode': stat.S_IMODE(item.lstat().st_mode), 'bytes': item.lstat().st_size, 'symlink': item.is_symlink()} for item in [state, credentials] + list(state.rglob('*')) + list(credentials.iterdir())}
private_write(capture / 'restore-metadata.private.json', (json.dumps(metadata, indent=2) + '\n').encode())
assert all(hmac.compare_digest((KEYS / name).read_bytes(), (credentials / name).read_bytes()) for name in KEY_NAMES)
with db_ro(state / 'control.sqlite3') as db:
    context = exact_context(db)
    columns = [r[1] for r in db.execute('PRAGMA table_info(upstream_credentials)')]
    index = columns.index('ciphertext')
    envelope = context['target'][index]
    parent_envelope = context['parent'][index]
    parent_id = context['parent_config'][0]
    master = bytearray((credentials / 'master-key').read_bytes())
    try:
        parent_plain = clear_open(parent_envelope, parent_id, master)
        try:
            if phase == 'pre':
                with db_ro(CAP / 'state/control.sqlite3') as original:
                    assert context == exact_context(original), 'fresh_target_parent_active_or_transition_drift'
                assert hmac.compare_digest(envelope, parent_envelope) and hashlib.sha256(envelope).hexdigest() == loc['ciphertext_sha256']
                try:
                    opened = clear_open(envelope, loc['config_version_id'], master)
                except InvalidTag:
                    pass
                else:
                    opened[:] = bytes(len(opened))
                    raise AssertionError('not_original_failure')
                nonce = os.urandom(24)
                assert nonce != envelope[1:25]
                replacement = bytes([1]) + nonce + ChaCha20Poly1305(crypto.hchacha(bytes(master), nonce[:16])).encrypt(bytes(4) + nonce[16:], bytes(parent_plain), aad(loc['config_version_id']))
                private_write(OPS / 'original-envelope.private.bin', envelope)
                private_write(OPS / 'repaired-envelope.private.bin', replacement)
                private_write(OPS / 'target.private.json', (DIAG / 'target.private.json').read_bytes())
            else:
                replacement = (OPS / 'repaired-envelope.private.bin').read_bytes()
                assert hmac.compare_digest(envelope, replacement), 'post_repair_target_drift'
            verified = clear_open(replacement, loc['config_version_id'], master)
            try:
                assert hmac.compare_digest(parent_plain, verified), 'plaintext_difference'
            finally:
                verified[:] = bytes(len(verified))
        finally:
            parent_plain[:] = bytes(len(parent_plain))
    finally:
        master[:] = bytes(len(master))
auth = crypto.authenticate(state / 'control.sqlite3', credentials)
assert auth['failed_envelopes']['upstream_credentials'] == (1 if phase == 'pre' else 0) and auth['failed_envelopes']['native_grok_accounts'] == 0
receipt('plan-' + phase + '-receipt.json', {'status': 'PASS', 'phase': phase, 'record_alias': 'AC-01', 'target_configuration_lifecycle': 'archived', 'credential_status': 'active', 'credential_revision': 2, 'configuration_revision': 0, 'direct_parent_authenticated': True, 'target_original_failure_reconfirmed': phase == 'pre', 'fresh_child_envelope_authentication': 'PASS', 'source_plaintext_equal': True, 'six_keys_and_old_binary_unit_current_archive_metadata_retained': True, 'target_not_active_or_current_rollback_predecessor': True, 'both_database_integrity_and_foreign_keys': 'PASS', 'original_table_count': sum(len(tables) for tables in baseline.values()), 'crypto': auth, 'completed_at_utc': utc(), 'production_writes_in_this_script': 0, 'real_provider_calls': 0})
