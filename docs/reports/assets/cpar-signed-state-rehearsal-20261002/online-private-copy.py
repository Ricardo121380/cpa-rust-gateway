"""Online guarded SQLite backup; private bytes and digest keys stay on new-vps."""
import ctypes
import datetime
import hashlib
import hmac
import json
import os
import pathlib
import shutil
import sqlite3
import stat
import subprocess
import time

os.umask(0o077)
ROOT = pathlib.Path("/opt/cpa-rust-gateway/staging/cpar-execution-arm64-20261002-run36956561884")
STATE = pathlib.Path("/var/lib/cpa-rust-gateway")
KEYS = pathlib.Path("/etc/cpa-rust-gateway/credentials")
SERVICE = "cpa-rust-gateway.service"
NAMES = ["management-key", "management-csrf", "master-key", "backup-key", "client-key-pepper", "grok-build-cache-key"]
DBS = ["control.sqlite3", "admin-auth.sqlite3"]
def unit():
    raw = subprocess.check_output(["systemctl", "show", SERVICE, "-p", "MainPID", "-p", "ActiveState", "-p", "SubState", "-p", "ExecMainStartTimestamp"], text=True)
    return dict(line.split("=", 1) for line in raw.splitlines() if "=" in line)
def direct(p):
    s = p.lstat()
    assert stat.S_ISREG(s.st_mode), "non_direct_runtime_file"
    return (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns, stat.S_IMODE(s.st_mode), s.st_uid, s.st_gid)
def key_state():
    return {n: (direct(KEYS/n), hashlib.sha256((KEYS/n).read_bytes()).hexdigest()) for n in NAMES}
def loaded_matches(pid):
    live = pathlib.Path(f"/proc/{pid}/root/run/credentials/{SERVICE}")
    return all((live/n).is_file() and hmac.compare_digest((KEYS/n).read_bytes(), (live/n).read_bytes()) for n in NAMES)
def q(name):
    return '"' + name.replace('"', '""') + '"'
def logical(db, key):
    result = {}
    for (table,) in db.execute("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"):
        columns = [r[1] for r in db.execute("PRAGMA table_info(" + q(table) + ")")]
        encoded = sorted(json.dumps(list(row), ensure_ascii=False, separators=(",", ":"), default=lambda b: {"sqlite_blob_hex": b.hex()}).encode() for row in db.execute("SELECT " + ",".join(q(c) for c in columns) + " FROM " + q(table)))
        digest = hmac.new(key, digestmod=hashlib.sha256)
        for row in encoded:
            digest.update(len(row).to_bytes(8, "big")); digest.update(row)
        result[table] = {"columns": columns, "count": len(encoded), "original_columns_hmac_sha256": digest.hexdigest()}
    return result
def ro(p):
    db = sqlite3.connect(f"file:{p}?mode=ro", uri=True, timeout=1)
    db.execute("PRAGMA query_only=ON")
    return db
def copy_file(source, dest):
    direct(source)
    shutil.copy2(source, dest)
    s = source.stat(); os.chown(dest, s.st_uid, s.st_gid)

ROOT.parent.mkdir(mode=0o700, parents=False, exist_ok=True)
assert ROOT.parent.lstat().st_uid == 0 and not ROOT.parent.is_symlink()
ROOT.mkdir(mode=0o700, parents=False, exist_ok=True)
assert ROOT.lstat().st_uid == 0 and stat.S_IMODE(ROOT.lstat().st_mode) == 0o700 and not ROOT.is_symlink()
if (ROOT/"backup-receipt.json").exists():
    result = json.loads((ROOT/"backup-receipt.json").read_text())
    assert result["status"] == "PASS"
    print(json.dumps({**result, "reused": True}, indent=2)); raise SystemExit(0)
before = unit(); assert before["ActiveState"] == "active" and before["SubState"] == "running"
pid = int(before["MainPID"])
old = pathlib.Path(f"/proc/{pid}/exe").read_bytes()
old_hash = hashlib.sha256(old).hexdigest()
assert old_hash == "0b516c319d901f5a544f0ca9e10bf0338601ba4dc40c92ea6921bb0706fbc54b"
assert loaded_matches(pid), "persistent_keys_do_not_match_running_service"
libc = ctypes.CDLL(None, use_errno=True)
libc.inotify_init1.argtypes = [ctypes.c_int]; libc.inotify_init1.restype = ctypes.c_int
libc.inotify_add_watch.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_uint32]; libc.inotify_add_watch.restype = ctypes.c_int
attempts = []
first_attempt = 1 + max([int(p.name.split("-")[1]) for p in ROOT.glob("capture-*") if p.name.split("-")[1].isdigit()] or [0])
for attempt in range(first_attempt, first_attempt + 3):
    path = ROOT/f"capture-{attempt}"; path.mkdir(mode=0o700)
    state = path/"state"; keys = path/"credentials"; state.mkdir(mode=0o700); keys.mkdir(mode=0o700)
    original_state = STATE.stat(); os.chown(state, original_state.st_uid, original_state.st_gid)
    watchers = libc.inotify_init1(os.O_NONBLOCK | os.O_CLOEXEC); assert watchers >= 0
    mask = 0x2 | 0x4 | 0x8 | 0x40 | 0x80 | 0x100 | 0x200 | 0x400 | 0x800
    extra_dirs = [STATE/"backups"] + [p for p in (STATE/"backups").rglob("*") if p.is_dir() and not p.is_symlink()]
    for p in [KEYS] + extra_dirs:
        assert not p.is_symlink()
        assert libc.inotify_add_watch(watchers, os.fsencode(p), mask) >= 0
    observers = {n: ro(STATE/n) for n in DBS}
    versions_before = {n: d.execute("PRAGMA data_version").fetchone()[0] for n, d in observers.items()}
    keys_before = key_state()
    started = datetime.datetime.now(datetime.timezone.utc).isoformat(); tick = time.monotonic()
    for n in DBS:
        direct(STATE/n)
        source = ro(STATE/n); source.execute("BEGIN")
        source.execute("SELECT count(*) FROM sqlite_master").fetchone()
        target = sqlite3.connect(state/n)
        source.backup(target, pages=64, sleep=0.01)
        target.close(); source.close()
        s = (STATE/n).stat(); os.chmod(state/n, stat.S_IMODE(s.st_mode)); os.chown(state/n, s.st_uid, s.st_gid)
    for n in NAMES:
        copy_file(KEYS/n, keys/n)
    shutil.copytree(STATE/"backups", state/"backups", copy_function=shutil.copy2, symlinks=True)
    versions_after = {n: d.execute("PRAGMA data_version").fetchone()[0] for n, d in observers.items()}
    keys_after = key_state()
    try:
        changes = os.read(watchers, 1024*1024)
    except BlockingIOError:
        changes = b""
    os.close(watchers)
    for d in observers.values(): d.close()
    stable = versions_before == versions_after and keys_before == keys_after and not changes and before == unit() and loaded_matches(pid)
    observation = {"attempt": attempt, "started_at_utc": started, "completed_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(), "duration_seconds": round(time.monotonic()-tick, 6), "database_data_versions_before": versions_before, "database_data_versions_after": versions_after, "persistent_keys_unchanged": keys_before == keys_after, "key_and_backup_directory_inotify_quiet": not changes, "cross_state_stable_window": stable}
    attempts.append(observation)
    (path/"capture-observation.json").write_text(json.dumps(observation, indent=2)+"\n")
    if not stable:
        continue
    integrity_key = os.urandom(32); (path/"integrity-hmac-key").write_bytes(integrity_key)
    tables = {}; db_hashes = {}
    for n in DBS:
        with ro(state/n) as db:
            assert db.execute("PRAGMA integrity_check").fetchall() == [("ok",)], "backup_integrity_failed"
            assert not db.execute("PRAGMA foreign_key_check").fetchall(), "backup_foreign_key_failed"
            tables[n] = logical(db, integrity_key)
            if n == "control.sqlite3":
                assert db.execute("SELECT max(version) FROM schema_migrations").fetchone()[0] == 31
        db_hashes[n] = hashlib.sha256((state/n).read_bytes()).hexdigest()
    assert all(hmac.compare_digest((KEYS/n).read_bytes(), (keys/n).read_bytes()) for n in NAMES)
    (path/"old-gateway").write_bytes(old); os.chmod(path/"old-gateway", 0o755)
    (path/"unit.private").write_bytes(subprocess.check_output(["systemctl", "cat", SERVICE]))
    (path/"old-current.private").write_text(str(pathlib.Path("/opt/cpa-rust-gateway/current").resolve())+"\n")
    metadata = {str(p.relative_to(path)): {"uid": p.lstat().st_uid, "gid": p.lstat().st_gid, "mode": stat.S_IMODE(p.lstat().st_mode), "bytes": p.lstat().st_size} for p in [state, keys] + list(state.rglob("*")) + list(keys.iterdir())}
    (path/"restore-metadata.private.json").write_text(json.dumps(metadata, indent=2)+"\n")
    (path/"logical-before.private.json").write_text(json.dumps(tables, indent=2)+"\n")
    result = {"status": "PASS", "layer": "GUARDED_ONLINE_PRODUCTION_PRIVATE_COPY", "source_candidate": "80f5838902eb56be6dccc3ce047eb227d6c18e6c", "private_capture_directory": str(path), "method": "SQLite Online Backup API for both databases; same-connection data_version unchanged for both over capture; keys/backup artifacts unchanged under inotify and stat/hash checks; persistent keys match running LoadCredential copies", "attempts": attempts, "control_schema": 31, "control_integrity": "ok", "admin_auth_user_version": 1, "admin_auth_integrity": "ok", "database_sha256": db_hashes, "tables": {n: {t: {k:v for k,v in r.items() if k != "columns"} for t,r in rows.items()} for n,rows in tables.items()}, "persistent_key_file_count": len(NAMES), "persistent_keys_match_loaded_credentials": True, "old_binary_sha256": old_hash, "old_source_sha": "b0cf36e387b774439acc23830221389eb3ac74d0", "service_unchanged": before == unit(), "no_write_pause_or_service_stop": True, "production_copy_exported": False, "real_provider_calls": 0}
    (ROOT/"backup-receipt.json").write_text(json.dumps(result, indent=2)+"\n")
    print(json.dumps(result, indent=2)); break
else:
    result = {"status": "BLOCKED", "reason": "no_proven_cross_database_and_key_stable_capture_window", "attempts": attempts, "no_write_pause_or_service_stop": True, "production_copy_exported": False}
    (ROOT/"backup-receipt.json").write_text(json.dumps(result, indent=2)+"\n")
    print(json.dumps(result, indent=2)); raise SystemExit(2)
